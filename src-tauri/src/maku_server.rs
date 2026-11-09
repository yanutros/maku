//! Maku Server (dossier `server/` du dépôt) : relais Seerr et Tracearr installé par l'administrateur
//! à côté de son serveur, pour ne pas exposer Seerr et Tracearr sur Internet.
//!
//! Ses adresses se terminent par `/seerr` ou `/tracearr`. Chaque appel y porte le jeton de
//! l'utilisateur auprès du serveur multimédia (`X-Maku-Token`), que le relais vérifie. Le jeton n'est
//! joint qu'après avoir vérifié, par `/maku/info`, que l'adresse est bien celle du relais de ce serveur.

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
}

/// En-têtes de chaque appel : le jeton du serveur quand l'adresse passe par Maku Server.
pub fn relay_headers(relay: Option<String>) -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    if let Some(mut v) = relay.and_then(|t| reqwest::header::HeaderValue::from_str(&t).ok()) {
        v.set_sensitive(true);
        h.insert(crate::maku_server::TOKEN_HEADER, v);
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
    /// Jeton à joindre aux appels vers `url` : celui du serveur, si `url` passe par son relais.
    pub async fn token_for(&self, http: &Client, url: &str, server: Option<&SavedServer>) -> Option<String> {
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
        serves(&found, server).then(|| server.token.clone())
    }

}

/// Cherche le relais sur la machine du serveur, à son port par défaut : à l'adresse enregistrée, et aux
/// autres adresses connues du serveur (`others` : celles du réseau local d'abord). Avec Plex, l'adresse
/// enregistrée est souvent l'adresse distante, que le port du relais ne traverse pas.
pub async fn discover(http: &Client, server: &SavedServer, others: Vec<String>) -> Option<(String, Info)> {
    let saved = Url::parse(&server.uri).ok().and_then(|u| u.host_str().map(str::to_owned));
    let mut hosts: Vec<String> = Vec::new();
    for h in others.into_iter().chain(saved) {
        let h = h.trim_start_matches('[').trim_end_matches(']').to_owned();
        if !h.is_empty() && !hosts.contains(&h) {
            hosts.push(h);
        }
    }
    for host in hosts {
        let host = if host.contains(':') { format!("[{host}]") } else { host };
        for scheme in ["http", "https"] {
            let base = format!("{scheme}://{host}:{PORT}");
            if let Some(i) = info(http, &base).await.filter(|i| serves(i, server)) {
                return Some((base, i));
            }
        }
    }
    None
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
        let mut info = Info { app: "maku-server".into(), server: "plex".into(), id: Some("abc".into()), seerr: true, tracearr: false };
        assert!(serves(&info, &server));
        info.id = Some("autre".into());
        assert!(!serves(&info, &server));
        info.id = Some("abc".into());
        info.server = "jellyfin".into();
        assert!(!serves(&info, &server));
    }
}
