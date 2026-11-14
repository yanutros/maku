//! Tracearr : connexion par le compte du serveur et statistiques de visionnage.

use crate::*;

#[tauri::command]
pub async fn tracearr_configure(state: State<'_, AppState>, url: String) -> CmdResult<()> {
    let url = url.trim().trim_end_matches('/').to_string();
    if !url.is_empty() {
        Tracearr::new(&url, None, None)?;
    }
    *state.tracearr.lock().await = None;
    state.update_cfg(|c| {
        c.tracearr_url = (!url.is_empty()).then_some(url);
        c.tracearr_cookies = None;
    })?;
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TracearrLogin {
    handle: String,
    auth_url: String,
}

/// Lance la validation Plex de Tracearr : dans le navigateur (`open`, par défaut), ou pas du tout
/// quand l'interface montre le lien autrement (QR code sur téléviseur, téléphone du relais).
#[tauri::command]
pub async fn tracearr_start_login(app: AppHandle, state: State<'_, AppState>, open: Option<bool>) -> CmdResult<TracearrLogin> {
    let t = state.tracearr().await?;
    let login = t.start_login().await?;
    if open.unwrap_or(true) {
        app.opener().open_url(&login.auth_url, None::<&str>)?;
    }
    Ok(TracearrLogin { handle: login.handle, auth_url: login.auth_url })
}

#[tauri::command]
pub async fn tracearr_poll_login(state: State<'_, AppState>, handle: String) -> CmdResult<LoginState> {
    let t = state.tracearr().await?;
    let result = t.check_login(&handle).await?;
    if matches!(result, LoginState::Done) {
        state.save_tracearr_cookies(&t)?;
    }
    Ok(result)
}

#[tauri::command]
pub async fn tracearr_dashboard(state: State<'_, AppState>, timezone: String) -> CmdResult<Dashboard> {
    let t = state.tracearr().await?;
    let plex = state.plex().ok();
    match t.dashboard(plex.as_ref(), &timezone).await {
        Ok(d) => {
            state.save_tracearr_cookies(&t)?;
            Ok(d)
        }
        Err(e) => {
            // Session expirée : on l'oublie pour reproposer la connexion.
            if e.to_string() == "SESSION_EXPIRED" {
                *state.tracearr.lock().await = None;
                state.update_cfg(|c| c.tracearr_cookies = None)?;
            }
            Err(e.into())
        }
    }
}

#[tauri::command]
pub async fn tracearr_logout(state: State<'_, AppState>) -> CmdResult<()> {
    if let Ok(t) = state.tracearr().await {
        t.logout().await;
    }
    *state.tracearr.lock().await = None;
    state.update_cfg(|c| c.tracearr_cookies = None)?;
    Ok(())
}
