//! Règles réseau communes aux clients HTTP de Maku.

use reqwest::redirect::{Attempt, Policy};

/// Nombre maximal de redirections suivies, comme le réglage par défaut de reqwest.
const MAX_REDIRECTS: usize = 10;

/// Serveurs (Plex, Jellyfin, Seerr, Tracearr) : une redirection n'est suivie que vers la même machine
/// (`http` → `https` compris, pour un serveur derrière un proxy inverse). Un serveur ne peut donc pas
/// renvoyer Maku, avec ses jetons, vers une autre adresse (réseau local, métadonnées du cloud…).
pub fn same_host() -> Policy {
    Policy::custom(|attempt| {
        let same = attempt.previous().last().and_then(|p| p.host_str()) == attempt.url().host_str();
        follow_if(attempt, same)
    })
}

/// Mises à jour : GitHub redirige le téléchargement des paquets vers son stockage ; seuls ses domaines
/// sont acceptés.
pub fn github_only() -> Policy {
    Policy::custom(|attempt| {
        let host = attempt.url().host_str().unwrap_or_default();
        let github = attempt.url().scheme() == "https"
            && (host == "github.com" || host.ends_with(".github.com") || host.ends_with(".githubusercontent.com"));
        follow_if(attempt, github)
    })
}

/// Message d'erreur sans ce qui ne doit ni s'afficher ni s'enregistrer : les paramètres des adresses qu'il cite
/// (une erreur réseau donne l'adresse appelée, et les jetons de Plex ou de Jellyfin y sont en paramètre), et la
/// valeur d'un jeton écrit hors d'une adresse.
pub fn redact(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(start) = ["http://", "https://"].iter().filter_map(|s| rest.find(s)).min() {
        let (before, url) = rest.split_at(start);
        out.push_str(before);
        let end = url.find(|c: char| c.is_whitespace() || matches!(c, ')' | '"' | '\'' | '<' | '>')).unwrap_or(url.len());
        let (url, after) = url.split_at(end);
        out.push_str(url.split(['?', '#']).next().unwrap_or(url));
        rest = after;
    }
    out.push_str(rest);
    for key in ["x-plex-token=", "api_key=", "apikey=", "access_token="] {
        let mut from = 0;
        while let Some(at) = out[from..].to_ascii_lowercase().find(key) {
            let value = from + at + key.len();
            let end = out[value..].find(|c: char| c.is_whitespace() || matches!(c, '&' | ')' | '"' | '\'' | ',' | ';')).map_or(out.len(), |e| value + e);
            out.replace_range(value..end, "…");
            from = value + "…".len();
        }
    }
    out
}

fn follow_if(attempt: Attempt, allowed: bool) -> reqwest::redirect::Action {
    if !allowed {
        attempt.stop()
    } else if attempt.previous().len() > MAX_REDIRECTS {
        attempt.error("too many redirects")
    } else {
        attempt.follow()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn redirect(policy: Policy, from: &str, to: &str) -> bool {
        // Petit serveur local qui répond une redirection vers `to`.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let to = to.to_owned();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf).await;
            let resp = format!("HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            let _ = s.write_all(resp.as_bytes()).await;
        });
        let client = reqwest::Client::builder().redirect(policy).build().unwrap();
        let url = from.replace("PORT", &port.to_string());
        let resp = client.get(url).send().await;
        // Redirection non suivie : la réponse 302 revient telle quelle.
        !matches!(resp, Ok(r) if r.status() == 302)
    }

    #[test]
    fn redact_removes_tokens_from_errors() {
        // Erreur de reqwest : l'adresse appelée, avec le jeton en paramètre.
        let e = "error sending request for url (http://10.0.0.2:32400/library/parts/1/file.mkv?download=1&X-Plex-Token=SECRET): connection closed";
        assert_eq!(redact(e), "error sending request for url (http://10.0.0.2:32400/library/parts/1/file.mkv): connection closed");
        assert_eq!(redact("HTTP 404 pour https://j.example/Videos/a/stream?api_key=SECRET&Static=true"), "HTTP 404 pour https://j.example/Videos/a/stream");
        // Deux adresses, et un jeton hors d'une adresse.
        let two = redact("de http://a/x?t=SECRET vers https://b/y?u=SECRET ; X-Plex-Token=SECRET, fin");
        assert_eq!(two, "de http://a/x vers https://b/y ; X-Plex-Token=…, fin");
        assert!(!redact("api_key=SECRET").contains("SECRET"));
        // Rien à retirer : le message reste tel quel.
        assert_eq!(redact("Serveur injoignable (délai dépassé)"), "Serveur injoignable (délai dépassé)");
    }

    #[tokio::test]
    async fn reqwest_error_is_redacted() {
        // Port fermé : l'erreur de reqwest cite l'adresse, jeton compris.
        let e = reqwest::Client::new().get("http://127.0.0.1:9/file?X-Plex-Token=SECRET").send().await.unwrap_err();
        let shown = redact(&format!("{:#}", anyhow::Error::from(e)));
        assert!(shown.contains("http://127.0.0.1:9/file"), "{shown}");
        assert!(!shown.contains("SECRET"), "{shown}");
    }

    #[tokio::test]
    async fn same_host_stops_other_hosts() {
        assert!(!redirect(same_host(), "http://127.0.0.1:PORT/", "http://169.254.169.254/latest/meta-data/").await);
        assert!(!redirect(same_host(), "http://127.0.0.1:PORT/", "http://localhost:9/").await);
    }

    #[tokio::test]
    async fn same_host_follows_same_machine() {
        // Même machine, autre port (fermé) : la redirection est tentée, donc la requête échoue.
        assert!(redirect(same_host(), "http://127.0.0.1:PORT/", "http://127.0.0.1:9/").await);
    }

    #[tokio::test]
    async fn github_only_stops_elsewhere() {
        assert!(!redirect(github_only(), "http://127.0.0.1:PORT/", "http://127.0.0.1:9/").await);
    }
}
