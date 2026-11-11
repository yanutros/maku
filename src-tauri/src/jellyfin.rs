//! Client Jellyfin et Emby : connexion (identifiant et mot de passe), bibliothèques, rangées, fiches, lecture
//! (fichier d'origine ou flux converti), progression et musique, dans les types communs de `plex.rs`.
//! Jellyfin est né d'Emby et en garde l'API : le même client sert aux deux. Seuls changent l'en-tête
//! d'authentification (envoyé sous les deux noms), Quick Connect (Jellyfin seulement), les miniatures
//! d'aperçu (Jellyfin seulement) et les marqueurs d'intro (segments chez Jellyfin, chapitres chez Emby).

use anyhow::{anyhow, bail, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, RequestBuilder};
use serde::Deserialize;

use crate::plex::{FilterField, FilterValue, Marker, PRODUCT, Section, SectionFilters, SortField, TrailerSource};

/// En-tête d'authentification de Jellyfin : l'application, l'appareil et, une fois connecté, le jeton.
fn authorization(client_id: &str, token: Option<&str>) -> String {
    let device = if cfg!(target_os = "android") { "Maku (Android)" } else { "Maku (PC)" };
    let mut value = format!(
        r#"MediaBrowser Client="{PRODUCT}", Device="{device}", DeviceId="{client_id}", Version="{}""#,
        env!("CARGO_PKG_VERSION")
    );
    if let Some(t) = token {
        value.push_str(&format!(r#", Token="{t}""#));
    }
    value
}

/// En-têtes d'authentification : `Authorization` pour Jellyfin, `X-Emby-Authorization` et `X-Emby-Token`
/// pour Emby. Chacun ignore ceux de l'autre.
fn auth(client_id: &str, token: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str(&authorization(client_id, token)) {
        headers.insert("Authorization", value.clone());
        headers.insert("X-Emby-Authorization", value);
    }
    if let Some(t) = token.and_then(|t| HeaderValue::from_str(t).ok()) {
        headers.insert("X-Emby-Token", t);
    }
    headers
}

/// Adresse saisie par l'utilisateur, sans barre finale ; « http:// » si le schéma manque.
pub fn normalize_url(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("http://{url}")
    }
}

/// Résultat d'une connexion réussie.
pub struct Login {
    pub token: String,
    pub user_id: String,
    pub server_id: String,
    pub server_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AuthResult {
    access_token: String,
    user: AuthUser,
    server_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AuthUser {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PublicInfo {
    server_name: String,
}

/// Connexion par identifiant et mot de passe.
pub async fn login(http: &Client, base: &str, client_id: &str, username: &str, password: &str) -> Result<Login> {
    let server_name = server_name(http, base).await?;
    let resp = http
        .post(format!("{base}/Users/AuthenticateByName"))
        .headers(auth(client_id, None))
        .json(&serde_json::json!({ "Username": username, "Pw": password }))
        .send()
        .await?;
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        bail!(crate::i18n::tr("jellyfin.badCredentials"));
    }
    let auth: AuthResult = resp.error_for_status()?.json().await?;
    Ok(Login { token: auth.access_token, user_id: auth.user.id, server_id: auth.server_id, server_name })
}

/// Quick Connect : un code à valider dans Jellyfin (Profil → Quick Connect), sans mot de passe.
pub struct QuickConnect {
    pub code: String,
    pub secret: String,
}

pub async fn quick_connect_start(http: &Client, base: &str, client_id: &str) -> Result<QuickConnect> {
    server_name(http, base).await?;
    let resp = http
        .post(format!("{base}/QuickConnect/Initiate"))
        .headers(auth(client_id, None))
        .send()
        .await?;
    if !resp.status().is_success() {
        bail!(crate::i18n::tr("jellyfin.quickConnectOff"));
    }
    let v: serde_json::Value = resp.json().await?;
    Ok(QuickConnect {
        code: v["Code"].as_str().unwrap_or_default().into(),
        secret: v["Secret"].as_str().unwrap_or_default().into(),
    })
}

/// Le code a-t-il été validé ? Si oui, la connexion.
pub async fn quick_connect_poll(http: &Client, base: &str, client_id: &str, secret: &str) -> Result<Option<Login>> {
    let v: serde_json::Value = http
        .get(format!("{base}/QuickConnect/Connect?Secret={secret}"))
        .headers(auth(client_id, None))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if v["Authenticated"].as_bool() != Some(true) {
        return Ok(None);
    }
    let auth: AuthResult = http
        .post(format!("{base}/Users/AuthenticateWithQuickConnect"))
        .headers(auth(client_id, None))
        .json(&serde_json::json!({ "Secret": secret }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let name = server_name(http, base).await?;
    Ok(Some(Login { token: auth.access_token, user_id: auth.user.id, server_id: auth.server_id, server_name: name }))
}

/// Ferme la session côté serveur : le jeton ne sert plus (Se déconnecter).
pub async fn logout(http: &Client, base: &str, client_id: &str, token: &str) {
    let _ = http
        .post(format!("{base}/Sessions/Logout"))
        .headers(auth(client_id, Some(token)))
        .send()
        .await;
}

/// Nom du serveur, et vérification que l'adresse est bien celle d'un serveur Jellyfin ou Emby.
async fn server_name(http: &Client, base: &str) -> Result<String> {
    let info: PublicInfo = http
        .get(format!("{base}/System/Info/Public"))
        .send()
        .await
        .map_err(|_| anyhow!(crate::i18n::tr("jellyfin.unreachable")))?
        .error_for_status()
        .map_err(|_| anyhow!(crate::i18n::tr("jellyfin.notJellyfin")))?
        .json()
        .await
        .map_err(|_| anyhow!(crate::i18n::tr("jellyfin.notJellyfin")))?;
    Ok(info.server_name)
}

/// Serveur Jellyfin ou Emby connecté.
#[derive(Clone)]
pub struct JellyfinServer {
    pub http: Client,
    pub base: String,
    pub token: String,
    pub user_id: String,
    pub client_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Views {
    items: Vec<View>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct View {
    id: String,
    name: String,
    collection_type: Option<String>,
    #[serde(rename = "Type")]
    kind: Option<String>,
}

impl JellyfinServer {
    fn get(&self, path: &str) -> RequestBuilder {
        self.http
            .get(format!("{}{}", self.base, path))
            .headers(auth(&self.client_id, Some(&self.token)))
    }

    /// Compte connecté : nom, photo de profil et rôle d'administrateur.
    pub async fn account(&self) -> Result<crate::plex_admin::Account> {
        let v: Value = self.get(&format!("/Users/{}", self.user_id)).send().await?.error_for_status()?.json().await?;
        Ok(crate::plex_admin::Account {
            username: v["Name"].as_str().unwrap_or_default().to_string(),
            email: None,
            thumb: v["PrimaryImageTag"]
                .as_str()
                .map(|tag| format!("{}/Users/{}/Images/Primary?maxWidth=200&tag={tag}", self.base, self.user_id)),
            plex_pass: false,
            admin: Some(v["Policy"]["IsAdministrator"].as_bool().unwrap_or(false)),
        })
    }

    /// Bibliothèques de l'utilisateur, avec le type que Maku connaît (films, séries, musique).
    pub async fn sections(&self) -> Result<Vec<Section>> {
        let views: Views = self.get(&format!("/Users/{}/Views", self.user_id)).send().await?.error_for_status()?.json().await?;
        Ok(views
            .items
            .into_iter()
            .filter_map(|v| {
                let kind = match v.collection_type.as_deref() {
                    Some("movies") => "movie",
                    Some("tvshows") => "show",
                    Some("music") => "artist",
                    // Bibliothèque « mixte » (films et séries ensemble, comme des animés).
                    None if v.kind.as_deref() == Some("CollectionFolder") => "show",
                    _ => return None,
                };
                Some(Section { key: v.id, title: v.name, kind: kind.into() })
            })
            .collect())
    }
}

// ---------- Étape 2 : bibliothèques, fiches, lecture ----------

use serde_json::Value;

use crate::plex::{Adjacent, CastMember, Genre, Hub, Item, ItemDetail, Page, PlayInfo, QueueTrack, SubFile, Track, WebStream};

/// Champs demandés avec chaque liste : résumé, genres, et ce qu'il faut pour les images.
const FIELDS: &str = "Fields=Overview,Genres,ParentId,ChildCount,RecursiveItemCount&EnableImageTypes=Primary,Backdrop,Thumb,Logo";

/// Durées Jellyfin (« ticks » de 100 ns) en millisecondes.
fn ms(ticks: &Value) -> Option<u64> {
    ticks.as_u64().map(|t| t / 10_000).filter(|&t| t > 0)
}

fn str_of(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}

/// Type Jellyfin → type Maku (celui de Plex).
fn kind_of(t: &str) -> &'static str {
    match t {
        "Movie" => "movie",
        "Series" => "show",
        "Season" => "season",
        "Episode" => "episode",
        "MusicArtist" => "artist",
        "MusicAlbum" => "album",
        "Audio" => "track",
        "Playlist" => "playlist",
        "BoxSet" => "collection",
        _ => "folder",
    }
}

impl JellyfinServer {
    fn send(&self, method: reqwest::Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}{}", self.base, path))
            .headers(auth(&self.client_id, Some(&self.token)))
    }

    /// Droit de télécharger pour la lecture hors ligne : « Autoriser le téléchargement de médias », réglé par
    /// l'administrateur pour chaque utilisateur (Jellyfin et Emby : `Policy.EnableContentDownloading`).
    pub async fn can_download(&self) -> Result<bool> {
        let v = self.json(&format!("/Users/{}", self.uid())).await?;
        Ok(v["Policy"]["EnableContentDownloading"].as_bool().unwrap_or(false))
    }

    async fn json(&self, path: &str) -> Result<Value> {
        Ok(self.get(path).send().await?.error_for_status()?.json().await?)
    }

    async fn items(&self, path: &str) -> Result<Vec<Item>> {
        let v = self.json(path).await?;
        let list = if v.is_array() { v.as_array().cloned() } else { v["Items"].as_array().cloned() };
        Ok(list.unwrap_or_default().iter().filter(|i| kind_of(i["Type"].as_str().unwrap_or("")) != "folder").map(|i| self.to_item(i)).collect())
    }

    fn uid(&self) -> &str {
        &self.user_id
    }

    /// Image d'un élément (`kind` : Primary, Backdrop, Thumb, Logo), réduite par le serveur.
    fn image(&self, id: &str, kind: &str, tag: &str, width: u32) -> String {
        format!("{}/Items/{id}/Images/{kind}?maxWidth={width}&quality=90&tag={tag}", self.base)
    }

    fn to_item(&self, v: &Value) -> Item {
        let id = v["Id"].as_str().unwrap_or_default().to_string();
        let kind = kind_of(v["Type"].as_str().unwrap_or(""));
        let tag = |k: &str| v["ImageTags"][k].as_str();
        let own_primary = tag("Primary").map(|t| self.image(&id, "Primary", t, 400));
        let poster = match kind {
            "episode" => str_of(&v["SeriesId"])
                .zip(v["SeriesPrimaryImageTag"].as_str())
                .map(|(sid, t)| self.image(&sid, "Primary", t, 400))
                .or(own_primary.clone()),
            "track" => str_of(&v["AlbumId"])
                .zip(v["AlbumPrimaryImageTag"].as_str())
                .map(|(aid, t)| self.image(&aid, "Primary", t, 400))
                .or(own_primary.clone()),
            "season" => own_primary.clone().or_else(|| {
                str_of(&v["SeriesId"]).zip(v["SeriesPrimaryImageTag"].as_str()).map(|(sid, t)| self.image(&sid, "Primary", t, 400))
            }),
            _ => own_primary.clone(),
        };
        let backdrop = v["BackdropImageTags"][0]
            .as_str()
            .map(|t| self.image(&id, "Backdrop", t, 1600))
            .or_else(|| {
                str_of(&v["ParentBackdropItemId"])
                    .zip(v["ParentBackdropImageTags"][0].as_str())
                    .map(|(pid, t)| self.image(&pid, "Backdrop", t, 1600))
            });
        // Logo du titre, ou de la série pour un épisode et une saison (Jellyfin les a presque toujours).
        let logo = tag("Logo").map(|t| self.image(&id, "Logo", t, 800)).or_else(|| {
            str_of(&v["ParentLogoItemId"]).zip(v["ParentLogoImageTag"].as_str()).map(|(pid, t)| self.image(&pid, "Logo", t, 800))
        });
        let thumb = match kind {
            "episode" => own_primary.clone(),
            _ => tag("Thumb").map(|t| self.image(&id, "Thumb", t, 640)).or_else(|| backdrop.clone()),
        };
        let user = &v["UserData"];
        let artist = str_of(&v["AlbumArtist"]).or_else(|| v["Artists"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")));
        let subtitle = match kind {
            "episode" => Some(format!(
                "{} · S{}E{}",
                v["SeriesName"].as_str().unwrap_or_default(),
                v["ParentIndexNumber"].as_u64().unwrap_or(0),
                v["IndexNumber"].as_u64().unwrap_or(0)
            )),
            "season" => str_of(&v["SeriesName"]),
            "album" | "track" => artist.clone(),
            "playlist" | "collection" => v["ChildCount"].as_u64().or(v["RecursiveItemCount"].as_u64()).map(|n| {
                let one = n == 1 || (n == 0 && crate::i18n::language() == "fr");
                let key = if one { "plex.itemCount_one" } else { "plex.itemCount_other" };
                crate::i18n::tr_with(key, &[("count", &n.to_string())])
            }),
            _ => None,
        };
        let (show_title, show_key) = match kind {
            "episode" | "season" => (str_of(&v["SeriesName"]), str_of(&v["SeriesId"])),
            "album" | "track" => (artist.clone(), str_of(&v["AlbumArtists"][0]["Id"]).or_else(|| str_of(&v["ArtistItems"][0]["Id"]))),
            _ => (None, None),
        };
        Item {
            last_played: v["UserData"]["LastPlayedDate"].as_str().and_then(iso_ms),
            rating_key: id,
            kind: kind.into(),
            title: v["Name"].as_str().unwrap_or_default().into(),
            subtitle,
            year: v["ProductionYear"].as_i64().map(|y| y as i32),
            summary: str_of(&v["Overview"]),
            poster,
            logo,
            logo_guid: None,
            thumb,
            backdrop,
            index: v["IndexNumber"].as_u64().map(|n| n as u32),
            parent_index: v["ParentIndexNumber"].as_u64().map(|n| n as u32),
            show_title,
            show_key,
            season_key: if kind == "episode" { str_of(&v["SeasonId"]) } else { None },
            album_title: if kind == "track" { str_of(&v["Album"]) } else { None },
            album_key: if kind == "track" { str_of(&v["AlbumId"]) } else { None },
            duration_ms: ms(&v["RunTimeTicks"]),
            view_offset_ms: ms(&user["PlaybackPositionTicks"]),
            watched: user["Played"].as_bool().unwrap_or(false),
            unwatched_count: user["UnplayedItemCount"].as_u64().map(|n| n as u32).filter(|_| matches!(kind, "show" | "season")),
            content_rating: str_of(&v["OfficialRating"]),
            rating: v["CommunityRating"].as_f64().map(|r| r as f32),
            // Jellyfin n'a pas d'étoiles : « J'aime » vaut 5 étoiles, « Je n'aime pas » 1.
            user_rating: user["Likes"].as_bool().map(|l| if l { 10.0 } else { 2.0 }),
            genres: v["Genres"].as_array().map(|g| g.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
            resolution: None,
            playable: matches!(kind, "movie" | "episode" | "track"),
        }
    }

    // ---------- Accueil et bibliothèques ----------

    /// En cours (films et épisodes), puis les épisodes suivants des séries commencées.
    pub async fn on_deck(&self) -> Result<Vec<Item>> {
        let resume_path = format!("/Users/{}/Items/Resume?Limit=24&MediaTypes=Video&{FIELDS}", self.uid());
        let next_path = format!("/Shows/NextUp?UserId={}&Limit=24&{FIELDS}", self.uid());
        let (resume, next) = tokio::join!(self.items(&resume_path), self.items(&next_path));
        let mut items = resume?;
        let shows: Vec<String> = items.iter().filter_map(|i| i.show_key.clone()).collect();
        items.extend(next.unwrap_or_default().into_iter().filter(|i| i.show_key.as_ref().is_none_or(|k| !shows.contains(k))));
        Ok(items)
    }

    pub async fn recently_added(&self) -> Result<Vec<Item>> {
        self.items(&format!("/Users/{}/Items/Latest?Limit=30&{FIELDS}", self.uid())).await
    }

    /// Contenu d'une bibliothèque. `kind` (types de Plex) : 8 artistes, 9 albums, 10 titres ; sinon
    /// films et séries (une bibliothèque Jellyfin « mixte » mêle les deux).
    pub async fn section_items(
        &self,
        key: &str,
        start: u32,
        size: u32,
        kind: Option<u32>,
        genre: Option<&str>,
        sort: Option<&str>,
        filters: &[(String, String)],
    ) -> Result<Page> {
        let types = match kind {
            Some(8) => "MusicArtist",
            Some(9) => "MusicAlbum",
            Some(10) => "Audio",
            _ => "Movie,Series",
        };
        let (field, order) = sort.and_then(|s| s.split_once(':')).unwrap_or((sort.unwrap_or("titleSort"), "asc"));
        let (by, extra) = match field {
            "addedAt" => ("DateCreated", ""),
            "lastViewedAt" => ("DatePlayed", "&IsPlayed=true"),
            "viewCount" => ("PlayCount", ""),
            "originallyAvailableAt" => ("PremiereDate", ""),
            "year" => ("ProductionYear,SortName", ""),
            "rating" => ("CriticRating", ""),
            "audienceRating" => ("CommunityRating", ""),
            "contentRating" => ("OfficialRating", ""),
            "duration" => ("Runtime", ""),
            "random" => ("Random", ""),
            _ => ("SortName", ""),
        };
        let order = if order == "desc" { "Descending" } else { "Ascending" };
        let mut genre = genre.map(|g| format!("&GenreIds={g}")).unwrap_or_default();
        for (name, value) in filters {
            genre.push_str(&jellyfin_filter(name, value));
        }
        let v = self
            .json(&format!(
                "/Users/{}/Items?ParentId={key}&Recursive=true&IncludeItemTypes={types}&StartIndex={start}&Limit={size}&SortBy={by}&SortOrder={order}{extra}{genre}&{FIELDS}",
                self.uid()
            ))
            .await?;
        let items = v["Items"].as_array().cloned().unwrap_or_default().iter().map(|i| self.to_item(i)).collect();
        Ok(Page { items, total: v["TotalRecordCount"].as_u64().unwrap_or(0) as u32 })
    }

    /// Filtres et tris d'une bibliothèque, avec les noms de ceux de Plex (l'interface les traduit).
    pub async fn section_filters(&self, _key: &str, _kind: &str) -> Result<SectionFilters> {
        let field = |id: &str, boolean| FilterField { id: id.into(), title: id.into(), boolean };
        let sort = |key: &str, desc| SortField { key: key.into(), title: key.into(), desc };
        Ok(SectionFilters {
            filters: vec![
                field("unwatched", true),
                field("inProgress", true),
                field("favorite", true),
                field("genre", false),
                field("year", false),
                field("decade", false),
                field("contentRating", false),
                field("studio", false),
                field("label", false),
            ],
            sorts: vec![
                sort("titleSort", false),
                sort("year", true),
                sort("originallyAvailableAt", true),
                sort("rating", true),
                sort("audienceRating", true),
                sort("contentRating", false),
                sort("duration", true),
                sort("viewCount", true),
                sort("addedAt", true),
                sort("lastViewedAt", true),
                sort("random", false),
            ],
        })
    }

    /// Valeurs d'un filtre, tirées des titres de la bibliothèque.
    pub async fn filter_values(&self, key: &str, kind: &str, filter: &str) -> Result<Vec<FilterValue>> {
        let types = if kind == "show" { "Series" } else { "Movie" };
        if filter == "studio" {
            let v = self.json(&format!("/Studios?ParentId={key}&UserId={}&IncludeItemTypes={types}&Recursive=true", self.uid())).await?;
            let list = v["Items"].as_array().cloned().unwrap_or_default();
            return Ok(list
                .iter()
                .map(|s| FilterValue { id: str_of(&s["Id"]).unwrap_or_default(), title: str_of(&s["Name"]).unwrap_or_default() })
                .collect());
        }
        let v = self.json(&format!("/Items/Filters?UserId={}&ParentId={key}&IncludeItemTypes={types}", self.uid())).await?;
        let names = |field: &str| -> Vec<String> {
            v[field].as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(String::from).or_else(|| x.as_u64().map(|n| n.to_string()))).collect()).unwrap_or_default()
        };
        let same = |list: Vec<String>| list.into_iter().map(|n| FilterValue { id: n.clone(), title: n }).collect();
        let mut years: Vec<u32> = names("Years").iter().filter_map(|y| y.parse().ok()).collect();
        years.sort_unstable_by(|a, b| b.cmp(a));
        Ok(match filter {
            "genre" => same(names("Genres")),
            "contentRating" => same(names("OfficialRatings")),
            "label" => same(names("Tags")),
            "year" => same(years.iter().map(u32::to_string).collect()),
            "decade" => {
                let mut decades: Vec<u32> = years.iter().map(|y| y / 10 * 10).collect();
                decades.dedup();
                decades.into_iter().map(|d| FilterValue { id: d.to_string(), title: format!("{d}s") }).collect()
            }
            _ => Vec::new(),
        })
    }

    /// Rangées d'une bibliothèque, comme celles de Plex.
    pub async fn section_hubs(&self, key: &str) -> Result<Vec<Hub>> {
        let u = self.uid();
        let music = self.json(&format!("/Users/{u}/Items/{key}")).await.ok().and_then(|v| str_of(&v["CollectionType"])).as_deref() == Some("music");
        let tr = |k: &str| crate::i18n::tr(&format!("jellyfin.hubs.{k}"));
        let rows: Vec<(String, &str, String)> = if music {
            vec![
                (tr("recentAlbums"), "jellyfin.recent", format!("/Users/{u}/Items/Latest?ParentId={key}&Limit=24&IncludeItemTypes=MusicAlbum&{FIELDS}")),
                (tr("recentlyPlayed"), "jellyfin.played", format!("/Users/{u}/Items?ParentId={key}&Recursive=true&IncludeItemTypes=MusicAlbum&SortBy=DatePlayed&SortOrder=Descending&IsPlayed=true&Limit=24&{FIELDS}")),
                (tr("playlists"), "plexr.playlists", format!("/Users/{u}/Items?IncludeItemTypes=Playlist&Recursive=true&MediaTypes=Audio&{FIELDS}")),
            ]
        } else {
            vec![
                (tr("continue"), "jellyfin.inprogress", format!("/Users/{u}/Items/Resume?ParentId={key}&Limit=24&{FIELDS}")),
                (tr("nextUp"), "jellyfin.nextup", format!("/Shows/NextUp?UserId={u}&ParentId={key}&Limit=24&{FIELDS}")),
                (tr("latest"), "jellyfin.recent", format!("/Users/{u}/Items/Latest?ParentId={key}&Limit=24&{FIELDS}")),
                (tr("topRated"), "jellyfin.toprated", format!("/Users/{u}/Items?ParentId={key}&Recursive=true&IncludeItemTypes=Movie,Series&SortBy=CommunityRating&SortOrder=Descending&Limit=24&{FIELDS}")),
                (tr("rediscover"), "jellyfin.rediscover", format!("/Users/{u}/Items?ParentId={key}&Recursive=true&IncludeItemTypes=Movie,Series&IsPlayed=true&SortBy=Random&Limit=24&{FIELDS}")),
            ]
        };
        let lists = futures::future::join_all(rows.iter().map(|(_, _, path)| self.items(path))).await;
        Ok(rows
            .into_iter()
            .zip(lists)
            .filter_map(|((title, identifier, _), items)| {
                let items = items.ok()?;
                (!items.is_empty()).then(|| Hub { title, identifier: identifier.into(), items })
            })
            .collect())
    }

    /// Genres d'une bibliothèque, chacun avec quelques affiches de ses titres les mieux notés.
    pub async fn genres(&self, key: &str) -> Result<Vec<Genre>> {
        let v = self.json(&format!("/Genres?ParentId={key}&UserId={}&Fields=ItemCounts", self.uid())).await?;
        let list = v["Items"].as_array().cloned().unwrap_or_default();
        let out = futures::future::join_all(list.iter().map(|g| async move {
            let id = g["Id"].as_str().unwrap_or_default().to_string();
            let page = self.section_items(key, 0, 4, None, Some(&id), Some("audienceRating:desc"), &[]).await.ok()?;
            (page.total > 0).then(|| Genre {
                id: id.clone(),
                title: g["Name"].as_str().unwrap_or_default().into(),
                count: page.total,
                posters: page.items.iter().filter_map(|i| i.poster.clone()).collect(),
                backdrop: page.items.iter().find_map(|i| i.backdrop.clone()),
            })
        }))
        .await;
        Ok(out.into_iter().flatten().collect())
    }

    pub async fn playlists(&self, _section: &str) -> Result<Vec<Item>> {
        self.items(&format!("/Users/{}/Items?IncludeItemTypes=Playlist&Recursive=true&MediaTypes=Audio&{FIELDS}", self.uid())).await
    }

    // ---------- Collections : « Ajouter à la collection » ----------

    /// Chez Jellyfin, les collections ne dépendent pas d'une bibliothèque : toutes sont proposées.
    pub async fn collection_choices(&self, _id: &str) -> Result<Vec<Item>> {
        self.collections("").await
    }

    pub async fn add_to_collection(&self, id: &str, collection: &str) -> Result<()> {
        self.send(reqwest::Method::POST, &format!("/Collections/{collection}/Items?ids={id}")).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn create_collection(&self, id: &str, title: &str) -> Result<()> {
        self.send(reqwest::Method::POST, &format!("/Collections?name={}&ids={id}", urlencoding::encode(title)))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    // ---------- Playlists : « Ajouter à une playlist » ----------

    /// Playlists de musique de l'utilisateur.
    pub async fn playlist_choices(&self) -> Result<Vec<Item>> {
        self.playlists("").await
    }

    /// Ajoute un titre, un album ou un artiste à une playlist (le serveur en reprend tous les titres).
    pub async fn add_to_playlist(&self, id: &str, playlist: &str) -> Result<()> {
        let ok = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        if !ok(playlist) || !ok(id) {
            bail!("élément inconnu");
        }
        self.send(reqwest::Method::POST, &format!("/Playlists/{playlist}/Items?ids={id}&userId={}", self.uid()))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn create_playlist(&self, id: &str, title: &str) -> Result<()> {
        // Paramètres dans l'adresse : Emby ne lit qu'eux, Jellyfin les accepte toujours.
        let path = format!("/Playlists?name={}&ids={id}&userId={}&mediaType=Audio", urlencoding::encode(title), self.uid());
        self.send(reqwest::Method::POST, &path).send().await?.error_for_status()?;
        Ok(())
    }

    /// « Voir les informations » d'un titre : son fichier et son format.
    pub async fn media_info(&self, id: &str) -> Result<crate::plex::MediaInfo> {
        let v = self.json(&format!("/Users/{}/Items/{id}?Fields=MediaSources,Path,DateCreated", self.uid())).await?;
        let source = &v["MediaSources"][0];
        let streams = source["MediaStreams"].as_array().cloned().unwrap_or_default();
        let of = |kind: &'static str| streams.iter().filter(move |s| s["Type"] == kind);
        let audio = of("Audio").find(|s| s["IsDefault"] == true).or_else(|| of("Audio").next()).cloned().unwrap_or_default();
        let video = of("Video").next().cloned().unwrap_or_default();
        let is_video = !video.is_null();
        let text = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned);
        let ticks = |v: &Value| v.as_u64().map(|t| t / 10_000);
        Ok(crate::plex::MediaInfo {
            container: text(&source["Container"]),
            codec: text(&audio["Codec"]),
            bitrate_kbps: source["Bitrate"].as_u64().or_else(|| audio["BitRate"].as_u64()).map(|b| b / 1000),
            channels: audio["Channels"].as_u64(),
            sampling_rate: audio["SampleRate"].as_u64(),
            bit_depth: audio["BitDepth"].as_u64(),
            size_bytes: source["Size"].as_u64(),
            duration_ms: ticks(&v["RunTimeTicks"]).or_else(|| ticks(&source["RunTimeTicks"])),
            file: text(&source["Path"]).or_else(|| text(&v["Path"])),
            added_at: v["DateCreated"].as_str().and_then(iso_ms),
            play_count: v["UserData"]["PlayCount"].as_u64(),
            video: text(&video["DisplayTitle"]),
            frame_rate: video["RealFrameRate"].as_f64().map(|f| format!("{}p", f.round())),
            audio: if is_video { text(&audio["DisplayTitle"]) } else { None },
            subtitles: is_video.then(|| of("Subtitle").count() as u64),
        })
    }

    pub async fn collections(&self, _key: &str) -> Result<Vec<Item>> {
        self.items(&format!("/Users/{}/Items?IncludeItemTypes=BoxSet&Recursive=true&SortBy=SortName&{FIELDS}", self.uid())).await
    }

    pub async fn collection_items(&self, id: &str) -> Result<Vec<Item>> {
        self.items(&format!("/Users/{}/Items?ParentId={id}&SortBy=PremiereDate,SortName&{FIELDS}", self.uid())).await
    }

    pub async fn related(&self, id: &str) -> Result<Vec<Item>> {
        self.items(&format!("/Items/{id}/Similar?UserId={}&Limit=24&{FIELDS}", self.uid())).await
    }

    /// Films et séries où joue une personne, du plus récent au plus ancien.
    pub async fn person_items(&self, id: &str) -> Result<Vec<Item>> {
        self.items(&format!(
            "/Users/{}/Items?PersonIds={id}&Recursive=true&IncludeItemTypes=Movie,Series&SortBy=PremiereDate,ProductionYear&SortOrder=Descending&{FIELDS}",
            self.uid()
        ))
        .await
    }

    pub async fn search(&self, query: &str) -> Result<Vec<Item>> {
        self.items(&format!(
            "/Users/{}/Items?searchTerm={}&Recursive=true&IncludeItemTypes=Movie,Series,Episode,MusicArtist,MusicAlbum,Audio&Limit=40&{FIELDS}",
            self.uid(),
            urlencoding::encode(query)
        ))
        .await
    }

    /// Saisons d'une série, épisodes d'une saison, titres d'un album, albums d'un artiste.
    pub async fn children(&self, id: &str) -> Result<Vec<Item>> {
        let u = self.uid();
        let v = self.json(&format!("/Users/{u}/Items/{id}")).await?;
        let path = match v["Type"].as_str().unwrap_or("") {
            "MusicArtist" => format!("/Users/{u}/Items?ArtistIds={id}&IncludeItemTypes=MusicAlbum&Recursive=true&SortBy=PremiereDate&SortOrder=Descending&{FIELDS}"),
            _ => format!("/Users/{u}/Items?ParentId={id}&SortBy=ParentIndexNumber,IndexNumber,SortName&{FIELDS}"),
        };
        self.items(&path).await
    }

    /// Épisodes précédent et suivant.
    pub async fn adjacent(&self, id: &str) -> Result<Adjacent> {
        let u = self.uid();
        let v = self.json(&format!("/Users/{u}/Items/{id}")).await?;
        let Some(series) = str_of(&v["SeriesId"]) else { return Ok(Adjacent { prev: None, next: None }) };
        let eps = self.json(&format!("/Shows/{series}/Episodes?UserId={u}&AdjacentTo={id}")).await?;
        let ids: Vec<String> = eps["Items"].as_array().cloned().unwrap_or_default().iter().filter_map(|e| str_of(&e["Id"])).collect();
        let pos = ids.iter().position(|x| x == id);
        Ok(Adjacent {
            prev: pos.and_then(|p| p.checked_sub(1)).and_then(|p| ids.get(p).cloned()),
            next: pos.and_then(|p| ids.get(p + 1).cloned()),
        })
    }

    // ---------- Fiche ----------

    pub async fn metadata(&self, id: &str) -> Result<ItemDetail> {
        let v = self.json(&format!("/Users/{}/Items/{id}", self.uid())).await?;
        let item = self.to_item(&v);
        let people = v["People"].as_array().cloned().unwrap_or_default();
        let names = |t: &str| people.iter().filter(|p| p["Type"] == t).filter_map(|p| str_of(&p["Name"])).collect::<Vec<_>>();
        let cast = people
            .iter()
            .filter(|p| p["Type"] == "Actor")
            .take(30)
            .map(|p| CastMember {
                id: str_of(&p["Id"]),
                name: p["Name"].as_str().unwrap_or_default().into(),
                role: str_of(&p["Role"]),
                photo: str_of(&p["Id"]).zip(p["PrimaryImageTag"].as_str()).map(|(pid, t)| self.image(&pid, "Primary", t, 300)),
            })
            .collect();
        let sources = v["MediaSources"].as_array().cloned().unwrap_or_default();
        let streams_of = |source: &Value| source["MediaStreams"].as_array().cloned().unwrap_or_default();
        let video_of = |streams: &[Value]| {
            streams.iter().find(|s| s["Type"] == "Video").map(|s| {
                // D'après la largeur : un film au format large en 4K fait 3840 × 1600, pas « 1600p ».
                let w = s["Width"].as_u64().unwrap_or(0);
                let res = match w {
                    w if w >= 3200 => "4K".to_string(),
                    w if w >= 1800 => "1080p".to_string(),
                    w if w >= 1200 => "720p".to_string(),
                    w if w > 0 => "SD".to_string(),
                    _ => String::new(),
                };
                format!("{res} ({})", s["Codec"].as_str().unwrap_or("").to_uppercase()).trim().to_string()
            })
        };
        let tracks_of = |streams: &[Value], t: &str| {
            streams
                .iter()
                .filter(|s| s["Type"] == t)
                .map(|s| Track {
                    id: s["Index"].as_u64().unwrap_or(0),
                    label: str_of(&s["DisplayTitle"]).unwrap_or_else(|| crate::i18n::tr_with("plex.track", &[("id", &s["Index"].to_string())])),
                    selected: s["IsDefault"].as_bool().unwrap_or(false),
                })
                .collect::<Vec<_>>()
        };
        let versions: Vec<crate::plex::Version> = sources
            .iter()
            .map(|source| {
                let streams = streams_of(source);
                let video = video_of(&streams);
                // Débit en b/s chez Jellyfin.
                let rate = source["Bitrate"].as_u64().map(|b| crate::plex::bitrate_label(b / 1000));
                let label = [video.clone(), rate].into_iter().flatten().collect::<Vec<_>>().join(" · ");
                crate::plex::Version {
                    label,
                    video,
                    audio_tracks: tracks_of(&streams, "Audio"),
                    subtitle_tracks: tracks_of(&streams, "Subtitle"),
                }
            })
            .collect();
        let streams = sources.first().map(streams_of).unwrap_or_default();
        let video = video_of(&streams);
        let tracks = |t: &str| tracks_of(&streams, t);
        // Jellyfin et Emby : la note du public (TMDB ou IMDb selon le fournisseur) et celle de la presse
        // (Rotten Tomatoes, en pourcentage).
        let mut site_ratings = Vec::new();
        if let Some(r) = v["CommunityRating"].as_f64().filter(|r| *r > 0.0) {
            site_ratings.push(crate::plex::SiteRating { source: "community", value: r as f32, good: true });
        }
        if let Some(r) = v["CriticRating"].as_f64().filter(|r| *r > 0.0) {
            site_ratings.push(crate::plex::SiteRating { source: "rtCritic", value: r as f32 / 10.0, good: r >= 60.0 });
        }
        Ok(ItemDetail {
            site_ratings,
            imdb_id: str_of(&v["ProviderIds"]["Imdb"]),
            tagline: str_of(&v["Taglines"][0]),
            original_title: str_of(&v["OriginalTitle"]).filter(|o| *o != item.title),
            backdrop_hd: v["BackdropImageTags"][0].as_str().map(|t| self.image(&item.rating_key, "Backdrop", t, 2560)).or_else(|| item.backdrop.clone()),
            studio: str_of(&v["Studios"][0]["Name"]),
            release_date: str_of(&v["PremiereDate"]).map(|d| d.chars().take(10).collect()),
            directors: names("Director"),
            writers: names("Writer"),
            cast,
            video,
            audio_tracks: tracks("Audio"),
            subtitle_tracks: tracks("Subtitle"),
            versions,
            item,
        })
    }

    // ---------- Lecture ----------

    fn stream_url(&self, id: &str, source: &str) -> String {
        format!("{}/Videos/{id}/stream?static=true&mediaSourceId={source}&api_key={}", self.base, self.token)
    }

    /// Lecture directe : le fichier d'origine et ses pistes (numérotées comme dans mpv).
    /// Segments du serveur (Jellyfin 10.10 et plus, remplis par exemple par le plugin Intro Skipper) :
    /// intro et générique (« Outro »). Sans segments, rien.
    /// Miniatures d'aperçu (« trickplay », Jellyfin 10.9 et plus) : des planches de vignettes,
    /// dans la plus petite largeur disponible.
    pub async fn previews(&self, id: &str, media: Option<usize>) -> Result<Option<crate::plex::Previews>> {
        let v = self.json(&format!("/Users/{}/Items/{id}?Fields=Trickplay", self.uid())).await?;
        let index = media.filter(|&i| v["MediaSources"][i].is_object()).unwrap_or(0);
        let source_id = v["MediaSources"][index]["Id"].as_str().unwrap_or(id).to_string();
        let Some(sizes) = v["Trickplay"][&source_id].as_object() else { return Ok(None) };
        let Some((width, info)) = sizes.iter().filter_map(|(w, i)| Some((w.parse::<u32>().ok()?, i))).min_by_key(|(w, _)| *w) else {
            return Ok(None);
        };
        let n = |k: &str| info[k].as_u64().unwrap_or(0) as u32;
        Ok(Some(crate::plex::Previews {
            kind: "tiles",
            url: format!(
                "{}/Videos/{id}/Trickplay/{width}/{{index}}.jpg?MediaSourceId={source_id}&api_key={}",
                self.base, self.token
            ),
            interval_ms: info["Interval"].as_u64().unwrap_or(10_000),
            width: n("Width"),
            height: n("Height"),
            columns: n("TileWidth").max(1),
            rows: n("TileHeight").max(1),
            count: n("ThumbnailCount"),
        }))
    }

    pub async fn markers(&self, id: &str) -> Result<Vec<Marker>> {
        let Ok(v) = self.json(&format!("/MediaSegments/{id}")).await else { return self.chapter_markers(id).await };
        Ok(v["Items"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|s| {
                        let kind = match s["Type"].as_str()? {
                            "Intro" => "intro",
                            "Outro" => "credits",
                            "Recap" => "recap",
                            "Preview" => "preview",
                            "Commercial" => "commercial",
                            _ => return None,
                        };
                        let (start, end) = (s["StartTicks"].as_u64()? / 10_000, s["EndTicks"].as_u64()? / 10_000);
                        (end > start).then(|| Marker { kind: kind.into(), start_ms: start, end_ms: end })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Emby : l'intro et le générique sont des chapitres marqués (`IntroStart`, `IntroEnd`, `CreditsStart`).
    async fn chapter_markers(&self, id: &str) -> Result<Vec<Marker>> {
        let Ok(v) = self.json(&format!("/Users/{}/Items/{id}?Fields=Chapters", self.uid())).await else { return Ok(Vec::new()) };
        Ok(chapter_markers(&v))
    }

    /// Bande-annonce : un fichier local du serveur, sinon un lien YouTube de la fiche ; et
    /// l'identifiant TMDB pour chercher en ligne à défaut.
    pub async fn trailer(&self, id: &str) -> Result<TrailerSource> {
        let v = self.json(&format!("/Users/{}/Items/{id}", self.uid())).await?;
        let mut local = Vec::new();
        if v["LocalTrailerCount"].as_u64().unwrap_or(0) > 0 {
            if let Ok(list) = self.json(&format!("/Users/{}/Items/{id}/LocalTrailers", self.uid())).await {
                local = list
                    .as_array()
                    .map(|a| a.iter().filter_map(|t| Some((str_of(&t["Id"])?, str_of(&t["Name"]).unwrap_or_default()))).collect())
                    .unwrap_or_default();
            }
        }
        let youtube = v["RemoteTrailers"]
            .as_array()
            .and_then(|a| a.iter().find_map(|t| crate::plex::youtube_id(t["Url"].as_str()?)));
        let tmdb_id = str_of(&v["ProviderIds"]["Tmdb"]);
        let tmdb_kind = Some(if v["Type"] == "Series" { "tv" } else { "movie" });
        Ok(TrailerSource { local, youtube, tmdb_kind, tmdb_id })
    }

    pub async fn play_info(&self, id: &str, media: Option<usize>, audio: Option<u64>, subtitle: Option<u64>) -> Result<PlayInfo> {
        let v = self.json(&format!("/Users/{}/Items/{id}", self.uid())).await?;
        let index = media.filter(|&i| v["MediaSources"][i].is_object()).unwrap_or(0);
        let source = &v["MediaSources"][index];
        let source_id = source["Id"].as_str().unwrap_or(id).to_string();
        let streams = source["MediaStreams"].as_array().cloned().unwrap_or_default();
        let audios: Vec<&Value> = streams.iter().filter(|s| s["Type"] == "Audio").collect();
        let aid = match audio {
            Some(i) => audios.iter().position(|s| s["Index"].as_u64() == Some(i)),
            None => audios.iter().position(|s| s["IsDefault"].as_bool() == Some(true)),
        }
        .map(|i| i + 1);
        let subs: Vec<&Value> = streams.iter().filter(|s| s["Type"] == "Subtitle").collect();
        let embedded: Vec<&&Value> = subs.iter().filter(|s| !s["IsExternal"].as_bool().unwrap_or(false)).collect();
        let external: Vec<&&Value> = subs.iter().filter(|s| s["IsExternal"].as_bool().unwrap_or(false)).collect();
        let wanted = match subtitle {
            Some(0) => None,
            Some(i) => Some(i),
            None => subs.iter().find(|s| s["IsDefault"].as_bool() == Some(true) || s["IsForced"].as_bool() == Some(true)).and_then(|s| s["Index"].as_u64()),
        };
        let sid = match wanted {
            None => "no".to_string(),
            Some(i) => embedded
                .iter()
                .position(|s| s["Index"].as_u64() == Some(i))
                .map(|p| p + 1)
                .or_else(|| external.iter().position(|s| s["Index"].as_u64() == Some(i)).map(|p| embedded.len() + p + 1))
                .map_or_else(|| "auto".to_string(), |n| n.to_string()),
        };
        let sub_tracks: Vec<SubFile> = external
            .iter()
            .map(|s| {
                let codec = s["Codec"].as_str().unwrap_or("srt").to_string();
                SubFile {
                    url: format!(
                        "{}/Videos/{id}/{source_id}/Subtitles/{}/Stream.{codec}?api_key={}",
                        self.base,
                        s["Index"].as_u64().unwrap_or(0),
                        self.token
                    ),
                    codec,
                    language: str_of(&s["Language"]),
                    title: str_of(&s["DisplayTitle"]),
                }
            })
            .collect();
        let (title, subtitle_line) = match (v["Type"].as_str(), str_of(&v["SeriesName"])) {
            (Some("Episode"), Some(show)) => (
                show,
                Some(crate::plex::episode_line(
                    v["ParentIndexNumber"].as_u64().unwrap_or(0) as u32,
                    v["IndexNumber"].as_u64().unwrap_or(0) as u32,
                    v["Name"].as_str().unwrap_or_default(),
                )),
            ),
            _ => (v["Name"].as_str().unwrap_or_default().to_string(), v["ProductionYear"].as_i64().map(|y| y.to_string())),
        };
        Ok(PlayInfo {
            url: self.stream_url(id, &source_id),
            title,
            subtitle: subtitle_line,
            offset_ms: ms(&v["UserData"]["PlaybackPositionTicks"]).unwrap_or(0),
            duration_ms: ms(&v["RunTimeTicks"]).unwrap_or(0),
            aid,
            sid,
            sub_files: sub_tracks.iter().map(|s| s.url.clone()).collect(),
            sub_tracks,
        })
    }

    /// Flux converti par le serveur (HLS, H.264 / AAC, 1080p), quand l'appareil ne lit pas le fichier.
    pub async fn web_stream(
        &self,
        id: &str,
        media: Option<usize>,
        audio: Option<u64>,
        subtitle: Option<u64>,
        max_kbps: Option<u32>,
        session: &str,
        copy_video: bool,
    ) -> Result<WebStream> {
        let info = self.play_info(id, media, audio, subtitle).await?;
        let v = self.json(&format!("/Users/{}/Items/{id}", self.uid())).await?;
        let index = media.filter(|&i| v["MediaSources"][i].is_object()).unwrap_or(0);
        let source_id = v["MediaSources"][index]["Id"].as_str().unwrap_or(id).to_string();
        let kbps = max_kbps.unwrap_or(crate::plex::DEFAULT_KBPS);
        let (width, _) = crate::plex::resolution_for(kbps);
        // Son seulement (lecteur Android, qualité d'origine) : la vidéo est copiée telle quelle (segments fMP4,
        // nécessaires au HEVC), seul le son est converti, en 5.1. Les sous-titres ne sont incrustés que si on en a
        // choisi un. Emby lit les mêmes paramètres.
        if copy_video {
            let mut url = format!(
                "{}/Videos/{id}/master.m3u8?MediaSourceId={source_id}&PlaySessionId={session}&DeviceId={}&api_key={}\
                 &VideoCodec=hevc,h264,av1,vp9&AllowVideoStreamCopy=true&EnableAutoStreamCopy=true&AllowAudioStreamCopy=false\
                 &AudioCodec=ac3,aac&TranscodingMaxAudioChannels=6&MaxStreamingBitrate=200000000&SegmentContainer=mp4",
                self.base, self.client_id, self.token
            );
            if let Some(a) = audio {
                url.push_str(&format!("&AudioStreamIndex={a}"));
            }
            if let Some(s) = subtitle.filter(|&s| s > 0) {
                url.push_str(&format!("&SubtitleStreamIndex={s}&SubtitleMethod=Encode"));
            }
            return Ok(WebStream {
                session: session.to_owned(),
                url,
                title: info.title,
                subtitle: info.subtitle,
                offset_ms: info.offset_ms,
                duration_ms: info.duration_ms,
            });
        }
        let mut url = format!(
            "{}/Videos/{id}/master.m3u8?MediaSourceId={source_id}&PlaySessionId={session}&DeviceId={}&api_key={}\
             &VideoCodec=h264&AudioCodec=aac&MaxStreamingBitrate={}&MaxWidth={width}&TranscodingMaxAudioChannels=2\
             &SegmentContainer=ts&SubtitleMethod=Encode",
            self.base,
            self.client_id,
            self.token,
            kbps * 1000
        );
        if let Some(a) = audio {
            url.push_str(&format!("&AudioStreamIndex={a}"));
        }
        if let Some(s) = subtitle.filter(|&s| s > 0) {
            url.push_str(&format!("&SubtitleStreamIndex={s}"));
        }
        Ok(WebStream {
            session: session.to_owned(),
            url,
            title: info.title,
            subtitle: info.subtitle,
            offset_ms: info.offset_ms,
            duration_ms: info.duration_ms,
        })
    }

    pub async fn stop_transcode(&self, session: &str) -> Result<()> {
        self.send(reqwest::Method::DELETE, &format!("/Videos/ActiveEncodings?DeviceId={}&PlaySessionId={session}", self.client_id))
            .send()
            .await?;
        Ok(())
    }

    /// Progression envoyée au serveur (alimente « Reprendre » et « À suivre »).
    pub async fn timeline(&self, id: &str, state: &str, time_ms: u64, _duration_ms: u64, session: &str) -> Result<()> {
        let path = if state == "stopped" { "/Sessions/Playing/Stopped" } else { "/Sessions/Playing/Progress" };
        self.send(reqwest::Method::POST, path)
            .json(&serde_json::json!({
                "ItemId": id,
                "PlaySessionId": session,
                "PositionTicks": time_ms * 10_000,
                "IsPaused": state == "paused",
                "CanSeek": true
            }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Note : Jellyfin ne connaît que « J'aime » / « Je n'aime pas » (3 étoiles et plus = j'aime).
    pub async fn rate(&self, id: &str, rating: Option<f32>) -> Result<()> {
        let path = format!("/Users/{}/Items/{id}/Rating", self.uid());
        let req = match rating {
            Some(r) => self.send(reqwest::Method::POST, &format!("{path}?likes={}", r >= 6.0)),
            None => self.send(reqwest::Method::DELETE, &path),
        };
        req.send().await?.error_for_status()?;
        Ok(())
    }

    /// Retire un titre de « Continuer à regarder » : sa position de lecture repart à zéro.
    pub async fn remove_from_continue(&self, id: &str) -> Result<()> {
        let body = serde_json::json!({ "PlaybackPositionTicks": 0 });
        let current = self
            .send(reqwest::Method::POST, &format!("/UserItems/{id}/UserData?userId={}", self.uid()))
            .json(&body)
            .send()
            .await?;
        if current.status().is_success() {
            return Ok(());
        }
        // Serveurs Jellyfin plus anciens (avant 10.9).
        self.send(reqwest::Method::POST, &format!("/Users/{}/Items/{id}/UserData", self.uid()))
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn set_watched(&self, id: &str, watched: bool) -> Result<()> {
        let method = if watched { reqwest::Method::POST } else { reqwest::Method::DELETE };
        self.send(method, &format!("/Users/{}/PlayedItems/{id}", self.uid())).send().await?.error_for_status()?;
        Ok(())
    }

    // ---------- Musique ----------

    /// Titres d'un album, d'une playlist ou d'un artiste.
    pub async fn tracks(&self, id: &str) -> Result<Vec<Item>> {
        let u = self.uid();
        let v = self.json(&format!("/Users/{u}/Items/{id}")).await?;
        let path = match v["Type"].as_str().unwrap_or("") {
            "Audio" => return Ok(vec![self.to_item(&v)]),
            "MusicArtist" => format!("/Users/{u}/Items?ArtistIds={id}&IncludeItemTypes=Audio&Recursive=true&SortBy=Album,ParentIndexNumber,IndexNumber&{FIELDS}"),
            "Playlist" => format!("/Playlists/{id}/Items?UserId={u}&{FIELDS}"),
            _ => format!("/Users/{u}/Items?ParentId={id}&IncludeItemTypes=Audio&Recursive=true&SortBy=ParentIndexNumber,IndexNumber,SortName&{FIELDS}"),
        };
        self.items(&path).await
    }

    /// Radio d'un artiste : le « mix instantané » de Jellyfin et d'Emby (ses titres et ceux d'artistes proches).
    pub async fn radio(&self, id: &str) -> Result<Vec<QueueTrack>> {
        let items = self.items(&format!("/Items/{id}/InstantMix?UserId={}&Limit=80&{FIELDS}", self.uid())).await?;
        if items.is_empty() {
            return Err(anyhow!(crate::i18n::tr("plex.noRadio")));
        }
        Ok(self.queue_tracks(items))
    }

    /// File de lecture musicale : les titres, chacun avec l'adresse de son fichier.
    pub async fn queue(&self, id: &str) -> Result<Vec<QueueTrack>> {
        Ok(self.queue_tracks(self.tracks(id).await?))
    }

    fn queue_tracks(&self, items: Vec<Item>) -> Vec<QueueTrack> {
        items
            .into_iter()
            .map(|t| QueueTrack {
                url: format!("{}/Audio/{}/stream?static=true&api_key={}", self.base, t.rating_key, self.token),
                rating_key: t.rating_key,
                title: t.title,
                artist: t.show_title,
                artist_key: t.show_key,
                album: t.album_title,
                album_key: t.album_key,
                cover: t.poster,
                duration_ms: t.duration_ms.unwrap_or(0),
            })
            .collect()
    }
}

/// Date ISO de Jellyfin et Emby (« 2026-11-05T20:12:34.0000000Z », en UTC) en millisecondes depuis 1970.
fn iso_ms(s: &str) -> Option<u64> {
    let num = |a: usize, b: usize| s.get(a..b)?.parse::<i64>().ok();
    let (y, m, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, min, sec) = (num(11, 13).unwrap_or(0), num(14, 16).unwrap_or(0), num(17, 19).unwrap_or(0));
    // Jours depuis 1970 (algorithme « days from civil » de Howard Hinnant).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(((days * 24 + h) * 60 + min) * 60 + sec).ok().map(|t| t * 1000).filter(|&t| t > 0)
}

/// Marqueurs tirés des chapitres d'Emby : intro de `IntroStart` à `IntroEnd`, générique de
/// `CreditsStart` à la fin du titre.
fn chapter_markers(item: &Value) -> Vec<Marker> {
    let chapters = item["Chapters"].as_array().cloned().unwrap_or_default();
    let at = |t: &str| chapters.iter().find(|c| c["MarkerType"] == t).and_then(|c| c["StartPositionTicks"].as_u64()).map(|t| t / 10_000);
    let mut out = Vec::new();
    if let (Some(start), Some(end)) = (at("IntroStart"), at("IntroEnd")) {
        if end > start {
            out.push(Marker { kind: "intro".into(), start_ms: start, end_ms: end });
        }
    }
    if let (Some(start), Some(end)) = (at("CreditsStart"), ms(&item["RunTimeTicks"])) {
        if end > start {
            out.push(Marker { kind: "credits".into(), start_ms: start, end_ms: end });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emby_chapters_give_intro_and_credits() {
        let item = serde_json::json!({
            "RunTimeTicks": 30_000_000_000u64,
            "Chapters": [
                { "StartPositionTicks": 0, "MarkerType": "Chapter" },
                { "StartPositionTicks": 600_000_000u64, "MarkerType": "IntroStart" },
                { "StartPositionTicks": 1_500_000_000u64, "MarkerType": "IntroEnd" },
                { "StartPositionTicks": 28_000_000_000u64, "MarkerType": "CreditsStart" }
            ]
        });
        let m = chapter_markers(&item);
        assert_eq!(m.len(), 2);
        assert_eq!((m[0].kind.as_str(), m[0].start_ms, m[0].end_ms), ("intro", 60_000, 150_000));
        assert_eq!((m[1].kind.as_str(), m[1].start_ms, m[1].end_ms), ("credits", 2_800_000, 3_000_000));
    }

    #[test]
    fn iso_dates() {
        assert_eq!(iso_ms("1970-01-02T00:00:00Z"), Some(86_400_000));
        assert_eq!(iso_ms("2026-11-05T20:12:34.0000000Z"), Some(1_793_909_554_000));
        assert_eq!(iso_ms("0001-01-01T00:00:00.0000000Z"), None);
        assert_eq!(iso_ms("n/a"), None);
    }

    #[test]
    fn auth_sends_both_header_names() {
        let h = auth("id", Some("tok"));
        assert!(h.contains_key("Authorization"));
        assert!(h.contains_key("X-Emby-Authorization"));
        assert_eq!(h["X-Emby-Token"], "tok");
    }
}

/// Un filtre de l'onglet Bibliothèque (noms de Plex) traduit en paramètre de Jellyfin.
fn jellyfin_filter(name: &str, value: &str) -> String {
    let v = urlencoding::encode(value);
    match name {
        "unwatched" => "&IsPlayed=false".into(),
        "inProgress" => "&Filters=IsResumable".into(),
        "favorite" => "&Filters=IsFavorite".into(),
        "genre" => format!("&Genres={v}"),
        "year" => format!("&Years={v}"),
        "decade" => value.parse::<u32>().map(|d| format!("&Years={}", (d..d + 10).map(|y| y.to_string()).collect::<Vec<_>>().join(","))).unwrap_or_default(),
        "contentRating" => format!("&OfficialRatings={v}"),
        "studio" => format!("&StudioIds={v}"),
        "label" => format!("&Tags={v}"),
        _ => String::new(),
    }
}
