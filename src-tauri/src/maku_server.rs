//! Maku Server (dossier `server/` du dépôt) : relais Seerr et Tracearr installé par l'administrateur
//! à côté de son serveur, pour ne pas exposer Seerr et Tracearr sur Internet. Avec Jellyfin, le plugin Maku
//! (dossier `plugins/jellyfin/`) fait de même depuis le serveur lui-même, à son adresse (`…/Maku`).
//!
//! Ses adresses se terminent par `/seerr` ou `/tracearr`. Chaque appel y porte le jeton de
//! l'utilisateur auprès du serveur multimédia (`X-Maku-Token` pour Maku Server, l'en-tête de Jellyfin pour
//! le plugin), que le relais vérifie. Le jeton n'est joint qu'après avoir vérifié, par `/maku/info`, que
//! l'adresse est bien celle du relais de ce serveur.
//!
//! L'identifiant du serveur, que le relais y annonce, est public : il ne prouve rien. Un Maku Server qui a une clé
//! (`MAKU_KEY`) signe donc le nombre au hasard que Maku lui envoie. Maku retient sa clé publique à la première
//! rencontre (`Config::relay_keys`), et ne joint ensuite le jeton qu'à qui sait signer avec elle : sur un autre
//! réseau, la machine qui occupe la même adresse ne reçoit rien. Le plugin, lui, répond à l'adresse du serveur même.

use crate::config::SavedServer;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine;
const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;
/// Une réponse du relais vaut cinq minutes : passé ce délai (autre réseau, peut-être), il prouve de nouveau qui il est.
const FRESH: Duration = Duration::from_secs(300);

/// Port du relais par défaut (`MAKU_LISTEN`).
pub const PORT: u16 = 32700;
pub const TOKEN_HEADER: &str = "X-Maku-Token";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Info {
    pub app: String,
    pub server: String,
    pub id: Option<String>,
    pub seerr: bool,
    pub tracearr: bool,
    /// Le plugin Maku de Jellyfin (et non Maku Server) : Seerr s'y utilise sans mot de passe.
    #[serde(default)]
    pub plugin: bool,
    /// Clé publique de Maku Server (base64 d'URL), s'il en a une.
    #[serde(default)]
    pub key: Option<String>,
    /// Sa signature du nombre envoyé avec la question.
    #[serde(default)]
    pub proof: Option<String>,
}

/// Ce qu'un relais a répondu, et la clé avec laquelle il vient de signer le nombre envoyé (s'il l'a fait).
#[derive(Debug, Clone)]
pub struct Seen {
    pub info: Info,
    pub proved: Option<String>,
    at: Instant,
}

/// Ce qui est signé : le type et l'identifiant du serveur, puis le nombre au hasard (même forme que dans
/// `server/src/proof.rs`).
fn message(kind: &str, id: &str, challenge: &[u8]) -> Vec<u8> {
    let mut m = format!("maku-relay-proof-v1\n{kind}\n{id}\n").into_bytes();
    m.extend_from_slice(challenge);
    m
}

/// La clé annoncée par le relais, s'il a bien signé `challenge` avec elle.
fn proved(info: &Info, challenge: &[u8]) -> Option<String> {
    let key = info.key.as_deref()?;
    let public = B64.decode(key).ok()?;
    let signature = B64.decode(info.proof.as_deref()?).ok()?;
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public)
        .verify(&message(&info.server, info.id.as_deref().unwrap_or(""), challenge), &signature)
        .ok()?;
    Some(key.to_owned())
}

/// Faut-il joindre le jeton aux appels vers ce relais ?
#[derive(Debug, Clone, PartialEq)]
pub enum Trust {
    Yes,
    /// Oui, et sa clé, vue pour la première fois, est à retenir pour ce serveur.
    Pin(String),
    No,
}

/// `base` est-elle une adresse du serveur lui-même (même protocole, même hôte, même port) ?
fn on_server(base: &str, server: &SavedServer) -> bool {
    match (Url::parse(base), Url::parse(&server.uri)) {
        (Ok(a), Ok(b)) => a.scheme() == b.scheme() && a.host_str() == b.host_str() && a.port_or_known_default() == b.port_or_known_default(),
        _ => false,
    }
}

/// `pinned` : la clé retenue pour ce serveur, s'il y en a une.
pub fn trust(seen: &Seen, base: &str, server: &SavedServer, pinned: Option<&str>) -> Trust {
    if !serves(&seen.info, server) {
        return Trust::No;
    }
    if seen.info.plugin {
        // Le plugin vit dans le serveur Jellyfin : son adresse est celle du serveur, qui reçoit déjà le jeton. Ailleurs,
        // seule une adresse en https (dont le certificat dit à qui l'on parle) est acceptée. Jamais avec Plex : se dire
        // « plugin » serait sinon un moyen de se passer de preuve.
        let ok = server.is_jellyfin() && (on_server(base, server) || base.starts_with("https://"));
        return if ok { Trust::Yes } else { Trust::No };
    }
    match (pinned, seen.proved.as_deref()) {
        (Some(pinned), Some(proved)) if pinned == proved => Trust::Yes,
        // Clé retenue, et le relais ne signe pas avec elle : ce n'est pas lui (ou il a été réinstallé avec une autre).
        (Some(_), _) => Trust::No,
        (None, Some(proved)) => Trust::Pin(proved.to_owned()),
        // Maku Server sans clé (version d'avant, ou MAKU_KEY absente) : reconnu à son seul identifiant, comme avant.
        (None, None) => Trust::Yes,
    }
}

/// Ce qu'il faut faire d'une adresse de Seerr ou de Tracearr.
#[derive(Debug, PartialEq)]
pub enum Decision {
    /// Ce n'est pas l'adresse d'un relais (ou il ne répond pas) : rien à joindre.
    Direct,
    /// Joindre ceci ; et retenir cette clé pour le serveur, s'il y en a une.
    Send(Relay, Option<String>),
    /// Un relais répond à cette adresse, mais ce n'est pas celui de ce serveur : le jeton n'y part pas.
    Refused,
}

/// Ce qu'il faut joindre aux appels qui passent par un relais.
#[derive(Debug, Clone, PartialEq)]
pub enum Relay {
    /// Maku Server : le jeton du serveur dans `X-Maku-Token`.
    Server(String),
    /// Plugin Maku de Jellyfin : l'en-tête d'authentification de Jellyfin, jeton compris.
    Plugin(String),
}

impl Relay {
    pub fn is_plugin(&self) -> bool {
        matches!(self, Relay::Plugin(_))
    }
}

/// En-têtes de chaque appel : le jeton du serveur quand l'adresse passe par Maku Server ou le plugin.
pub fn relay_headers(relay: Option<Relay>) -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    let (names, value): (&[&str], String) = match relay {
        Some(Relay::Server(token)) => (&[TOKEN_HEADER], token),
        Some(Relay::Plugin(authorization)) => (&["Authorization", "X-Emby-Authorization"], authorization),
        None => return h,
    };
    if let Ok(mut v) = reqwest::header::HeaderValue::from_str(&value) {
        v.set_sensitive(true);
        for name in names {
            h.insert(*name, v.clone());
        }
    }
    h
}

/// Adresse du relais d'une adresse Seerr ou Tracearr (« https://hôte:32700/seerr » → « https://hôte:32700 »).
pub fn relay_base(url: &str) -> Option<&str> {
    let url = url.trim_end_matches('/');
    url.strip_suffix("/seerr").or_else(|| url.strip_suffix("/tracearr")).filter(|b| b.contains("://"))
}

pub async fn info(http: &Client, base: &str) -> Option<Seen> {
    // Un nombre neuf à chaque question : une signature entendue une fois ne ressert pas.
    let mut challenge = [0u8; 32];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut challenge).ok()?;
    let r = http
        .get(format!("{base}/maku/info"))
        .query(&[("challenge", B64.encode(challenge))])
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .ok()?;
    let info: Info = r.json().await.ok()?;
    if info.app != "maku-server" {
        return None;
    }
    let proved = proved(&info, &challenge);
    Some(Seen { info, proved, at: Instant::now() })
}

/// Ce relais est-il celui de ce serveur ?
fn serves(info: &Info, server: &SavedServer) -> bool {
    let kind = if server.kind.is_empty() { "plex" } else { server.kind.as_str() };
    info.server == kind && info.id.as_deref() == Some(server.machine_id.as_str())
}

/// Relais déjà interrogés, par adresse.
#[derive(Default)]
pub struct Known(Mutex<HashMap<String, Seen>>);

impl Known {
    /// Ce qu'il faut joindre aux appels vers `url` : le jeton du serveur, si `url` passe par son relais.
    /// `pinned` : la clé retenue pour ce serveur. `authorization` : l'en-tête de Jellyfin de ce compte, pour le plugin.
    pub async fn token_for(
        &self,
        http: &Client,
        url: &str,
        server: Option<&SavedServer>,
        pinned: Option<&str>,
        authorization: impl FnOnce(&str) -> String,
    ) -> Decision {
        let (Some(server), Some(base)) = (server, relay_base(url)) else { return Decision::Direct };
        let cached = self.0.lock().unwrap().get(base).filter(|s| s.at.elapsed() < FRESH).cloned();
        let seen = match cached {
            Some(s) => s,
            None => {
                // Injoignable : réessayé à la prochaine connexion.
                let Some(s) = info(http, base).await else { return Decision::Direct };
                self.0.lock().unwrap().insert(base.to_owned(), s.clone());
                s
            }
        };
        let relay = || if seen.info.plugin { Relay::Plugin(authorization(&server.token)) } else { Relay::Server(server.token.clone()) };
        match trust(&seen, base, server, pinned) {
            Trust::Yes => Decision::Send(relay(), None),
            Trust::Pin(key) => Decision::Send(relay(), Some(key)),
            Trust::No => Decision::Refused,
        }
    }

    /// Oublie ce qui a été répondu : la prochaine demande repose la question.
    pub fn forget(&self) {
        self.0.lock().unwrap().clear();
    }
}

/// Un relais trouvé : son adresse, ce qu'il propose, et sa clé si elle est à retenir.
pub struct Found {
    pub base: String,
    pub info: Info,
    pub pin: Option<String>,
}

/// Cherche le relais sur la machine du serveur, à son port par défaut. `pinned` : la clé retenue pour ce serveur.
pub async fn discover(http: &Client, server: &SavedServer, pinned: Option<&str>) -> Option<Found> {
    // Jellyfin : le plugin Maku, à l'adresse même du serveur (sur le réseau local comme depuis Internet).
    let plugin = server.is_jellyfin().then(|| format!("{}/Maku", server.uri.trim_end_matches('/')));
    for base in plugin.into_iter().chain(candidates(server)) {
        let Some(seen) = info(http, &base).await else { continue };
        let pin = match trust(&seen, &base, server, pinned) {
            Trust::Yes => None,
            Trust::Pin(key) => Some(key),
            Trust::No => continue,
        };
        return Some(Found { base, info: seen.info, pin });
    }
    None
}

/// Adresses où chercher Maku Server : la machine du serveur, à son port par défaut. En `https` d'abord : l'adresse
/// retenue est celle où partiront ensuite les jetons, qui ne passent en clair que si le relais ne répond pas
/// autrement (réseau local, sans proxy inverse).
///
/// La machine du serveur est celle de la connexion en cours, la seule dont Maku vient de vérifier qu'elle est bien
/// ce serveur. Ses autres adresses connues (celles du réseau local, quand on est ailleurs) ne sont pas essayées :
/// hors de chez soi, elles sont celles d'autres machines, à qui Maku demandait à chaque démarrage si elles étaient
/// son relais. Chez soi, la connexion en cours est déjà l'adresse locale.
fn candidates(server: &SavedServer) -> Vec<String> {
    let Some(host) = Url::parse(&server.uri).ok().and_then(|u| u.host_str().map(str::to_owned)) else { return Vec::new() };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let host = if host.contains(':') { format!("[{host}]") } else { host.to_owned() };
    ["https", "http"].iter().map(|scheme| format!("{scheme}://{host}:{PORT}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresses_du_relais() {
        assert_eq!(relay_base("https://h:32700/seerr"), Some("https://h:32700"));
        assert_eq!(relay_base("http://h:32700/relais/tracearr/"), Some("http://h:32700/relais"));
        assert_eq!(relay_base("https://seerr.exemple.fr"), None);
        assert_eq!(relay_base("/seerr"), None);
    }

    #[test]
    fn relais_du_bon_serveur() {
        let server = SavedServer { name: "s".into(), machine_id: "abc".into(), uri: "http://h:32400".into(), token: "t".into(), kind: String::new(), user_id: None };
        let mut info = Info { app: "maku-server".into(), server: "plex".into(), id: Some("abc".into()), seerr: true, tracearr: false, plugin: false, key: None, proof: None };
        assert!(serves(&info, &server));
        info.id = Some("autre".into());
        assert!(!serves(&info, &server));
        info.id = Some("abc".into());
        info.server = "jellyfin".into();
        assert!(!serves(&info, &server));
    }

    fn server(kind: &str, uri: &str) -> SavedServer {
        SavedServer { name: "s".into(), machine_id: "abc".into(), uri: uri.into(), token: "t".into(), kind: kind.into(), user_id: None }
    }

    #[test]
    fn relais_cherche_sur_la_machine_du_serveur_en_https_d_abord() {
        assert_eq!(candidates(&server("", "https://h.example:32400")), ["https://h.example:32700", "http://h.example:32700"]);
        assert_eq!(candidates(&server("", "http://[fe80::1]:32400")), ["https://[fe80::1]:32700", "http://[fe80::1]:32700"]);
        assert!(candidates(&server("", "pas une adresse")).is_empty());
    }

    /// Clé et signature données par `server/src/proof.rs` pour la phrase « une phrase secrète de test », le serveur
    /// Plex « abc » et le nombre fait de 32 octets à 7 (voir son test `preuve_connue`) : les deux programmes
    /// s'entendent sur ce qui est signé.
    const KEY: &str = "61yEhfgSy-fJvLSLxDs2eaxmvD1ncYslE4wkNqnj8kw";
    const PROOF: &str = "jDuWUtVRWFeR2olwqP7x8QS_v_1ilkxgy1M2_kh8MMtBYPivTunXH16Mm1zW7IXkTI5A9XJdt4UAu5XsCl1dBw";

    fn seen(key: Option<&str>, proof: Option<&str>, plugin: bool, kind: &str) -> Seen {
        let info = Info {
            app: "maku-server".into(),
            server: kind.into(),
            id: Some("abc".into()),
            seerr: true,
            tracearr: false,
            plugin,
            key: key.map(str::to_owned),
            proof: proof.map(str::to_owned),
        };
        let proved = proved(&info, &[7u8; 32]);
        Seen { info, proved, at: Instant::now() }
    }

    #[test]
    fn signature_du_relais() {
        assert_eq!(seen(Some(KEY), Some(PROOF), false, "plex").proved.as_deref(), Some(KEY));
        // Autre nombre, autre serveur, signature abîmée ou absente : rien n'est prouvé.
        let info = seen(Some(KEY), Some(PROOF), false, "plex").info;
        assert_eq!(proved(&info, &[8u8; 32]), None);
        assert_eq!(proved(&Info { id: Some("autre".into()), ..info.clone() }, &[7u8; 32]), None);
        assert_eq!(proved(&Info { server: "jellyfin".into(), ..info.clone() }, &[7u8; 32]), None);
        let mut broken = PROOF.to_owned();
        broken.replace_range(0..1, if PROOF.starts_with('A') { "B" } else { "A" });
        assert_eq!(proved(&Info { proof: Some(broken), ..info.clone() }, &[7u8; 32]), None);
        assert_eq!(proved(&Info { proof: None, ..info.clone() }, &[7u8; 32]), None);
        assert_eq!(proved(&Info { key: Some("pas une clé".into()), ..info }, &[7u8; 32]), None);
    }

    #[test]
    fn confiance_dans_le_relais() {
        let plex = server("", "http://10.0.0.2:32400");
        let base = "http://10.0.0.2:32700";
        let signed = seen(Some(KEY), Some(PROOF), false, "plex");
        let unsigned = seen(None, None, false, "plex");
        // Première rencontre : la clé est retenue. Ensuite, elle seule ouvre la porte.
        assert_eq!(trust(&signed, base, &plex, None), Trust::Pin(KEY.into()));
        assert_eq!(trust(&signed, base, &plex, Some(KEY)), Trust::Yes);
        assert_eq!(trust(&signed, base, &plex, Some("une autre clé")), Trust::No);
        // Clé retenue, et la machine qui répond ne signe pas, ou annonce la clé sans savoir signer avec.
        assert_eq!(trust(&unsigned, base, &plex, Some(KEY)), Trust::No);
        assert_eq!(trust(&seen(Some(KEY), None, false, "plex"), base, &plex, Some(KEY)), Trust::No);
        assert_eq!(trust(&seen(Some(KEY), Some("AAAA"), false, "plex"), base, &plex, Some(KEY)), Trust::No);
        // Maku Server sans clé, jamais vu avec une : accepté comme avant.
        assert_eq!(trust(&unsigned, base, &plex, None), Trust::Yes);
        // Le relais d'un autre serveur n'est jamais le bon.
        assert_eq!(trust(&signed, base, &SavedServer { machine_id: "autre".into(), ..plex.clone() }, None), Trust::No);
    }

    #[test]
    fn se_dire_plugin_ne_dispense_pas_de_preuve() {
        let plex = server("", "http://10.0.0.2:32400");
        // Avec Plex, il n'y a pas de plugin : la réponse est refusée, clé retenue ou non.
        let fake = seen(None, None, true, "plex");
        assert_eq!(trust(&fake, "http://10.0.0.2:32700", &plex, Some(KEY)), Trust::No);
        assert_eq!(trust(&fake, "http://10.0.0.2:32700", &plex, None), Trust::No);
        // Jellyfin : le plugin répond à l'adresse du serveur, ou en https ; pas à une autre adresse en clair.
        let jellyfin = server("jellyfin", "http://10.0.0.2:8096");
        let plugin = seen(None, None, true, "jellyfin");
        assert_eq!(trust(&plugin, "http://10.0.0.2:8096/Maku", &jellyfin, None), Trust::Yes);
        assert_eq!(trust(&plugin, "https://jellyfin.example/Maku", &jellyfin, None), Trust::Yes);
        assert_eq!(trust(&plugin, "http://10.0.0.2:32700", &jellyfin, None), Trust::No);
        assert_eq!(trust(&plugin, "http://10.0.0.3:8096/Maku", &jellyfin, None), Trust::No);
    }

    /// Faux Maku Server du serveur Plex « abc », sur cette machine. `seed` : la graine de sa clé, s'il en a une.
    async fn fake_relay(seed: Option<[u8; 32]>) -> String {
        use ring::signature::KeyPair;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else { return };
                let mut buf = vec![0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).into_owned();
                let challenge = request.split("challenge=").nth(1).and_then(|r| r.split([' ', '&']).next()).and_then(|c| B64.decode(c).ok());
                let mut info = serde_json::json!({ "app": "maku-server", "server": "plex", "id": "abc", "seerr": true, "tracearr": false });
                if let (Some(seed), Some(challenge)) = (seed, challenge) {
                    let pair = ring::signature::Ed25519KeyPair::from_seed_unchecked(&seed).unwrap();
                    info["key"] = B64.encode(pair.public_key().as_ref()).into();
                    info["proof"] = B64.encode(pair.sign(&message("plex", "abc", &challenge)).as_ref()).into();
                }
                let body = info.to_string();
                let _ = socket
                    .write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes())
                    .await;
            }
        });
        base
    }

    #[tokio::test]
    async fn le_jeton_ne_part_qu_au_relais_qui_a_la_cle_retenue() {
        let http = Client::new();
        let plex = server("", "http://127.0.0.1:32400");
        let ask = |base: String, pinned: Option<String>| {
            let (http, plex) = (http.clone(), plex.clone());
            async move { Known::default().token_for(&http, &format!("{base}/seerr"), Some(&plex), pinned.as_deref(), |_| String::new()).await }
        };
        // Première rencontre : le jeton part, et la clé du relais est à retenir.
        let home = fake_relay(Some([1u8; 32])).await;
        let Decision::Send(Relay::Server(token), Some(key)) = ask(home.clone(), None).await else { panic!("le relais devait être reconnu") };
        assert_eq!(token, "t");
        // Ensuite : le même relais passe, sans rien de plus à retenir.
        assert_eq!(ask(home, Some(key.clone())).await, Decision::Send(Relay::Server("t".into()), None));
        // Une autre machine, qui annonce le même serveur : avec une autre clé, ou sans clé, elle ne reçoit rien.
        assert_eq!(ask(fake_relay(Some([2u8; 32])).await, Some(key.clone())).await, Decision::Refused);
        assert_eq!(ask(fake_relay(None).await, Some(key)).await, Decision::Refused);
        // Un Maku Server sans clé, jamais vu avec une : accepté comme avant.
        assert_eq!(ask(fake_relay(None).await, None).await, Decision::Send(Relay::Server("t".into()), None));
        // Une adresse qui n'est pas celle d'un relais, ou un relais qui ne répond pas : rien à joindre.
        assert_eq!(Known::default().token_for(&http, "https://seerr.example", Some(&plex), None, |_| String::new()).await, Decision::Direct);
        assert_eq!(ask("http://127.0.0.1:9".into(), None).await, Decision::Direct);
    }

    #[test]
    fn en_tetes_du_relais() {
        let server = relay_headers(Some(Relay::Server("jeton".into())));
        assert_eq!(server.get(TOKEN_HEADER).unwrap(), "jeton");
        assert!(server.get("Authorization").is_none());
        // Plugin : l'en-tête de Jellyfin, sous ses deux noms, et jamais X-Maku-Token.
        let plugin = relay_headers(Some(Relay::Plugin(r#"MediaBrowser Token="jeton""#.into())));
        assert_eq!(plugin.get("Authorization").unwrap(), r#"MediaBrowser Token="jeton""#);
        assert_eq!(plugin.get("X-Emby-Authorization").unwrap(), r#"MediaBrowser Token="jeton""#);
        assert!(plugin.get(TOKEN_HEADER).is_none());
        assert!(relay_headers(None).is_empty());
    }

    #[test]
    fn info_du_plugin_et_de_maku_server() {
        let server: Info = serde_json::from_str(r#"{"app":"maku-server","server":"plex","id":"a","seerr":true,"tracearr":true}"#).unwrap();
        assert!(!server.plugin);
        let plugin: Info = serde_json::from_str(r#"{"app":"maku-server","plugin":true,"version":"1.0.0","server":"jellyfin","id":"a","seerr":true,"tracearr":false}"#).unwrap();
        assert!(plugin.plugin);
    }
}
