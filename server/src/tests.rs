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

// ---------- Les trois serveurs, et ce que le relais répond quand quelque chose ne va pas ----------

use std::sync::atomic::{AtomicUsize, Ordering};

/// Faux Jellyfin ou Emby : `/System/Info` ne s'ouvre qu'avec le jeton « bon », donné comme chacun le donne
/// (Jellyfin : en-tête `Authorization` ; Emby : `X-Emby-Token`). Compte les vérifications reçues.
async fn fake_media(emby: bool) -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let count = hits.clone();
    let base = serve(
        Router::new()
            .route(
                "/System/Info",
                get(move |h: HeaderMap| {
                    let count = count.clone();
                    async move {
                        count.fetch_add(1, Ordering::SeqCst);
                        let given = if emby {
                            h.get("x-emby-token").and_then(|v| v.to_str().ok()).map(str::to_owned)
                        } else {
                            h.get("authorization").and_then(|v| v.to_str().ok()).map(str::to_owned)
                        };
                        let expected = if emby { "bon".to_owned() } else { "MediaBrowser Token=\"bon\"".to_owned() };
                        if given == Some(expected) {
                            StatusCode::OK
                        } else {
                            StatusCode::UNAUTHORIZED
                        }
                    }
                }),
            )
            .route("/System/Info/Public", get(|| async { Json(json!({ "Id": "serveur-1" })) })),
    )
    .await;
    (base, hits)
}

async fn relay_for(key: &'static str, media: String, seerr: Option<String>, tracearr: Option<String>) -> String {
    let cfg = Config::from_env(|k| match k {
        k if k == key => Some(media.clone()),
        "SEERR_URL" => seerr.clone(),
        "TRACEARR_URL" => tracearr.clone(),
        _ => None,
    })
    .unwrap();
    serve(app(cfg).unwrap()).await
}

async fn status(base: &str, path: &str, token: &str) -> u16 {
    reqwest::Client::new().get(format!("{base}{path}")).header("X-Maku-Token", token).send().await.unwrap().status().as_u16()
}

#[tokio::test]
async fn jellyfin_et_emby_verifient_le_jeton_a_leur_facon() {
    for (key, emby, name) in [("JELLYFIN_URL", false, "jellyfin"), ("EMBY_URL", true, "emby")] {
        let (media, _) = fake_media(emby).await;
        let base = relay_for(key, media, Some(fake_seerr().await), None).await;
        let info: Value = reqwest::get(format!("{base}/maku/info")).await.unwrap().json().await.unwrap();
        assert_eq!((info["server"].as_str(), info["id"].as_str()), (Some(name), Some("serveur-1")));
        assert_eq!(status(&base, "/seerr/api/v1/auth/me", "bon").await, 200, "{name}");
        assert_eq!(status(&base, "/seerr/api/v1/auth/me", "mauvais").await, 401, "{name}");
        assert_eq!(status(&base, "/seerr/api/v1/auth/me", "").await, 401, "{name}");
    }
}

#[tokio::test]
async fn tracearr_relaye_et_seerr_absent_introuvable() {
    let (media, _) = fake_media(false).await;
    // Seul Tracearr est configuré (le faux Seerr sert de faux Tracearr : il renvoie ce qu'il reçoit).
    let base = relay_for("JELLYFIN_URL", media, None, Some(fake_seerr().await)).await;
    let r = reqwest::Client::new().get(format!("{base}/tracearr/api/v1/stats?x=1")).header("X-Maku-Token", "bon").send().await.unwrap();
    assert_eq!(r.status(), 200);
    let seen: Value = r.json().await.unwrap();
    assert_eq!((seen["path"].as_str(), seen["query"].as_str()), (Some("/api/v1/stats"), Some("x=1")));
    assert_eq!(status(&base, "/seerr/api/v1/auth/me", "bon").await, 404);
}

#[tokio::test]
async fn jeton_verifie_une_fois_puis_garde() {
    let (media, hits) = fake_media(false).await;
    let base = relay_for("JELLYFIN_URL", media, Some(fake_seerr().await), None).await;
    // Accepté : le serveur multimédia n'est interrogé qu'une fois pour les appels suivants.
    for _ in 0..4 {
        assert_eq!(status(&base, "/seerr/api/v1/auth/me", "bon").await, 200);
    }
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    // Refusé : gardé aussi, pour ne pas le redemander à chaque essai.
    for _ in 0..3 {
        assert_eq!(status(&base, "/seerr/api/v1/auth/me", "mauvais").await, 401);
    }
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    // Un jeton démesuré est refusé sans même interroger le serveur.
    assert_eq!(status(&base, "/seerr/api/v1/auth/me", &"x".repeat(513)).await, 401);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn serveur_multimedia_injoignable_n_ouvre_rien() {
    // Rien n'écoute à cette adresse : impossible de vérifier le jeton, donc ni accepté ni refusé.
    let base = relay_for("PLEX_URL", "http://127.0.0.1:9".into(), Some(fake_seerr().await), None).await;
    assert_eq!(status(&base, "/seerr/api/v1/auth/me", "bon").await, 502);
    // Et la panne n'est pas gardée comme un refus : le jeton est revérifié à l'appel suivant.
    assert_eq!(status(&base, "/seerr/api/v1/auth/me", "bon").await, 502);
    let info: Value = reqwest::get(format!("{base}/maku/info")).await.unwrap().json().await.unwrap();
    assert!(info["id"].is_null());
}

#[tokio::test]
async fn service_injoignable_et_envoi_trop_gros() {
    let plex = fake_plex().await;
    let base = relay_for("PLEX_URL", plex.clone(), Some("http://127.0.0.1:9".into()), None).await;
    assert_eq!(status(&base, "/seerr/api/v1/auth/me", "bon").await, 502);

    let base = relay_for("PLEX_URL", plex, Some(fake_seerr().await), None).await;
    let big = reqwest::Client::new().post(format!("{base}/seerr/api/v1/request")).header("X-Maku-Token", "bon").body(vec![b'x'; 4 * 1024 * 1024 + 1]).send().await.unwrap();
    assert_eq!(big.status(), 413);
    let fits = reqwest::Client::new().post(format!("{base}/seerr/api/v1/request")).header("X-Maku-Token", "bon").body(vec![b'x'; 1024]).send().await.unwrap();
    assert_eq!(fits.status(), 200);
}

#[tokio::test]
async fn chemins_qui_sortent_de_l_api_refuses() {
    let (base, http) = relay().await;
    for path in ["/seerr/api/v1/%2E%2E/settings", "/seerr/api/v1/..%2fsettings", "/seerr/api/v1/a%5c..%5cb", "/seerr/api/v1/a%2Fb", "/seerr/api/v10/x", "/seerrx/api/v1/x", "/seerr/api/v2/x"] {
        let r = http.get(format!("{base}{path}")).header("X-Maku-Token", "bon").send().await.unwrap();
        assert_eq!(r.status(), 404, "{path}");
    }
    // Sans jeton valide, même un chemin correct ne dit rien du service.
    assert_eq!(http.get(format!("{base}/seerr/api/v1/status")).send().await.unwrap().status(), 401);
}

#[tokio::test]
async fn redirection_du_service_rendue_telle_quelle() {
    // Le service redirige ailleurs : le relais ne suit pas, il rend la redirection à Maku (qui a ses propres règles).
    let seerr = serve(Router::new().fallback(any(|| async { (StatusCode::FOUND, [("location", "http://ailleurs.invalid/")]) }))).await;
    let base = relay_for("PLEX_URL", fake_plex().await, Some(seerr), None).await;
    let http = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap();
    let r = http.get(format!("{base}/seerr/api/v1/auth/me")).header("X-Maku-Token", "bon").send().await.unwrap();
    assert_eq!(r.status(), 302);
    assert_eq!(r.headers()["location"], "http://ailleurs.invalid/");
}

