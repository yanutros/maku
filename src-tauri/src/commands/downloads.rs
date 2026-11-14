//! Téléchargements pour la lecture hors ligne (Android) et progression renvoyée au serveur plus tard.

use crate::*;

/// Téléchargements de l'appareil, avec le dossier où sont leurs fichiers.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OfflineList {
    dir: String,
    items: Vec<offline::Download>,
}

/// Le compte en cours a-t-il le droit de télécharger sur ce serveur (réglé par son administrateur) ?
#[tauri::command]
pub async fn offline_allowed(state: State<'_, AppState>) -> CmdResult<bool> {
    Ok(state.server()?.can_download().await?)
}

#[tauri::command]
pub fn offline_list(state: State<'_, AppState>) -> OfflineList {
    OfflineList { dir: state.offline.dir().to_string_lossy().into_owned(), items: state.offline.list() }
}

/// Relance la file des téléchargements du serveur en cours.
pub(crate) fn offline_kick(app: &AppHandle, state: &AppState) {
    let (Ok(server), Some(key)) = (state.server(), state.cfg().server.map(|s| s.key())) else { return };
    state.offline.kick(app.clone(), state.download_http.clone(), server, key);
}

#[tauri::command]
pub async fn offline_add(app: AppHandle, state: State<'_, AppState>, requests: Vec<offline::Request>) -> CmdResult<Vec<offline::Download>> {
    // Pas de téléchargement sans le droit donné par l'administrateur du serveur.
    if !state.server()?.can_download().await? {
        return Err(anyhow!(crate::i18n::tr("offline.notAllowed")).into());
    }
    let key = state.cfg().server.map(|s| s.key()).ok_or_else(|| anyhow!(crate::i18n::tr("app.noServer")))?;
    let added = requests.into_iter().map(|r| state.offline.add(&key, r)).collect();
    offline_kick(&app, &state);
    Ok(added)
}

#[tauri::command]
pub fn offline_pause(state: State<'_, AppState>, id: String) {
    state.offline.pause(&id);
}

#[tauri::command]
pub fn offline_resume(app: AppHandle, state: State<'_, AppState>, id: String) {
    state.offline.resume(&id);
    offline_kick(&app, &state);
}

#[tauri::command]
pub fn offline_remove(state: State<'_, AppState>, id: String) {
    state.offline.remove(&id);
}

/// Progression d'une lecture hors ligne, renvoyée au serveur plus tard (`offline_sync`).
#[tauri::command]
pub fn offline_progress(state: State<'_, AppState>, id: String, position_ms: u64, watched: bool) {
    state.offline.set_progress(&id, position_ms, watched);
}

/// Serveur joignable : la progression jouée hors ligne lui est renvoyée, et les téléchargements en attente
/// reprennent. Renvoie le nombre de progressions envoyées.
#[tauri::command]
pub async fn offline_sync(app: AppHandle, state: State<'_, AppState>) -> CmdResult<usize> {
    let (Ok(server), Some(key)) = (state.server(), state.cfg().server.map(|s| s.key())) else { return Ok(0) };
    let sent = state.offline.sync(&server, &key).await;
    offline_kick(&app, &state);
    Ok(sent)
}
