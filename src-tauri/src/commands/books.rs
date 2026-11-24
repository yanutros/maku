//! Kavita : livres, BD et mangas (voir kavita.rs). Réglé par serveur, comme Seerr et Tracearr : saisi à la main
//! (adresse et clé d'API d'un compte Kavita), ou fourni par Maku Server, qui donne à chaque utilisateur son propre
//! compte Kavita et laisse l'administrateur choisir qui lit quoi (Administration › Livres).

use crate::kavita::{BookLibrary, BookSeries, ChapterInfo, Kavita, Progress, SeriesDetail, SeriesPage, TocItem};
use crate::*;

impl AppState {
    /// Kavita du serveur en cours, s'il est configuré ; par Maku Server, avec le jeton du serveur.
    async fn kavita(&self) -> anyhow::Result<Kavita> {
        let c = self.cfg();
        let (Some(url), Some(key)) = (c.kavita_url, c.kavita_key) else { return Err(anyhow!(crate::i18n::tr("kavita.notConfigured"))) };
        let k = Kavita::new(self.http.clone(), &url, &key)?;
        if !c.kavita_relay {
            return Ok(k);
        }
        let relay = self.relay_token(&url).await?;
        Ok(k.through(maku_server::relay_headers(relay)))
    }
}

/// Connecte Kavita (adresse et clé d'API), après avoir vérifié que la clé ouvre bien ses bibliothèques ; une
/// adresse vide le déconnecte. Rend l'adresse retenue (nettoyée).
#[tauri::command]
pub async fn kavita_configure(state: State<'_, AppState>, url: String, key: String) -> CmdResult<Option<String>> {
    if url.trim().is_empty() {
        state.update_cfg(|c| {
            c.kavita_url = None;
            c.kavita_key = None;
            c.kavita_relay = false;
        })?;
        return Ok(None);
    }
    let k = Kavita::new(state.http.clone(), &url, &key)?;
    k.libraries().await?;
    let base = k.base().to_owned();
    let key = key.trim().to_owned();
    state.update_cfg(|c| {
        c.kavita_url = Some(base.clone());
        c.kavita_key = Some(key);
        c.kavita_relay = false;
    })?;
    Ok(Some(base))
}

/// Accès à Kavita fourni par Maku Server.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KavitaAccess {
    /// Le propriétaire du serveur : il règle l'accès des autres (Administration › Livres).
    admin: bool,
    /// L'adresse ou la clé vient de changer : les bibliothèques sont à relire.
    changed: bool,
}

/// Kavita par Maku Server : le relais du serveur (celui de Seerr ou de Tracearr, ou trouvé sur le réseau local) donne
/// la clé du compte Kavita de l'utilisateur, si l'administrateur lui a ouvert des bibliothèques. Un Kavita saisi à la
/// main n'est pas touché ; un accès retiré par l'administrateur est oublié.
#[tauri::command]
pub async fn kavita_relay(state: State<'_, AppState>) -> CmdResult<Option<KavitaAccess>> {
    let c = state.cfg();
    let Some(server) = c.server.clone() else { return Ok(None) };
    if c.kavita_url.is_some() && !c.kavita_relay {
        return Ok(None);
    }
    let mut bases: Vec<String> = [c.kavita_url.as_deref(), c.seerr_url.as_deref(), c.tracearr_url.as_deref()]
        .into_iter()
        .flatten()
        .filter_map(maku_server::relay_base)
        .map(str::to_owned)
        .collect();
    if bases.is_empty() {
        let pinned = c.relay_keys.get(&server.key()).cloned();
        if let Some(found) = maku_server::discover(&state.http, &server, pinned.as_deref()).await {
            if let Some(key) = found.pin {
                state.update_cfg(|c| void(c.relay_keys.insert(server.key(), key)))?;
            }
            if found.info.kavita && !found.info.plugin {
                bases.push(found.base);
            }
        }
    }
    bases.dedup();
    for base in bases {
        let url = format!("{base}/kavita");
        // Seulement le relais de ce serveur (Maku Server : le plugin Jellyfin ne relaie pas Kavita).
        let Some(relay) = state.relay_token(&url).await.ok().flatten().filter(|r| !r.is_plugin()) else { continue };
        let Ok(resp) = state.http.get(format!("{base}/maku/kavita")).headers(maku_server::relay_headers(Some(relay))).timeout(Duration::from_secs(20)).send().await else {
            continue;
        };
        let status = resp.status();
        if status == reqwest::StatusCode::FORBIDDEN {
            // Pas (ou plus) de bibliothèque ouverte par l'administrateur.
            if c.kavita_relay {
                state.update_cfg(|c| {
                    c.kavita_url = None;
                    c.kavita_key = None;
                    c.kavita_relay = false;
                })?;
            }
            return Ok(None);
        }
        if !status.is_success() {
            continue;
        }
        let v: Value = resp.json().await.unwrap_or(Value::Null);
        let Some(key) = v["key"].as_str().map(str::to_owned) else { continue };
        let changed = c.kavita_url.as_deref() != Some(url.as_str()) || c.kavita_key.as_deref() != Some(key.as_str());
        state.update_cfg(|c| {
            c.kavita_url = Some(url.clone());
            c.kavita_key = Some(key);
            c.kavita_relay = true;
        })?;
        return Ok(Some(KavitaAccess { admin: v["admin"].as_bool().unwrap_or(false), changed }));
    }
    Ok(None)
}

/// Appel d'administration à Maku Server (propriétaire du serveur).
async fn relay_admin(state: &AppState, method: reqwest::Method, path: &str, body: Option<Value>) -> anyhow::Result<Value> {
    let c = state.cfg();
    let base = c
        .kavita_url
        .as_deref()
        .filter(|_| c.kavita_relay)
        .and_then(maku_server::relay_base)
        .map(str::to_owned)
        .ok_or_else(|| anyhow!(crate::i18n::tr("kavita.notConfigured")))?;
    let relay = state.relay_token(&format!("{base}/kavita")).await?;
    let mut req = state.http.request(method, format!("{base}{path}")).headers(maku_server::relay_headers(relay)).timeout(Duration::from_secs(60));
    if let Some(body) = body {
        req = req.json(&body);
    }
    let resp = req.send().await.map_err(|e| anyhow!(crate::i18n::tr_with("kavita.unreachable", &[("error", &e.without_url().to_string())])))?;
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let reason = v["error"].as_str().map(str::to_owned).unwrap_or_else(|| status.as_u16().to_string());
        anyhow::bail!(crate::i18n::tr_with("kavita.relayFailed", &[("error", &reason)]));
    }
    Ok(v)
}

/// Bibliothèques Kavita et accès de chacun (Administration › Livres).
#[tauri::command]
pub async fn kavita_admin(state: State<'_, AppState>) -> CmdResult<Value> {
    Ok(relay_admin(&state, reqwest::Method::GET, "/maku/kavita/admin", None).await?)
}

/// Bibliothèques Kavita ouvertes à un utilisateur ; la première crée son compte Kavita.
#[tauri::command]
pub async fn kavita_admin_set(state: State<'_, AppState>, id: String, name: String, libraries: Vec<i64>) -> CmdResult<Value> {
    let path = format!("/maku/kavita/admin/users/{}", urlencoding::encode(&id));
    Ok(relay_admin(&state, reqwest::Method::PUT, &path, Some(serde_json::json!({ "name": name, "libraries": libraries }))).await?)
}

#[tauri::command]
pub async fn kavita_libraries(state: State<'_, AppState>) -> CmdResult<Vec<BookLibrary>> {
    Ok(state.kavita().await?.libraries().await?)
}

#[tauri::command]
pub async fn kavita_series(state: State<'_, AppState>, library_id: i64, page: u32, size: u32) -> CmdResult<SeriesPage> {
    Ok(state.kavita().await?.library_series(library_id, page, size).await?)
}

#[tauri::command]
pub async fn kavita_on_deck(state: State<'_, AppState>, library_id: i64) -> CmdResult<Vec<BookSeries>> {
    Ok(state.kavita().await?.on_deck(library_id).await?)
}

#[tauri::command]
pub async fn kavita_series_detail(state: State<'_, AppState>, series_id: i64) -> CmdResult<SeriesDetail> {
    Ok(state.kavita().await?.series_detail(series_id).await?)
}

#[tauri::command]
pub async fn kavita_chapter(state: State<'_, AppState>, chapter_id: i64) -> CmdResult<ChapterInfo> {
    Ok(state.kavita().await?.chapter_info(chapter_id).await?)
}

/// Livre EPUB : une page (section du livre), en HTML préparé par Kavita.
#[tauri::command]
pub async fn kavita_book_page(state: State<'_, AppState>, chapter_id: i64, page: i64) -> CmdResult<String> {
    Ok(state.kavita().await?.book_page(chapter_id, page).await?)
}

#[tauri::command]
pub async fn kavita_book_toc(state: State<'_, AppState>, chapter_id: i64) -> CmdResult<Vec<TocItem>> {
    Ok(state.kavita().await?.book_toc(chapter_id).await?)
}

#[tauri::command]
pub async fn kavita_progress(state: State<'_, AppState>, progress: Progress) -> CmdResult<()> {
    Ok(state.kavita().await?.save_progress(&progress).await?)
}
