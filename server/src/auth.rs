//! Vérification du jeton de l'utilisateur auprès du serveur multimédia, avec un petit cache.

use crate::{App, Kind};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Un jeton accepté l'est pour 10 minutes ; un jeton refusé, pour 30 secondes.
const VALID_FOR: Duration = Duration::from_secs(600);
const INVALID_FOR: Duration = Duration::from_secs(30);
/// Au-delà, le cache repart de zéro (il ne garde que des jetons, en mémoire).
const MAX_ENTRIES: usize = 2000;

#[derive(Default)]
pub struct Cache(Mutex<HashMap<String, (bool, Instant)>>);

pub enum Verdict {
    Allowed,
    Denied,
    /// Le serveur multimédia n'a pas répondu : impossible de trancher.
    Unavailable,
}

impl Cache {
    fn get(&self, token: &str) -> Option<bool> {
        let map = self.0.lock().unwrap();
        let (ok, at) = map.get(token)?;
        let ttl = if *ok { VALID_FOR } else { INVALID_FOR };
        (at.elapsed() < ttl).then_some(*ok)
    }

    fn put(&self, token: &str, ok: bool) {
        let mut map = self.0.lock().unwrap();
        if map.len() >= MAX_ENTRIES {
            map.clear();
        }
        map.insert(token.to_owned(), (ok, Instant::now()));
    }
}

/// Le jeton est-il celui d'un utilisateur du serveur ?
pub async fn check(app: &App, token: &str) -> Verdict {
    if token.is_empty() || token.len() > 512 {
        return Verdict::Denied;
    }
    if let Some(ok) = app.auth.get(token) {
        return if ok { Verdict::Allowed } else { Verdict::Denied };
    }
    let base = &app.cfg.media;
    // Une page qui demande d'être connecté : la liste des bibliothèques (Plex, où un ami n'y voit que
    // celles qu'on lui partage), ou les informations du serveur (Jellyfin, Emby).
    let req = match app.cfg.kind {
        Kind::Plex => app.http.get(format!("{base}/library/sections")).header("X-Plex-Token", token),
        Kind::Jellyfin => app
            .http
            .get(format!("{base}/System/Info"))
            .header("Authorization", format!("MediaBrowser Token=\"{token}\"")),
        Kind::Emby => app.http.get(format!("{base}/System/Info")).header("X-Emby-Token", token),
    };
    match req.header("Accept", "application/json").timeout(Duration::from_secs(10)).send().await {
        Ok(r) if r.status().is_success() => {
            app.auth.put(token, true);
            Verdict::Allowed
        }
        Ok(r) if matches!(r.status().as_u16(), 401 | 403) => {
            app.auth.put(token, false);
            Verdict::Denied
        }
        _ => Verdict::Unavailable,
    }
}

/// Identifiant du serveur multimédia (le même que celui que voit Maku), s'il répond.
pub async fn server_id(app: &App) -> Option<String> {
    let base = &app.cfg.media;
    let (url, key) = match app.cfg.kind {
        Kind::Plex => (format!("{base}/identity"), "/MediaContainer/machineIdentifier"),
        Kind::Jellyfin | Kind::Emby => (format!("{base}/System/Info/Public"), "/Id"),
    };
    let r = app.http.get(url).header("Accept", "application/json").timeout(Duration::from_secs(5)).send().await.ok()?;
    let v: serde_json::Value = r.json().await.ok()?;
    v.pointer(key)?.as_str().map(str::to_owned)
}
