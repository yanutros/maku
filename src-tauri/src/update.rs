//! Mises à jour : la dernière version publiée sur GitHub, le paquet qui convient à cet appareil,
//! son téléchargement et son installation.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

const LATEST: &str = "https://api.github.com/repos/yanutros/maku/releases/latest";
/// Seuls les fichiers publiés du dépôt de Maku peuvent être téléchargés et installés.
const ASSETS: &str = "https://api.github.com/repos/yanutros/maku/releases/assets/";

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current: String,
    pub notes: String,
    pub date: Option<String>,
    pub asset_url: String,
    pub asset_name: String,
    pub size: u64,
    /// Empreinte SHA-256 publiée par GitHub pour ce fichier (en hexadécimal).
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    body: Option<String>,
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    /// Adresse de l'API : avec `Accept: application/octet-stream`, elle renvoie le fichier.
    url: String,
    size: u64,
    /// « sha256:… », calculée par GitHub à la publication du fichier.
    #[serde(default)]
    digest: Option<String>,
}

/// « v1.2.3 » → (1, 2, 3).
fn parse(v: &str) -> (u64, u64, u64) {
    let mut n = v.trim_start_matches('v').split(['.', '-']).map(|p| p.parse().unwrap_or(0));
    (n.next().unwrap_or(0), n.next().unwrap_or(0), n.next().unwrap_or(0))
}

/// Paquet adapté à cet appareil, d'après la fin de son nom.
fn pick(assets: &[Asset]) -> Option<&Asset> {
    let suffix = match std::env::consts::OS {
        "windows" => "_x64-setup.exe",
        "macos" => ".dmg",
        "android" => "_android.apk",
        "linux" if std::env::var_os("APPIMAGE").is_some() => ".AppImage",
        "linux" if Path::new("/etc/debian_version").exists() => ".deb",
        "linux" => ".rpm",
        _ => return None,
    };
    assets.iter().find(|a| a.name.ends_with(suffix))
}

fn request(http: &reqwest::Client, url: &str) -> reqwest::RequestBuilder {
    let mut req = http.get(url).header("User-Agent", "Maku").header("X-GitHub-Api-Version", "2022-11-28");
    // Développement seulement : un jeton permet de tester avant la publication du dépôt.
    if cfg!(debug_assertions) {
        if let Ok(token) = std::env::var("PLEXR_GH_TOKEN") {
            req = req.bearer_auth(token);
        }
    }
    req
}

/// Notes de la version installée (le rappel des nouveautés, depuis les Réglages).
pub async fn installed_notes(http: &reqwest::Client) -> Result<UpdateInfo> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let url = format!("https://api.github.com/repos/yanutros/maku/releases/tags/v{current}");
    let release: Release = request(http, &url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(UpdateInfo {
        version: current.clone(),
        current,
        notes: release.body.unwrap_or_default(),
        date: release.published_at,
        asset_url: String::new(),
        asset_name: String::new(),
        size: 0,
        sha256: None,
    })
}

/// Installé depuis le Microsoft Store (paquet MSIX, dans `WindowsApps`) : c'est le Store qui met Maku
/// à jour, la mise à jour intégrée se tait.
pub fn from_store() -> bool {
    cfg!(windows)
        && std::env::current_exe()
            .map(|p| p.to_string_lossy().to_lowercase().contains(r"\windowsapps\"))
            .unwrap_or(false)
}

/// Version plus récente que celle-ci, s'il y en a une avec un paquet pour cet appareil.
pub async fn check(http: &reqwest::Client) -> Result<Option<UpdateInfo>> {
    if from_store() {
        return Ok(None);
    }
    let mut current = env!("CARGO_PKG_VERSION").to_string();
    // Développement seulement : se faire passer pour une ancienne version.
    if cfg!(debug_assertions) {
        if let Ok(v) = std::env::var("PLEXR_UPDATE_FROM") {
            current = v;
        }
    }
    let resp = request(http, LATEST).header("Accept", "application/vnd.github+json").send().await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let release: Release = resp.error_for_status()?.json().await?;
    if release.draft || release.prerelease || parse(&release.tag_name) <= parse(&current) {
        return Ok(None);
    }
    let Some(asset) = pick(&release.assets) else { return Ok(None) };
    Ok(Some(UpdateInfo {
        version: release.tag_name.trim_start_matches('v').to_string(),
        current,
        notes: release.body.unwrap_or_default(),
        date: release.published_at,
        asset_url: asset.url.clone(),
        asset_name: asset.name.clone(),
        size: asset.size,
        sha256: asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")).map(str::to_lowercase),
    }))
}

/// Télécharge le paquet dans `dir`, en signalant l'avancement (événement `update-progress`, de 0 à 1).
/// Le fichier doit venir des versions publiées du dépôt, et son empreinte SHA-256 correspondre à celle
/// que publie GitHub : sinon il est supprimé, et rien n'est installé.
pub async fn download(app: &AppHandle, http: &reqwest::Client, info: &UpdateInfo, dir: &Path) -> Result<PathBuf> {
    if !info.asset_url.starts_with(ASSETS) {
        bail!(crate::i18n::tr("update.badSource"));
    }
    let expected = info.sha256.clone().ok_or_else(|| anyhow!(crate::i18n::tr("update.noChecksum")))?;
    // Le nom vient de GitHub : on n'en garde que la partie fichier.
    let name = Path::new(&info.asset_name).file_name().ok_or_else(|| anyhow!(crate::i18n::tr("update.invalidName")))?;
    tokio::fs::create_dir_all(dir).await?;
    let path = dir.join(name);
    let mut resp = request(http, &info.asset_url)
        .header("Accept", "application/octet-stream")
        .send()
        .await?
        .error_for_status()?;
    let total = resp.content_length().unwrap_or(info.size).max(1);
    let mut file = tokio::fs::File::create(&path).await?;
    let mut done = 0u64;
    let mut last = 0.0;
    let mut hasher = Sha256::new();
    while let Some(chunk) = resp.chunk().await? {
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        done += chunk.len() as u64;
        let progress = done as f64 / total as f64;
        if progress - last >= 0.01 {
            last = progress;
            let _ = app.emit("update-progress", progress.min(1.0));
        }
    }
    file.flush().await?;
    drop(file);
    if done == 0 {
        bail!(crate::i18n::tr("update.emptyDownload"));
    }
    let actual: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if actual != expected {
        let _ = tokio::fs::remove_file(&path).await;
        bail!(crate::i18n::tr("update.badChecksum"));
    }
    Ok(path)
}

/// Lance l'installation sur ordinateur. Renvoie `true` si Maku doit se fermer pour la laisser faire.
/// (Sur Android, c'est l'installateur du système qui prend la main, depuis l'interface.)
pub fn install(path: &Path) -> Result<bool> {
    use std::process::Command;
    match std::env::consts::OS {
        // Installeur NSIS de Tauri : /P affiche juste la progression, /R relance Maku à la fin.
        "windows" => {
            Command::new(path).args(["/P", "/R"]).spawn()?;
            Ok(true)
        }
        "macos" => {
            Command::new("open").arg(path).spawn()?;
            Ok(false)
        }
        "linux" => install_linux(path),
        os => bail!(crate::i18n::tr_with("update.unsupported", &[("os", os)])),
    }
}

#[cfg(target_os = "linux")]
fn install_linux(path: &Path) -> Result<bool> {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    if let Some(current) = std::env::var_os("APPIMAGE") {
        // AppImage : on remplace le fichier puis on relance.
        let current = PathBuf::from(current);
        let next = current.with_extension("new");
        std::fs::copy(path, &next)?;
        std::fs::set_permissions(&next, std::fs::Permissions::from_mode(0o755))?;
        std::fs::rename(&next, &current)?;
        Command::new(&current).spawn()?;
        Ok(true)
    } else {
        // RPM / DEB : la logithèque du système installe le paquet (mot de passe administrateur).
        Command::new("xdg-open").arg(path).spawn()?;
        Ok(false)
    }
}

#[cfg(not(target_os = "linux"))]
fn install_linux(_path: &Path) -> Result<bool> {
    unreachable!()
}
