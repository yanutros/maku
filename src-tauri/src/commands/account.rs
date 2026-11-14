//! Comptes et serveurs : connexion Plex (code PIN), Jellyfin et Emby, profils Plex Home, choix du serveur.

use crate::*;

#[tauri::command]
pub async fn plex_start_login(app: AppHandle, state: State<'_, AppState>, link: Option<bool>) -> CmdResult<LoginPin> {
    let link = link.unwrap_or(false);
    let pin = plex::create_pin(&state.http, &state.cfg().client_id, link).await?;
    if !link {
        app.opener().open_url(&pin.auth_url, None::<&str>)?;
    }
    Ok(pin)
}

#[tauri::command]
pub async fn plex_poll_login(state: State<'_, AppState>, pin_id: u64) -> CmdResult<bool> {
    let Some(token) = plex::check_pin(&state.http, &state.cfg().client_id, pin_id).await? else {
        return Ok(false);
    };
    state.update_cfg(|c| {
        c.plex_token = Some(token);
        // Un serveur Jellyfin en cours reste gardé pour y revenir.
        if let Some(s) = c.server.take().filter(|s| s.is_jellyfin()) {
            remember_saved(c, s);
        }
        remember_server_kinds(c);
    })?;
    state.choosing.store(false, std::sync::atomic::Ordering::Relaxed);
    *state.seerr.lock().await = None;
    Ok(true)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HomeUsers {
    users: Vec<plex::HomeUser>,
    /// Profil connecté.
    current: Option<String>,
}

/// Profils du Plex Home (vide hors Plex Home), pour l'écran « Qui regarde ? ».
#[tauri::command]
pub async fn plex_home_users(state: State<'_, AppState>) -> CmdResult<HomeUsers> {
    let c = state.cfg();
    let token = c.plex_token.ok_or_else(|| anyhow!(crate::i18n::tr("app.notLoggedIn")))?;
    let (users, current) = plex::home_users(&state.http, &c.client_id, &token).await?;
    Ok(HomeUsers { users, current })
}

/// Passe sur un autre profil du Plex Home. Le serveur en cours est gardé si ce profil y a accès
/// (avec son propre jeton : le serveur applique ses restrictions), sinon on repasse par le choix du serveur.
#[tauri::command]
pub async fn plex_switch_user(state: State<'_, AppState>, uuid: String, pin: Option<String>) -> CmdResult<()> {
    let c = state.cfg();
    let token = c.plex_token.clone().ok_or_else(|| anyhow!(crate::i18n::tr("app.notLoggedIn")))?;
    let user_token = plex::switch_home_user(&state.http, &c.client_id, &token, &uuid, pin.as_deref()).await?;
    let saved = match c.server.as_ref().filter(|s| !s.is_jellyfin()) {
        Some(current) => {
            let res = plex::servers(&state.http, &c.client_id, &user_token)
                .await?
                .into_iter()
                .find(|r| r.client_identifier == current.machine_id);
            match res {
                Some(res) => {
                    let uri = plex::pick_connection(&state.http, &c.client_id, &res).await?;
                    Some(SavedServer {
                        name: res.name,
                        machine_id: res.client_identifier,
                        uri,
                        token: res.access_token.unwrap_or_else(|| user_token.clone()),
                        kind: String::new(),
                        user_id: None,
                    })
                }
                None => None,
            }
        }
        None => c.server.clone(),
    };
    stop_player(&state);
    state.update_cfg(|c| {
        c.plex_token = Some(user_token);
        c.server = saved;
    })?;
    // Seerr s'ouvre avec le compte Plex : il faut s'y reconnecter avec le nouveau profil.
    *state.seerr.lock().await = None;
    Ok(())
}

#[tauri::command]
pub fn set_auto_sign_in(state: State<'_, AppState>, on: bool) -> CmdResult<()> {
    state.update_cfg(|c| c.auto_sign_in = on)?;
    Ok(())
}

/// Enregistre le serveur Jellyfin (ou Emby) auquel on vient de se connecter : il devient le serveur de Maku.
pub(crate) fn save_jellyfin(state: &AppState, base: String, login: jellyfin::Login, emby: bool) -> anyhow::Result<String> {
    let name = login.server_name.clone();
    let saved = SavedServer {
        name: login.server_name,
        machine_id: login.server_id,
        uri: base,
        token: login.token,
        kind: if emby { "emby" } else { "jellyfin" }.into(),
        user_id: Some(login.user_id),
    };
    state.update_cfg(|c| {
        remember_saved(c, saved.clone());
        c.server = Some(saved);
        remember_server_kinds(c);
    })?;
    state.choosing.store(false, std::sync::atomic::Ordering::Relaxed);
    Ok(name)
}

/// Écran de choix : Plex avec le compte déjà connecté (on passe au choix du serveur Plex).
#[tauri::command]
pub fn resume_plex(state: State<'_, AppState>) -> CmdResult<()> {
    if state.cfg().plex_token.is_none() {
        return Err(anyhow!(crate::i18n::tr("app.notLoggedIn")).into());
    }
    state.choosing.store(false, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

/// Écran de choix : retour au serveur Jellyfin (ou Emby, `emby`) gardé, après vérification que la
/// session vit encore. Sans précision : le dernier des deux utilisé.
#[tauri::command]
pub async fn resume_jellyfin(state: State<'_, AppState>, emby: Option<bool>) -> CmdResult<()> {
    let c = state.cfg();
    let emby = emby.unwrap_or_else(|| c.emby_saved_at.unwrap_or(0) > c.jellyfin_saved_at.unwrap_or(0));
    let saved = if emby { remembered_emby(&c) } else { remembered_jellyfin(&c) };
    let saved = saved.ok_or_else(|| anyhow!(crate::i18n::tr("jellyfin.sessionExpired")))?;
    let server = jellyfin::JellyfinServer {
        http: state.http.clone(),
        base: saved.uri.clone(),
        token: saved.token.clone(),
        user_id: saved.user_id.clone().unwrap_or_default(),
        client_id: c.client_id,
    };
    if let Err(e) = server.account().await {
        // Jeton refusé (session fermée dans Jellyfin) : il faut se reconnecter.
        let refused = e.downcast_ref::<reqwest::Error>().and_then(|e| e.status()).is_some_and(|s| s.as_u16() == 401);
        if refused {
            state.update_cfg(|c| {
                if emby {
                    c.emby_saved = None;
                    c.emby_saved_at = None;
                } else {
                    c.jellyfin_saved = None;
                    c.jellyfin_saved_at = None;
                }
            })?;
            return Err(anyhow!(crate::i18n::tr("jellyfin.sessionExpired")).into());
        }
        return Err(e.into());
    }
    state.update_cfg(|c| {
        remember_saved(c, saved.clone());
        c.server = Some(saved);
    })?;
    state.choosing.store(false, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

/// Connexion à un serveur Jellyfin ou Emby (`emby`) par identifiant et mot de passe.
#[tauri::command]
pub async fn jellyfin_login(state: State<'_, AppState>, url: String, username: String, password: String, emby: Option<bool>) -> CmdResult<String> {
    let base = jellyfin::normalize_url(&url);
    let client_id = state.cfg().client_id;
    let login = jellyfin::login(&state.http, &base, &client_id, username.trim(), &password).await?;
    Ok(save_jellyfin(&state, base, login, emby.unwrap_or(false))?)
}

/// Quick Connect : demande un code à valider dans Jellyfin.
#[derive(Serialize)]
pub(crate) struct QuickCode {
    code: String,
    secret: String,
}

#[tauri::command]
pub async fn jellyfin_quick_start(state: State<'_, AppState>, url: String) -> CmdResult<QuickCode> {
    let qc = jellyfin::quick_connect_start(&state.http, &jellyfin::normalize_url(&url), &state.cfg().client_id).await?;
    Ok(QuickCode { code: qc.code, secret: qc.secret })
}

/// Le code Quick Connect est-il validé ? Si oui, Maku se connecte (renvoie `true`).
#[tauri::command]
pub async fn jellyfin_quick_poll(state: State<'_, AppState>, url: String, secret: String) -> CmdResult<bool> {
    let base = jellyfin::normalize_url(&url);
    match jellyfin::quick_connect_poll(&state.http, &base, &state.cfg().client_id, &secret).await? {
        Some(login) => {
            save_jellyfin(&state, base, login, false)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

#[tauri::command]
pub async fn plex_logout(state: State<'_, AppState>) -> CmdResult<()> {
    // Les sessions Jellyfin (serveur en cours et connexion gardée) sont fermées côté serveur.
    let c = state.cfg();
    for s in c.server.iter().chain(c.jellyfin_saved.iter()).chain(c.emby_saved.iter()).filter(|s| s.is_jellyfin()) {
        jellyfin::logout(&state.http, &s.uri, &c.client_id, &s.token).await;
    }
    state.choosing.store(false, std::sync::atomic::Ordering::Relaxed);
    state.update_cfg(|c| {
        c.plex_token = None;
        c.server = None;
        c.jellyfin_saved = None;
        c.jellyfin_saved_at = None;
        c.emby_saved = None;
        c.emby_saved_at = None;
        // Les sessions Seerr des comptes Jellyfin et Emby partent avec eux.
        c.seerr_cookies = None;
        for s in c.services.values_mut() {
            s.seerr_cookies = None;
        }
    })?;
    *state.seerr.lock().await = None;
    Ok(())
}

/// « Changer de compte Plex » : seul le compte Plex est oublié (on se reconnecte avec un autre) ; les
/// connexions Jellyfin et Emby gardées restent, comme les réglages Seerr et Tracearr de chaque serveur.
#[tauri::command]
pub async fn plex_switch_account(state: State<'_, AppState>) -> CmdResult<()> {
    *state.seerr.lock().await = None;
    state.update_cfg(|c| {
        c.plex_token = None;
        match c.server.take() {
            Some(s) if s.is_jellyfin() => remember_saved(c, s),
            _ => {}
        }
    })?;
    state.choosing.store(false, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

/// « Changer de serveur » : retour à l'écran de choix Plex / Jellyfin, sans perdre les connexions.
#[tauri::command]
pub async fn plex_forget_server(state: State<'_, AppState>) -> CmdResult<()> {
    *state.seerr.lock().await = None;
    state.update_cfg(|c| {
        if let Some(s) = c.server.take().filter(|s| s.is_jellyfin()) {
            remember_saved(c, s);
        }
    })?;
    // Plex seulement : directement la liste de ses serveurs, sans écran de choix.
    let plex_only = server_kinds(&state.cfg()) == "plex";
    state.choosing.store(!plex_only, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerSummary {
    name: String,
    machine_id: String,
    owned: bool,
    /// Nom du propriétaire (serveurs partagés) et photo de son compte Plex.
    owner: Option<String>,
    owner_thumb: Option<String>,
}

#[tauri::command]
pub async fn plex_servers(state: State<'_, AppState>) -> CmdResult<Vec<ServerSummary>> {
    let c = state.cfg();
    let token = c.plex_token.ok_or_else(|| anyhow!(crate::i18n::tr("app.notLoggedIn")))?;
    let (servers, (own, friends)) = futures::join!(
        plex::servers(&state.http, &c.client_id, &token),
        plex::avatars(&state.http, &c.client_id, &token)
    );
    Ok(servers?
        .into_iter()
        .map(|r| {
            let owner_thumb = if r.owned {
                own.clone()
            } else {
                r.owner_id.and_then(|id| friends.iter().find(|(f, _)| *f == id).map(|(_, t)| t.clone()))
            };
            ServerSummary {
                name: r.name,
                machine_id: r.client_identifier,
                owned: r.owned,
                owner: (!r.owned).then_some(r.source_title).flatten(),
                owner_thumb,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn plex_select_server(state: State<'_, AppState>, machine_id: String) -> CmdResult<String> {
    let c = state.cfg();
    let token = c.plex_token.ok_or_else(|| anyhow!(crate::i18n::tr("app.notLoggedIn")))?;
    let res = plex::servers(&state.http, &c.client_id, &token)
        .await?
        .into_iter()
        .find(|r| r.client_identifier == machine_id)
        .ok_or_else(|| anyhow!(crate::i18n::tr("app.serverNotFound")))?;
    let uri = plex::pick_connection(&state.http, &c.client_id, &res).await?;
    let name = res.name.clone();
    let saved = SavedServer {
        name: res.name,
        machine_id,
        uri,
        token: res.access_token.unwrap_or(token),
        kind: String::new(),
        user_id: None,
    };
    state.update_cfg(|c| c.server = Some(saved))?;
    Ok(name)
}

/// La meilleure adresse du serveur peut avoir changé (autre réseau, Wi-Fi ↔ 4G, adresse locale
/// devenue joignable) : on la revérifie et on la garde si elle diffère. `quick` : si l'adresse en
/// cours répond, on s'en contente (retour dans l'application) ; sinon on refait le tour des adresses,
/// comme au lancement. Renvoie vrai quand l'adresse a changé.
pub(crate) async fn refresh_connection(state: &AppState, quick: bool) -> anyhow::Result<bool> {
    let c = state.cfg();
    let (Some(token), Some(saved)) = (c.plex_token, c.server) else { return Ok(false) };
    if saved.is_jellyfin() {
        return Ok(false);
    }
    if quick {
        let alive = plex::with_plex_headers(state.http.get(format!("{}/identity", saved.uri)), &c.client_id, Some(&saved.token))
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success());
        if alive {
            return Ok(false);
        }
    }
    let Some(res) = plex::servers(&state.http, &c.client_id, &token)
        .await?
        .into_iter()
        .find(|r| r.client_identifier == saved.machine_id)
    else {
        return Ok(false);
    };
    let uri = plex::pick_connection(&state.http, &c.client_id, &res).await?;
    if uri == saved.uri {
        return Ok(false);
    }
    state.update_cfg(|c| {
        if let Some(s) = c.server.as_mut() {
            s.uri = uri;
        }
    })?;
    Ok(true)
}

/// Retour dans l'application, réseau revenu : l'adresse du serveur répond-elle toujours ? Sinon, Maku
/// en cherche une autre (vrai si elle a changé : la page se recharge).
#[tauri::command]
pub async fn reconnect_server(state: State<'_, AppState>) -> CmdResult<bool> {
    Ok(refresh_connection(&state, true).await?)
}
