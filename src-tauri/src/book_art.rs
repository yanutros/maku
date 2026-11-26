//! Illustrations en ligne des séries Kavita (mangas et light novels), pour une fiche aussi immersive que celle
//! d'un film : la bannière d'AniList (base libre, sans clé) et, avec la clé TMDB de l'utilisateur, le logo et le
//! fond de l'anime adapté de la série. Seul le nom de la série part en ligne (ou son numéro AniList, si Kavita le
//! connaît). Une correspondance approximative ne suffit pas : le titre doit être le même, sans quoi la série
//! garde sa couverture (une BD n'a ni bannière AniList ni anime).

use reqwest::Client;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

#[derive(Serialize, Default, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BookArt {
    /// Grande image de fond (paysage).
    pub backdrop: Option<String>,
    /// Logo du titre (« clear logo »), sur fond transparent.
    pub logo: Option<String>,
}

/// Réponses déjà obtenues pendant la session (une série n'est cherchée qu'une fois).
static CACHE: LazyLock<Mutex<HashMap<String, BookArt>>> = LazyLock::new(Default::default);

/// Titre ramené à l'essentiel pour comparer : minuscules, sans accents ni ponctuation, « × » lu comme « x ».
pub fn simplify(title: &str) -> String {
    title
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'ā' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'ē' => 'e',
            'î' | 'ï' | 'í' | 'ī' => 'i',
            'ô' | 'ö' | 'ó' | 'õ' | 'ō' => 'o',
            'ù' | 'û' | 'ü' | 'ú' | 'ū' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            '×' => 'x',
            c => c,
        })
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Fiche AniList retenue : sa bannière et ses titres (pour chercher l'anime sur TMDB).
#[derive(Debug, PartialEq)]
pub struct AniListHit {
    pub banner: Option<String>,
    pub titles: Vec<String>,
}

/// La fiche AniList de la réponse, si l'un de ses titres est celui de la série (ou si elle vient du numéro AniList
/// donné par Kavita : `trusted`).
pub fn pick_anilist(v: &Value, name: &str, trusted: bool) -> Option<AniListHit> {
    let m = &v["data"]["Media"];
    if !m.is_object() {
        return None;
    }
    let mut titles: Vec<String> = ["english", "romaji", "native"].iter().filter_map(|k| m["title"][*k].as_str().map(str::to_owned)).collect();
    titles.extend(m["synonyms"].as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_owned)));
    let wanted = simplify(name);
    if !trusted && !titles.iter().any(|t| simplify(t) == wanted) {
        return None;
    }
    let banner = m["bannerImage"].as_str().filter(|u| u.starts_with("https://")).map(str::to_owned);
    Some(AniListHit { banner, titles })
}

/// La série animée japonaise de la recherche TMDB dont le titre (ou le titre original) est l'un des noms donnés.
pub fn pick_tmdb(v: &Value, names: &[String]) -> Option<u64> {
    let wanted: Vec<String> = names.iter().map(|n| simplify(n)).filter(|n| !n.is_empty()).collect();
    v["results"].as_array()?.iter().find_map(|r| {
        let animated = r["genre_ids"].as_array().is_some_and(|g| g.iter().any(|x| x == 16));
        let japanese = r["original_language"] == "ja" || r["origin_country"].as_array().is_some_and(|c| c.iter().any(|x| x == "JP"));
        let same = ["name", "original_name"].iter().any(|k| r[*k].as_str().is_some_and(|t| wanted.contains(&simplify(t))));
        (animated && japanese && same).then(|| r["id"].as_u64()).flatten()
    })
}

/// Logo d'une série TMDB : dans la langue de l'interface, sinon en anglais, sinon sans texte (jamais en SVG).
pub fn pick_logo(images: &Value, ui: &str) -> Option<String> {
    let logos = images["logos"].as_array()?;
    let pick = |lang: Option<&str>| logos.iter().find(|l| l["iso_639_1"].as_str() == lang && l["file_path"].as_str().is_some_and(|p| !p.ends_with(".svg")));
    let logo = pick(Some(ui)).or_else(|| pick(Some("en"))).or_else(|| pick(None))?;
    Some(format!("https://image.tmdb.org/t/p/w500{}", logo["file_path"].as_str()?))
}

/// Fond d'une série TMDB : le mieux noté des fonds sans texte, sinon le premier.
pub fn pick_backdrop(images: &Value) -> Option<String> {
    let all = images["backdrops"].as_array()?;
    let score = |b: &&Value| (b["vote_average"].as_f64().unwrap_or(0.0) * 1000.0) as i64;
    let best = all.iter().filter(|b| b["iso_639_1"].is_null()).max_by_key(score).or_else(|| all.first())?;
    Some(format!("https://image.tmdb.org/t/p/w1280{}", best["file_path"].as_str()?))
}

const ANILIST_QUERY: &str =
    "query($id:Int,$search:String){Media(id:$id,search:$search,type:MANGA){id bannerImage synonyms title{english romaji native}}}";

async fn anilist(http: &Client, name: &str, id: Option<i64>) -> Result<Option<AniListHit>, ()> {
    let variables = match id {
        Some(id) => json!({ "id": id }),
        None => json!({ "search": name }),
    };
    let resp = http
        .post("https://graphql.anilist.co")
        .timeout(Duration::from_secs(8))
        .header("Accept", "application/json")
        .json(&json!({ "query": ANILIST_QUERY, "variables": variables }))
        .send()
        .await
        .map_err(|_| ())?;
    // Fiche introuvable : AniList répond 404, c'est une réponse (pas une panne).
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let v: Value = resp.error_for_status().map_err(|_| ())?.json().await.map_err(|_| ())?;
    Ok(pick_anilist(&v, name, id.is_some()))
}

async fn tmdb_json(http: &Client, key: &str, path: &str) -> Result<Value, ()> {
    crate::plex::tmdb_get(http, key, path)
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .map_err(|_| ())?
        .error_for_status()
        .map_err(|_| ())?
        .json()
        .await
        .map_err(|_| ())
}

/// Logo et fond de l'anime adapté de la série, sur TMDB.
async fn tmdb(http: &Client, key: &str, names: &[String], ui: &str) -> Result<BookArt, ()> {
    for name in names.iter().take(3) {
        let found = tmdb_json(http, key, &format!("/search/tv?query={}&language={ui}", urlencoding::encode(name))).await?;
        if let Some(id) = pick_tmdb(&found, names) {
            let images = tmdb_json(http, key, &format!("/tv/{id}/images?include_image_language={ui},en,null")).await?;
            return Ok(BookArt { backdrop: pick_backdrop(&images), logo: pick_logo(&images, ui) });
        }
    }
    Ok(BookArt::default())
}

/// Films (`movie`) et séries (`tv`) d'une recherche TMDB dont le titre, ou le titre original, est exactement l'un des
/// noms donnés.
pub fn exact_matches(v: &Value, kind: &'static str, names: &[String]) -> Vec<(&'static str, u64)> {
    let wanted: Vec<String> = names.iter().map(|n| simplify(n)).filter(|n| !n.is_empty()).collect();
    let keys: [&str; 2] = if kind == "tv" { ["name", "original_name"] } else { ["title", "original_title"] };
    v["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| keys.iter().any(|k| r[*k].as_str().is_some_and(|t| wanted.contains(&simplify(t)))))
        .filter_map(|r| Some((kind, r["id"].as_u64()?)))
        .collect()
}

/// La seule adaptation (série ou film) au titre exact de la BD ; aucune, ou plusieurs (le doute) : rien.
pub fn single_adaptation(tv: &Value, movie: &Value, name: &str) -> Option<(&'static str, u64)> {
    let names = [name.to_owned()];
    let mut found = exact_matches(tv, "tv", &names);
    found.extend(exact_matches(movie, "movie", &names));
    match found[..] {
        [one] => Some(one),
        _ => None,
    }
}

/// BD et comics : le logo et le fond de leur adaptation (film ou série, animée ou non) sur TMDB. Aucune base libre
/// ne donne de logos de BD ; une adaptation en a souvent. Il faut un titre identique et une seule adaptation possible :
/// deux œuvres du même nom, c'est le doute, et la série garde sa couverture.
async fn tmdb_adaptation(http: &Client, key: &str, name: &str, ui: &str) -> Result<BookArt, ()> {
    let query = urlencoding::encode(name);
    let tv = tmdb_json(http, key, &format!("/search/tv?query={query}&language={ui}")).await?;
    let movie = tmdb_json(http, key, &format!("/search/movie?query={query}&language={ui}")).await?;
    let Some((kind, id)) = single_adaptation(&tv, &movie, name) else { return Ok(BookArt::default()) };
    let images = tmdb_json(http, key, &format!("/{kind}/{id}/images?include_image_language={ui},en,null")).await?;
    Ok(BookArt { backdrop: pick_backdrop(&images), logo: pick_logo(&images, ui) })
}

/// Illustrations d'une série : pour un manga ou un light novel, la bannière AniList, puis le logo et le fond TMDB de
/// son anime (préféré à la bannière, plus grand) ; pour une BD ou un comic (`kind` « comic »), ceux de son
/// adaptation sur TMDB (avec la clé de l'utilisateur).
pub async fn find(http: &Client, name: &str, kind: &str, anilist_id: Option<i64>, tmdb_key: Option<&str>, ui: &str) -> BookArt {
    if name.trim().is_empty() || (kind == "comic" && tmdb_key.is_none()) {
        return BookArt::default();
    }
    let cache_key = format!("{kind}|{anilist_id:?}|{}|{}", tmdb_key.is_some(), simplify(name));
    if let Some(art) = CACHE.lock().unwrap().get(&cache_key) {
        return art.clone();
    }
    if let (true, Some(key)) = (kind == "comic", tmdb_key) {
        // TMDB injoignable : rien n'est retenu, on cherchera de nouveau.
        let Ok(art) = tmdb_adaptation(http, key, name, ui).await else { return BookArt::default() };
        CACHE.lock().unwrap().insert(cache_key, art.clone());
        return art;
    }
    let Ok(hit) = anilist(http, name, anilist_id).await else { return BookArt::default() };
    let mut names = vec![name.to_owned()];
    for t in hit.as_ref().map(|h| h.titles.clone()).unwrap_or_default() {
        if !names.iter().any(|n| simplify(n) == simplify(&t)) {
            names.push(t);
        }
    }
    let online = match tmdb_key {
        Some(key) => match tmdb(http, key, &names, ui).await {
            Ok(art) => art,
            // TMDB injoignable : rien n'est retenu, on cherchera de nouveau.
            Err(()) => return BookArt { backdrop: hit.and_then(|h| h.banner), logo: None },
        },
        None => BookArt::default(),
    };
    let art = BookArt { backdrop: online.backdrop.or(hit.and_then(|h| h.banner)), logo: online.logo };
    CACHE.lock().unwrap().insert(cache_key, art.clone());
    art
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titres_compares_sans_accents_ni_ponctuation() {
        assert_eq!(simplify("Lune × Acier"), "lunexacier");
        assert_eq!(simplify("LUNE x ACIER"), "lunexacier");
        assert_eq!(simplify("Shingeki no Kyōjin"), "shingekinokyojin");
        assert_eq!(simplify("L'Été, déjà !"), "letedeja");
    }

    #[test]
    fn anilist_seulement_si_le_titre_est_le_meme() {
        let v = json!({ "data": { "Media": {
            "id": 12345, "bannerImage": "https://s4.anilist.co/banner.jpg", "synonyms": ["Kaze no Kōro"],
            "title": { "english": "Wind Road", "romaji": "KAZE NO MICHI", "native": "風の道" }
        } } });
        let hit = pick_anilist(&v, "wind road", false).unwrap();
        assert_eq!(hit.banner.as_deref(), Some("https://s4.anilist.co/banner.jpg"));
        assert!(hit.titles.contains(&"KAZE NO MICHI".to_owned()));
        // Un synonyme compte aussi.
        assert!(pick_anilist(&v, "Kaze no Koro", false).is_some());
        // Une recherche approximative (« Prairie Bleue » → autre chose) ne suffit pas…
        assert_eq!(pick_anilist(&v, "Prairie Bleue", false), None);
        // … sauf avec le numéro AniList que donne Kavita.
        assert!(pick_anilist(&v, "Prairie Bleue", true).is_some());
        assert_eq!(pick_anilist(&json!({ "data": { "Media": null } }), "x", true), None);
    }

    #[test]
    fn tmdb_l_anime_japonais_du_meme_titre() {
        let v = json!({ "results": [
            { "id": 1, "name": "Lune x Acier", "original_name": "Lune x Acier", "genre_ids": [18], "origin_country": ["US"], "original_language": "en" },
            { "id": 2, "name": "Lune × Acier (1999)", "original_name": "ルナ×アシエ", "genre_ids": [16], "origin_country": ["JP"] },
            { "id": 46298, "name": "Lune x Acier", "original_name": "LUNE×ACIER", "genre_ids": [16, 10759], "origin_country": ["JP"], "original_language": "ja" }
        ] });
        // Pas la série filmée du même nom, pas l'anime d'un autre titre : celui-là.
        assert_eq!(pick_tmdb(&v, &["Lune X Acier".into()]), Some(46298));
        assert_eq!(pick_tmdb(&v, &["Prairie Bleue".into()]), None);
    }

    #[test]
    fn logo_et_fond_tmdb() {
        let images = json!({
            "logos": [
                { "iso_639_1": "ja", "file_path": "/ja.png" },
                { "iso_639_1": "en", "file_path": "/en.svg" },
                { "iso_639_1": "en", "file_path": "/en.png" },
                { "iso_639_1": null, "file_path": "/sans.png" }
            ],
            "backdrops": [
                { "iso_639_1": "en", "file_path": "/texte.jpg", "vote_average": 9.0 },
                { "iso_639_1": null, "file_path": "/b1.jpg", "vote_average": 5.2 },
                { "iso_639_1": null, "file_path": "/b2.jpg", "vote_average": 5.6 }
            ]
        });
        assert_eq!(pick_logo(&images, "fr").as_deref(), Some("https://image.tmdb.org/t/p/w500/en.png"));
        assert_eq!(pick_logo(&images, "ja").as_deref(), Some("https://image.tmdb.org/t/p/w500/ja.png"));
        // Le mieux noté des fonds sans texte.
        assert_eq!(pick_backdrop(&images).as_deref(), Some("https://image.tmdb.org/t/p/w1280/b2.jpg"));
        assert_eq!(pick_logo(&json!({}), "fr"), None);
    }

    #[tokio::test]
    async fn bd_sans_cle_tmdb_rien_a_chercher() {
        // Ni AniList (il ne connaît pas les BD) ni TMDB sans clé : aucune requête.
        assert_eq!(find(&Client::new(), "Prairie Bleue", "comic", None, None, "fr").await, BookArt::default());
    }

    #[test]
    fn bd_l_adaptation_du_meme_titre() {
        let tv = json!({ "results": [
            { "id": 7, "name": "Capitaine Comète", "original_name": "Capitaine Comète" },
            { "id": 8, "name": "Les Nouvelles Aventures de Capitaine Comète", "original_name": "Les Nouvelles Aventures de Capitaine Comète" }
        ] });
        let movie = json!({ "results": [
            { "id": 70, "title": "Orion et Véga : Mission Lune", "original_title": "Orion & Véga : Mission Lune" },
            { "id": 71, "title": "The Adventures of Zephyr", "original_title": "Les Aventures de Zéphyr" }
        ] });
        // Le titre exact (ou le titre original), sans les accents : rien d'approchant.
        assert_eq!(exact_matches(&tv, "tv", &["Capitaine Comète".into()]), [("tv", 7)]);
        assert_eq!(exact_matches(&movie, "movie", &["Les aventures de Zephyr".into()]), [("movie", 71)]);
        assert!(exact_matches(&movie, "movie", &["Orion".into()]).is_empty());
        assert!(exact_matches(&tv, "tv", &["Prairie Bleue".into()]).is_empty());
        // Une seule adaptation : retenue ; la même œuvre en série et en film, c'est le doute : rien.
        assert_eq!(single_adaptation(&tv, &movie, "Capitaine Comète"), Some(("tv", 7)));
        let film = json!({ "results": [{ "id": 90, "title": "Capitaine Comète", "original_title": "Capitaine Comète" }] });
        assert_eq!(single_adaptation(&tv, &film, "Capitaine Comète"), None);
        assert_eq!(single_adaptation(&tv, &movie, "Prairie Bleue"), None);
    }
}
