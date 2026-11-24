//! Kavita (livres, BD, mangas) : un compte Kavita par utilisateur du serveur multimédia, créé et réglé par le relais.
//!
//! Kavita ne connaît pas les comptes Plex, Jellyfin ou Emby. Le relais garde donc, dans son dossier de données, le
//! compte Kavita de chaque utilisateur à qui l'administrateur a ouvert des bibliothèques (`kavita.json`), et le crée
//! avec la clé d'API de l'administrateur Kavita (`KAVITA_KEY`), qui ne sort jamais du relais. Chacun lit ensuite avec
//! son propre compte : Kavita refuse ce qui ne lui est pas ouvert, et la progression de lecture est la sienne. Le
//! propriétaire du serveur, lui, lit avec le compte administrateur Kavita, celui de la clé.
//!
//! - `GET /maku/kavita` : la clé Kavita de l'utilisateur (jeton du serveur dans `X-Maku-Token`), s'il a accès.
//! - `GET /maku/kavita/admin` : bibliothèques Kavita et accès de chacun (propriétaire du serveur seulement).
//! - `PUT /maku/kavita/admin/users/{id}` : bibliothèques ouvertes à un utilisateur ; la première crée son compte.
//! - `/kavita/api/…` : l'API de Kavita, avec le jeton du serveur ; les images (chargées par la page, sans en-tête) avec
//!   une clé Kavita vue depuis moins d'un jour auprès d'un utilisateur reconnu.

use crate::auth::{self, Verdict};
use crate::{App, Kind, Shared};
use anyhow::{anyhow, bail, Context, Result};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Une clé Kavita vue auprès d'un utilisateur reconnu ouvre les images pendant un jour.
const SEEN_FOR: Duration = Duration::from_secs(24 * 3600);
/// L'identité d'un jeton est gardée 10 minutes, comme sa validité.
const WHO_FOR: Duration = Duration::from_secs(600);
/// Sans limite d'âge (`AgeRating.NotApplicable` de Kavita).
const NO_AGE_LIMIT: i64 = -1;
/// Rôle Kavita qui autorise la connexion (avec « Pleb », qu'il ajoute lui-même).
const LOGIN_ROLE: &str = "Login";

/// Compte Kavita d'un utilisateur du serveur multimédia.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// Nom affiché de l'utilisateur, tel que l'administrateur le voit.
    pub name: String,
    pub kavita_id: i64,
    pub username: String,
    /// Clé d'API créée pour Maku sur ce compte.
    pub key: String,
    /// Bibliothèques Kavita ouvertes (vide : plus d'accès, mais le compte et sa progression restent).
    pub libraries: Vec<i64>,
}

#[derive(Serialize, Deserialize, Default)]
struct Store {
    /// Par utilisateur du serveur multimédia : « plex:123 », « jellyfin:<guid> », « emby:<id> ».
    users: BTreeMap<String, Account>,
}

/// Qui porte un jeton du serveur multimédia.
#[derive(Clone, Debug, PartialEq)]
pub struct Who {
    pub id: String,
    pub name: String,
    /// Propriétaire (Plex) ou administrateur (Jellyfin, Emby) du serveur.
    pub admin: bool,
}

pub struct Kavita {
    pub url: String,
    admin_key: String,
    file: PathBuf,
    store: Mutex<Store>,
    /// Clés Kavita vues auprès d'un utilisateur reconnu, et quand.
    seen: Mutex<HashMap<String, Instant>>,
    who: Mutex<HashMap<String, (Who, Instant)>>,
    /// Une modification des comptes à la fois.
    writing: tokio::sync::Mutex<()>,
}

/// Authentification d'un appel à Kavita.
enum Auth<'a> {
    Key(&'a str),
    Bearer(&'a str),
    Anonymous,
}

impl Kavita {
    /// Ouvre le dossier de données (créé s'il manque) et les comptes déjà créés.
    pub fn open(url: String, admin_key: String, data: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&data).with_context(|| format!("MAKU_DATA : impossible de créer le dossier {}", data.display()))?;
        let file = data.join("kavita.json");
        let store = match std::fs::read(&file) {
            Ok(bytes) => serde_json::from_slice(&bytes).with_context(|| format!("{} : fichier illisible", file.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Store::default(),
            Err(e) => return Err(anyhow!("{} : {e}", file.display())),
        };
        Ok(Kavita {
            url,
            admin_key,
            file,
            store: Mutex::new(store),
            seen: Mutex::default(),
            who: Mutex::default(),
            writing: tokio::sync::Mutex::new(()),
        })
    }

    /// Enregistre les comptes : un fichier écrit à côté puis renommé, lisible par le seul relais.
    fn save(&self) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&*self.store.lock().unwrap())?;
        let tmp = self.file.with_extension("json.tmp");
        std::fs::write(&tmp, bytes).with_context(|| format!("{} : écriture impossible", tmp.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, &self.file).with_context(|| format!("{} : écriture impossible", self.file.display()))?;
        Ok(())
    }

    pub fn account(&self, id: &str) -> Option<Account> {
        self.store.lock().unwrap().users.get(id).cloned()
    }

    fn mark_seen(&self, keys: impl IntoIterator<Item = String>) {
        let mut seen = self.seen.lock().unwrap();
        if seen.len() > 5000 {
            seen.retain(|_, at| at.elapsed() < SEEN_FOR);
        }
        for k in keys {
            seen.insert(k, Instant::now());
        }
    }

    /// Clé vue depuis moins d'un jour auprès d'un utilisateur reconnu.
    pub fn fresh(&self, key: &str) -> bool {
        self.seen.lock().unwrap().get(key).is_some_and(|at| at.elapsed() < SEEN_FOR)
    }

    async fn call(&self, http: &reqwest::Client, method: Method, path: &str, auth: Auth<'_>, body: Option<Value>) -> Result<Value> {
        let mut req = http.request(method, format!("{}{path}", self.url)).header("Accept", "application/json").timeout(Duration::from_secs(30));
        req = match auth {
            Auth::Key(k) => req.header("x-api-key", k),
            Auth::Bearer(t) => req.bearer_auth(t),
            Auth::Anonymous => req,
        };
        if let Some(body) = body {
            req = req.json(&body);
        }
        let resp = req.send().await.map_err(|e| anyhow!("Kavita injoignable : {}", e.without_url()))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            let detail: String = text.chars().take(300).collect();
            bail!("Kavita ({path}) : {} {detail}", status.as_u16());
        }
        Ok(if text.trim().is_empty() { Value::Null } else { serde_json::from_str(&text).unwrap_or(Value::String(text)) })
    }

    /// Toutes les clés d'un compte (celle de Maku, et celle que Kavita met dans les adresses des images d'un livre).
    async fn keys_of(&self, http: &reqwest::Client, key: &str) -> Vec<String> {
        let mut keys = vec![key.to_owned()];
        if let Ok(Value::Array(list)) = self.call(http, Method::GET, "/api/Account/auth-keys", Auth::Key(key), None).await {
            keys.extend(list.iter().filter_map(|k| k["key"].as_str().map(str::to_owned)));
        }
        keys
    }

    pub async fn libraries(&self, http: &reqwest::Client) -> Result<Value> {
        let list = self.call(http, Method::GET, "/api/Library/libraries", Auth::Key(&self.admin_key), None).await?;
        Ok(json!(list
            .as_array()
            .into_iter()
            .flatten()
            .map(|l| json!({ "id": l["id"], "name": l["name"], "type": l["type"] }))
            .collect::<Vec<_>>()))
    }

    /// Crée le compte Kavita d'un utilisateur : invitation (sans e-mail, Kavita rend le lien d'activation), activation
    /// avec un mot de passe tiré au hasard, puis une clé d'API pour Maku.
    async fn create(&self, http: &reqwest::Client, id: &str, name: &str, libraries: &[i64]) -> Result<Account> {
        let email = format!("maku-{}@maku.invalid", id.replace(':', "-"));
        let username = username_for(name, id);
        let invite = json!({
            "email": email,
            "roles": [LOGIN_ROLE],
            "libraries": libraries,
            "ageRestriction": { "ageRating": NO_AGE_LIMIT, "includeUnknowns": true },
        });
        let link = match self.call(http, Method::POST, "/api/Account/invite", Auth::Key(&self.admin_key), Some(invite)).await {
            Ok(v) => v["emailLink"].as_str().map(str::to_owned).ok_or_else(|| anyhow!("Kavita : invitation sans lien d'activation"))?,
            // Une création interrompue : le compte existe, pas encore activé ; on reprend son lien.
            Err(e) if e.to_string().contains("already") => self.pending_link(http, &email).await.context(e)?,
            Err(e) => return Err(e),
        };
        let token = reqwest::Url::parse(&link)
            .ok()
            .and_then(|u| u.query_pairs().find(|(k, _)| k == "token").map(|(_, v)| v.into_owned()))
            .ok_or_else(|| anyhow!("Kavita : lien d'activation sans jeton"))?;
        let password = random_hex(24)?;
        let confirm = json!({ "email": email, "token": token, "password": password, "username": username });
        let user = self.call(http, Method::POST, "/api/Account/confirm-email", Auth::Anonymous, Some(confirm)).await?;
        let kavita_id = user["id"].as_i64().ok_or_else(|| anyhow!("Kavita : compte activé sans numéro"))?;
        let jwt = user["token"].as_str().ok_or_else(|| anyhow!("Kavita : compte activé sans jeton"))?;
        let created = self
            .call(http, Method::POST, "/api/Account/create-auth-key", Auth::Bearer(jwt), Some(json!({ "keyLength": 32, "name": "Maku" })))
            .await?;
        let key = created["key"].as_str().ok_or_else(|| anyhow!("Kavita : clé d'API absente"))?.to_owned();
        Ok(Account { name: name.to_owned(), kavita_id, username, key, libraries: libraries.to_vec() })
    }

    /// Lien d'activation d'un compte invité mais pas encore activé.
    async fn pending_link(&self, http: &reqwest::Client, email: &str) -> Result<String> {
        let users = self.call(http, Method::GET, "/api/Users?includePending=true", Auth::Key(&self.admin_key), None).await?;
        let id = users
            .as_array()
            .into_iter()
            .flatten()
            .find(|u| u["email"].as_str().is_some_and(|e| e.eq_ignore_ascii_case(email)))
            .and_then(|u| u["id"].as_i64())
            .ok_or_else(|| anyhow!("Kavita : un compte {email} existe déjà"))?;
        let link = self.call(http, Method::GET, &format!("/api/Account/invite-url?userId={id}&withBaseUrl=true"), Auth::Key(&self.admin_key), None).await?;
        link.as_str().map(str::to_owned).ok_or_else(|| anyhow!("Kavita : lien d'activation introuvable"))
    }

    async fn set_libraries(&self, http: &reqwest::Client, account: &Account, libraries: &[i64]) -> Result<()> {
        let body = json!({
            "userId": account.kavita_id,
            "username": account.username,
            "roles": [LOGIN_ROLE],
            "libraries": libraries,
            "ageRestriction": { "ageRating": NO_AGE_LIMIT, "includeUnknowns": true },
            "email": null,
            "identityProvider": 0,
        });
        self.call(http, Method::POST, "/api/Account/update", Auth::Key(&self.admin_key), Some(body)).await?;
        Ok(())
    }
}

/// Nom du compte Kavita : celui de l'utilisateur (lettres et chiffres permis par Kavita), puis la fin de son
/// identifiant, pour que deux homonymes ne se gênent pas.
pub fn username_for(name: &str, id: &str) -> String {
    let mut base: String = name
        .chars()
        .map(fold)
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_') { c } else { '-' })
        .collect::<String>()
        .trim_matches(|c| c == '-' || c == '.')
        .chars()
        .take(24)
        .collect();
    if base.is_empty() {
        base = "lecteur".into();
    }
    let raw = id.split_once(':').map_or(id, |(_, r)| r);
    let tail: String = raw.chars().rev().filter(|c| c.is_ascii_alphanumeric()).take(6).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{base}.{tail}")
}

/// Lettre accentuée → sa lettre de base (Kavita n'accepte que l'ASCII dans un nom de compte).
fn fold(c: char) -> char {
    const FROM: &str = "àâäáãåÀÂÄÁÃÅéèêëÉÈÊËîïíìÎÏÍÌôöóòõÔÖÓÒÕûüúùÛÜÚÙçÇñÑÿŸ";
    const TO: &str = "aaaaaaAAAAAAeeeeEEEEiiiiIIIIoooooOOOOOuuuuUUUUcCnNyY";
    FROM.chars().position(|f| f == c).and_then(|i| TO.chars().nth(i)).unwrap_or(c)
}

fn random_hex(bytes: usize) -> Result<String> {
    use ring::rand::SecureRandom;
    let mut buf = vec![0u8; bytes];
    ring::rand::SystemRandom::new().fill(&mut buf).map_err(|_| anyhow!("tirage au hasard impossible"))?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

/// Identifiant d'utilisateur accepté : celui du serveur multimédia du relais, lettres, chiffres et tirets.
pub fn valid_user_id(kind: Kind, id: &str) -> bool {
    let Some(rest) = id.strip_prefix(kind.name()).and_then(|r| r.strip_prefix(':')) else { return false };
    !rest.is_empty() && rest.len() <= 64 && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Qui porte ce jeton : `Ok(None)` s'il n'est pas un utilisateur du serveur, une erreur si le serveur ne répond pas.
/// `account` : avec Plex, le jeton du compte plex.tv de l'utilisateur (`X-Maku-Plex`).
pub async fn who(app: &App, k: &Kavita, token: &str, account: Option<&str>) -> Result<Option<Who>> {
    match auth::check(app, token).await {
        Verdict::Allowed => {}
        Verdict::Denied => return Ok(None),
        Verdict::Unavailable => bail!("media server unreachable"),
    }
    let key = format!("{token}|{}", account.unwrap_or_default());
    if let Some((w, at)) = k.who.lock().unwrap().get(&key) {
        if at.elapsed() < WHO_FOR {
            return Ok(Some(w.clone()));
        }
    }
    let w = match app.cfg.kind {
        Kind::Plex => match plex_who(app, account.unwrap_or(token)).await? {
            Some(w) => w,
            None => return Ok(None),
        },
        Kind::Jellyfin | Kind::Emby => media_who(app, token).await?,
    };
    let mut cache = k.who.lock().unwrap();
    if cache.len() > 2000 {
        cache.clear();
    }
    cache.insert(key, (w.clone(), Instant::now()));
    Ok(Some(w))
}

/// Plex : le compte plex.tv de ce jeton, s'il a accès à ce serveur, et s'il le possède. Le jeton d'un serveur partagé
/// (celui d'un ami, `X-Maku-Token`) n'ouvre pas plex.tv : Maku joint donc celui du compte (`X-Maku-Plex`), qu'il confie
/// déjà au relais pour se connecter à Seerr. Un compte qui n'a pas ce serveur parmi les siens n'est pas reconnu.
async fn plex_who(app: &App, token: &str) -> Result<Option<Who>> {
    let get = |path: &str| {
        app.http
            .get(format!("{}{path}", app.cfg.plextv))
            .header("X-Plex-Token", token)
            .header("X-Plex-Client-Identifier", "maku-server")
            .header("X-Plex-Product", "Maku Server")
            .header("Accept", "application/json")
            .timeout(Duration::from_secs(15))
            .send()
    };
    let (user, resources) = tokio::join!(get("/api/v2/user"), get("/api/v2/resources?includeHttps=1"));
    let user = user.context("plex.tv injoignable")?;
    if matches!(user.status().as_u16(), 401 | 403) {
        return Ok(None);
    }
    let user: Value = user.error_for_status()?.json().await?;
    let id = user["id"].as_i64().ok_or_else(|| anyhow!("plex.tv : compte sans numéro"))?;
    let name = ["title", "friendlyName", "username"].iter().find_map(|f| user[*f].as_str().filter(|s| !s.is_empty())).unwrap_or("").to_owned();
    let server = auth::server_id(app).await;
    let resources: Value = resources.context("plex.tv injoignable")?.error_for_status()?.json().await?;
    let Some(server) = server else { bail!("media server unreachable") };
    let Some(mine) = resources.as_array().into_iter().flatten().find(|r| r["clientIdentifier"].as_str() == Some(server.as_str())) else {
        return Ok(None);
    };
    Ok(Some(Who { id: format!("plex:{id}"), name, admin: mine["owned"].as_bool() == Some(true) }))
}

/// Jellyfin et Emby : l'utilisateur du jeton, et s'il administre le serveur.
async fn media_who(app: &App, token: &str) -> Result<Who> {
    let req = app.http.get(format!("{}/Users/Me", app.cfg.media));
    let req = match app.cfg.kind {
        Kind::Jellyfin => req.header("Authorization", format!("MediaBrowser Token=\"{token}\"")),
        _ => req.header("X-Emby-Token", token),
    };
    let me: Value = req.header("Accept", "application/json").timeout(Duration::from_secs(10)).send().await?.error_for_status()?.json().await?;
    let id = me["Id"].as_str().ok_or_else(|| anyhow!("utilisateur sans identifiant"))?;
    Ok(Who {
        id: format!("{}:{id}", app.cfg.kind.name()),
        name: me["Name"].as_str().unwrap_or("").to_owned(),
        admin: me["Policy"]["IsAdministrator"].as_bool() == Some(true),
    })
}

// --- Routes ---

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

/// L'utilisateur reconnu, avec le Kavita du relais, ou la réponse à rendre.
async fn caller<'a>(app: &'a App, headers: &HeaderMap) -> Result<(Who, &'a Kavita), Box<Response>> {
    let Some(k) = app.kavita.as_ref() else { return Err(Box::new(error(StatusCode::NOT_FOUND, "not found"))) };
    let token = headers.get(crate::proxy::TOKEN_HEADER).and_then(|v| v.to_str().ok()).unwrap_or_default();
    let account = headers.get(crate::proxy::PLEX_HEADER).and_then(|v| v.to_str().ok()).filter(|v| !v.is_empty() && v.len() <= 512);
    match who(app, k, token, account).await {
        Ok(Some(w)) => Ok((w, k)),
        Ok(None) => Err(Box::new(error(StatusCode::UNAUTHORIZED, "not a user of this server"))),
        Err(_) => Err(Box::new(error(StatusCode::BAD_GATEWAY, "media server unreachable"))),
    }
}

/// La clé Kavita de l'utilisateur : celle de l'administrateur Kavita pour le propriétaire du serveur, sinon celle
/// de son compte, s'il a au moins une bibliothèque.
pub async fn session(State(app): State<Shared>, headers: HeaderMap) -> Response {
    let (w, k) = match caller(&app, &headers).await {
        Ok(c) => c,
        Err(r) => return *r,
    };
    let key = if w.admin {
        k.admin_key.clone()
    } else {
        match k.account(&w.id) {
            Some(a) if !a.libraries.is_empty() => a.key,
            _ => return error(StatusCode::FORBIDDEN, "no access"),
        }
    };
    let keys = k.keys_of(&app.http, &key).await;
    k.mark_seen(keys);
    Json(json!({ "key": key, "admin": w.admin })).into_response()
}

/// Bibliothèques Kavita, et celles ouvertes à chaque utilisateur.
pub async fn admin_list(State(app): State<Shared>, headers: HeaderMap) -> Response {
    let (w, k) = match caller(&app, &headers).await {
        Ok(c) => c,
        Err(r) => return *r,
    };
    if !w.admin {
        return error(StatusCode::FORBIDDEN, "owner only");
    }
    let libraries = match k.libraries(&app.http).await {
        Ok(l) => l,
        Err(e) => return error(StatusCode::BAD_GATEWAY, &e.to_string()),
    };
    let users: Vec<Value> = k
        .store
        .lock()
        .unwrap()
        .users
        .iter()
        .map(|(id, a)| json!({ "id": id, "name": a.name, "username": a.username, "libraries": a.libraries }))
        .collect();
    // Jellyfin et Emby : leurs utilisateurs, que le relais lit avec le jeton de l'administrateur (Maku a déjà ceux de Plex).
    let token = headers.get(crate::proxy::TOKEN_HEADER).and_then(|v| v.to_str().ok()).unwrap_or_default();
    let members = match app.cfg.kind {
        Kind::Plex => Value::Null,
        _ => media_members(&app, token, &w.id).await.unwrap_or_else(|_| json!([])),
    };
    Json(json!({ "libraries": libraries, "users": users, "members": members })).into_response()
}

/// Utilisateurs de Jellyfin ou d'Emby, sauf l'administrateur lui-même.
async fn media_members(app: &App, token: &str, me: &str) -> Result<Value> {
    let req = app.http.get(format!("{}/Users", app.cfg.media));
    let req = match app.cfg.kind {
        Kind::Jellyfin => req.header("Authorization", format!("MediaBrowser Token=\"{token}\"")),
        _ => req.header("X-Emby-Token", token),
    };
    let list: Value = req.header("Accept", "application/json").timeout(Duration::from_secs(15)).send().await?.error_for_status()?.json().await?;
    Ok(json!(list
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|u| {
            let id = format!("{}:{}", app.cfg.kind.name(), u["Id"].as_str()?);
            (id != me).then(|| json!({ "id": id, "name": u["Name"] }))
        })
        .collect::<Vec<_>>()))
}

#[derive(Deserialize)]
pub struct Grant {
    name: String,
    libraries: Vec<i64>,
}

/// Ouvre des bibliothèques à un utilisateur (la première fois, son compte Kavita est créé), ou les lui retire.
pub async fn admin_set(State(app): State<Shared>, headers: HeaderMap, Path(id): Path<String>, Json(grant): Json<Grant>) -> Response {
    let (w, k) = match caller(&app, &headers).await {
        Ok(c) => c,
        Err(r) => return *r,
    };
    if !w.admin {
        return error(StatusCode::FORBIDDEN, "owner only");
    }
    if !valid_user_id(app.cfg.kind, &id) || id == w.id {
        return error(StatusCode::BAD_REQUEST, "bad user");
    }
    let name: String = grant.name.trim().chars().take(100).collect();
    let mut libraries = grant.libraries.clone();
    libraries.sort_unstable();
    libraries.dedup();
    let _writing = k.writing.lock().await;
    let result = match k.account(&id) {
        Some(mut a) => {
            if a.libraries != libraries {
                if let Err(e) = k.set_libraries(&app.http, &a, &libraries).await {
                    return error(StatusCode::BAD_GATEWAY, &e.to_string());
                }
            }
            a.libraries = libraries;
            if !name.is_empty() {
                a.name = name.clone();
            }
            Some(a)
        }
        None if libraries.is_empty() => None,
        None => match k.create(&app.http, &id, &name, &libraries).await {
            Ok(a) => Some(a),
            Err(e) => return error(StatusCode::BAD_GATEWAY, &e.to_string()),
        },
    };
    let Some(account) = result else { return Json(json!({ "id": id, "name": name, "libraries": [] })).into_response() };
    k.store.lock().unwrap().users.insert(id.clone(), account.clone());
    if let Err(e) = k.save() {
        return error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string());
    }
    Json(json!({ "id": id, "name": account.name, "username": account.username, "libraries": account.libraries })).into_response()
}

/// Appel de l'API de Kavita par le relais : avec le jeton du serveur (l'API, depuis le moteur de Maku), ou avec une clé
/// vue récemment (les images, que la page charge sans en-tête). Avec le jeton, la clé utilisée devient « vue ».
pub async fn gate(app: &Shared, headers: &HeaderMap, query: Option<&str>) -> Result<(), Box<Response>> {
    let Some(k) = app.kavita.as_ref() else { return Err(Box::new(error(StatusCode::NOT_FOUND, "not found"))) };
    let key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .or_else(|| query.and_then(|q| reqwest::Url::parse(&format!("http://x/?{q}")).ok()).and_then(|u| u.query_pairs().find(|(n, _)| n == "apiKey").map(|(_, v)| v.into_owned())))
        .unwrap_or_default();
    let token = headers.get(crate::proxy::TOKEN_HEADER).and_then(|v| v.to_str().ok()).unwrap_or_default();
    if token.is_empty() {
        return if !key.is_empty() && k.fresh(&key) { Ok(()) } else { Err(Box::new(error(StatusCode::UNAUTHORIZED, "not a user of this server"))) };
    }
    match auth::check(app, token).await {
        Verdict::Allowed => {}
        Verdict::Denied => return Err(Box::new(error(StatusCode::UNAUTHORIZED, "not a user of this server"))),
        Verdict::Unavailable => return Err(Box::new(error(StatusCode::BAD_GATEWAY, "media server unreachable"))),
    }
    // Clé d'un utilisateur reconnu : elle et ses voisines (celle des images d'un livre) ouvrent les images.
    if !key.is_empty() && !k.fresh(&key) {
        let app = app.clone();
        tokio::spawn(async move {
            if let Some(k) = app.kavita.as_ref() {
                let keys = k.keys_of(&app.http, &key).await;
                k.mark_seen(keys);
            }
        });
    }
    Ok(())
}
