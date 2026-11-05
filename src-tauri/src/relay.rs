//! Relais d'invitation : pendant que l'écran « Configurer avec ton téléphone » est ouvert, Maku
//! sert sur le réseau local une petite page à une adresse secrète (affichée en QR code). Le
//! téléphone y envoie le code d'invitation de l'administrateur ; l'interface l'applique et publie
//! l'avancement (dont le lien d'autorisation Tracearr), que la page du téléphone affiche.
//!
//! Même relais pour se connecter à Jellyfin ou Emby sur un téléviseur : le téléphone saisit l'adresse,
//! l'identifiant et le mot de passe (page `relay-login.html`), la télé se connecte et publie le résultat.

use anyhow::{anyhow, Result};
use std::net::{IpAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const PAGE: &str = include_str!("relay.html");
const LOGIN_PAGE: &str = include_str!("relay-login.html");
/// Lecture des QR codes sur le téléphone (jsQR, Apache-2.0), intégrée à la page.
const JSQR: &str = include_str!("../../node_modules/jsqr/dist/jsQR.js");

pub struct Relay {
    pub url: String,
    status: Arc<Mutex<String>>,
    task: tauri::async_runtime::JoinHandle<()>,
}

impl Relay {
    pub fn set_status(&self, json: String) {
        *self.status.lock().unwrap() = json;
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
    let status = Arc::new(Mutex::new(r#"{"step":"waiting"}"#.to_string()));
    // Textes de la page dans la langue de l'interface.
    let page = match login {
        None => PAGE.replace("/*I18N*/{}", &crate::i18n::section_json("relay")).replace("/*JSQR*/", JSQR),
        Some((server, url)) => LOGIN_PAGE
            .replace("/*I18N*/{}", &crate::i18n::section_json("relayLogin"))
            .replace("/*SERVER*/", &js_text(server))
            .replace("/*URL*/", &js_text(url)),
    };
    let page = Arc::new(page.replace("<html lang=\"fr\">", &format!("<html lang=\"{}\">", crate::i18n::language())));
    let route = if login.is_some() { "login" } else { "code" };

    let task = {
        let (token, status) = (token.clone(), status.clone());
        tauri::async_runtime::spawn(async move {
            while let Ok((conn, _)) = listener.accept().await {
                let (app, token, status, page) = (app.clone(), token.clone(), status.clone(), page.clone());
                tauri::async_runtime::spawn(async move {
                    let _ = serve(conn, &app, &token, &status, &page, route).await;
                });
            }
        })
    };
    Ok(Relay { url: format!("http://{ip}:{port}/{token}"), status, task })
}

/// Texte placé dans une chaîne JavaScript de la page, sans pouvoir en sortir.
fn js_text(text: &str) -> String {
    let quoted = serde_json::to_string(text).unwrap_or_default();
    quoted[1..quoted.len().saturating_sub(1)].replace('<', "\\u003c")
}

async fn serve(mut conn: TcpStream, app: &AppHandle, token: &str, status: &Mutex<String>, page: &str, route: &str) -> Result<()> {
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

    let base = format!("/{token}");
    let (code, kind, content) = match (method, path) {
        ("GET", p) if p == base => ("200 OK", "text/html; charset=utf-8", page.to_string()),
        ("GET", p) if p == format!("{base}/status") => ("200 OK", "application/json", status.lock().unwrap().clone()),
        ("POST", p) if p == format!("{base}/{route}") => {
            *status.lock().unwrap() = r#"{"step":"received"}"#.to_string();
            app.emit(if route == "login" { "relay-login" } else { "relay-code" }, body.trim().to_string())?;
            ("200 OK", "application/json", r#"{"ok":true}"#.to_string())
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
