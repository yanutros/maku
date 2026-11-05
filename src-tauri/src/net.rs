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
