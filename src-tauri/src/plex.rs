use anyhow::{anyhow, Result};
use futures::future::join_all;
use reqwest::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const PRODUCT: &str = "Maku";
const PLEX_TV: &str = "https://plex.tv/api/v2";

/// En-têtes attendus par plex.tv et par les serveurs Plex.
pub fn with_plex_headers(rb: RequestBuilder, client_id: &str, token: Option<&str>) -> RequestBuilder {
    let rb = rb
        .header("Accept", "application/json")
        .header("X-Plex-Product", PRODUCT)
        .header("X-Plex-Version", env!("CARGO_PKG_VERSION"))
        .header("X-Plex-Client-Identifier", client_id)
        .header("X-Plex-Platform", std::env::consts::OS)
        .header("X-Plex-Device-Name", "Maku (PC)")
        // Titres des rangées (« Musique lue récemment »…) en français.
        // Titres des rangées, genres… dans la langue de l'interface.
        .header("X-Plex-Language", crate::i18n::language());
    match token {
        Some(t) => rb.header("X-Plex-Token", t),
        None => rb,
    }
}

// ---------- plex.tv : authentification par code PIN ----------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pin {
    id: u64,
    code: String,
    auth_token: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginPin {
    pub pin_id: u64,
    pub auth_url: String,
    /// Code à saisir sur plex.tv/link (connexion depuis un autre appareil).
    pub code: String,
}

/// `link` : code court à 4 caractères, saisi sur plex.tv/link depuis un téléphone ou un ordinateur
/// (pour les téléviseurs) ; sinon code long validé dans le navigateur de l'appareil.
pub async fn create_pin(http: &Client, client_id: &str, link: bool) -> Result<LoginPin> {
    let pin: Pin = with_plex_headers(http.post(format!("{PLEX_TV}/pins?strong={}", !link)), client_id, None)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let auth_url = format!(
        "https://app.plex.tv/auth#?clientID={}&code={}&context%5Bdevice%5D%5Bproduct%5D={}",
        urlencoding::encode(client_id),
        urlencoding::encode(&pin.code),
        PRODUCT
    );
    Ok(LoginPin { pin_id: pin.id, auth_url, code: pin.code })
}

/// Renvoie le jeton une fois que l'utilisateur a validé le code dans son navigateur.
pub async fn check_pin(http: &Client, client_id: &str, pin_id: u64) -> Result<Option<String>> {
    let pin: Pin = with_plex_headers(http.get(format!("{PLEX_TV}/pins/{pin_id}")), client_id, None)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(pin.auth_token.filter(|t| !t.is_empty()))
}

// ---------- plex.tv : découverte des serveurs ----------

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub name: String,
    pub client_identifier: String,
    #[serde(default)]
    pub provides: String,
    pub access_token: Option<String>,
    #[serde(default)]
    pub owned: bool,
    /// Propriétaire d'un serveur partagé (identifiant plex.tv et nom).
    pub owner_id: Option<u64>,
    pub source_title: Option<String>,
    #[serde(default)]
    pub connections: Vec<Connection>,
}

/// Logo d'un film ou d'une série sur le service de métadonnées de Plex, quand le serveur n'en a
/// pas (ses métadonnées sont plus anciennes ou moins complètes). Adresse publique de l'image.
/// Titre d'épisode générique (« Épisode 6 », « Episode #1.6 »…) : il répète le code « S1 E6 ».
fn generic_episode_title(title: &str) -> bool {
    let t = title.trim().to_lowercase();
    let rest = ["épisode", "episode", "episodio", "folge", "aflevering", "ep.", "ep", "e"]
        .iter()
        .find_map(|p| t.strip_prefix(p))
        .map(|r| r.trim_start().trim_start_matches('#').trim());
    matches!(rest, Some(r) if !r.is_empty() && r.chars().all(|c| c.is_ascii_digit() || c == '.' || c == 'x'))
}

/// « S1 · E6 — Titre », ou « S1 · E6 » seul quand le titre n'apprend rien.
pub(crate) fn episode_line(season: u32, episode: u32, title: &str) -> String {
    let code = [("season", season.to_string()), ("episode", episode.to_string())];
    if generic_episode_title(title) {
        format!("S{season} · E{episode}")
    } else {
        crate::i18n::tr_with(
            "plex.episodeLine",
            &[("season", code[0].1.as_str()), ("episode", code[1].1.as_str()), ("title", title)],
        )
    }
}

/// Booléen que Plex envoie tantôt tel quel (playlists), tantôt en texte « 1 » / « 0 » (collections).
fn flag<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<bool, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Flag {
        Bool(bool),
        Num(i64),
        Text(String),
    }
    Ok(match Option::<Flag>::deserialize(d)? {
        Some(Flag::Bool(b)) => b,
        Some(Flag::Num(n)) => n != 0,
        Some(Flag::Text(t)) => t == "1" || t.eq_ignore_ascii_case("true"),
        None => false,
    })
}

/// Logo d'un titre, cherché en ligne : sur le service de Plex, puis — en dernier recours, sans aucun
/// logo chez Plex — sur TMDB si l'utilisateur a donné sa clé. `fallback` : à défaut de logo dans la langue de l'interface, celui en anglais (souvent
/// le seul, comme sur Jellyfin) ; sans `fallback`, seulement un logo traduit (pour remplacer celui du
/// serveur, souvent en anglais).
pub async fn remote_logo(
    http: &Client,
    client_id: &str,
    token: &str,
    guid: &str,
    fallback: bool,
    tmdb_key: Option<&str>,
) -> Result<Option<String>> {
    let id = guid.rsplit('/').next().unwrap_or_default();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Ok(None);
    }
    let ui = crate::i18n::language();
    let mut langs = vec![];
    if ui != "en" {
        langs.push(ui);
    }
    if fallback {
        langs.push("en");
    }
    let mut tmdb_id = None;
    for lang in &langs {
        // Sans `includeImages`, le service ne renvoie pas la liste des images (logos compris).
        let v: serde_json::Value = http
            .get(format!("https://metadata.provider.plex.tv/library/metadata/{id}?includeImages=1"))
            .header("Accept", "application/json")
            .header("X-Plex-Product", PRODUCT)
            .header("X-Plex-Client-Identifier", client_id)
            .header("X-Plex-Language", *lang)
            .header("X-Plex-Token", token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let meta = &v["MediaContainer"]["Metadata"][0];
        tmdb_id = tmdb_id.or_else(|| {
            meta["Guid"].as_array()?.iter().find_map(|g| g["id"].as_str()?.strip_prefix("tmdb://").map(str::to_owned))
        });
        let images = meta["Image"].as_array().cloned().unwrap_or_default();
        // Logo normal, sinon sa version large (c'est parfois la seule traduite, chez Fanart.tv).
        let logo = ["clearLogo", "clearLogoWide"]
            .iter()
            .find_map(|kind| images.iter().find(|i| i["type"] == *kind).and_then(|i| i["url"].as_str()))
            .map(|u| u.replacen("http://assets.fanart.tv/", "https://assets.fanart.tv/", 1))
            .filter(|u| u.starts_with("https://"));
        if logo.is_some() {
            return Ok(logo);
        }
    }
    // En dernier recours, quand Plex n'a aucun logo : TMDB (langue de l'interface, anglais, sans texte).
    if let (true, Some(key), Some(tid)) = (fallback, tmdb_key, tmdb_id.as_deref()) {
        let kind = if guid.contains("/show/") { "tv" } else { "movie" };
        return Ok(tmdb_logo(http, key, kind, tid, ui, true).await);
    }
    Ok(None)
}

/// Requête TMDB avec la clé de l'utilisateur : clé d'API (v3) ou jeton d'accès en lecture (v4).
fn tmdb_get(http: &Client, key: &str, path: &str) -> RequestBuilder {
    let url = format!("https://api.themoviedb.org/3{path}");
    if key.contains('.') {
        http.get(url).bearer_auth(key)
    } else {
        let sep = if path.contains('?') { '&' } else { '?' };
        http.get(format!("{url}{sep}api_key={key}"))
    }
}

/// Vérifie une clé TMDB (avant de l'enregistrer).
pub async fn tmdb_check(http: &Client, key: &str) -> Result<()> {
    tmdb_get(http, key, "/configuration").send().await?.error_for_status()?;
    Ok(())
}

/// Bandes-annonces YouTube d'un film (`movie`) ou d'une série (`tv`) sur TMDB, dans la langue de
/// l'interface et en anglais : `(identifiant, dans la langue de l'interface)`, la meilleure d'abord
/// (langue de l'interface, puis bande-annonce plutôt que teaser, puis officielle).
pub async fn tmdb_trailers(http: &Client, key: &str, kind: &str, id: &str, lang: &str) -> Vec<(String, bool)> {
    let path = format!("/{kind}/{id}/videos?language={lang}&include_video_language={lang},en");
    let Ok(resp) = tmdb_get(http, key, &path).send().await else { return Vec::new() };
    let Ok(v) = resp.error_for_status() else { return Vec::new() };
    let Ok(v) = v.json::<serde_json::Value>().await else { return Vec::new() };
    let mut videos: Vec<&serde_json::Value> = v["results"]
        .as_array()
        .map(|a| a.iter().filter(|t| t["site"] == "YouTube" && (t["type"] == "Trailer" || t["type"] == "Teaser")).collect())
        .unwrap_or_default();
    let score = |t: &serde_json::Value| {
        (t["iso_639_1"] == lang) as u8 * 4 + (t["type"] == "Trailer") as u8 * 2 + t["official"].as_bool().unwrap_or(false) as u8
    };
    videos.sort_by_key(|t| std::cmp::Reverse(score(t)));
    videos
        .iter()
        .filter_map(|t| Some((t["key"].as_str()?.to_string(), t["iso_639_1"] == lang)))
        .collect()
}

/// Logo d'un film (`movie`) ou d'une série (`tv`) sur TMDB : dans la langue de l'interface, sinon
/// (avec `any`) en anglais ou sans texte.
async fn tmdb_logo(http: &Client, key: &str, kind: &str, id: &str, ui: &str, any: bool) -> Option<String> {
    let v: serde_json::Value = tmdb_get(http, key, &format!("/{kind}/{id}/images?include_image_language={ui},en,null"))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;
    let logos = v["logos"].as_array()?;
    let pick = |lang: Option<&str>| {
        logos.iter().find(|l| l["iso_639_1"].as_str() == lang && l["file_path"].as_str().is_some_and(|p| !p.ends_with(".svg")))
    };
    let logo = pick(Some(ui)).or_else(|| if any { pick(Some("en")).or_else(|| pick(None)) } else { None })?;
    Some(format!("https://image.tmdb.org/t/p/w500{}", logo["file_path"].as_str()?))
}

/// Identifiant du service en ligne de Plex (« plex://movie/5d77… » → « 5d77… »).
fn discover_id(guid: &str) -> Option<&str> {
    let id = guid.rsplit('/').next()?;
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric())).then_some(id)
}

/// Le titre est-il dans la liste de visionnage du compte ?
pub async fn watchlisted(http: &Client, client_id: &str, token: &str, guid: &str) -> Result<bool> {
    let Some(id) = discover_id(guid) else { return Ok(false) };
    let v: serde_json::Value = with_plex_headers(
        http.get(format!("https://discover.provider.plex.tv/library/metadata/{id}/userState")),
        client_id,
        Some(token),
    )
    .send()
    .await?
    .error_for_status()?
    .json()
    .await?;
    let state = &v["MediaContainer"]["UserState"];
    let state = if state.is_array() { &state[0] } else { state };
    Ok(state["watchlistedAt"].is_number() || state["watchlistedAt"].is_string())
}

/// Ajoute le titre à la liste de visionnage du compte, ou l'en retire.
pub async fn set_watchlist(http: &Client, client_id: &str, token: &str, guid: &str, on: bool) -> Result<()> {
    let id = discover_id(guid).ok_or_else(|| anyhow!("guid"))?;
    let action = if on { "addToWatchlist" } else { "removeFromWatchlist" };
    with_plex_headers(
        http.put(format!("https://discover.provider.plex.tv/actions/{action}?ratingKey={id}")),
        client_id,
        Some(token),
    )
    .send()
    .await?
    .error_for_status()?;
    Ok(())
}

/// Photos des comptes Plex : celle de l'utilisateur et celles de ses amis (propriétaires des
/// serveurs partagés), par identifiant. Facultatif : en cas d'échec, pas de photo.
pub async fn avatars(http: &Client, client_id: &str, token: &str) -> (Option<String>, Vec<(u64, String)>) {
    let get = |url: &'static str| async move {
        with_plex_headers(http.get(url), client_id, Some(token)).send().await.ok()?.error_for_status().ok()?.text().await.ok()
    };
    let (me, users) = futures::join!(get("https://plex.tv/api/v2/user"), get("https://plex.tv/api/users"));
    let own = me
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["thumb"].as_str().map(str::to_owned));
    // Ancienne API en XML (la liste d'amis en JSON n'existe plus) : un <User …/> par ami.
    let friends = users
        .unwrap_or_default()
        .split("<User ")
        .skip(1)
        .filter_map(|tag| {
            // L'espace retirée par le découpage précède le premier attribut.
            let tag = format!(" {}", &tag[..tag.find('>').unwrap_or(tag.len())]);
            Some((xml_attr(&tag, "id")?.parse().ok()?, xml_attr(&tag, "thumb")?))
        })
        .collect();
    (own, friends)
}

/// Valeur d'un attribut dans une balise XML (`nom="valeur"`), entités courantes décodées.
fn xml_attr(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let end = start + tag[start..].find('"')?;
    let value = tag[start..end].replace("&amp;", "&").replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">");
    (!value.is_empty()).then_some(value)
}

#[derive(Deserialize, Clone)]
pub struct Connection {
    pub uri: String,
    #[serde(default)]
    pub local: bool,
    #[serde(default)]
    pub relay: bool,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub port: u16,
}

pub async fn servers(http: &Client, client_id: &str, token: &str) -> Result<Vec<Resource>> {
    let all: Vec<Resource> = with_plex_headers(
        http.get(format!("{PLEX_TV}/resources?includeHttps=1&includeRelay=1")),
        client_id,
        Some(token),
    )
    .send()
    .await?
    .error_for_status()?
    .json()
    .await?;
    Ok(all
        .into_iter()
        .filter(|r| r.provides.split(',').any(|p| p == "server"))
        .collect())
}

/// Teste toutes les adresses du serveur en parallèle et garde la meilleure joignable : locale
/// d'abord, puis distante, le relais Plex en dernier recours.
///
/// Les adresses locales sécurisées de Plex (`10-0-0-2.<id>.plex.direct`) sont bloquées par les
/// routeurs et DNS qui protègent du « DNS rebinding » : comme Plex Web, on essaie alors l'adresse IP
/// locale directement (`http://10.0.0.2:32400`), plutôt que de faire le tour par l'adresse publique
/// (que la boucle du routeur, le « NAT hairpin », rend souvent capricieuse).
pub async fn pick_connection(http: &Client, client_id: &str, res: &Resource) -> Result<String> {
    let token = res.access_token.as_deref().unwrap_or_default();
    let mut candidates: Vec<(u8, String)> = Vec::new();
    for c in &res.connections {
        if c.relay {
            candidates.push((3, c.uri.clone()));
        } else if c.local {
            candidates.push((0, c.uri.clone()));
            if !c.address.is_empty() && c.port != 0 {
                candidates.push((1, format!("http://{}:{}", c.address, c.port)));
            }
        } else {
            candidates.push((2, c.uri.clone()));
        }
    }
    candidates.sort();
    candidates.dedup_by(|a, b| a.1 == b.1);
    let checks = candidates.iter().map(|(_, uri)| async move {
        let ok = with_plex_headers(http.get(format!("{uri}/identity")), client_id, Some(token))
            .timeout(Duration::from_secs(4))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        (uri.clone(), ok)
    });
    join_all(checks)
        .await
        .into_iter()
        .find(|(_, ok)| *ok)
        .map(|(uri, _)| uri)
        .ok_or_else(|| anyhow!(crate::i18n::tr_with("plex.noReachableAddress", &[("name", &res.name)])))
}

// ---------- Format JSON des serveurs Plex ----------

#[derive(Deserialize)]
struct Envelope {
    #[serde(rename = "MediaContainer")]
    media_container: MediaContainer,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct MediaContainer {
    #[serde(rename = "Directory")]
    directory: Vec<RawDirectory>,
    #[serde(rename = "Metadata")]
    metadata: Vec<RawMetadata>,
    #[serde(rename = "Hub")]
    hub: Vec<RawHub>,
    #[serde(rename = "totalSize")]
    total_size: Option<u32>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawDirectory {
    key: String,
    title: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
struct RawHub {
    title: String,
    /// Identifiant indépendant de la langue (« movie.inprogress.1 », « music.recent.added.3 »…).
    hub_identifier: String,
    #[serde(rename = "Metadata")]
    metadata: Vec<RawMetadata>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RawMetadata {
    rating_key: String,
    #[serde(rename = "type")]
    kind: String,
    title: String,
    year: Option<i32>,
    summary: Option<String>,
    thumb: Option<String>,
    /// Mosaïque de pochettes d'une playlist.
    composite: Option<String>,
    art: Option<String>,
    parent_thumb: Option<String>,
    grandparent_thumb: Option<String>,
    parent_title: Option<String>,
    grandparent_title: Option<String>,
    /// Artiste d'un titre lorsqu'il diffère de celui de l'album (compilations).
    original_title: Option<String>,
    parent_rating_key: Option<String>,
    grandparent_rating_key: Option<String>,
    /// Identifiants Plex universels (« plex://movie/… ») du titre, de sa saison, de sa série.
    guid: Option<String>,
    parent_guid: Option<String>,
    grandparent_guid: Option<String>,
    index: Option<u32>,
    parent_index: Option<u32>,
    duration: Option<u64>,
    view_offset: Option<u64>,
    view_count: Option<u32>,
    leaf_count: Option<u32>,
    viewed_leaf_count: Option<u32>,
    /// Nombre d'éléments d'une collection.
    child_count: Option<u32>,
    content_rating: Option<String>,
    /// Playlist intelligente (générée par Plex selon des règles).
    #[serde(deserialize_with = "flag")]
    smart: bool,
    audience_rating: Option<f32>,
    /// Note donnée par l'utilisateur (0 à 10).
    user_rating: Option<f32>,
    tagline: Option<String>,
    studio: Option<String>,
    originally_available_at: Option<String>,
    #[serde(rename = "Media")]
    media: Vec<RawMedia>,
    #[serde(rename = "Genre")]
    genre: Vec<RawTag>,
    #[serde(rename = "Director")]
    director: Vec<RawTag>,
    #[serde(rename = "Writer")]
    writer: Vec<RawTag>,
    #[serde(rename = "Role")]
    role: Vec<RawRole>,
    /// Images associées (affiche, fond, logo du titre…) ; pour un épisode, celles de sa série.
    #[serde(rename = "Image")]
    images: Vec<RawImage>,
    /// Identifiants externes (« tmdb://… », « imdb://… »).
    #[serde(rename = "Guid")]
    guids: Vec<RawGuid>,
    /// Repères détectés par le serveur : intro, générique (« credits »).
    #[serde(rename = "Marker")]
    marker: Vec<RawMarker>,
    /// Bandes-annonces et autres extras.
    #[serde(rename = "Extras")]
    extras: Option<RawExtras>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawGuid {
    id: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RawMarker {
    #[serde(rename = "type")]
    kind: String,
    start_time_offset: u64,
    end_time_offset: u64,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawExtras {
    #[serde(rename = "Metadata")]
    metadata: Vec<RawExtra>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RawExtra {
    rating_key: String,
    subtype: String,
    title: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawImage {
    #[serde(rename = "type")]
    kind: String,
    url: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RawMedia {
    video_resolution: Option<String>,
    video_codec: Option<String>,
    #[serde(rename = "Part")]
    part: Vec<RawPart>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawPart {
    id: u64,
    key: String,
    #[serde(rename = "Stream")]
    stream: Vec<RawStream>,
}

/// Piste d'un fichier : 1 = vidéo, 2 = audio, 3 = sous-titres.
/// Une piste avec `key` est un sous-titre externe servi à part par Plex.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RawStream {
    id: u64,
    stream_type: u8,
    display_title: Option<String>,
    extended_display_title: Option<String>,
    selected: bool,
    key: Option<String>,
    /// Format d'un sous-titre externe (« srt », « ass », « vtt »…), pour le lecteur Android.
    codec: Option<String>,
    language_code: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawTag {
    tag: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawRole {
    /// Identifiant de la personne sur le serveur (filtre « actor=… »).
    id: Option<u64>,
    tag: String,
    role: Option<String>,
    thumb: Option<String>,
}

// ---------- Modèles envoyés à l'interface ----------

#[derive(Serialize)]
pub struct Section {
    pub key: String,
    pub title: String,
    pub kind: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub rating_key: String,
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub year: Option<i32>,
    pub summary: Option<String>,
    /// Affiche verticale (pour un épisode : celle de la série).
    pub poster: Option<String>,
    /// Logo officiel du titre (« clear logo », PNG transparent), quand Plex en a un.
    pub logo: Option<String>,
    /// Sans logo sur le serveur : identifiant Plex (film ou série) pour le chercher en ligne.
    pub logo_guid: Option<String>,
    /// Vignette propre à l'élément (image d'épisode en 16:9).
    pub thumb: Option<String>,
    pub backdrop: Option<String>,
    pub index: Option<u32>,
    /// Numéro de saison d'un épisode.
    pub parent_index: Option<u32>,
    /// Série d'un épisode ou d'une saison.
    pub show_title: Option<String>,
    pub show_key: Option<String>,
    /// Album d'un titre.
    pub album_title: Option<String>,
    pub album_key: Option<String>,
    pub duration_ms: Option<u64>,
    pub view_offset_ms: Option<u64>,
    pub watched: bool,
    pub unwatched_count: Option<u32>,
    pub content_rating: Option<String>,
    pub rating: Option<f32>,
    /// Note de l'utilisateur, de 0 à 10 (5 étoiles = 10).
    pub user_rating: Option<f32>,
    pub genres: Vec<String>,
    pub resolution: Option<String>,
    pub playable: bool,
}

#[derive(Serialize)]
pub struct Page {
    pub items: Vec<Item>,
    pub total: u32,
}

/// Fiche complète : l'élément et ses informations détaillées.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDetail {
    #[serde(flatten)]
    pub item: Item,
    pub tagline: Option<String>,
    /// Titre d'origine, s'il diffère du titre affiché (« Practical Magic » sous « Les Ensorceleuses »).
    pub original_title: Option<String>,
    /// Fond en haute définition, pour le bandeau plein écran de la fiche.
    pub backdrop_hd: Option<String>,
    pub studio: Option<String>,
    pub release_date: Option<String>,
    pub directors: Vec<String>,
    pub writers: Vec<String>,
    pub cast: Vec<CastMember>,
    /// Ex. « 1080p (HEVC) ».
    pub video: Option<String>,
    pub audio_tracks: Vec<Track>,
    pub subtitle_tracks: Vec<Track>,
}

#[derive(Serialize)]
pub struct CastMember {
    /// Identifiant de la personne sur le serveur (nombre chez Plex, texte chez Jellyfin).
    pub id: Option<String>,
    pub name: String,
    pub role: Option<String>,
    pub photo: Option<String>,
}

#[derive(Serialize)]
pub struct Track {
    pub id: u64,
    pub label: String,
    pub selected: bool,
}

/// Rangée d'une page de bibliothèque (« Ajoutés récemment », « Écoutés récemment »…).
#[derive(Serialize)]
pub struct Hub {
    pub title: String,
    pub identifier: String,
    pub items: Vec<Item>,
}

/// Épisodes voisins d'un épisode dans l'ordre de la série (saisons enchaînées).
#[derive(Serialize, Default)]
pub struct Adjacent {
    pub prev: Option<String>,
    pub next: Option<String>,
}

/// Genre d'une bibliothèque, avec quelques affiches pour l'illustrer.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Genre {
    pub id: String,
    pub title: String,
    pub count: u32,
    pub posters: Vec<String>,
    pub backdrop: Option<String>,
}

/// Titre d'une file de lecture musicale.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueTrack {
    pub rating_key: String,
    pub title: String,
    pub artist: Option<String>,
    pub artist_key: Option<String>,
    pub album: Option<String>,
    pub album_key: Option<String>,
    pub cover: Option<String>,
    pub duration_ms: u64,
    /// Fichier audio, lu directement par le lecteur intégré sur mobile.
    pub url: String,
}

/// Vidéo prête pour le lecteur intégré (mobile, TV) : flux HLS converti par le serveur Plex.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebStream {
    pub session: String,
    pub url: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub offset_ms: u64,
    pub duration_ms: u64,
}

pub struct PlayInfo {
    pub url: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub offset_ms: u64,
    pub duration_ms: u64,
    /// Numéro de piste mpv (`--aid`), sinon choix par défaut de mpv.
    pub aid: Option<usize>,
    /// Valeur de `--sid` : numéro de piste ou « no ».
    pub sid: String,
    /// Sous-titres externes à charger, dans l'ordre (leurs numéros suivent les pistes intégrées).
    pub sub_files: Vec<String>,
    /// Les mêmes, avec leur format et leur langue (lecteur natif d'Android).
    pub sub_tracks: Vec<SubFile>,
}

/// Débit d'une conversion quand aucune qualité n'est choisie (kb/s).
pub const DEFAULT_KBPS: u32 = 12000;

/// Définition maximale d'une conversion selon son débit, comme dans les applications Plex :
/// 1080p à partir de 8 Mb/s, 720p à partir de 2 Mb/s, 480p en dessous.
pub fn resolution_for(kbps: u32) -> (u32, u32) {
    match kbps {
        8000.. => (1920, 1080),
        2000.. => (1280, 720),
        _ => (854, 480),
    }
}

/// Repère d'un épisode : `intro` ou `credits` (générique), en millisecondes.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub kind: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

/// Bande-annonce : un élément du serveur (lisible comme un titre) ou une vidéo YouTube.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Trailer {
    pub rating_key: Option<String>,
    pub youtube: Option<String>,
}

/// Ce que le serveur sait d'une bande-annonce, et de quoi la chercher sur TMDB sinon.
pub struct TrailerSource {
    /// Bandes-annonces du serveur, avec leur titre (qui dit souvent la langue : « French Trailer »).
    pub local: Vec<(String, String)>,
    /// Lien YouTube donné par le serveur (Jellyfin).
    pub youtube: Option<String>,
    /// « movie » ou « tv ».
    pub tmdb_kind: Option<&'static str>,
    pub tmdb_id: Option<String>,
}

/// Identifiant d'une vidéo YouTube dans une adresse (`watch?v=`, `youtu.be/`, `/embed/`, ou
/// `videoid=` des adresses Kodi que Jellyfin reprend).
pub(crate) fn youtube_id(url: &str) -> Option<String> {
    let rest = ["v=", "videoid=", "youtu.be/", "/embed/", "/shorts/"].iter().find_map(|m| url.split(m).nth(1))?;
    let id: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    (id.len() == 11).then_some(id)
}

/// Sous-titre externe servi par Plex.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubFile {
    pub url: String,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
}

fn is_music(kind: &str) -> bool {
    matches!(kind, "artist" | "album" | "track" | "playlist")
}

// ---------- Client d'un serveur Plex ----------

#[derive(Clone)]
pub struct PlexServer {
    pub http: Client,
    pub base: String,
    pub token: String,
    pub client_id: String,
}

impl PlexServer {
    pub(crate) fn get(&self, path: &str) -> RequestBuilder {
        with_plex_headers(self.http.get(format!("{}{}", self.base, path)), &self.client_id, Some(&self.token))
    }

    pub(crate) fn request(&self, method: reqwest::Method, path: &str) -> RequestBuilder {
        with_plex_headers(
            self.http.request(method, format!("{}{}", self.base, path)),
            &self.client_id,
            Some(&self.token),
        )
    }

    async fn container(&self, path: &str) -> Result<MediaContainer> {
        let env: Envelope = self.get(path).send().await?.error_for_status()?.json().await?;
        Ok(env.media_container)
    }

    /// Adresse d'un fichier servi par le serveur, avec le jeton : les clés des extras (bandes-annonces
    /// de plex.tv) portent déjà des paramètres, le jeton s'ajoute alors avec `&`.
    pub(crate) fn file_url(&self, key: &str) -> String {
        let sep = if key.contains('?') { '&' } else { '?' };
        format!("{}{}{sep}X-Plex-Token={}", self.base, key, self.token)
    }

    pub(crate) fn image(&self, path: &str, width: u32, height: u32) -> String {
        format!(
            "{}/photo/:/transcode?width={width}&height={height}&minSize=1&upscale=1&url={}&X-Plex-Token={}",
            self.base,
            urlencoding::encode(path),
            self.token
        )
    }

    /// Logo d'un titre : réduit sans être recadré, en PNG pour garder la transparence.
    fn logo(&self, path: &str) -> String {
        format!(
            "{}/photo/:/transcode?width=800&height=320&format=png&url={}&X-Plex-Token={}",
            self.base,
            urlencoding::encode(path),
            self.token
        )
    }

    /// Image du serveur (chemin Plex « /library/metadata/… »), redimensionnée.
    pub fn image_url(&self, path: &str, width: u32, height: u32) -> String {
        self.image(path, width, height)
    }

    fn to_item(&self, m: RawMetadata) -> Item {
        let music = is_music(&m.kind);
        let poster_path = match m.kind.as_str() {
            "episode" => m.grandparent_thumb.as_ref().or(m.parent_thumb.as_ref()).or(m.thumb.as_ref()),
            "season" | "album" => m.thumb.as_ref().or(m.parent_thumb.as_ref()),
            "track" => m.parent_thumb.as_ref().or(m.grandparent_thumb.as_ref()).or(m.thumb.as_ref()),
            "playlist" => m.thumb.as_ref().or(m.composite.as_ref()),
            _ => m.thumb.as_ref(),
        };
        let subtitle = match m.kind.as_str() {
            "episode" => Some(format!(
                "{} · S{}E{}",
                m.grandparent_title.as_deref().unwrap_or_default(),
                m.parent_index.unwrap_or(0),
                m.index.unwrap_or(0)
            )),
            "season" | "album" => m.parent_title.clone(),
            "track" => m.original_title.clone().or_else(|| m.grandparent_title.clone()),
            "collection" => m.child_count.map(|n| {
                let key = if n == 1 { "plex.itemCount_one" } else { "plex.itemCount_other" };
                crate::i18n::tr_with(key, &[("count", &n.to_string())])
            }),
            "playlist" => m.leaf_count.map(|n| {
                // Singulier pour 1 (et pour 0 en français, comme avant).
                let one = n == 1 || (n == 0 && crate::i18n::language() == "fr");
                let key = if one { "plex.trackCount_one" } else { "plex.trackCount_other" };
                crate::i18n::tr_with(key, &[("count", &n.to_string())])
            }),
            _ => None,
        };
        let watched = match m.kind.as_str() {
            _ if music => false,
            "show" | "season" => m.leaf_count.is_some() && m.leaf_count == m.viewed_leaf_count,
            _ => m.view_count.unwrap_or(0) > 0,
        };
        let unwatched_count = m.leaf_count.filter(|_| !music && m.kind != "collection").map(|l| l.saturating_sub(m.viewed_leaf_count.unwrap_or(0)));
        let (show_title, show_key) = match m.kind.as_str() {
            "episode" => (m.grandparent_title.clone(), m.grandparent_rating_key.clone()),
            "season" | "album" => (m.parent_title.clone(), m.parent_rating_key.clone()),
            "track" => (m.grandparent_title.clone(), m.grandparent_rating_key.clone()),
            _ => (None, None),
        };
        let (album_title, album_key) = match m.kind.as_str() {
            "track" => (m.parent_title.clone(), m.parent_rating_key.clone()),
            _ => (None, None),
        };
        let media = m.media.first();
        Item {
            show_title,
            show_key,
            album_title,
            album_key,
            parent_index: m.parent_index,
            // Les pochettes de musique sont carrées.
            poster: poster_path.map(|p| if music { self.image(p, 400, 400) } else { self.image(p, 300, 450) }),
            thumb: m.thumb.as_ref().map(|p| self.image(p, 480, 270)),
            backdrop: m.art.as_ref().map(|p| self.image(p, 1600, 900)),
            logo: m.images.iter().find(|i| i.kind == "clearLogo").map(|i| self.logo(&i.url)),
            logo_guid: match m.kind.as_str() {
                "episode" => m.grandparent_guid.clone(),
                "season" => m.parent_guid.clone(),
                "movie" | "show" => m.guid.clone(),
                _ => None,
            }
            .filter(|g| g.starts_with("plex://")),
            playable: matches!(m.kind.as_str(), "movie" | "episode" | "track"),
            resolution: media.and_then(|x| x.video_resolution.clone()),
            genres: m.genre.into_iter().map(|g| g.tag).collect(),
            subtitle,
            watched,
            unwatched_count,
            rating_key: m.rating_key,
            kind: m.kind,
            title: m.title,
            year: m.year,
            summary: m.summary,
            index: m.index,
            duration_ms: m.duration,
            view_offset_ms: m.view_offset,
            content_rating: m.content_rating,
            rating: m.audience_rating,
            user_rating: m.user_rating,
        }
    }

    fn to_items(&self, list: Vec<RawMetadata>) -> Vec<Item> {
        list.into_iter().map(|m| self.to_item(m)).collect()
    }

    pub async fn sections(&self) -> Result<Vec<Section>> {
        let c = self.container("/library/sections").await?;
        Ok(c.directory
            .into_iter()
            .filter(|d| matches!(d.kind.as_str(), "movie" | "show" | "artist"))
            .map(|d| Section { key: d.key, title: d.title, kind: d.kind })
            .collect())
    }

    /// Contenu d'une bibliothèque. `kind` filtre le type (9 = albums dans une bibliothèque musicale),
    /// `genre` l'identifiant d'un genre, `sort` l'ordre Plex (par ex. « addedAt:desc », titre par défaut).
    pub async fn section_items(
        &self,
        key: &str,
        start: u32,
        size: u32,
        kind: Option<u32>,
        genre: Option<&str>,
        sort: Option<&str>,
    ) -> Result<Page> {
        let sort = sort.filter(|s| s.chars().all(|c| c.is_ascii_alphanumeric() || c == ':')).unwrap_or("titleSort");
        let mut filter = kind.map(|k| format!("&type={k}")).unwrap_or_default();
        if let Some(g) = genre {
            filter.push_str(&format!("&genre={}", urlencoding::encode(g)));
        }
        let c = self
            .container(&format!(
                "/library/sections/{key}/all?sort={sort}{filter}&X-Plex-Container-Start={start}&X-Plex-Container-Size={size}"
            ))
            .await?;
        let total = c.total_size.unwrap_or(c.metadata.len() as u32);
        Ok(Page { items: self.to_items(c.metadata), total })
    }

    /// Films et séries d'une personne (acteur) dans toutes les bibliothèques vidéo du serveur.
    pub async fn person_items(&self, id: &str) -> Result<Vec<Item>> {
        let sections = self.sections().await?;
        let lists = join_all(
            sections
                .iter()
                .filter(|s| s.kind == "movie" || s.kind == "show")
                .map(|s| self.container_owned(format!("/library/sections/{}/all?actor={id}&sort=year:desc", s.key))),
        )
        .await;
        let mut items: Vec<Item> = lists.into_iter().filter_map(Result::ok).flat_map(|c| self.to_items(c.metadata)).collect();
        items.sort_by(|a, b| b.year.cmp(&a.year));
        Ok(items)
    }

    async fn container_owned(&self, path: String) -> Result<MediaContainer> {
        self.container(&path).await
    }

    pub async fn on_deck(&self) -> Result<Vec<Item>> {
        let c = self.container("/library/onDeck").await?;
        Ok(self.to_items(c.metadata))
    }

    /// Derniers films et séries ajoutés ; la musique a sa propre page.
    pub async fn recently_added(&self) -> Result<Vec<Item>> {
        let c = self
            .container("/library/recentlyAdded?X-Plex-Container-Start=0&X-Plex-Container-Size=60")
            .await?;
        let videos = c.metadata.into_iter().filter(|m| !is_music(&m.kind)).take(30).collect();
        Ok(self.to_items(videos))
    }

    /// Genres d'une bibliothèque, chacun avec ses affiches les mieux notées et son nombre de titres.
    pub async fn genres(&self, key: &str) -> Result<Vec<Genre>> {
        let list = self.container(&format!("/library/sections/{key}/genre")).await?.directory;
        let filled = join_all(list.into_iter().map(|g| async move {
            let path = format!(
                "/library/sections/{key}/all?genre={}&sort=audienceRating:desc&X-Plex-Container-Start=0&X-Plex-Container-Size=4",
                urlencoding::encode(&g.key)
            );
            let c = self.container(&path).await.ok()?;
            let count = c.total_size.unwrap_or(c.metadata.len() as u32);
            (count > 0).then(|| Genre {
                posters: c.metadata.iter().filter_map(|m| m.thumb.as_deref()).map(|p| self.image(p, 300, 450)).collect(),
                backdrop: c.metadata.iter().find_map(|m| m.art.as_deref()).map(|p| self.image(p, 960, 540)),
                id: g.key,
                title: g.title,
                count,
            })
        }))
        .await;
        Ok(filled.into_iter().flatten().collect())
    }

    /// Rangées de la page d'accueil d'une bibliothèque (écoutés / ajoutés récemment, playlists…).
    pub async fn section_hubs(&self, key: &str) -> Result<Vec<Hub>> {
        let hubs_path = format!("/hubs/sections/{key}?count=24");
        let (c, playlists) = tokio::join!(self.container(&hubs_path), self.playlists(key));
        let mut hubs: Vec<Hub> = c?
            .hub
            .into_iter()
            .filter_map(|h| {
                let items: Vec<RawMetadata> = h
                    .metadata
                    .into_iter()
                    .filter(|m| is_music(&m.kind) || matches!(m.kind.as_str(), "movie" | "show" | "season" | "episode"))
                    .collect();
                (!items.is_empty()).then(|| Hub { title: h.title, identifier: h.hub_identifier, items: self.to_items(items) })
            })
            .collect();
        // Les playlists ne font pas partie des rangées du serveur : on les place en troisième, comme Plex.
        if let Ok(items) = playlists {
            if !items.is_empty() {
                hubs.insert(hubs.len().min(2), Hub { title: crate::i18n::tr("plex.playlists"), identifier: "plexr.playlists".into(), items });
            }
        }
        Ok(hubs)
    }

    /// Playlists audio d'une bibliothèque : celles créées à la main d'abord, puis les intelligentes.
    pub async fn playlists(&self, section: &str) -> Result<Vec<Item>> {
        let mut list = self
            .container(&format!("/playlists?playlistType=audio&sectionID={section}"))
            .await?
            .metadata;
        list.sort_by_key(|m| m.smart);
        Ok(self.to_items(list))
    }

    async fn raw_metadata(&self, rating_key: &str) -> Result<RawMetadata> {
        // Avec les repères (intro, générique) et les extras (bande-annonce).
        let found = match self.container(&format!("/library/metadata/{rating_key}?includeMarkers=1&includeExtras=1")).await {
            Ok(c) => c.metadata.into_iter().next(),
            Err(_) => None,
        };
        // Les playlists ne sont pas des éléments de bibliothèque.
        let found = match found {
            Some(m) => Some(m),
            None => self.container(&format!("/playlists/{rating_key}")).await?.metadata.into_iter().next(),
        };
        found.ok_or_else(|| anyhow!(crate::i18n::tr_with("plex.notFound", &[("id", rating_key)])))
    }

    /// Épisode précédent et suivant ; rien pour un film.
    pub async fn adjacent(&self, rating_key: &str) -> Result<Adjacent> {
        let m = self.raw_metadata(rating_key).await?;
        let Some(show) = m.grandparent_rating_key.filter(|_| m.kind == "episode") else {
            return Ok(Adjacent::default());
        };
        let all = self.container(&format!("/library/metadata/{show}/allLeaves")).await?.metadata;
        let Some(i) = all.iter().position(|e| e.rating_key == rating_key) else {
            return Ok(Adjacent::default());
        };
        Ok(Adjacent {
            prev: i.checked_sub(1).map(|j| all[j].rating_key.clone()),
            next: all.get(i + 1).map(|e| e.rating_key.clone()),
        })
    }

    /// Titres d'un artiste, d'un album, d'une playlist, ou le titre lui-même.
    async fn raw_tracks(&self, rating_key: &str) -> Result<Vec<RawMetadata>> {
        let m = self.raw_metadata(rating_key).await?;
        let path = match m.kind.as_str() {
            "track" => return Ok(vec![m]),
            "album" => format!("/library/metadata/{rating_key}/children"),
            "artist" => format!("/library/metadata/{rating_key}/allLeaves"),
            "playlist" => format!("/playlists/{rating_key}/items"),
            _ => return Err(anyhow!(crate::i18n::tr_with("plex.noMusic", &[("title", &m.title)]))),
        };
        Ok(self
            .container(&path)
            .await?
            .metadata
            .into_iter()
            .filter(|t| t.kind == "track")
            .collect())
    }

    pub async fn tracks(&self, rating_key: &str) -> Result<Vec<Item>> {
        Ok(self.to_items(self.raw_tracks(rating_key).await?))
    }

    /// File de lecture prête pour mpv : chaque titre avec l'adresse de son fichier.
    pub async fn queue(&self, rating_key: &str) -> Result<Vec<QueueTrack>> {
        Ok(self
            .raw_tracks(rating_key)
            .await?
            .into_iter()
            .filter_map(|t| {
                let part = t.media.first().and_then(|x| x.part.first())?;
                let url = self.file_url(&part.key);
                let cover = t.parent_thumb.as_ref().or(t.grandparent_thumb.as_ref()).or(t.thumb.as_ref());
                Some(QueueTrack {
                    url,
                    cover: cover.map(|p| self.image(p, 600, 600)),
                    artist: t.original_title.or(t.grandparent_title),
                    artist_key: t.grandparent_rating_key,
                    album: t.parent_title,
                    album_key: t.parent_rating_key,
                    duration_ms: t.duration.unwrap_or(0),
                    rating_key: t.rating_key,
                    title: t.title,
                })
            })
            .collect())
    }

    pub async fn metadata(&self, rating_key: &str) -> Result<ItemDetail> {
        let mut m = self.raw_metadata(rating_key).await?;
        let media = m.media.first();
        let video = media.and_then(|x| x.video_resolution.as_ref()).map(|res| {
            let res = if res.chars().all(|c| c.is_ascii_digit()) { format!("{res}p") } else { res.to_uppercase() };
            match media.and_then(|x| x.video_codec.as_ref()) {
                Some(codec) => format!("{res} ({})", codec.to_uppercase()),
                None => res,
            }
        });
        let streams: &[RawStream] = media.and_then(|x| x.part.first()).map_or(&[], |p| &p.stream);
        let tracks = |kind: u8| -> Vec<Track> {
            streams
                .iter()
                .filter(|s| s.stream_type == kind)
                .map(|s| Track {
                    id: s.id,
                    label: s
                        .extended_display_title
                        .clone()
                        .or_else(|| s.display_title.clone())
                        .unwrap_or_else(|| crate::i18n::tr_with("plex.track", &[("id", &s.id.to_string())])),
                    selected: s.selected,
                })
                .collect()
        };
        let audio_tracks = tracks(2);
        let subtitle_tracks = tracks(3);
        let cast = std::mem::take(&mut m.role)
            .into_iter()
            .take(30)
            .map(|r| CastMember { id: r.id.map(|i| i.to_string()), name: r.tag, role: r.role, photo: r.thumb })
            .collect();
        let original_title = m.original_title.take().filter(|o| !o.trim().is_empty() && *o != m.title);
        let backdrop_hd = m.art.as_ref().map(|p| self.image(p, 2560, 1440));
        Ok(ItemDetail {
            tagline: m.tagline.take(),
            original_title,
            backdrop_hd,
            studio: m.studio.take(),
            release_date: m.originally_available_at.take(),
            directors: std::mem::take(&mut m.director).into_iter().map(|t| t.tag).collect(),
            writers: std::mem::take(&mut m.writer).into_iter().map(|t| t.tag).collect(),
            cast,
            video,
            audio_tracks,
            subtitle_tracks,
            item: self.to_item(m),
        })
    }

    /// Collections d'une bibliothèque (« 28 … plus tard », « Batman »…), par titre.
    pub async fn collections(&self, key: &str) -> Result<Vec<Item>> {
        let c = self.container(&format!("/library/sections/{key}/collections?sort=titleSort")).await?;
        Ok(self.to_items(c.metadata))
    }

    /// Films d'une collection, du plus ancien au plus récent (l'ordre d'une saga).
    pub async fn collection_items(&self, rating_key: &str) -> Result<Vec<Item>> {
        let c = self.container(&format!("/library/collections/{rating_key}/children")).await?;
        let mut items = self.to_items(c.metadata);
        items.sort_by_key(|i| i.year.unwrap_or(i32::MAX));
        Ok(items)
    }

    /// « Plus de ce genre » : les titres que Plex associe à un film ou une série (films / séries associés).
    pub async fn related(&self, rating_key: &str) -> Result<Vec<Item>> {
        let c = self.container(&format!("/library/metadata/{rating_key}/related?count=24")).await?;
        let mut seen = std::collections::HashSet::new();
        let items: Vec<RawMetadata> = c
            .hub
            .into_iter()
            .flat_map(|h| h.metadata)
            .filter(|m| matches!(m.kind.as_str(), "movie" | "show") && m.rating_key != rating_key && seen.insert(m.rating_key.clone()))
            .take(30)
            .collect();
        Ok(self.to_items(items))
    }

    pub async fn children(&self, rating_key: &str) -> Result<Vec<Item>> {
        let c = self.container(&format!("/library/metadata/{rating_key}/children")).await?;
        Ok(self.to_items(c.metadata))
    }

    pub async fn search(&self, query: &str) -> Result<Vec<Item>> {
        let c = self
            .container(&format!("/hubs/search?query={}&limit=20", urlencoding::encode(query)))
            .await?;
        let found = c
            .hub
            .into_iter()
            .flat_map(|h| h.metadata)
            .filter(|m| matches!(m.kind.as_str(), "movie" | "show" | "season" | "episode") || is_music(&m.kind))
            .collect();
        Ok(self.to_items(found))
    }

    /// Flux HLS du transcodeur Plex, lisible par le lecteur vidéo du système (Android, iOS).
    /// Les pistes choisies sont enregistrées sur le fichier avant de lancer la conversion ;
    /// les sous-titres sont incrustés dans l'image.
    pub async fn web_stream(
        &self,
        rating_key: &str,
        audio: Option<u64>,
        subtitle: Option<u64>,
        max_kbps: Option<u32>,
        session: &str,
    ) -> Result<WebStream> {
        let m = self.raw_metadata(rating_key).await?;
        let part = m
            .media
            .first()
            .and_then(|x| x.part.first())
            .ok_or_else(|| anyhow!(crate::i18n::tr_with("plex.noPlayableFile", &[("title", &m.title)])))?;
        if audio.is_some() || subtitle.is_some() {
            let mut q = String::from("allParts=1");
            if let Some(a) = audio {
                q.push_str(&format!("&audioStreamID={a}"));
            }
            if let Some(s) = subtitle {
                q.push_str(&format!("&subtitleStreamID={s}"));
            }
            with_plex_headers(
                self.http.put(format!("{}/library/parts/{}?{q}", self.base, part.id)),
                &self.client_id,
                Some(&self.token),
            )
            .send()
            .await?;
        }
        let (title, subtitle_line) = match (m.kind.as_str(), &m.grandparent_title) {
            ("episode", Some(show)) => (
                show.clone(),
                Some(episode_line(m.parent_index.unwrap_or(0), m.index.unwrap_or(0), &m.title)),
            ),
            _ => (m.title.clone(), m.year.map(|y| y.to_string())),
        };
        let path = urlencoding::encode(&format!("/library/metadata/{rating_key}")).into_owned();
        // Profil « navigateur » : le serveur produit du H.264 / AAC en HLS, lisible partout, au débit
        // choisi dans le lecteur (par défaut 12 Mb/s en 1080p, pour ménager le serveur et le réseau).
        let kbps = max_kbps.unwrap_or(DEFAULT_KBPS);
        let (width, height) = resolution_for(kbps);
        let url = format!(
            "{base}/video/:/transcode/universal/start.m3u8?path={path}&mediaIndex=0&partIndex=0&protocol=hls\
             &fastSeek=1&directPlay=0&directStream=1&directStreamAudio=1&videoQuality=100&maxVideoBitrate={kbps}\
             &videoResolution={width}x{height}&subtitles=burn&subtitleSize=100&audioBoost=100&location=lan\
             &session={session}&X-Plex-Session-Identifier={session}&X-Plex-Client-Identifier={cid}\
             &X-Plex-Product=Maku&X-Plex-Platform=Chrome&X-Plex-Device=Maku&X-Plex-Token={token}",
            base = self.base,
            cid = urlencoding::encode(&self.client_id),
            token = self.token,
        );
        Ok(WebStream {
            session: session.to_owned(),
            url,
            title,
            subtitle: subtitle_line,
            offset_ms: m.view_offset.unwrap_or(0),
            duration_ms: m.duration.unwrap_or(0),
        })
    }

    /// Arrête une conversion lancée par `web_stream`.
    pub async fn stop_transcode(&self, session: &str) -> Result<()> {
        self.get(&format!("/video/:/transcode/universal/stop?session={}", urlencoding::encode(session)))
            .send()
            .await?;
        Ok(())
    }

    /// Prépare la lecture. `audio` / `subtitle` sont des identifiants de piste Plex ;
    /// `None` reprend les choix enregistrés dans Plex, `Some(0)` désactive les sous-titres.
    /// Repères (intro, générique) détectés par le serveur. Plex les renvoie à tous les comptes :
    /// seules ses propres applications réservent le bouton « Passer » au Plex Pass.
    pub async fn markers(&self, rating_key: &str) -> Result<Vec<Marker>> {
        let m = self.raw_metadata(rating_key).await?;
        Ok(m.marker
            .iter()
            .filter(|k| k.end_time_offset > k.start_time_offset)
            .map(|k| Marker { kind: k.kind.clone(), start_ms: k.start_time_offset, end_ms: k.end_time_offset })
            .collect())
    }

    /// Bande-annonce du serveur (extra), et l'identifiant TMDB pour en chercher une en ligne sinon.
    pub async fn trailer(&self, rating_key: &str) -> Result<TrailerSource> {
        let m = self.raw_metadata(rating_key).await?;
        let local = m
            .extras
            .as_ref()
            .map(|e| e.metadata.iter().filter(|x| x.subtype == "trailer").map(|x| (x.rating_key.clone(), x.title.clone())).collect())
            .unwrap_or_default();
        let tmdb_id = m.guids.iter().find_map(|g| g.id.strip_prefix("tmdb://").map(str::to_owned));
        let tmdb_kind = Some(if m.kind == "show" { "tv" } else { "movie" });
        Ok(TrailerSource { local, youtube: None, tmdb_kind, tmdb_id })
    }

    pub async fn play_info(&self, rating_key: &str, audio: Option<u64>, subtitle: Option<u64>) -> Result<PlayInfo> {
        let m = self.raw_metadata(rating_key).await?;
        let part = m
            .media
            .first()
            .and_then(|x| x.part.first())
            .ok_or_else(|| anyhow!(crate::i18n::tr_with("plex.noPlayableFile", &[("title", &m.title)])))?;
        let (title, subtitle_line) = match (m.kind.as_str(), &m.grandparent_title) {
            ("episode", Some(show)) => (
                show.clone(),
                Some(episode_line(m.parent_index.unwrap_or(0), m.index.unwrap_or(0), &m.title)),
            ),
            _ => (m.title.clone(), m.year.map(|y| y.to_string())),
        };

        // mpv numérote les pistes de chaque type à partir de 1, dans l'ordre du fichier ;
        // les sous-titres externes ajoutés par --sub-file viennent après les intégrés.
        let audio_streams: Vec<&RawStream> = part.stream.iter().filter(|s| s.stream_type == 2).collect();
        let aid = match audio {
            Some(id) => audio_streams.iter().position(|s| s.id == id),
            None => audio_streams.iter().position(|s| s.selected),
        }
        .map(|i| i + 1);

        let subs: Vec<&RawStream> = part.stream.iter().filter(|s| s.stream_type == 3).collect();
        let embedded: Vec<&&RawStream> = subs.iter().filter(|s| s.key.is_none()).collect();
        let external: Vec<&&RawStream> = subs.iter().filter(|s| s.key.is_some()).collect();
        let wanted = match subtitle {
            Some(0) => None,
            Some(id) => Some(id),
            None => subs.iter().find(|s| s.selected).map(|s| s.id),
        };
        let sid = match wanted {
            None => "no".to_string(),
            Some(id) => embedded
                .iter()
                .position(|s| s.id == id)
                .map(|i| i + 1)
                .or_else(|| external.iter().position(|s| s.id == id).map(|i| embedded.len() + i + 1))
                .map_or_else(|| "auto".to_string(), |n| n.to_string()),
        };
        let sub_files = external
            .iter()
            .filter_map(|s| s.key.as_ref())
            .map(|key| self.file_url(key))
            .collect();
        let sub_tracks = external
            .iter()
            .filter_map(|s| {
                Some(SubFile {
                    url: self.file_url(s.key.as_ref()?),
                    codec: s.codec.clone().unwrap_or_else(|| "srt".into()),
                    language: s.language_code.clone(),
                    title: s.display_title.clone(),
                })
            })
            .collect();

        Ok(PlayInfo {
            url: self.file_url(&part.key),
            title,
            subtitle: subtitle_line,
            offset_ms: m.view_offset.unwrap_or(0),
            duration_ms: m.duration.unwrap_or(0),
            aid,
            sid,
            sub_files,
            sub_tracks,
        })
    }

    /// Informe le serveur de la progression (alimente « Reprendre » et « À suivre »).
    /// `session` relie la lecture à sa conversion (`web_stream`) : sans lui, Plex (et Tautulli, Tracearr)
    /// la croit lue en direct, dans la qualité du fichier d'origine.
    pub async fn timeline(&self, rating_key: &str, state: &str, time_ms: u64, duration_ms: u64, session: &str) -> Result<()> {
        self.get(&format!(
            "/:/timeline?ratingKey={rating_key}&key=%2Flibrary%2Fmetadata%2F{rating_key}&state={state}&time={time_ms}&duration={duration_ms}&identifier=com.plexapp.plugins.library"
        ))
        .header("X-Plex-Session-Identifier", session)
        .send()
        .await?
        .error_for_status()?;
        Ok(())
    }

    /// Note de l'utilisateur (0 à 10), ou `None` pour la retirer.
    pub async fn rate(&self, rating_key: &str, rating: Option<f32>) -> Result<()> {
        let value = rating.map_or("-1".to_string(), |r| r.clamp(0.0, 10.0).to_string());
        self.request(
            reqwest::Method::PUT,
            &format!("/:/rate?key={rating_key}&identifier=com.plexapp.plugins.library&rating={value}"),
        )
        .send()
        .await?
        .error_for_status()?;
        Ok(())
    }

    /// Retire un titre de « Continuer à regarder » sans toucher à sa progression ailleurs.
    pub async fn remove_from_continue(&self, rating_key: &str) -> Result<()> {
        self.request(reqwest::Method::PUT, &format!("/actions/removeFromContinueWatching?ratingKey={rating_key}"))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Marque l'élément (ou toute une série / saison) comme vu ou non vu.
    pub async fn set_watched(&self, rating_key: &str, watched: bool) -> Result<()> {
        let action = if watched { "scrobble" } else { "unscrobble" };
        self.get(&format!("/:/{action}?identifier=com.plexapp.plugins.library&key={rating_key}"))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}
