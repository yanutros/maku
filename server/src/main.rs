//! Maku Server : relais Seerr et Tracearr, installé à côté d'un serveur Plex, Jellyfin ou Emby.
//!
//! Seerr et Tracearr restent sur le réseau local ; seul ce relais est joignable, et il ne laisse passer
//! que les utilisateurs du serveur multimédia : chaque appel porte le jeton de l'utilisateur
//! (`X-Maku-Token`), vérifié auprès du serveur. Il transmet ensuite l'appel tel quel (cookies de
//! session compris) : Seerr et Tracearr voient le compte de l'utilisateur, avec ses droits.
//!
//! - `GET /maku/info` : ce que le relais propose (sans jeton), pour que Maku le reconnaisse.
//! - `/seerr/api/v1/…` et `/tracearr/api/v1/…` : l'API de chacun, et rien d'autre (ni leur site web).

mod auth;
mod proxy;

use anyhow::{bail, Context, Result};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Plex,
    Jellyfin,
    Emby,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Plex => "plex",
            Kind::Jellyfin => "jellyfin",
            Kind::Emby => "emby",
        }
    }
}

pub struct Config {
    pub kind: Kind,
    /// Adresse du serveur multimédia, vue depuis le relais.
    pub media: String,
    pub seerr: Option<String>,
    pub tracearr: Option<String>,
    pub listen: SocketAddr,
}

impl Config {
    /// Lue dans l'environnement : `PLEX_URL`, `JELLYFIN_URL` ou `EMBY_URL` (un seul), `SEERR_URL`,
    /// `TRACEARR_URL`, et `MAKU_LISTEN` (par défaut `0.0.0.0:32700`).
    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let url = |key: &str| -> Result<Option<String>> {
            let Some(v) = get(key).map(|v| v.trim().trim_end_matches('/').to_owned()).filter(|v| !v.is_empty()) else {
                return Ok(None);
            };
            let parsed = reqwest::Url::parse(&v).with_context(|| format!("{key} : adresse invalide ({v})"))?;
            if !matches!(parsed.scheme(), "http" | "https") {
                bail!("{key} : l'adresse doit commencer par http:// ou https://");
            }
            Ok(Some(v))
        };
        let servers: Vec<(Kind, String)> = [(Kind::Plex, "PLEX_URL"), (Kind::Jellyfin, "JELLYFIN_URL"), (Kind::Emby, "EMBY_URL")]
            .into_iter()
            .filter_map(|(k, key)| url(key).transpose().map(|v| v.map(|v| (k, v))))
            .collect::<Result<_>>()?;
        let (kind, media) = match servers.len() {
            1 => servers.into_iter().next().unwrap(),
            0 => bail!("indiquez l'adresse du serveur : PLEX_URL, JELLYFIN_URL ou EMBY_URL"),
            _ => bail!("un seul serveur à la fois : PLEX_URL, JELLYFIN_URL ou EMBY_URL"),
        };
        let seerr = url("SEERR_URL")?;
        let tracearr = url("TRACEARR_URL")?;
        if seerr.is_none() && tracearr.is_none() {
            bail!("rien à relayer : indiquez SEERR_URL, TRACEARR_URL, ou les deux");
        }
        let listen = get("MAKU_LISTEN").unwrap_or_else(|| "0.0.0.0:32700".into());
        let listen = listen.parse().with_context(|| format!("MAKU_LISTEN : adresse invalide ({listen})"))?;
        Ok(Config { kind, media, seerr, tracearr, listen })
    }
}

pub struct App {
    pub cfg: Config,
    pub http: reqwest::Client,
    pub auth: auth::Cache,
}

pub type Shared = Arc<App>;

pub fn app(cfg: Config) -> Result<Router> {
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        // Les redirections sont rendues à Maku telles quelles.
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let state = Arc::new(App { cfg, http, auth: auth::Cache::default() });
    Ok(Router::new()
        .route("/", get(|| async { format!("Maku Server {VERSION}\n") }))
        .route("/maku/info", get(info))
        .fallback(proxy::forward)
        .with_state(state))
}

/// Ce que propose le relais, et l'identité du serveur multimédia (pour que Maku vérifie qu'il est
/// bien celui de son serveur).
async fn info(State(app): State<Shared>) -> Json<Value> {
    Json(json!({
        "app": "maku-server",
        "version": VERSION,
        "server": app.cfg.kind.name(),
        "id": auth::server_id(&app).await,
        "seerr": app.cfg.seerr.is_some(),
        "tracearr": app.cfg.tracearr.is_some(),
    }))
}

#[tokio::main]
async fn main() {
    let cfg = match Config::from_env(|k| std::env::var(k).ok()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Maku Server : {e:#}");
            std::process::exit(2);
        }
    };
    let listen = cfg.listen;
    println!(
        "Maku Server {VERSION} : {} {} ; Seerr {} ; Tracearr {} ; écoute sur {listen}",
        cfg.kind.name(),
        cfg.media,
        cfg.seerr.as_deref().unwrap_or("non"),
        cfg.tracearr.as_deref().unwrap_or("non"),
    );
    let router = match app(cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Maku Server : {e:#}");
            std::process::exit(1);
        }
    };
    let listener = match tokio::net::TcpListener::bind(listen).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Maku Server : impossible d'écouter sur {listen} : {e}");
            if e.kind() == std::io::ErrorKind::AddrInUse {
                eprintln!("Maku Server : ce port est déjà pris par un autre programme ; choisissez-en un autre avec MAKU_LISTEN (par exemple 0.0.0.0:32701).");
            }
            std::process::exit(1);
        }
    };
    let server = axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(shutdown());
    if let Err(e) = server.await {
        eprintln!("Maku Server : {e}");
        std::process::exit(1);
    }
}

/// Arrêt propre sur Ctrl+C, ou sur SIGTERM (`docker stop`).
async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
}

#[cfg(test)]
mod tests;
