//! Relais complet face à un faux serveur Plex et un faux Seerr, sur la machine.

use super::*;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::any;

async fn serve(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
    });
    format!("http://{addr}")
}

/// Faux Plex : seul le jeton « bon » ouvre la liste des bibliothèques.
async fn fake_plex() -> String {
    serve(
        Router::new()
            .route(
                "/library/sections",
                get(|h: HeaderMap| async move {
                    if h.get("x-plex-token").is_some_and(|t| t == "bon") {
                        StatusCode::OK
                    } else {
                        StatusCode::UNAUTHORIZED
                    }
                }),
            )
            .route("/identity", get(|| async { Json(json!({ "MediaContainer": { "machineIdentifier": "abc123" } })) })),
    )
    .await
}

/// Faux Seerr : renvoie ce qu'il a reçu, et pose un cookie de session lié à son domaine.
async fn fake_seerr() -> String {
    serve(Router::new().fallback(any(|req: axum::extract::Request| async move {
        let h = req.headers();
        let seen = json!({
            "method": req.method().as_str(),
            "path": req.uri().path(),
            "query": req.uri().query(),
            "cookie": h.get("cookie").and_then(|v| v.to_str().ok()),
            "origin": h.get("origin").and_then(|v| v.to_str().ok()),
            "token": h.get("x-maku-token").is_some(),
            "forwarded": h.get("x-forwarded-for").and_then(|v| v.to_str().ok()),
        });
        let body = axum::body::to_bytes(req.into_body(), 1 << 20).await.unwrap();
        let mut seen = seen;
        seen["body"] = json!(String::from_utf8_lossy(&body));
        ([("set-cookie", "connect.sid=s1; Path=/; Domain=seerr.local; HttpOnly")], Json(seen)).into_response()
    })))
    .await
}

async fn relay() -> (String, reqwest::Client) {
    let plex = fake_plex().await;
    let seerr = fake_seerr().await;
    let cfg = Config::from_env(|k| match k {
        "PLEX_URL" => Some(plex.clone()),
        "SEERR_URL" => Some(format!("{seerr}/")),
        _ => None,
    })
    .unwrap();
    (serve(app(cfg).unwrap()).await, reqwest::Client::new())
}

#[tokio::test]
async fn info_sans_jeton() {
    let (base, http) = relay().await;
    let v: Value = http.get(format!("{base}/maku/info")).send().await.unwrap().json().await.unwrap();
    assert_eq!(v["server"], "plex");
    assert_eq!(v["id"], "abc123");
    assert_eq!((v["seerr"].as_bool(), v["tracearr"].as_bool()), (Some(true), Some(false)));
}

#[tokio::test]
async fn refuse_sans_bon_jeton() {
    let (base, http) = relay().await;
    let none = http.get(format!("{base}/seerr/api/v1/auth/me")).send().await.unwrap();
    assert_eq!(none.status(), 401);
    let bad = http.get(format!("{base}/seerr/api/v1/auth/me")).header("X-Maku-Token", "mauvais").send().await.unwrap();
    assert_eq!(bad.status(), 401);
}

#[tokio::test]
async fn transmet_avec_bon_jeton() {
    let (base, http) = relay().await;
    let r = http
        .post(format!("{base}/seerr/api/v1/request?x=1"))
        .header("X-Maku-Token", "bon")
        .header("Cookie", "connect.sid=s0")
        .header("Origin", "http://relais.example")
        .body(r#"{"mediaId":1}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let cookie = r.headers()["set-cookie"].to_str().unwrap().to_owned();
    assert!(!cookie.to_lowercase().contains("domain"), "{cookie}");
    assert!(cookie.starts_with("connect.sid=s1"));
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["method"], "POST");
    assert_eq!(v["path"], "/api/v1/request");
    assert_eq!(v["query"], "x=1");
    assert_eq!(v["cookie"], "connect.sid=s0");
    assert_eq!(v["body"], r#"{"mediaId":1}"#);
    // Le jeton reste au relais ; l'origine devient celle du service.
    assert_eq!(v["token"], false);
    assert!(v["origin"].as_str().unwrap().starts_with("http://127.0.0.1:"));
    assert_eq!(v["forwarded"], "127.0.0.1");
}

#[tokio::test]
async fn seulement_l_api() {
    let (base, http) = relay().await;
    for path in ["/seerr/", "/seerr/login", "/seerr/api/v1/../../etc", "/seerr/api/v1/%2e%2e/settings", "/tracearr/api/v1/auth/me", "/autre"] {
        let r = http.get(format!("{base}{path}")).header("X-Maku-Token", "bon").send().await.unwrap();
        assert_eq!(r.status(), 404, "{path}");
    }
}

#[test]
fn configuration() {
    let env = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(a, _)| *a == k).map(|(_, v)| v.to_string());
    assert!(Config::from_env(env(&[("SEERR_URL", "http://s")])).is_err());
    assert!(Config::from_env(env(&[("PLEX_URL", "http://p")])).is_err());
    assert!(Config::from_env(env(&[("PLEX_URL", "http://p"), ("JELLYFIN_URL", "http://j"), ("SEERR_URL", "http://s")])).is_err());
    assert!(Config::from_env(env(&[("EMBY_URL", "ftp://e"), ("SEERR_URL", "http://s")])).is_err());
    let c = Config::from_env(env(&[("JELLYFIN_URL", "http://j:8096/"), ("TRACEARR_URL", "http://t:3000")])).unwrap();
    assert_eq!((c.kind, c.media.as_str(), c.listen.port()), (Kind::Jellyfin, "http://j:8096", 32700));
}

#[test]
fn domaine_retire() {
    assert_eq!(proxy::strip_domain("a=1; Path=/; Domain=x.y; Secure"), "a=1; Path=/; Secure");
}
