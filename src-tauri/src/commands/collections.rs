//! Collections (administrateur) : collections intelligentes, ordre, conversion, ajout en masse.

use crate::*;

/// Nouvelle collection intelligente (administrateur) : les titres de la bibliothèque qui répondent aux filtres.
#[tauri::command]
pub async fn plex_create_smart_collection(
    state: State<'_, AppState>,
    key: String,
    kind: String,
    title: String,
    filters: Vec<(String, String)>,
    sort: Option<String>,
) -> CmdResult<String> {
    Ok(state.admin_plex().await?.create_smart_collection(&key, &kind, &title, &filters, sort.as_deref()).await?)
}

/// Nouveaux filtres d'une collection intelligente (administrateur).
#[tauri::command]
pub async fn plex_update_smart_collection(
    state: State<'_, AppState>,
    rating_key: String,
    key: String,
    kind: String,
    filters: Vec<(String, String)>,
    sort: Option<String>,
) -> CmdResult<()> {
    Ok(state.admin_plex().await?.update_smart_collection(&rating_key, &key, &kind, &filters, sort.as_deref()).await?)
}

/// Plusieurs titres d'un coup dans une collection (administrateur).
#[tauri::command]
pub async fn plex_add_many_to_collection(state: State<'_, AppState>, keys: Vec<String>, collection: String) -> CmdResult<()> {
    Ok(state.admin_plex().await?.add_many_to_collection(&keys, &collection).await?)
}

/// Nouvelle collection ordinaire avec ces titres (administrateur) ; rend son identifiant.
#[tauri::command]
pub async fn plex_create_collection_with(state: State<'_, AppState>, key: String, kind: String, title: String, keys: Vec<String>) -> CmdResult<String> {
    Ok(state.admin_plex().await?.create_collection_with(&key, &kind, &title, &keys).await?)
}

/// Ordre d'une collection ordinaire (administrateur) : 0 date de sortie, 1 alphabétique, 2 personnalisé.
#[tauri::command]
pub async fn plex_set_collection_sort(state: State<'_, AppState>, rating_key: String, sort: u32) -> CmdResult<()> {
    Ok(state.admin_plex().await?.set_collection_sort(&rating_key, sort).await?)
}

/// Ordre personnalisé d'une collection (administrateur) : `item` juste après `after`, en tête sans.
#[tauri::command]
pub async fn plex_move_in_collection(state: State<'_, AppState>, rating_key: String, item: String, after: Option<String>) -> CmdResult<()> {
    Ok(state.admin_plex().await?.move_in_collection(&rating_key, &item, after.as_deref()).await?)
}

/// Collection intelligente changée en collection ordinaire (administrateur) ; rend son nouvel identifiant.
#[tauri::command]
pub async fn plex_convert_collection(state: State<'_, AppState>, rating_key: String) -> CmdResult<String> {
    Ok(state.admin_plex().await?.convert_collection(&rating_key).await?)
}

/// Filtres d'une collection intelligente et sa bibliothèque.
#[tauri::command]
pub async fn plex_smart_collection(state: State<'_, AppState>, rating_key: String) -> CmdResult<plex::SmartCollection> {
    Ok(state.server()?.smart_collection(&rating_key).await?)
}
