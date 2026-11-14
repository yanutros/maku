//! Utilisateurs du serveur Plex (administrateur) : partages, bibliothèques, droits, invitations.

use crate::*;

/// Compte Plex et serveur en cours, pour les appels de plex_users.rs.
pub(crate) fn plex_owner(state: &AppState) -> anyhow::Result<(String, String, String)> {
    let c = state.cfg();
    let server = c.server.filter(|s| !s.is_jellyfin()).ok_or_else(|| anyhow!(crate::i18n::tr("users.plexOnly")))?;
    let token = c.plex_token.ok_or_else(|| anyhow!(crate::i18n::tr("users.ownerOnly")))?;
    Ok((c.client_id, token, server.machine_id))
}

macro_rules! plex_users {
    ($state:expr, $u:ident => $body:expr) => {{
        let (client_id, token, machine) = plex_owner(&$state)?;
        let $u = plex_users::PlexUsers { http: &$state.http, client_id: &client_id, token: &token, machine: &machine };
        Ok($body.await?)
    }};
}

#[tauri::command]
pub async fn plex_users(state: State<'_, AppState>) -> CmdResult<Vec<plex_users::SharedUser>> {
    plex_users!(state, u => u.list())
}

#[tauri::command]
pub async fn plex_server_libraries(state: State<'_, AppState>) -> CmdResult<Vec<plex_users::Library>> {
    plex_users!(state, u => u.server_libraries())
}

#[tauri::command]
pub async fn plex_user_libraries(state: State<'_, AppState>, share_id: u64, library_ids: Vec<u64>) -> CmdResult<()> {
    plex_users!(state, u => u.set_libraries(share_id, &library_ids))
}

#[tauri::command]
pub async fn plex_user_permissions(state: State<'_, AppState>, user_id: u64, permissions: plex_users::Permissions) -> CmdResult<()> {
    plex_users!(state, u => u.set_permissions(user_id, &permissions))
}

#[tauri::command]
pub async fn plex_user_remove(state: State<'_, AppState>, share_id: u64) -> CmdResult<()> {
    plex_users!(state, u => u.remove(share_id))
}

#[tauri::command]
pub async fn plex_user_invite(state: State<'_, AppState>, who: String, library_ids: Vec<u64>, allow_sync: bool) -> CmdResult<()> {
    plex_users!(state, u => u.invite(&who, &library_ids, allow_sync))
}
