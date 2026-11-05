//! Relais d'invitation : pendant que l'écran « Configurer avec ton téléphone » est ouvert, Maku
//! sert sur le réseau local une petite page à une adresse secrète (affichée en QR code). Le
//! téléphone y envoie le code d'invitation de l'administrateur ; l'interface l'applique et publie
//! l'avancement (dont le lien d'autorisation Tracearr), que la page du téléphone affiche.
//!
//! Même relais pour se connecter à Jellyfin ou Emby sur un téléviseur : le téléphone saisit l'adresse,
//! l'identifiant et le mot de passe (page `relay-login.html`), la télé se connecte et publie le résultat.
//!
//! Le relais des Réglages sert aussi, à l'adresse `…/tmdb`, une page où coller sa clé d'API TMDB
//! (`relay-tmdb.html`), avec son propre avancement.

use anyhow::{anyhow, Result};
use std::net::{IpAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const PAGE: &str = include_str!("relay.html");
const LOGIN_PAGE: &str = include_str!("relay-login.html");
const TMDB_PAGE: &str = include_str!("relay-tmdb.html");
/// Lecture des QR codes sur le téléphone (jsQR, Apache-2.0), intégrée à la page.
const JSQR: &str = include_str!("../../node_modules/jsqr/dist/jsQR.js");

pub struct Relay {
    pub url: String,
    shared: Arc<Shared>,
    task: tauri::async_runtime::JoinHandle<()>,
}

/// Pages servies et avancement publié par l'interface, pour chacune.
struct Shared {
    token: String,
    page: String,
    /// Invitation (« code ») ou connexion (« login ») : chemin où la page envoie ce qu'on a saisi.
    route: &'static str,
    status: Mutex<String>,
    /// Page de la clé TMDB (relais des Réglages seulement).
    tmdb_page: Option<String>,
    tmdb_status: Mutex<String>,
}

const WAITING: &str = r#"{"step":"waiting"}"#;
const RECEIVED: &str = r#"{"step":"received"}"#;

impl Relay {
    /// Avancement de la page principale, ou de celle de la clé TMDB (`tmdb`).
    pub fn set_status(&self, json: String, tmdb: bool) {
        let slot = if tmdb { &self.shared.tmdb_status } else { &self.shared.status };
        *slot.lock().unwrap() = json;
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Adresse de l'appareil sur le réseau local (celle de l'interface qui mène vers l'extérieur).
fn lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    // Aucun paquet n'est envoyé : la « connexion » UDP choisit seulement l'interface de sortie.
    socket.connect("192.0.2.1:9").ok()?;
    socket.local_addr().ok().map(|a| a.ip())
}

/// Page servie : l'invitation (`login` vide), ou la connexion au serveur nommé (« Jellyfin », « Emby »),
/// avec l'adresse déjà saisie sur la télé (`url`).
pub async fn start(app: AppHandle, login: Option<(&str, &str)>) -> Result<Relay> {
    let ip = lan_ip().ok_or_else(|| anyhow!(crate::i18n::tr("relay.noNetwork")))?;
    let listener = TcpListener::bind("0.0.0.0:0").await?;
    let port = listener.local_addr()?.port();
    let token = uuid::Uuid::new_v4().simple().to_string();
    // Textes des pages dans la langue de l'interface.
    let lang = |page: String| page.replace("<html lang=\"fr\">", &format!("<html lang=\"{}\">", crate::i18n::language()));
    let page = match login {
        None => PAGE.replace("/*I18N*/{}", &crate::i18n::section_json("relay")).replace("/*JSQR*/", JSQR),
        Some((server, url)) => LOGIN_PAGE
            .replace("/*I18N*/{}", &crate::i18n::section_json("relayLogin"))
            .replace("/*SERVER*/", &js_text(server))
            .replace("/*URL*/", &js_text(url)),
    };
    let shared = Arc::new(Shared {
        token: token.clone(),
        page: lang(page),
        route: if login.is_some() { "login" } else { "code" },
        status: Mutex::new(WAITING.into()),
        tmdb_page: login.is_none().then(|| lang(TMDB_PAGE.replace("/*I18N*/{}", &crate::i18n::section_json("relayTmdb")))),
        tmdb_status: Mutex::new(WAITING.into()),
    });

    let task = {
        let shared = shared.clone();
        tauri::async_runtime::spawn(async move {
            while let Ok((conn, _)) = listener.accept().await {
                let (app, shared) = (app.clone(), shared.clone());
                tauri::async_runtime::spawn(async move {
                    let _ = serve(conn, &app, &shared).await;
                });
            }
        })
    };
    Ok(Relay { url: format!("http://{ip}:{port}/{token}"), shared, task })
}

/// Texte placé dans une chaîne JavaScript de la page, sans pouvoir en sortir.
fn js_text(text: &str) -> String {
    let quoted = serde_json::to_string(text).unwrap_or_default();
    quoted[1..quoted.len().saturating_sub(1)].replace('<', "\\u003c")
}

async fn serve(mut conn: TcpStream, app: &AppHandle, shared: &Shared) -> Result<()> {
    // En-têtes puis corps (8 Ko au plus : un code d'invitation est bien plus court).
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = conn.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > 16 * 1024 {
            return Ok(());
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut parts = head.lines().next().unwrap_or_default().split(' ');
    let (method, path) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
    let length = head
        .lines()
        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0)))
        .unwrap_or(0usize)
        .min(8 * 1024);
    while buf.len() < header_end + length {
        let n = conn.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&buf[header_end..buf.len().min(header_end + length)]).to_string();

    let base = format!("/{}", shared.token);
    let tmdb = format!("{base}/tmdb");
    let html = "text/html; charset=utf-8";
    let ok = || ("200 OK", "application/json", r#"{"ok":true}"#.to_string());
    let (code, kind, content) = match (method, path, shared.tmdb_page.as_ref()) {
        ("GET", p, _) if p == base => ("200 OK", html, shared.page.clone()),
        ("GET", p, _) if p == format!("{base}/status") => ("200 OK", "application/json", shared.status.lock().unwrap().clone()),
        ("POST", p, _) if p == format!("{base}/{}", shared.route) => {
            *shared.status.lock().unwrap() = RECEIVED.into();
            app.emit(if shared.route == "login" { "relay-login" } else { "relay-code" }, body.trim().to_string())?;
            ok()
        }
        ("GET", p, Some(page)) if p == tmdb => ("200 OK", html, page.clone()),
        ("GET", p, Some(_)) if p == format!("{tmdb}/status") => ("200 OK", "application/json", shared.tmdb_status.lock().unwrap().clone()),
        ("POST", p, Some(_)) if p == tmdb => {
            *shared.tmdb_status.lock().unwrap() = RECEIVED.into();
            app.emit("relay-tmdb", body.trim().to_string())?;
            ok()
        }
        _ => ("404 Not Found", "text/plain", crate::i18n::tr("relay.notFound")),
    };
    let response = format!(
        "HTTP/1.1 {code}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{content}",
        content.len()
    );
    conn.write_all(response.as_bytes()).await?;
    conn.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn js_text_stays_in_its_string() {
        assert_eq!(super::js_text("Emby"), "Emby");
        assert_eq!(super::js_text("a\"b</script>"), "a\\\"b\\u003c/script>");
    }
}
