//! Invitation par téléphone : relais local pour configurer Maku (ou se connecter) depuis un téléphone.

use crate::*;

/// Ouvre le relais sur le réseau local ; renvoie l'adresse à mettre dans le QR code. Avec `login`
/// (« Jellyfin » ou « Emby ») : la page de connexion à ce serveur, avec l'adresse `url` déjà saisie.
#[tauri::command]
pub async fn relay_start(app: AppHandle, state: State<'_, AppState>, login: Option<String>, url: Option<String>) -> CmdResult<String> {
    let login = login.filter(|l| matches!(l.as_str(), "Jellyfin" | "Emby"));
    let url = url.unwrap_or_default();
    let relay = relay::start(app, login.as_deref().map(|l| (l, url.as_str()))).await?;
    let url = relay.url.clone();
    *state.relay.lock().unwrap() = Some(relay);
    Ok(url)
}

/// Avancement de la configuration, affiché par la page du téléphone (JSON) ; `tmdb` : celle de la clé TMDB.
#[tauri::command]
pub fn relay_status(state: State<'_, AppState>, status: String, tmdb: Option<bool>) {
    if let Some(relay) = state.relay.lock().unwrap().as_ref() {
        relay.set_status(status, tmdb.unwrap_or(false));
    }
}

/// Journal de diagnostic d'Android, servi par le relais à `…/journal`.
#[tauri::command]
pub fn relay_journal(state: State<'_, AppState>, text: String) {
    if let Some(relay) = state.relay.lock().unwrap().as_ref() {
        relay.set_journal(text);
    }
}

#[tauri::command]
pub fn relay_stop(state: State<'_, AppState>) {
    state.relay.lock().unwrap().take();
}
