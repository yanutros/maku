//! Seerr : connexion, découverte, fiches et demandes de films et de séries.

use crate::*;

/// Enregistre l'adresse de Seerr et s'y connecte avec le compte Plex (adresse vide = désactivé).
#[tauri::command]
pub async fn seerr_configure(state: State<'_, AppState>, url: String) -> CmdResult<()> {
    let url = url.trim().trim_end_matches('/').to_string();
    let mut slot = state.seerr.lock().await;
    if url.is_empty() {
        *slot = None;
        state.update_cfg(|c| {
            c.seerr_url = None;
            c.seerr_cookies = None;
        })?;
        return Ok(());
    }
    let relay = state.relay_token(&url).await;
    let seerr = Seerr::connect(&url, state.cfg().plex_token, None, relay).await?;
    state.update_cfg(|c| {
        c.seerr_url = Some(url);
        c.seerr_cookies = None;
    })?;
    *slot = Some(Arc::new(seerr));
    Ok(())
}

/// Enregistre l'adresse de Seerr et s'y connecte avec un compte Jellyfin : seule la session est gardée.
#[tauri::command]
pub async fn seerr_configure_jellyfin(state: State<'_, AppState>, url: String, username: String, password: String) -> CmdResult<()> {
    let url = url.trim().trim_end_matches('/').to_string();
    let mut slot = state.seerr.lock().await;
    let relay = state.relay_token(&url).await;
    let seerr = Seerr::connect_jellyfin(&url, username.trim(), &password, relay).await?;
    let cookies = seerr.saved_cookies();
    state.update_cfg(|c| {
        c.seerr_url = Some(url);
        c.seerr_cookies = cookies;
    })?;
    *slot = Some(Arc::new(seerr));
    Ok(())
}

#[tauri::command]
pub async fn seerr_search(state: State<'_, AppState>, query: String, page: u32) -> CmdResult<Vec<SeerrMedia>> {
    Ok(state.seerr().await?.search(&query, page).await?)
}

#[tauri::command]
pub async fn seerr_trending(state: State<'_, AppState>, page: u32) -> CmdResult<Vec<SeerrMedia>> {
    Ok(state.seerr().await?.trending(page).await?)
}

#[tauri::command]
pub async fn seerr_details(state: State<'_, AppState>, media_type: String, tmdb_id: u64) -> CmdResult<SeerrDetail> {
    Ok(state.seerr().await?.details(&media_type, tmdb_id).await?)
}

#[tauri::command]
pub async fn seerr_related(
    state: State<'_, AppState>,
    media_type: String,
    tmdb_id: u64,
    relation: String,
) -> CmdResult<Vec<SeerrMedia>> {
    Ok(state.seerr().await?.related(&media_type, tmdb_id, &relation).await?)
}

#[tauri::command]
pub async fn seerr_person_credits(state: State<'_, AppState>, person_id: u64) -> CmdResult<Vec<SeerrMedia>> {
    Ok(state.seerr().await?.person_credits(person_id).await?)
}

#[tauri::command]
pub async fn seerr_slider(state: State<'_, AppState>, slider: Slider, page: u32) -> CmdResult<Vec<SeerrMedia>> {
    Ok(state.seerr().await?.slider(slider, page).await?)
}

#[tauri::command]
pub async fn seerr_recent_requests(state: State<'_, AppState>) -> CmdResult<Vec<SeerrMedia>> {
    Ok(state.seerr().await?.recent_requests().await?)
}

#[tauri::command]
pub async fn seerr_watchlist(state: State<'_, AppState>) -> CmdResult<Vec<SeerrMedia>> {
    Ok(state.seerr().await?.watchlist().await?)
}

#[tauri::command]
pub async fn seerr_tv_seasons(state: State<'_, AppState>, tmdb_id: u64) -> CmdResult<Vec<SeerrSeason>> {
    Ok(state.seerr().await?.tv_seasons(tmdb_id).await?)
}

#[tauri::command]
pub async fn seerr_request(
    state: State<'_, AppState>,
    media_type: String,
    tmdb_id: u64,
    seasons: Option<Vec<u32>>,
) -> CmdResult<()> {
    Ok(state.seerr().await?.create_request(&media_type, tmdb_id, seasons).await?)
}
