//! Utilisateurs du serveur Plex (administrateur seulement), comme « Gérer l'accès à la bibliothèque » de Plex :
//! comptes partagés, bibliothèques de chacun, autorisations (téléchargements…) et restrictions de contenu,
//! invitation et retrait d'un accès. Tout passe par plex.tv, avec le compte Plex du propriétaire du serveur ;
//! plex.tv refuse ces appels à tout autre compte.

use anyhow::{anyhow, bail, Result};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;

const API: &str = "https://plex.tv/api";

pub struct PlexUsers<'a> {
    pub http: &'a reqwest::Client,
    pub client_id: &'a str,
    /// Jeton du compte Plex (propriétaire du serveur).
    pub token: &'a str,
    /// Identifiant du serveur (`machineIdentifier`).
    pub machine: &'a str,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    /// Identifiant plex.tv de la bibliothèque (pas sa clé locale) : c'est lui qu'on partage.
    pub id: u64,
    pub key: String,
    pub title: String,
    pub kind: String,
    pub shared: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SharedUser {
    /// Partage du serveur avec ce compte (bibliothèques, retrait).
    pub share_id: u64,
    /// Compte plex.tv (autorisations et restrictions).
    pub user_id: u64,
    pub name: String,
    pub username: String,
    pub email: String,
    pub thumb: Option<String>,
    /// Membre de Plex Home.
    pub home: bool,
    /// Invitation pas encore acceptée.
    pub pending: bool,
    pub all_libraries: bool,
    pub libraries: Vec<Library>,
    pub allow_sync: bool,
    pub allow_camera_upload: bool,
    pub allow_channels: bool,
    pub allow_subtitle_admin: bool,
    /// Restrictions de contenu de plex.tv, telles quelles (`contentRating=…|label=…`) : vides = aucune.
    pub filter_movies: String,
    pub filter_television: String,
    pub filter_music: String,
}

/// Autorisations et restrictions d'un compte (toutes envoyées ensemble : plex.tv remet à zéro celles qui manquent).
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    pub allow_sync: bool,
    pub allow_camera_upload: bool,
    pub allow_channels: bool,
    pub allow_subtitle_admin: bool,
    pub filter_movies: String,
    pub filter_television: String,
    pub filter_music: String,
}

fn flag(b: bool) -> &'static str {
    if b {
        "1"
    } else {
        "0"
    }
}

fn libraries(node: roxmltree::Node) -> Vec<Library> {
    node.children()
        .filter(|n| n.has_tag_name("Section"))
        .filter_map(|s| {
            Some(Library {
                id: s.attribute("id")?.parse().ok()?,
                key: s.attribute("key").unwrap_or_default().into(),
                title: s.attribute("title").unwrap_or_default().into(),
                kind: s.attribute("type").unwrap_or_default().into(),
                shared: s.attribute("shared") != Some("0"),
            })
        })
        .collect()
}

impl PlexUsers<'_> {
    fn request(&self, method: Method, url: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .header("X-Plex-Token", self.token)
            .header("X-Plex-Client-Identifier", self.client_id)
            .header("X-Plex-Product", "Maku")
    }

    async fn xml(&self, url: &str) -> Result<String> {
        let resp = self.request(Method::GET, url).send().await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED || resp.status() == reqwest::StatusCode::FORBIDDEN {
            bail!(crate::i18n::tr("users.ownerOnly"));
        }
        Ok(resp.error_for_status()?.text().await?)
    }

    /// Bibliothèques du serveur (pour une invitation).
    pub async fn server_libraries(&self) -> Result<Vec<Library>> {
        let text = self.xml(&format!("{API}/servers/{}", self.machine)).await?;
        let doc = roxmltree::Document::parse(&text)?;
        let server = doc.descendants().find(|n| n.has_tag_name("Server")).ok_or_else(|| anyhow!(crate::i18n::tr("users.ownerOnly")))?;
        Ok(libraries(server).into_iter().map(|l| Library { shared: false, ..l }).collect())
    }

    /// Comptes avec qui le serveur est partagé, avec leurs bibliothèques et leurs droits.
    pub async fn list(&self) -> Result<Vec<SharedUser>> {
        let (shares_url, users_url) = (format!("{API}/servers/{}/shared_servers", self.machine), format!("{API}/users"));
        let (shares, users) = tokio::try_join!(self.xml(&shares_url), self.xml(&users_url))?;
        let users_doc = roxmltree::Document::parse(&users)?;
        let accounts: Vec<roxmltree::Node> = users_doc.descendants().filter(|n| n.has_tag_name("User")).collect();
        let doc = roxmltree::Document::parse(&shares)?;
        let mut list: Vec<SharedUser> = doc
            .descendants()
            .filter(|n| n.has_tag_name("SharedServer"))
            .filter_map(|s| {
                let user_id: u64 = s.attribute("userID")?.parse().ok()?;
                let account = accounts.iter().find(|u| u.attribute("id").and_then(|v| v.parse::<u64>().ok()) == Some(user_id));
                let attr = |name: &str| s.attribute(name).unwrap_or_default().to_owned();
                let on = |name: &str| s.attribute(name) == Some("1");
                let username = attr("username");
                let title = account.and_then(|a| a.attribute("title")).unwrap_or_default();
                Some(SharedUser {
                    share_id: s.attribute("id")?.parse().ok()?,
                    user_id,
                    name: if title.is_empty() { username.clone() } else { title.to_owned() },
                    username,
                    email: attr("email"),
                    thumb: account.and_then(|a| a.attribute("thumb")).filter(|t| !t.is_empty()).map(str::to_owned),
                    home: account.and_then(|a| a.attribute("home")) == Some("1"),
                    pending: s.attribute("acceptedAt").is_none_or(|v| v.is_empty() || v == "0"),
                    all_libraries: on("allLibraries"),
                    libraries: libraries(s),
                    allow_sync: on("allowSync"),
                    allow_camera_upload: on("allowCameraUpload"),
                    allow_channels: on("allowChannels"),
                    allow_subtitle_admin: on("allowSubtitleAdmin"),
                    filter_movies: attr("filterMovies"),
                    filter_television: attr("filterTelevision"),
                    filter_music: attr("filterMusic"),
                })
            })
            .collect();
        list.sort_by_key(|u| u.name.to_lowercase());
        Ok(list)
    }

    /// Bibliothèques partagées avec un compte (identifiants plex.tv).
    pub async fn set_libraries(&self, share_id: u64, library_ids: &[u64]) -> Result<()> {
        if library_ids.is_empty() {
            bail!(crate::i18n::tr("users.needLibrary"));
        }
        let body = json!({ "server_id": self.machine, "shared_server": { "library_section_ids": library_ids } });
        self.request(Method::PUT, &format!("{API}/servers/{}/shared_servers/{share_id}", self.machine))
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Autorisations et restrictions d'un compte.
    pub async fn set_permissions(&self, user_id: u64, p: &Permissions) -> Result<()> {
        let url = format!(
            "{API}/friends/{user_id}?allowSync={}&allowCameraUpload={}&allowChannels={}&allowSubtitleAdmin={}&filterMovies={}&filterTelevision={}&filterMusic={}",
            flag(p.allow_sync),
            flag(p.allow_camera_upload),
            flag(p.allow_channels),
            flag(p.allow_subtitle_admin),
            urlencoding::encode(&p.filter_movies),
            urlencoding::encode(&p.filter_television),
            urlencoding::encode(&p.filter_music),
        );
        self.request(Method::PUT, &url).send().await?.error_for_status()?;
        Ok(())
    }

    /// Retire au compte l'accès à ce serveur (le compte plex.tv et l'amitié restent).
    pub async fn remove(&self, share_id: u64) -> Result<()> {
        self.request(Method::DELETE, &format!("{API}/servers/{}/shared_servers/{share_id}", self.machine))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Invite un compte (adresse e-mail ou nom d'utilisateur Plex) sur certaines bibliothèques.
    pub async fn invite(&self, who: &str, library_ids: &[u64], allow_sync: bool) -> Result<()> {
        let who = who.trim();
        if who.is_empty() {
            bail!(crate::i18n::tr("users.needWho"));
        }
        if library_ids.is_empty() {
            bail!(crate::i18n::tr("users.needLibrary"));
        }
        let body = json!({
            "server_id": self.machine,
            "shared_server": { "library_section_ids": library_ids, "invited_email": who },
            "sharing_settings": { "allowSync": flag(allow_sync), "allowCameraUpload": "0", "allowChannels": "0", "filterMovies": "", "filterTelevision": "", "filterMusic": "" },
        });
        let resp = self.request(Method::POST, &format!("{API}/servers/{}/shared_servers", self.machine)).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            // plex.tv explique le refus (compte inconnu, déjà invité…) dans sa réponse.
            let reason = text.split("<error>").nth(1).and_then(|r| r.split("</error>").next()).unwrap_or("").trim().to_owned();
            bail!(if reason.is_empty() { format!("plex.tv : {status}") } else { reason });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn lit_les_bibliotheques_d_un_partage() {
        let xml = r#"<SharedServer id="1" userID="2"><Section id="10" key="1" title="Films" type="movie" shared="1"/><Section id="11" key="3" title="Musique" type="artist" shared="0"/></SharedServer>"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let libs = super::libraries(doc.root_element());
        assert_eq!(libs.len(), 2);
        assert!(libs[0].shared && !libs[1].shared);
        assert_eq!(libs[1].id, 11);
    }
}
