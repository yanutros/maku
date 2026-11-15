//! Maku Server (dossier `server/` du dépôt) : relais Seerr et Tracearr installé par l'administrateur
//! à côté de son serveur, pour ne pas exposer Seerr et Tracearr sur Internet. Avec Jellyfin, le plugin Maku
//! (dossier `plugins/jellyfin/`) fait de même depuis le serveur lui-même, à son adresse (`…/Maku`).
//!
//! Ses adresses se terminent par `/seerr` ou `/tracearr`. Chaque appel y porte le jeton de
//! l'utilisateur auprès du serveur multimédia (`X-Maku-Token` pour Maku Server, l'en-tête de Jellyfin pour
//! le plugin), que le relais vérifie. Le jeton n'est joint qu'après avoir vérifié, par `/maku/info`, que
//! l'adresse est bien celle du relais de ce serveur.

use crate::config::SavedServer;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

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

pub async fn info(http: &Client, base: &str) -> Option<Info> {
    let r = http.get(format!("{base}/maku/info")).timeout(Duration::from_secs(3)).send().await.ok()?;
    let info: Info = r.json().await.ok()?;
    (info.app == "maku-server").then_some(info)
}

/// Ce relais est-il celui de ce serveur ?
fn serves(info: &Info, server: &SavedServer) -> bool {
    let kind = if server.kind.is_empty() { "plex" } else { server.kind.as_str() };
    info.server == kind && info.id.as_deref() == Some(server.machine_id.as_str())
}

/// Relais déjà interrogés, par adresse.
#[derive(Default)]
pub struct Known(Mutex<HashMap<String, Info>>);

impl Known {
    /// Ce qu'il faut joindre aux appels vers `url` : le jeton du serveur, si `url` passe par son relais.
    /// `authorization` : l'en-tête de Jellyfin de ce compte, pour le plugin.
    pub async fn token_for(&self, http: &Client, url: &str, server: Option<&SavedServer>, authorization: impl FnOnce(&str) -> String) -> Option<Relay> {
        let server = server?;
        let base = relay_base(url)?.to_owned();
        let cached = self.0.lock().unwrap().get(&base).cloned();
        let found = match cached {
            Some(i) => i,
            None => {
                // Injoignable : réessayé à la prochaine connexion.
                let i = info(http, &base).await?;
                self.0.lock().unwrap().insert(base, i.clone());
                i
            }
        };
        if !serves(&found, server) {
            return None;
        }
        Some(if found.plugin { Relay::Plugin(authorization(&server.token)) } else { Relay::Server(server.token.clone()) })
    }

}

/// Cherche le relais sur la machine du serveur, à son port par défaut : à l'adresse enregistrée, et aux
/// autres adresses connues du serveur (`others` : celles du réseau local d'abord). Avec Plex, l'adresse
/// enregistrée est souvent l'adresse distante, que le port du relais ne traverse pas.
pub async fn discover(http: &Client, server: &SavedServer, others: Vec<String>) -> Option<(String, Info)> {
    // Jellyfin : le plugin Maku, à l'adresse même du serveur (sur le réseau local comme depuis Internet).
    if server.is_jellyfin() {
        let base = format!("{}/Maku", server.uri.trim_end_matches('/'));
        if let Some(i) = info(http, &base).await.filter(|i| i.plugin && serves(i, server)) {
            return Some((base, i));
        }
    }
    for base in candidates(server, others) {
        if let Some(i) = info(http, &base).await.filter(|i| serves(i, server)) {
            return Some((base, i));
        }
    }
    None
}

/// Adresses où chercher Maku Server : chaque machine connue du serveur, à son port par défaut. En `https`
/// d'abord : l'adresse retenue est celle où partiront ensuite les jetons, qui ne passent en clair que si le
/// relais ne répond pas autrement (réseau local, sans proxy inverse).
fn candidates(server: &SavedServer, others: Vec<String>) -> Vec<String> {
    let saved = Url::parse(&server.uri).ok().and_then(|u| u.host_str().map(str::to_owned));
    let mut hosts: Vec<String> = Vec::new();
    for h in others.into_iter().chain(saved) {
        let h = h.trim_start_matches('[').trim_end_matches(']').to_owned();
        if !h.is_empty() && !hosts.contains(&h) {
            hosts.push(h);
        }
    }
    let mut bases = Vec::new();
    for host in hosts {
        let host = if host.contains(':') { format!("[{host}]") } else { host };
        for scheme in ["https", "http"] {
            bases.push(format!("{scheme}://{host}:{PORT}"));
        }
    }
    bases
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
        let mut info = Info { app: "maku-server".into(), server: "plex".into(), id: Some("abc".into()), seerr: true, tracearr: false, plugin: false };
        assert!(serves(&info, &server));
        info.id = Some("autre".into());
        assert!(!serves(&info, &server));
        info.id = Some("abc".into());
        info.server = "jellyfin".into();
        assert!(!serves(&info, &server));
    }

    #[test]
    fn relais_cherche_en_https_d_abord() {
        let server = SavedServer { name: "s".into(), machine_id: "abc".into(), uri: "https://h.example:32400".into(), token: "t".into(), kind: String::new(), user_id: None };
        let found = candidates(&server, vec!["10.0.0.2".into(), "fe80::1".into(), "10.0.0.2".into()]);
        assert_eq!(
            found,
            [
                "https://10.0.0.2:32700",
                "http://10.0.0.2:32700",
                "https://[fe80::1]:32700",
                "http://[fe80::1]:32700",
                "https://h.example:32700",
                "http://h.example:32700",
            ]
        );
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
