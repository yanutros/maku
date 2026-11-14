//! Actions sur un titre : note, vu / pas vu, « Continuer à regarder », liste de lecture Plex, collections et playlists.

use crate::*;

#[tauri::command]
pub async fn plex_rate(state: State<'_, AppState>, rating_key: String, rating: Option<f32>) -> CmdResult<()> {
    Ok(state.server()?.rate(&rating_key, rating).await?)
}

#[tauri::command]
pub async fn plex_remove_from_continue(state: State<'_, AppState>, rating_key: String) -> CmdResult<()> {
    Ok(state.server()?.remove_from_continue(&rating_key).await?)
}

/// Liste de visionnage du compte Plex : état d'un titre (identifiant plex://).
#[tauri::command]
pub async fn plex_watchlisted(state: State<'_, AppState>, guid: String) -> CmdResult<bool> {
    let c = state.cfg();
    let Some(token) = c.plex_token else { return Ok(false) };
    Ok(plex::watchlisted(&state.http, &c.client_id, &token, &guid).await?)
}

#[tauri::command]
pub async fn plex_set_watchlist(state: State<'_, AppState>, guid: String, on: bool) -> CmdResult<()> {
    let c = state.cfg();
    let token = c.plex_token.ok_or_else(|| anyhow!(crate::i18n::tr("app.notLoggedIn")))?;
    Ok(plex::set_watchlist(&state.http, &c.client_id, &token, &guid, on).await?)
}

#[tauri::command]
pub async fn plex_set_watched(state: State<'_, AppState>, rating_key: String, watched: bool) -> CmdResult<()> {
    Ok(state.server()?.set_watched(&rating_key, watched).await?)
}

/// Collections où ranger un titre : celles de sa bibliothèque.
#[tauri::command]
pub async fn collection_choices(state: State<'_, AppState>, rating_key: String) -> CmdResult<Vec<Item>> {
    Ok(state.server()?.collection_choices(&rating_key).await?)
}

#[tauri::command]
pub async fn add_to_collection(state: State<'_, AppState>, rating_key: String, collection: String) -> CmdResult<()> {
    let server = state.server()?;
    if matches!(server, media::MediaServer::Plex(_)) {
        state.admin_plex().await?;
    }
    Ok(server.add_to_collection(&rating_key, &collection).await?)
}

#[tauri::command]
pub async fn create_collection(state: State<'_, AppState>, rating_key: String, title: String) -> CmdResult<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err(anyhow!(crate::i18n::tr("collection.noTitle")).into());
    }
    let server = state.server()?;
    if matches!(server, media::MediaServer::Plex(_)) {
        state.admin_plex().await?;
    }
    Ok(server.create_collection(&rating_key, title).await?)
}

/// Playlists de musique du compte, pour « Ajouter à une playlist » (chacun range dans les siennes).
#[tauri::command]
pub async fn playlist_choices(state: State<'_, AppState>) -> CmdResult<Vec<Item>> {
    Ok(state.server()?.playlist_choices().await?)
}

#[tauri::command]
pub async fn add_to_playlist(state: State<'_, AppState>, rating_key: String, playlist: String) -> CmdResult<()> {
    Ok(state.server()?.add_to_playlist(&rating_key, &playlist).await?)
}

#[tauri::command]
pub async fn create_playlist(state: State<'_, AppState>, rating_key: String, title: String) -> CmdResult<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err(anyhow!(crate::i18n::tr("playlist.noTitle")).into());
    }
    Ok(state.server()?.create_playlist(&rating_key, title).await?)
}
