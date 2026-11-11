//! Fonctions réservées aux administrateurs du serveur Plex : édition des métadonnées,
//! images, actualisation, réglages du serveur, lectures en cours et entretien des bibliothèques.
//! Plex refuse ces appels (401 / 403) pour un utilisateur partagé.

use crate::plex::PlexServer;
use anyhow::{anyhow, bail, Result};
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Numéro de type Plex, utilisé par l'API d'édition.
fn type_id(kind: &str) -> Option<u32> {
    Some(match kind {
        "movie" => 1,
        "show" => 2,
        "season" => 3,
        "episode" => 4,
        "artist" => 8,
        "album" => 9,
        "track" => 10,
        // Collection (films ou séries) : son titre, son résumé… et ses images.
        "collection" => 18,
        _ => return None,
    })
}

/// Champs modifiables par type, avec la clé de traduction de leur libellé (« admin.fields.… ») et le type de saisie.
fn fields_for(kind: &str) -> &'static [(&'static str, &'static str, &'static str)] {
    match kind {
        "movie" => &[
            ("title", "title", "text"),
            ("titleSort", "titleSort", "text"),
            ("originalTitle", "originalTitle", "text"),
            ("originallyAvailableAt", "releaseDate", "date"),
            ("contentRating", "contentRating", "text"),
            ("studio", "studio", "text"),
            ("tagline", "tagline", "text"),
            ("summary", "summary", "long"),
        ],
        "show" => &[
            ("title", "title", "text"),
            ("titleSort", "titleSort", "text"),
            ("originalTitle", "originalTitle", "text"),
            ("originallyAvailableAt", "firstAired", "date"),
            ("contentRating", "contentRating", "text"),
            ("studio", "network", "text"),
            ("tagline", "tagline", "text"),
            ("summary", "summary", "long"),
        ],
        "season" => &[("title", "title", "text"), ("summary", "summary", "long")],
        "episode" => &[
            ("title", "title", "text"),
            ("titleSort", "titleSort", "text"),
            ("originallyAvailableAt", "airDate", "date"),
            ("contentRating", "contentRating", "text"),
            ("summary", "summary", "long"),
        ],
        "artist" => &[
            ("title", "artist", "text"),
            ("titleSort", "artistSort", "text"),
            ("summary", "biography", "long"),
        ],
        "album" => &[
            ("title", "album", "text"),
            ("titleSort", "albumSort", "text"),
            ("originallyAvailableAt", "releaseDate", "date"),
            ("studio", "label", "text"),
            ("summary", "albumSummary", "long"),
        ],
        "track" => &[
            ("title", "title", "text"),
            ("titleSort", "titleSort", "text"),
            ("originalTitle", "trackArtist", "text"),
            ("index", "trackNumber", "text"),
        ],
        "collection" => &[
            ("title", "title", "text"),
            ("titleSort", "titleSort", "text"),
            ("contentRating", "contentRating", "text"),
            ("summary", "summary", "long"),
        ],
        _ => &[],
    }
}

fn has_genres(kind: &str) -> bool {
    matches!(kind, "movie" | "show" | "artist" | "album")
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditField {
    pub name: String,
    pub label: String,
    /// « text », « long » (zone de texte) ou « date » (AAAA-MM-JJ).
    pub input: String,
    pub value: String,
    pub locked: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditInfo {
    pub rating_key: String,
    pub kind: String,
    pub title: String,
    pub fields: Vec<EditField>,
    /// `None` quand ce type d'élément n'a pas de genres.
    pub genres: Option<Vec<String>>,
    pub genres_locked: bool,
}

#[derive(Deserialize)]
pub struct FieldChange {
    pub name: String,
    pub value: String,
    pub locked: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageChoice {
    /// Identifiant à renvoyer pour choisir l'image.
    pub key: String,
    pub preview: String,
    pub selected: bool,
    pub provider: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pref {
    pub id: String,
    pub label: String,
    pub summary: String,
    /// « bool », « int », « double » ou « text ».
    pub kind: String,
    pub value: String,
    pub default: String,
    pub advanced: bool,
    pub group: String,
    /// Valeurs proposées (valeur, libellé) pour une liste déroulante.
    pub options: Vec<(String, String)>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSession {
    pub user: String,
    pub user_thumb: Option<String>,
    pub title: String,
    pub subtitle: Option<String>,
    pub poster: Option<String>,
    pub progress_ms: u64,
    pub duration_ms: u64,
    pub state: String,
    pub player: String,
    pub local: bool,
    pub transcode: bool,
    pub bandwidth_kbps: Option<u64>,
}

impl PlexServer {
    async fn json(&self, method: Method, path: &str) -> Result<Value> {
        let resp = self.request(method, path).send().await?;
        match resp.status() {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                bail!(crate::i18n::tr("admin.adminOnly"))
            }
            s if !s.is_success() => bail!(crate::i18n::tr_with("admin.serverReplied", &[("status", &s.to_string())])),
            _ => Ok(resp.json().await.unwrap_or_default()),
        }
    }

    async fn send(&self, method: Method, path: &str) -> Result<()> {
        self.json(method, path).await.map(|_| ())
    }

    async fn metadata_value(&self, rating_key: &str) -> Result<(Value, Value)> {
        let c = self.json(Method::GET, &format!("/library/metadata/{rating_key}")).await?;
        let container = c["MediaContainer"].clone();
        let item = container["Metadata"][0].clone();
        if item.is_null() {
            bail!(crate::i18n::tr_with("admin.notFound", &[("id", rating_key)]));
        }
        Ok((container, item))
    }

    pub async fn edit_info(&self, rating_key: &str) -> Result<EditInfo> {
        let (_, m) = self.metadata_value(rating_key).await?;
        let kind = m["type"].as_str().unwrap_or_default().to_owned();
        let locked: Vec<String> = m["Field"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|f| f["locked"].as_bool() == Some(true))
            .filter_map(|f| f["name"].as_str().map(str::to_owned))
            .collect();
        let fields = fields_for(&kind)
            .iter()
            .map(|(name, label, input)| EditField {
                name: (*name).into(),
                label: crate::i18n::tr(&format!("admin.fields.{label}")),
                input: (*input).into(),
                value: text(&m[*name]),
                locked: locked.iter().any(|l| l == name),
            })
            .collect();
        let genres = has_genres(&kind).then(|| {
            m["Genre"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|g| g["tag"].as_str().map(str::to_owned))
                .collect()
        });
        Ok(EditInfo {
            rating_key: rating_key.into(),
            title: text(&m["title"]),
            genres_locked: locked.iter().any(|l| l == "genre"),
            kind,
            fields,
            genres,
        })
    }

    /// Enregistre les champs modifiés (et leur verrou) puis, s'il y a lieu, la liste des genres.
    pub async fn edit(&self, rating_key: &str, changes: Vec<FieldChange>, genres: Option<Vec<String>>) -> Result<()> {
        let (container, m) = self.metadata_value(rating_key).await?;
        let kind = m["type"].as_str().unwrap_or_default();
        let type_id = type_id(kind).ok_or_else(|| anyhow!(crate::i18n::tr("admin.notEditable")))?;
        let section = m["librarySectionID"]
            .as_u64()
            .or_else(|| container["librarySectionID"].as_u64())
            .ok_or_else(|| anyhow!(crate::i18n::tr("admin.libraryNotFound")))?;
        let allowed: Vec<&str> = fields_for(kind).iter().map(|f| f.0).collect();

        let mut query = format!("type={type_id}&id={rating_key}&includeExternalMedia=1");
        for c in changes.iter().filter(|c| allowed.contains(&c.name.as_str())) {
            query.push_str(&format!(
                "&{0}.value={1}&{0}.locked={2}",
                c.name,
                urlencoding::encode(&c.value),
                u8::from(c.locked)
            ));
        }
        if let (Some(new), true) = (genres, has_genres(kind)) {
            let old: Vec<String> = m["Genre"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|g| g["tag"].as_str().map(str::to_owned))
                .collect();
            for (i, g) in new.iter().enumerate() {
                query.push_str(&format!("&genre[{i}].tag.tag={}", urlencoding::encode(g)));
            }
            let removed: Vec<String> = old
                .iter()
                .filter(|g| !new.contains(g))
                .map(|g| urlencoding::encode(g).into_owned())
                .collect();
            if !removed.is_empty() {
                query.push_str(&format!("&genre[].tag.tag-={}", removed.join(",")));
            }
            query.push_str("&genre.locked=1");
        }
        self.send(Method::PUT, &format!("/library/sections/{section}/all?{query}")).await
    }

    /// Affiches (`posters`) ou fonds d'écran (`arts`) proposés pour un élément.
    pub async fn images(&self, rating_key: &str, kind: &str) -> Result<Vec<ImageChoice>> {
        let kind = if kind == "arts" { "arts" } else { "posters" };
        let c = self.json(Method::GET, &format!("/library/metadata/{rating_key}/{kind}")).await?;
        let (w, h) = if kind == "arts" { (480, 270) } else { (240, 360) };
        Ok(c["MediaContainer"]["Metadata"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|i| {
                let key = i["key"].as_str()?.to_owned();
                let thumb = i["thumb"].as_str().unwrap_or(&key);
                let preview = if thumb.starts_with('/') { self.image(thumb, w, h) } else { thumb.to_owned() };
                Some(ImageChoice {
                    selected: i["selected"].as_bool().unwrap_or(false),
                    provider: i["provider"].as_str().map(str::to_owned),
                    preview,
                    key,
                })
            })
            .take(60)
            .collect())
    }

    /// Choisit une image proposée, ou en ajoute une depuis une adresse web.
    pub async fn set_image(&self, rating_key: &str, kind: &str, url: &str) -> Result<()> {
        let (list, one) = if kind == "arts" { ("arts", "art") } else { ("posters", "poster") };
        let url = urlencoding::encode(url);
        if url.starts_with("http") {
            // Une image extérieure est d'abord ajoutée à la liste de l'élément.
            self.send(Method::POST, &format!("/library/metadata/{rating_key}/{list}?url={url}")).await?;
        }
        self.send(Method::PUT, &format!("/library/metadata/{rating_key}/{one}?url={url}")).await
    }

    /// Envoie une image (affiche ou fond d'écran) depuis l'ordinateur, comme « Choisir une image » de Plex :
    /// elle rejoint la liste des images de l'élément et devient celle affichée.
    pub async fn upload_image(&self, rating_key: &str, kind: &str, bytes: Vec<u8>) -> Result<()> {
        let list = if kind == "arts" { "arts" } else { "posters" };
        let resp = self.request(Method::POST, &format!("/library/metadata/{rating_key}/{list}")).body(bytes).send().await?;
        match resp.status() {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => bail!(crate::i18n::tr("admin.adminOnly")),
            s if !s.is_success() => bail!(crate::i18n::tr_with("admin.serverReplied", &[("status", &s.to_string())])),
            _ => Ok(()),
        }
    }

    pub async fn refresh_metadata(&self, rating_key: &str) -> Result<()> {
        self.send(Method::PUT, &format!("/library/metadata/{rating_key}/refresh?force=1")).await
    }

    pub async fn analyze(&self, rating_key: &str) -> Result<()> {
        self.send(Method::PUT, &format!("/library/metadata/{rating_key}/analyze")).await
    }

    /// Réglages visibles du serveur (ceux que Plex Web affiche aussi).
    pub async fn prefs(&self) -> Result<Vec<Pref>> {
        let c = self.json(Method::GET, "/:/prefs").await?;
        Ok(c["MediaContainer"]["Setting"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|s| s["hidden"].as_bool() != Some(true))
            .filter(|s| !s["group"].as_str().unwrap_or_default().is_empty())
            .map(|s| Pref {
                id: text(&s["id"]),
                label: text(&s["label"]),
                summary: text(&s["summary"]),
                kind: text(&s["type"]),
                value: text(&s["value"]),
                default: text(&s["default"]),
                advanced: s["advanced"].as_bool().unwrap_or(false),
                group: text(&s["group"]),
                options: text(&s["enumValues"])
                    .split('|')
                    .filter(|o| !o.is_empty())
                    .map(|o| match o.split_once(':') {
                        Some((v, l)) => (v.to_owned(), l.to_owned()),
                        None => (o.to_owned(), o.to_owned()),
                    })
                    .collect(),
            })
            .collect())
    }

    pub async fn set_prefs(&self, changes: HashMap<String, String>) -> Result<()> {
        if changes.is_empty() {
            return Ok(());
        }
        let query: Vec<String> = changes
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect();
        self.send(Method::PUT, &format!("/:/prefs?{}", query.join("&"))).await
    }

    /// Lectures en cours sur le serveur, tous utilisateurs confondus.
    pub async fn sessions(&self) -> Result<Vec<ServerSession>> {
        let c = self.json(Method::GET, "/status/sessions").await?;
        Ok(c["MediaContainer"]["Metadata"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|m| {
                let kind = m["type"].as_str().unwrap_or_default();
                let (title, subtitle) = match kind {
                    "episode" => (
                        text(&m["grandparentTitle"]),
                        Some(crate::i18n::tr_with(
                            "admin.episodeLine",
                            &[
                                ("season", &m["parentIndex"].as_u64().unwrap_or(0).to_string()),
                                ("episode", &m["index"].as_u64().unwrap_or(0).to_string()),
                                ("title", &text(&m["title"])),
                            ],
                        )),
                    ),
                    "track" => (text(&m["title"]), Some(format!("{} — {}", text(&m["grandparentTitle"]), text(&m["parentTitle"])))),
                    _ => (text(&m["title"]), m["year"].as_u64().map(|y| y.to_string())),
                };
                let poster = m["grandparentThumb"]
                    .as_str()
                    .filter(|_| kind == "episode")
                    .or(m["parentThumb"].as_str().filter(|_| kind == "track"))
                    .or(m["thumb"].as_str())
                    .map(|p| self.image(p, 200, 300));
                let video = m["TranscodeSession"]["videoDecision"].as_str();
                let audio = m["TranscodeSession"]["audioDecision"].as_str();
                ServerSession {
                    user: text(&m["User"]["title"]),
                    user_thumb: m["User"]["thumb"].as_str().map(str::to_owned),
                    title,
                    subtitle,
                    poster,
                    progress_ms: m["viewOffset"].as_u64().unwrap_or(0),
                    duration_ms: m["duration"].as_u64().unwrap_or(0),
                    state: text(&m["Player"]["state"]),
                    player: [text(&m["Player"]["title"]), text(&m["Player"]["product"])]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · "),
                    local: m["Player"]["local"].as_bool().unwrap_or(false),
                    transcode: video == Some("transcode") || audio == Some("transcode"),
                    bandwidth_kbps: m["Session"]["bandwidth"].as_u64(),
                }
            })
            .collect())
    }

    /// Recherche les nouveaux fichiers d'une bibliothèque.
    pub async fn scan_section(&self, key: &str) -> Result<()> {
        self.send(Method::GET, &format!("/library/sections/{key}/refresh")).await
    }

    /// Supprime de la bibliothèque les éléments dont le fichier a disparu.
    pub async fn empty_trash(&self, key: &str) -> Result<()> {
        self.send(Method::PUT, &format!("/library/sections/{key}/emptyTrash")).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editable_types() {
        assert_eq!(type_id("movie"), Some(1));
        assert_eq!(type_id("track"), Some(10));
        assert_eq!(type_id("playlist"), None);
        assert!(fields_for("movie").iter().any(|f| f.0 == "summary"));
        assert!(has_genres("album") && !has_genres("episode"));
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub username: String,
    pub email: Option<String>,
    pub thumb: Option<String>,
    /// Plex Pass actif.
    pub plex_pass: bool,
    /// Jellyfin : le compte administre-t-il le serveur ? (Plex : voir `is_admin`.)
    pub admin: Option<bool>,
}

/// Compte plex.tv connecté (nom, adresse, Plex Pass).
pub async fn account(http: &reqwest::Client, client_id: &str, token: &str) -> Result<Account> {
    let v: Value = crate::plex::with_plex_headers(http.get("https://plex.tv/api/v2/user"), client_id, Some(token))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(Account {
        username: text(&v["username"]),
        email: v["email"].as_str().map(str::to_owned),
        thumb: v["thumb"].as_str().map(str::to_owned),
        plex_pass: v["subscription"]["active"].as_bool().unwrap_or(false),
        admin: None,
    })
}
