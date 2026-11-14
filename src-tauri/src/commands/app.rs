//! Application : langue, icône aux couleurs du thème, mises à jour.

use crate::*;

/// Langue de l'interface, pour les messages du backend.
#[tauri::command]
pub fn set_language(language: String) {
    i18n::set_language(&language);
}

/// Icône de la fenêtre (et de la barre des tâches) aux couleurs du thème choisi.
#[tauri::command]
pub fn set_app_icon(app: AppHandle, theme: String) -> CmdResult<()> {
    #[cfg(desktop)]
    {
        let bytes: &[u8] = match theme.as_str() {
            "rouge" => include_bytes!("../../icons/themes/rouge.png"),
            "vert" => include_bytes!("../../icons/themes/vert.png"),
            "violet" => include_bytes!("../../icons/themes/violet.png"),
            _ => include_bytes!("../../icons/themes/jaune.png"),
        };
        if let Some(window) = app.get_webview_window("main") {
            let icon = tauri::image::Image::from_bytes(bytes).map_err(anyhow::Error::from)?;
            window.set_icon(icon).map_err(anyhow::Error::from)?;
        }
    }
    #[cfg(mobile)]
    let _ = (app, theme);
    Ok(())
}

/// Notes de version de la version installée.
#[tauri::command]
pub async fn release_notes(state: State<'_, AppState>) -> CmdResult<update::UpdateInfo> {
    Ok(update::installed_notes(&state.http).await?)
}

#[tauri::command]
pub async fn update_check(state: State<'_, AppState>) -> CmdResult<Option<update::UpdateInfo>> {
    Ok(update::check(&state.http).await?)
}

/// Télécharge la nouvelle version et lance son installation. Sur ordinateur, Maku se ferme si
/// l'installeur le remplace ; sur Android, renvoie le fichier, que l'interface confie à Android.
///
/// La version à installer est redemandée ici à GitHub : rien de ce que transmet la page (adresse,
/// empreinte) n'est repris, seul son numéro sert à vérifier qu'il s'agit bien de la même.
#[tauri::command]
pub async fn update_install(app: AppHandle, state: State<'_, AppState>, info: update::UpdateInfo) -> CmdResult<Option<String>> {
    let info = match update::check(&state.http).await? {
        Some(fresh) if fresh.version == info.version => fresh,
        _ => return Err(anyhow!(crate::i18n::tr("update.noUpdate")).into()),
    };
    // Le téléchargement peut être long : pas de délai maximal, contrairement aux requêtes habituelles.
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .redirect(net::github_only())
        .build()
        .map_err(anyhow::Error::from)?;
    let dir = app.path().app_cache_dir().map_err(anyhow::Error::from)?.join("updates");
    let path = update::download(&app, &http, &info, &dir).await?;
    if cfg!(target_os = "android") {
        return Ok(Some(path.to_string_lossy().into_owned()));
    }
    if update::install(&path)? {
        app.exit(0);
    }
    Ok(None)
}
