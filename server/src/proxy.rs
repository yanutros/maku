//! Transmission des appels à Seerr ou Tracearr, une fois l'utilisateur reconnu.

use crate::auth::{self, Verdict};
use crate::Shared;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_util::TryStreamExt;
use std::net::SocketAddr;

/// En-tête qui porte le jeton de l'utilisateur (jamais transmis plus loin).
pub const TOKEN_HEADER: &str = "x-maku-token";
/// Taille maximale d'un envoi (les demandes, réglages et connexions sont tout petits).
const MAX_BODY: usize = 4 * 1024 * 1024;

/// En-têtes propres à chaque connexion, ou que le relais refait lui-même.
const DROPPED: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "proxy-connection",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "host",
    "content-length",
    TOKEN_HEADER,
    "origin",
    "referer",
    "forwarded",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-forwarded-proto",
    "x-real-ip",
];

fn error(status: StatusCode, message: &str) -> Response {
    (status, axum::Json(serde_json::json!({ "error": message }))).into_response()
}

/// `/seerr/api/v1/…` ou `/tracearr/api/v1/…` → adresse de base du service, et chemin à appeler.
/// Rien d'autre que l'API, et pas de chemin qui remonte d'un dossier (`..`, même encodé).
pub fn target<'a>(app: &'a crate::App, path: &'a str) -> Option<(&'a str, &'a str)> {
    let (base, rest) = if let Some(rest) = path.strip_prefix("/seerr") {
        (app.cfg.seerr.as_deref()?, rest)
    } else if let Some(rest) = path.strip_prefix("/tracearr") {
        (app.cfg.tracearr.as_deref()?, rest)
    } else {
        return None;
    };
    let api = rest == "/api/v1" || rest.starts_with("/api/v1/");
    let lower = rest.to_ascii_lowercase();
    let escapes = lower.contains("..") || lower.contains("%2e") || lower.contains('\\') || lower.contains("%5c") || lower.contains("%2f");
    (api && !escapes).then_some((base, rest))
}

pub async fn forward(State(app): State<Shared>, ConnectInfo(peer): ConnectInfo<SocketAddr>, req: Request) -> Response {
    let path = req.uri().path().to_owned();
    let Some((base, rest)) = target(&app, &path) else {
        return error(StatusCode::NOT_FOUND, "not found");
    };
    let token = req.headers().get(TOKEN_HEADER).and_then(|v| v.to_str().ok()).unwrap_or_default().to_owned();
    match auth::check(&app, &token).await {
        Verdict::Allowed => {}
        Verdict::Denied => return error(StatusCode::UNAUTHORIZED, "not a user of this server"),
        Verdict::Unavailable => return error(StatusCode::BAD_GATEWAY, "media server unreachable"),
    }

    let query = req.uri().query().map(|q| format!("?{q}")).unwrap_or_default();
    let url = format!("{base}{rest}{query}");
    let method = req.method().clone();
    let had_origin = req.headers().contains_key(header::ORIGIN);
    let mut headers = HeaderMap::new();
    for (name, value) in req.headers() {
        if !DROPPED.contains(&name.as_str()) {
            headers.append(name.clone(), value.clone());
        }
    }
    // Le service vérifie parfois l'origine des requêtes qui ouvrent une session (Tracearr) : c'est
    // désormais la sienne.
    if had_origin {
        if let Some(origin) = reqwest::Url::parse(base).ok().map(|u| u.origin().ascii_serialization()) {
            if let Ok(v) = HeaderValue::from_str(&origin) {
                headers.insert(header::ORIGIN, v);
            }
        }
    }
    if let Ok(v) = HeaderValue::from_str(&peer.ip().to_string()) {
        headers.insert(HeaderName::from_static("x-forwarded-for"), v);
    }
    let body = match axum::body::to_bytes(req.into_body(), MAX_BODY).await {
        Ok(b) => b,
        Err(_) => return error(StatusCode::PAYLOAD_TOO_LARGE, "request too large"),
    };

    let mut upstream = app.http.request(method, &url).headers(headers);
    if !body.is_empty() {
        upstream = upstream.body(body);
    }
    let resp = match upstream.send().await {
        Ok(r) => r,
        Err(_) => return error(StatusCode::BAD_GATEWAY, "service unreachable"),
    };

    let mut out = Response::builder().status(resp.status().as_u16());
    for (name, value) in resp.headers() {
        let n = name.as_str();
        if DROPPED.contains(&n) {
            continue;
        }
        if n == "set-cookie" {
            // Le cookie appartient désormais à l'adresse du relais : sans domaine imposé.
            if let Some(v) = value.to_str().ok().map(strip_domain).and_then(|v| HeaderValue::from_str(&v).ok()) {
                out = out.header(name, v);
            }
            continue;
        }
        out = out.header(name, value);
    }
    let stream = resp.bytes_stream().map_err(std::io::Error::other);
    out.body(Body::from_stream(stream)).unwrap_or_else(|_| error(StatusCode::BAD_GATEWAY, "bad response"))
}

/// Retire l'attribut `Domain=…` d'un `Set-Cookie`.
pub fn strip_domain(cookie: &str) -> String {
    cookie
        .split(';')
        .filter(|part| !part.trim_start().to_ascii_lowercase().starts_with("domain="))
        .collect::<Vec<_>>()
        .join(";")
}
