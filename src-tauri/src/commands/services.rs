//! Services annexes : clé TMDB, et Maku Server trouvé sur le réseau local.

use crate::*;

/// Enregistre l'adresse de Tracearr ; la session précédente est oubliée.
/// Enregistre (après vérification) ou retire la clé TMDB ; les logos déjà cherchés le sont à nouveau.
#[tauri::command]
pub async fn tmdb_configure(state: State<'_, AppState>, key: String) -> CmdResult<()> {
    let key = key.trim().to_string();
    if !key.is_empty() {
        plex::tmdb_check(&state.http, &key)
            .await
            .map_err(|_| anyhow!(crate::i18n::tr("tmdb.invalidKey")))?;
    }
    state.update_cfg(|c| c.tmdb_key = (!key.is_empty()).then_some(key))?;
    state.remote_logos.lock().unwrap().clear();
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelayFound {
    url: String,
    seerr: bool,
    tracearr: bool,
    /// Des adresses manquantes ont été remplies par celles du relais.
    applied: bool,
}

/// Cherche le relais Maku Server sur la machine du serveur. Trouvé, il remplit les adresses de Seerr et
/// de Tracearr qui manquent : Seerr seulement s'il accepte le compte Plex (avec Jellyfin et Emby, il
/// demande un mot de passe : l'adresse est proposée dans les Réglages), Tracearr en attendant sa connexion.
#[tauri::command]
pub async fn relay_discover(state: State<'_, AppState>) -> CmdResult<Option<RelayFound>> {
    let c = state.cfg();
    let Some(server) = c.server.clone() else { return Ok(None) };
    // Plex : ses adresses connues (réseau local d'abord, sans les relais de Plex), en plus de celle enregistrée.
    let mut others = Vec::new();
    if !server.is_jellyfin() {
        if let Some(token) = c.plex_token.as_deref() {
            if let Ok(list) = plex::servers(&state.http, &c.client_id, token).await {
                if let Some(r) = list.into_iter().find(|r| r.client_identifier == server.machine_id) {
                    let mut conns: Vec<_> = r.connections.into_iter().filter(|c| !c.relay && !c.address.is_empty()).collect();
                    conns.sort_by_key(|c| !c.local);
                    others = conns.into_iter().map(|c| c.address).collect();
                }
            }
        }
    }
    let Some((base, info)) = maku_server::discover(&state.http, &server, others).await else { return Ok(None) };
    let mut applied = false;
    if info.seerr && c.seerr_url.is_none() && !server.is_jellyfin() {
        let url = format!("{base}/seerr");
        if let Ok(seerr) = Seerr::connect(&url, c.plex_token.clone(), None, Some(server.token.clone())).await {
            let mut slot = state.seerr.lock().await;
            state.update_cfg(|c| {
                c.seerr_url = Some(url);
                c.seerr_cookies = None;
            })?;
            *slot = Some(Arc::new(seerr));
            applied = true;
        }
    }
    if info.tracearr && c.tracearr_url.is_none() {
        *state.tracearr.lock().await = None;
        state.update_cfg(|c| {
            c.tracearr_url = Some(format!("{base}/tracearr"));
            c.tracearr_cookies = None;
        })?;
        applied = true;
    }
    Ok(Some(RelayFound { url: base, seerr: info.seerr, tracearr: info.tracearr, applied }))
}
