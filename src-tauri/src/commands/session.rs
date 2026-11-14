//! Session et réglages de l'application : serveur retenu, chemin de mpv, ouverture de liens.

use crate::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Session {
    logged_in: bool,
    /// Connexions gardées pour l'écran de choix : compte Plex, serveur Jellyfin (nom, adresse).
    plex_remembered: bool,
    jellyfin_remembered: Option<RememberedServer>,
    emby_remembered: Option<RememberedServer>,
    /// « plex », « jellyfin » ou « both ».
    server_kinds: String,
    /// « plex » ou « jellyfin » (Emby compris : même API).
    server_kind: &'static str,
    /// Nom affiché du type de serveur : « Plex », « Jellyfin » ou « Emby ».
    server_brand: &'static str,
    server_name: Option<String>,
    seerr_url: Option<String>,
    mpv_path: Option<String>,
    tracearr_url: Option<String>,
    tracearr_connected: bool,
    /// Une clé TMDB est enregistrée (elle-même n'est jamais renvoyée à la page).
    tmdb: bool,
    /// Plex Home : connexion automatique au dernier profil, sans écran de choix.
    auto_sign_in: bool,
    /// Version d'un magasin (Microsoft Store, Google Play) : pas de mise à jour intégrée ni de dons
    /// par un lien externe, que leurs règles interdisent.
    store: bool,
}

#[derive(Serialize)]
pub(crate) struct RememberedServer {
    name: String,
    url: String,
    /// Serveur Emby (sinon Jellyfin).
    emby: bool,
}

/// Une connexion Jellyfin gardée ne sert plus au-delà de 30 jours sans utilisation.
pub(crate) const JELLYFIN_REMEMBER_SECS: u64 = 30 * 24 * 3600;

pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Connexion Jellyfin gardée et encore récente.
pub(crate) fn remembered_jellyfin(c: &Config) -> Option<SavedServer> {
    let at = c.jellyfin_saved_at?;
    c.jellyfin_saved.clone().filter(|_| now_secs().saturating_sub(at) < JELLYFIN_REMEMBER_SECS)
}

/// Connexion Emby gardée et encore récente.
pub(crate) fn remembered_emby(c: &Config) -> Option<SavedServer> {
    let at = c.emby_saved_at?;
    c.emby_saved.clone().filter(|_| now_secs().saturating_sub(at) < JELLYFIN_REMEMBER_SECS)
}

/// Garde la connexion Jellyfin ou Emby, chacune à sa place, pour y revenir sans se reconnecter.
pub(crate) fn remember_saved(c: &mut Config, s: SavedServer) {
    if s.is_emby() {
        c.emby_saved = Some(s);
        c.emby_saved_at = Some(now_secs());
    } else {
        c.jellyfin_saved = Some(s);
        c.jellyfin_saved_at = Some(now_secs());
    }
}

/// Serveurs utilisés. Sans choix enregistré : d'après les connexions gardées.
pub(crate) fn server_kinds(c: &Config) -> String {
    c.server_kinds.clone().unwrap_or_else(|| {
        let jellyfin = remembered_jellyfin(c).is_some() || remembered_emby(c).is_some() || c.server.as_ref().is_some_and(|s| s.is_jellyfin());
        match (c.plex_token.is_some(), jellyfin) {
            (true, false) => "plex",
            (false, true) => "jellyfin",
            _ => "both",
        }
        .to_string()
    })
}

/// Première connexion : le type de serveur choisi devient celui de Maku (modifiable dans les Réglages).
pub(crate) fn remember_server_kinds(c: &mut Config) {
    if c.server_kinds.is_none() {
        c.server_kinds = Some(server_kinds(c));
    }
}

/// Réglages : « plex », « jellyfin » ou « both ».
#[tauri::command]
pub fn set_server_kinds(state: State<'_, AppState>, kinds: String) -> CmdResult<()> {
    if !matches!(kinds.as_str(), "plex" | "jellyfin" | "both") {
        return Err(anyhow!("{kinds}").into());
    }
    state.update_cfg(|c| c.server_kinds = Some(kinds))?;
    Ok(())
}

#[tauri::command]
pub fn get_session(state: State<'_, AppState>) -> Session {
    let c = state.cfg();
    let choosing = state.choosing.load(std::sync::atomic::Ordering::Relaxed);
    Session {
        logged_in: !choosing && (c.plex_token.is_some() || c.server.as_ref().is_some_and(|s| s.is_jellyfin())),
        plex_remembered: c.plex_token.is_some(),
        server_kinds: server_kinds(&c),
        jellyfin_remembered: remembered_jellyfin(&c).map(|s| RememberedServer { emby: false, name: s.name, url: s.uri }),
        emby_remembered: remembered_emby(&c).map(|s| RememberedServer { emby: true, name: s.name, url: s.uri }),
        server_kind: if c.server.as_ref().is_some_and(|s| s.is_jellyfin()) { "jellyfin" } else { "plex" },
        server_brand: match c.server.as_ref() {
            Some(s) if s.is_emby() => "Emby",
            Some(s) if s.is_jellyfin() => "Jellyfin",
            _ => "Plex",
        },
        server_name: c.server.map(|s| s.name),
        seerr_url: c.seerr_url,
        mpv_path: c.mpv_path,
        tracearr_url: c.tracearr_url,
        tracearr_connected: c.tracearr_cookies.is_some(),
        tmdb: c.tmdb_key.is_some(),
        auto_sign_in: c.auto_sign_in,
        store: update::from_store(),
    }
}

/// Ouvre une page web (bande-annonce, fiche IMDb…) dans le navigateur.
#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> CmdResult<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(anyhow!(crate::i18n::tr_with("app.urlRefused", &[("url", &url)])).into());
    }
    app.opener().open_url(&url, None::<&str>)?;
    Ok(())
}

#[tauri::command]
pub fn set_mpv_path(state: State<'_, AppState>, path: Option<String>) -> CmdResult<()> {
    let path = path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
    state.update_cfg(|c| c.mpv_path = path)?;
    Ok(())
}
