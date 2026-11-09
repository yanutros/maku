//! Notes Allociné (presse et spectateurs) d'un film ou d'une série. Allociné n'a pas d'API publique et
//! Plex ne fournit pas ses notes : l'identifiant IMDb du titre donne son identifiant Allociné sur Wikidata
//! (propriétés P1265 pour un film, P1267 pour une série), puis la fiche Allociné donne les notes.
//! Seul l'identifiant IMDb part vers Wikidata, et l'identifiant Allociné vers Allociné ; l'utilisateur peut
//! couper cette recherche dans Réglages → Interface. Les réponses sont gardées le temps de la session.

use anyhow::{bail, Result};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::plex::SiteRating;

/// Wikidata demande aux applications de se présenter.
const AGENT: &str = concat!("Maku/", env!("CARGO_PKG_VERSION"), " (https://github.com/yanutros/maku)");

static CACHE: LazyLock<Mutex<HashMap<String, Vec<SiteRating>>>> = LazyLock::new(Default::default);

/// Notes Allociné du titre d'identifiant IMDb `imdb` (aucune si Allociné ne le connaît pas).
pub async fn ratings(http: &reqwest::Client, imdb: &str, show: bool) -> Result<Vec<SiteRating>> {
    if !valid_imdb(imdb) {
        bail!("identifiant IMDb invalide");
    }
    let key = format!("{imdb}:{show}");
    if let Some(hit) = CACHE.lock().unwrap().get(&key) {
        return Ok(hit.clone());
    }
    let found = match allocine_id(http, imdb, show).await? {
        Some(id) => {
            let url = if show {
                format!("https://www.allocine.fr/series/ficheserie_gen_cserie={id}.html")
            } else {
                format!("https://www.allocine.fr/film/fichefilm_gen_cfilm={id}.html")
            };
            let html = http.get(url).header("User-Agent", AGENT).send().await?.error_for_status()?.text().await?;
            parse(&html)
        }
        None => Vec::new(),
    };
    let mut cache = CACHE.lock().unwrap();
    if cache.len() > 500 {
        cache.clear();
    }
    cache.insert(key, found.clone());
    Ok(found)
}

fn valid_imdb(id: &str) -> bool {
    id.len() >= 3 && id.len() <= 14 && id.starts_with("tt") && id[2..].chars().all(|c| c.is_ascii_digit())
}

/// Identifiant Allociné d'après Wikidata.
async fn allocine_id(http: &reqwest::Client, imdb: &str, show: bool) -> Result<Option<String>> {
    let search: serde_json::Value = http
        .get(format!("https://www.wikidata.org/w/api.php?action=query&list=search&srsearch=haswbstatement:P345={imdb}&format=json"))
        .header("User-Agent", AGENT)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let Some(entity) = search["query"]["search"][0]["title"].as_str().filter(|q| q.starts_with('Q') && q[1..].chars().all(|c| c.is_ascii_digit())) else {
        return Ok(None);
    };
    let data: serde_json::Value = http
        .get(format!("https://www.wikidata.org/wiki/Special:EntityData/{entity}.json"))
        .header("User-Agent", AGENT)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let claims = &data["entities"][entity]["claims"];
    let prop = if show { "P1267" } else { "P1265" };
    let id = claims[prop][0]["mainsnak"]["datavalue"]["value"].as_str();
    Ok(id.filter(|i| !i.is_empty() && i.chars().all(|c| c.is_ascii_digit())).map(str::to_owned))
}

/// Notes de la fiche : chaque bloc `rating-item` porte un intitulé (« Presse », « Spectateurs ») et une note
/// sur 5 (`stareval-note`, « 4,2 »), ramenée ici sur 10.
fn parse(html: &str) -> Vec<SiteRating> {
    let mut out = Vec::new();
    for block in html.split("class=\"rating-item").skip(1) {
        let block = &block[..block.len().min(2000)];
        let text = strip_tags(block);
        let source = if text.contains("Presse") {
            "allocinePress"
        } else if text.contains("Spectateurs") {
            "allocineUsers"
        } else {
            continue;
        };
        let Some(note) = block.split("stareval-note").nth(1).and_then(|s| s.split_once('>')).map(|(_, s)| s.split('<').next().unwrap_or("").trim().replace(',', ".")) else {
            continue;
        };
        if let Ok(v) = note.parse::<f32>() {
            if v > 0.0 && v <= 5.0 && !out.iter().any(|r: &SiteRating| r.source == source) {
                out.push(SiteRating { source, value: v * 2.0, good: true });
            }
        }
    }
    out
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lit_les_notes_de_la_fiche() {
        let html = r#"<div class="rating-item"><div class="rating-item-content"><span class="rating-title"> Presse </span>
            <div class="stareval"><span class="stareval-note">3,4</span></div></div></div>
            <div class="rating-item"><span class="rating-title">Spectateurs</span><span class="stareval-note">4,1</span></div>
            <div class="rating-item"><span class="rating-title">Mes amis</span><span class="stareval-note">--</span></div>"#;
        let r = parse(html);
        assert_eq!(r.len(), 2);
        assert_eq!((r[0].source, r[0].value), ("allocinePress", 6.8));
        assert_eq!(r[1].source, "allocineUsers");
        assert!((r[1].value - 8.2).abs() < 0.01);
    }

    #[test]
    fn refuse_un_identifiant_douteux() {
        assert!(valid_imdb("tt0322259"));
        assert!(!valid_imdb("tt12&x=1"));
        assert!(!valid_imdb("nm0000123"));
    }

    /// Recherche réelle (lancée à la main : `cargo test allocine_reel -- --ignored --nocapture`).
    #[tokio::test]
    #[ignore]
    async fn allocine_reel() {
        let http = reqwest::Client::new();
        println!("{:?}", ratings(&http, "tt0322259", false).await.unwrap());
        println!("{:?}", ratings(&http, "tt1305826", true).await.unwrap());
    }
}
