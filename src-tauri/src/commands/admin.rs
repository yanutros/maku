//! Administration du serveur Plex : modification des métadonnées et des images, réglages, tableau de bord.

use crate::*;

#[tauri::command]
pub async fn plex_is_admin(state: State<'_, AppState>) -> CmdResult<bool> {
    Ok(state.owns_plex_server().await)
}

#[tauri::command]
pub async fn plex_account(state: State<'_, AppState>) -> CmdResult<Account> {
    if let Ok(media::MediaServer::Jellyfin(j)) = state.server() {
        return Ok(j.account().await?);
    }
    let c = state.cfg();
    let token = c.plex_token.ok_or_else(|| anyhow!(crate::i18n::tr("app.notLoggedIn")))?;
    Ok(plex_admin::account(&state.http, &c.client_id, &token).await?)
}

#[tauri::command]
pub async fn plex_edit_info(state: State<'_, AppState>, rating_key: String) -> CmdResult<EditInfo> {
    Ok(state.admin_plex().await?.edit_info(&rating_key).await?)
}

#[tauri::command]
pub async fn plex_edit(
    state: State<'_, AppState>,
    rating_key: String,
    changes: Vec<FieldChange>,
    genres: Option<Vec<String>>,
) -> CmdResult<()> {
    Ok(state.admin_plex().await?.edit(&rating_key, changes, genres).await?)
}

#[tauri::command]
pub async fn plex_images(state: State<'_, AppState>, rating_key: String, kind: String) -> CmdResult<Vec<ImageChoice>> {
    Ok(state.admin_plex().await?.images(&rating_key, &kind).await?)
}

/// Image choisie sur l'ordinateur (`data` : son contenu en base64) ou déposée sur la fenêtre (`path`),
/// envoyée comme affiche ou fond d'écran (administrateur). JPEG, PNG ou WebP, 25 Mo au plus.
#[tauri::command]
pub async fn plex_upload_image(state: State<'_, AppState>, rating_key: String, kind: String, data: Option<String>, path: Option<String>) -> CmdResult<()> {
    use base64::Engine;
    let bytes = match (data, path) {
        (Some(b64), _) => base64::engine::general_purpose::STANDARD
            .decode(b64.as_bytes())
            .map_err(|_| anyhow::anyhow!(i18n::tr("admin.badImage")))?,
        (None, Some(p)) => {
            let meta = std::fs::metadata(&p).map_err(|_| anyhow::anyhow!(i18n::tr("admin.badImage")))?;
            if meta.len() > 25 * 1024 * 1024 {
                return Err(anyhow::anyhow!(i18n::tr("admin.badImage")).into());
            }
            std::fs::read(&p).map_err(|_| anyhow::anyhow!(i18n::tr("admin.badImage")))?
        }
        _ => return Err(anyhow::anyhow!(i18n::tr("admin.badImage")).into()),
    };
    // Une vraie image, d'après ses premiers octets (et pas n'importe quel fichier déposé).
    let image = bytes.starts_with(&[0xFF, 0xD8, 0xFF])
        || bytes.starts_with(b"\x89PNG")
        || (bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP");
    if !image || bytes.len() > 25 * 1024 * 1024 {
        return Err(anyhow::anyhow!(i18n::tr("admin.badImage")).into());
    }
    Ok(state.admin_plex().await?.upload_image(&rating_key, &kind, bytes).await?)
}

#[tauri::command]
pub async fn plex_set_image(state: State<'_, AppState>, rating_key: String, kind: String, url: String) -> CmdResult<()> {
    Ok(state.admin_plex().await?.set_image(&rating_key, &kind, &url).await?)
}

#[tauri::command]
pub async fn plex_refresh_metadata(state: State<'_, AppState>, rating_key: String) -> CmdResult<()> {
    Ok(state.admin_plex().await?.refresh_metadata(&rating_key).await?)
}

#[tauri::command]
pub async fn plex_analyze(state: State<'_, AppState>, rating_key: String) -> CmdResult<()> {
    Ok(state.admin_plex().await?.analyze(&rating_key).await?)
}

#[tauri::command]
pub async fn plex_prefs(state: State<'_, AppState>) -> CmdResult<Vec<Pref>> {
    Ok(state.admin_plex().await?.prefs().await?)
}

#[tauri::command]
pub async fn plex_set_prefs(state: State<'_, AppState>, changes: HashMap<String, String>) -> CmdResult<()> {
    Ok(state.admin_plex().await?.set_prefs(changes).await?)
}

#[tauri::command]
pub async fn plex_sessions(state: State<'_, AppState>) -> CmdResult<Vec<ServerSession>> {
    Ok(state.admin_plex().await?.sessions().await?)
}

#[tauri::command]
pub async fn plex_scan_section(state: State<'_, AppState>, key: String) -> CmdResult<()> {
    Ok(state.admin_plex().await?.scan_section(&key).await?)
}

#[tauri::command]
pub async fn plex_empty_trash(state: State<'_, AppState>, key: String) -> CmdResult<()> {
    Ok(state.admin_plex().await?.empty_trash(&key).await?)
}

/// Débit du serveur (administrateur) : `timespan` 6 secondes, 4 heures, 3 jours, 2 semaines ; depuis `since`.
#[tauri::command]
pub async fn plex_bandwidth(state: State<'_, AppState>, timespan: u32, since: i64) -> CmdResult<plex_admin::Bandwidth> {
    Ok(state.admin_plex().await?.bandwidth(timespan, since).await?)
}

/// Lectures par compte et par type (administrateur).
#[tauri::command]
pub async fn plex_media_stats(state: State<'_, AppState>, timespan: u32, since: i64) -> CmdResult<plex_admin::MediaStats> {
    Ok(state.admin_plex().await?.media_stats(timespan, since).await?)
}

/// Historique des lectures de tous les comptes (administrateur).
#[tauri::command]
pub async fn plex_history(state: State<'_, AppState>, since: i64) -> CmdResult<plex_admin::History> {
    Ok(state.admin_plex().await?.history(since).await?)
}

/// Arrête une lecture en cours avec un message (administrateur, Plex Pass).
#[tauri::command]
pub async fn plex_stop_session(state: State<'_, AppState>, session_id: String, reason: String) -> CmdResult<()> {
    Ok(state.admin_plex().await?.stop_session(&session_id, &reason).await?)
}
