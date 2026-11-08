//! Paroles des morceaux, synchronisées quand c'est possible, depuis lrclib.net (gratuit, sans compte ni clé).
//! On lui envoie l'artiste, le titre, l'album et la durée du morceau, rien d'autre ; l'utilisateur peut couper
//! la recherche dans Réglages → Lecteur.

use anyhow::Result;
use serde::{Deserialize, Serialize};

const API: &str = "https://lrclib.net/api";
/// lrclib demande aux applications de se présenter.
const AGENT: &str = concat!("Maku/", env!("CARGO_PKG_VERSION"), " (https://github.com/yanutros/maku)");

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    /// Paroles au format LRC (`[01:23.45] texte`), quand lrclib les a synchronisées.
    pub synced: Option<String>,
    /// Paroles sans minutage.
    pub plain: Option<String>,
    pub instrumental: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    synced_lyrics: Option<String>,
    plain_lyrics: Option<String>,
    #[serde(default)]
    instrumental: bool,
    #[serde(default)]
    duration: f64,
}

impl From<Record> for Lyrics {
    fn from(r: Record) -> Self {
        let keep = |s: Option<String>| s.filter(|s| !s.trim().is_empty());
        Lyrics { synced: keep(r.synced_lyrics), plain: keep(r.plain_lyrics), instrumental: r.instrumental }
    }
}

/// Paroles d'un morceau : la fiche exacte (artiste, titre, album, durée), sinon la meilleure d'une recherche
/// (synchronisée et de durée proche d'abord). Rien de trouvé : des paroles vides.
pub async fn fetch(http: &reqwest::Client, artist: &str, title: &str, album: Option<&str>, duration_s: f64) -> Result<Lyrics> {
    let mut query = vec![("artist_name", artist.to_owned()), ("track_name", title.to_owned()), ("duration", format!("{}", duration_s.round()))];
    if let Some(album) = album.filter(|a| !a.is_empty()) {
        query.push(("album_name", album.to_owned()));
    }
    let resp = http.get(format!("{API}/get")).header("User-Agent", AGENT).query(&query).send().await?;
    if resp.status().is_success() {
        return Ok(resp.json::<Record>().await?.into());
    }
    if resp.status() != reqwest::StatusCode::NOT_FOUND {
        resp.error_for_status()?;
    }
    let found: Vec<Record> = http
        .get(format!("{API}/search"))
        .header("User-Agent", AGENT)
        .query(&[("artist_name", artist), ("track_name", title)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let best = found
        .into_iter()
        // Une version d'une autre durée (live, remix) aurait des paroles décalées.
        .filter(|r| duration_s <= 0.0 || (r.duration - duration_s).abs() <= 3.0)
        .max_by_key(|r| (r.synced_lyrics.is_some(), r.plain_lyrics.is_some()));
    Ok(best.map(Lyrics::from).unwrap_or_default())
}
