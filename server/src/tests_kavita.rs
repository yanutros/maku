//! Kavita par le relais, face à un faux Plex, un faux plex.tv, un faux Jellyfin et un faux Kavita.

use super::*;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::any;
use std::sync::Mutex;

async fn serve(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
    });
    format!("http://{addr}")
}

/// Jetons acceptés par le serveur : « serveur-ami » est celui qu'un ami reçoit pour ce serveur partagé (plex.tv le refuse).
const TOKENS: [&str; 3] = ["proprio", "ami", "serveur-ami"];

/// Faux Plex : les jetons « proprio » et « ami » ouvrent le serveur « abc123 ».
async fn fake_plex() -> String {
    serve(
        Router::new()
            .route(
                "/library/sections",
                get(|h: HeaderMap| async move {
                    let ok = h.get("x-plex-token").and_then(|t| t.to_str().ok()).is_some_and(|t| TOKENS.contains(&t));
                    if ok {
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

/// Faux plex.tv : « proprio » (compte 1) possède le serveur, « ami » (compte 42) y est invité.
async fn fake_plextv() -> String {
    let token = |h: &HeaderMap| h.get("x-plex-token").and_then(|t| t.to_str().ok()).unwrap_or_default().to_owned();
    serve(
        Router::new()
            .route(
                "/api/v2/user",
                get(move |h: HeaderMap| async move {
                    match token(&h).as_str() {
                        "proprio" => Json(json!({ "id": 1, "title": "Propriétaire" })).into_response(),
                        "ami" => Json(json!({ "id": 42, "title": "Élodie Lectrice", "username": "elo" })).into_response(),
                        "intrus" => Json(json!({ "id": 77, "title": "Intrus" })).into_response(),
                        _ => StatusCode::UNAUTHORIZED.into_response(),
                    }
                }),
            )
            .route(
                "/api/v2/resources",
                get(move |h: HeaderMap| async move {
                    match token(&h).as_str() {
                        t @ ("proprio" | "ami") => Json(json!([{ "clientIdentifier": "autre", "owned": true }, { "clientIdentifier": "abc123", "owned": t == "proprio" }])).into_response(),
                        // Un compte plex.tv qui n'a pas ce serveur.
                        "intrus" => Json(json!([{ "clientIdentifier": "autre", "owned": true }])).into_response(),
                        _ => StatusCode::UNAUTHORIZED.into_response(),
                    }
                }),
            ),
    )
    .await
}

/// Ce que le faux Kavita a reçu : (méthode, chemin et paramètres, clé ou jeton, corps).
type Seen = Arc<Mutex<Vec<(String, String, String, Value)>>>;

/// Faux Kavita : clé administrateur « cle-admin » ; le compte créé reçoit la clé « cle-ami » et la clé des images
/// « image-ami ».
async fn fake_kavita() -> (String, Seen) {
    let seen: Seen = Arc::default();
    let log = seen.clone();
    let url = serve(Router::new().fallback(any(move |req: Request| {
        let log = log.clone();
        async move {
            let h = req.headers().clone();
            let path = req.uri().path().to_owned();
            let target = req.uri().to_string();
            let key = h
                .get("x-api-key")
                .or_else(|| h.get("authorization"))
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
                .or_else(|| req.uri().query().and_then(|q| q.split('&').find_map(|p| p.strip_prefix("apiKey=").map(str::to_owned))))
                .unwrap_or_default();
            let method = req.method().to_string();
            let token_forwarded = h.contains_key("x-maku-token") || h.contains_key("x-maku-plex");
            let body = axum::body::to_bytes(req.into_body(), 1 << 20).await.unwrap();
            let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            log.lock().unwrap().push((method, target.clone(), key.clone(), body.clone()));
            let admin = key == "cle-admin";
            let reply = match path.as_str() {
                "/api/Library/libraries" if admin => json!([{ "id": 1, "name": "Mangas", "type": 0 }, { "id": 2, "name": "BD", "type": 1 }]),
                "/api/Account/invite" if admin => json!({
                    "emailLink": format!("http://kavita.interne:5000/registration/confirm-email?token=jeton%2Bsp%C3%A9cial&email={}", body["email"].as_str().unwrap_or("")),
                    "emailSent": false,
                }),
                "/api/Account/confirm-email" if body["token"] == "jeton+spécial" => json!({ "id": 7, "username": body["username"], "token": "jwt-7" }),
                "/api/Account/create-auth-key" if key == "Bearer jwt-7" => json!({ "id": 3, "key": "cle-ami", "name": body["name"] }),
                "/api/Account/update" if admin => Value::Null,
                "/api/Account/auth-keys" if key == "cle-ami" => json!([{ "key": "cle-ami" }, { "key": "image-ami" }]),
                "/api/Account/auth-keys" if admin => json!([{ "key": "cle-admin" }, { "key": "image-admin" }]),
                p if p.starts_with("/api/Series/") || p.starts_with("/api/image/") => {
                    if key.is_empty() {
                        return StatusCode::UNAUTHORIZED.into_response();
                    }
                    json!({ "path": p, "target": target, "key": key, "token": token_forwarded })
                }
                _ => return StatusCode::BAD_REQUEST.into_response(),
            };
            Json(reply).into_response()
        }
    })))
    .await;
    (url, seen)
}

fn data_dir() -> PathBuf {
    let mut n = [0u8; 8];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut n).unwrap();
    std::env::temp_dir().join(format!("maku-server-test-{}", n.iter().map(|b| format!("{b:02x}")).collect::<String>()))
}

async fn relay(media: &str, plextv: &str, kavita: &str, data: &std::path::Path, kind: &str) -> String {
    let mut cfg = Config::from_env(|k| match k {
        k if k == kind => Some(media.to_owned()),
        "KAVITA_URL" => Some(format!("{kavita}/")),
        "KAVITA_KEY" => Some("cle-admin".into()),
        "MAKU_DATA" => Some(data.display().to_string()),
        _ => None,
    })
    .unwrap();
    cfg.plextv = plextv.to_owned();
    serve(app(cfg).unwrap()).await
}

struct Bench {
    base: String,
    http: reqwest::Client,
    seen: Seen,
    data: PathBuf,
    plex: String,
    plextv: String,
    kavita: String,
}

async fn bench() -> Bench {
    let (plex, plextv, (kavita, seen)) = (fake_plex().await, fake_plextv().await, fake_kavita().await);
    let data = data_dir();
    let base = relay(&plex, &plextv, &kavita, &data, "PLEX_URL").await;
    Bench { base, http: reqwest::Client::new(), seen, data, plex, plextv, kavita }
}

impl Bench {
    async fn get(&self, path: &str, token: Option<&str>) -> reqwest::Response {
        let mut r = self.http.get(format!("{}{path}", self.base));
        if let Some(t) = token {
            r = r.header("X-Maku-Token", t);
        }
        r.send().await.unwrap()
    }

    async fn grant(&self, token: &str, id: &str, libraries: &[i64]) -> reqwest::Response {
        self.http
            .put(format!("{}/maku/kavita/admin/users/{id}", self.base))
            .header("X-Maku-Token", token)
            .json(&json!({ "name": "Élodie Lectrice", "libraries": libraries }))
            .send()
            .await
            .unwrap()
    }

    fn calls(&self, path: &str) -> Vec<(String, String, String, Value)> {
        self.seen.lock().unwrap().iter().filter(|c| c.1.starts_with(path)).cloned().collect()
    }
}

#[tokio::test]
async fn proprietaire_et_utilisateur_sans_acces() {
    let b = bench().await;
    let info: Value = b.get("/maku/info", None).await.json().await.unwrap();
    assert_eq!(info["kavita"], true);
    // Le propriétaire du serveur lit avec le compte administrateur de Kavita.
    let me: Value = b.get("/maku/kavita", Some("proprio")).await.json().await.unwrap();
    assert_eq!(me, json!({ "key": "cle-admin", "admin": true }));
    // Un invité sans bibliothèque ouverte : rien.
    assert_eq!(b.get("/maku/kavita", Some("ami")).await.status(), 403);
    assert_eq!(b.get("/maku/kavita", Some("inconnu")).await.status(), 401);
    assert_eq!(b.get("/maku/kavita", None).await.status(), 401);
    // L'administration : au propriétaire seulement.
    assert_eq!(b.get("/maku/kavita/admin", Some("ami")).await.status(), 403);
    assert_eq!(b.grant("ami", "plex:42", &[2]).await.status(), 403);
    let admin: Value = b.get("/maku/kavita/admin", Some("proprio")).await.json().await.unwrap();
    assert_eq!(admin["libraries"], json!([{ "id": 1, "name": "Mangas", "type": 0 }, { "id": 2, "name": "BD", "type": 1 }]));
    assert_eq!(admin["users"], json!([]));
    // Plex : Maku connaît déjà ses utilisateurs.
    assert_eq!(admin["members"], Value::Null);
    // Pas de compte pour soi-même, ni pour un utilisateur d'un autre type de serveur.
    assert_eq!(b.grant("proprio", "plex:1", &[1]).await.status(), 400);
    assert_eq!(b.grant("proprio", "jellyfin:42", &[1]).await.status(), 400);
    assert_eq!(b.grant("proprio", "plex:4%2F2", &[1]).await.status(), 400);
    // Rien d'ouvert, rien de créé.
    assert_eq!(b.grant("proprio", "plex:42", &[]).await.status(), 200);
    assert!(b.calls("/api/Account/invite").is_empty());
}

#[tokio::test]
async fn compte_cree_ouvert_puis_retire() {
    let b = bench().await;
    let r = b.grant("proprio", "plex:42", &[2, 2]).await;
    assert_eq!(r.status(), 200);
    let r: Value = r.json().await.unwrap();
    assert_eq!(r, json!({ "id": "plex:42", "name": "Élodie Lectrice", "username": "Elodie-Lectrice.42", "libraries": [2] }));
    // Invitation avec la clé administrateur, sans limite d'âge, puis activation et clé d'API pour Maku.
    let invite = &b.calls("/api/Account/invite")[0];
    assert_eq!(invite.2, "cle-admin");
    assert_eq!(invite.3["email"], "maku-plex-42@maku.invalid");
    assert_eq!((&invite.3["roles"], &invite.3["libraries"]), (&json!(["Login"]), &json!([2])));
    assert_eq!(invite.3["ageRestriction"]["ageRating"], -1);
    let confirm = &b.calls("/api/Account/confirm-email")[0];
    assert_eq!((confirm.2.as_str(), &confirm.3["username"]), ("", &json!("Elodie-Lectrice.42")));
    assert_eq!(confirm.3["password"].as_str().unwrap().len(), 48);
    assert_eq!(b.calls("/api/Account/create-auth-key")[0].2, "Bearer jwt-7");

    // L'utilisateur reçoit la clé de son compte.
    let me: Value = b.get("/maku/kavita", Some("ami")).await.json().await.unwrap();
    assert_eq!(me, json!({ "key": "cle-ami", "admin": false }));
    // Retenu dans le dossier de données : un relais redémarré le connaît.
    let saved: Value = serde_json::from_slice(&std::fs::read(b.data.join("kavita.json")).unwrap()).unwrap();
    assert_eq!(saved["users"]["plex:42"]["key"], "cle-ami");
    let again = relay(&b.plex, &b.plextv, &b.kavita, &b.data, "PLEX_URL").await;
    let admin: Value = b.http.get(format!("{again}/maku/kavita/admin")).header("X-Maku-Token", "proprio").send().await.unwrap().json().await.unwrap();
    assert_eq!(admin["users"], json!([{ "id": "plex:42", "name": "Élodie Lectrice", "username": "Elodie-Lectrice.42", "libraries": [2] }]));

    // Changer ses bibliothèques, puis tout retirer : le compte reste, l'accès part.
    assert_eq!(b.grant("proprio", "plex:42", &[1, 2]).await.status(), 200);
    assert_eq!(b.grant("proprio", "plex:42", &[]).await.status(), 200);
    let updates = b.calls("/api/Account/update");
    assert_eq!(updates.len(), 2);
    assert_eq!((&updates[0].3["userId"], &updates[0].3["libraries"]), (&json!(7), &json!([1, 2])));
    assert_eq!(updates[1].3["libraries"], json!([]));
    assert_eq!(updates[1].3["roles"], json!(["Login"]));
    assert_eq!(b.calls("/api/Account/invite").len(), 1, "un seul compte créé");
    assert_eq!(b.get("/maku/kavita", Some("ami")).await.status(), 403);
    let _ = std::fs::remove_dir_all(&b.data);
}

#[tokio::test]
async fn ami_plex_reconnu_par_son_compte() {
    let b = bench().await;
    b.grant("proprio", "plex:42", &[2]).await;
    let session = |server: &'static str, account: Option<&'static str>| {
        let mut r = b.http.get(format!("{}/maku/kavita", b.base)).header("X-Maku-Token", server);
        if let Some(a) = account {
            r = r.header("X-Maku-Plex", a);
        }
        r.send()
    };
    // Le jeton du serveur partagé ouvre le serveur, pas plex.tv : seul, il ne dit pas qui l'on est.
    assert_eq!(session("serveur-ami", None).await.unwrap().status(), 401);
    // Avec le jeton de son compte : reconnu, et sa clé lui revient.
    let me: Value = session("serveur-ami", Some("ami")).await.unwrap().json().await.unwrap();
    assert_eq!(me, json!({ "key": "cle-ami", "admin": false }));
    // Un compte plex.tv qui n'a pas ce serveur ne se fait pas passer pour un utilisateur.
    assert_eq!(session("serveur-ami", Some("intrus")).await.unwrap().status(), 401);
    assert_eq!(session("serveur-ami", Some("inconnu")).await.unwrap().status(), 401);
    // Ni l'un ni l'autre jeton ne part vers Kavita.
    let r = b.http.get(format!("{}/kavita/api/Series/5", b.base)).header("X-Maku-Token", "serveur-ami").header("X-Maku-Plex", "ami").header("x-api-key", "cle-ami").send().await.unwrap();
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["token"], false);
    let _ = std::fs::remove_dir_all(&b.data);
}

#[tokio::test]
async fn api_et_images_de_kavita() {
    let b = bench().await;
    b.grant("proprio", "plex:42", &[2]).await;
    // L'API : avec le jeton du serveur, qui reste au relais ; Kavita vérifie la clé.
    let r = b.http.get(format!("{}/kavita/api/Series/5?x=1", b.base)).header("X-Maku-Token", "ami").header("x-api-key", "cle-ami").send().await.unwrap();
    assert_eq!(r.status(), 200);
    let v: Value = r.json().await.unwrap();
    assert_eq!((&v["path"], &v["key"], &v["token"]), (&json!("/api/Series/5"), &json!("cle-ami"), &json!(false)));
    assert_eq!(b.http.get(format!("{}/kavita/api/Series/5", b.base)).header("X-Maku-Token", "inconnu").header("x-api-key", "cle-ami").send().await.unwrap().status(), 401);
    // Les images, chargées sans en-tête : seulement avec une clé vue auprès d'un utilisateur reconnu.
    let image = format!("{}/kavita/api/image/series-cover?seriesId=5&apiKey=image-ami", b.base);
    let fresh = relay(&b.plex, &b.plextv, &b.kavita, &data_dir(), "PLEX_URL").await;
    let cold = format!("{fresh}/kavita/api/image/series-cover?seriesId=5&apiKey=image-ami");
    assert_eq!(b.http.get(&cold).send().await.unwrap().status(), 401, "clé jamais vue par ce relais");
    b.get("/maku/kavita", Some("ami")).await;
    assert_eq!(b.http.get(&image).send().await.unwrap().status(), 200);
    assert_eq!(b.http.get(format!("{}/kavita/api/image/x?apiKey=autre", b.base)).send().await.unwrap().status(), 401);
    // Ni le site de Kavita, ni un chemin qui sort de l'API.
    for path in ["/kavita/", "/kavita/login", "/kavita/api/../registration", "/kavita/api/%2e%2e/x"] {
        assert_eq!(b.get(path, Some("ami")).await.status(), 404, "{path}");
    }
    let _ = std::fs::remove_dir_all(&b.data);
}

#[tokio::test]
async fn cle_utilisee_avec_le_jeton_ouvre_ses_images() {
    let b = bench().await;
    b.grant("proprio", "plex:42", &[2]).await;
    // Relais redémarré : la page charge ses images avec la clé des livres, que seul un appel de l'API fait connaître.
    let fresh = relay(&b.plex, &b.plextv, &b.kavita, &b.data, "PLEX_URL").await;
    let image = format!("{fresh}/kavita/api/image/x?apiKey=image-ami");
    assert_eq!(b.http.get(&image).send().await.unwrap().status(), 401);
    b.http.get(format!("{fresh}/kavita/api/Series/1")).header("X-Maku-Token", "ami").header("x-api-key", "cle-ami").send().await.unwrap();
    let mut ok = false;
    for _ in 0..50 {
        if b.http.get(&image).send().await.unwrap().status() == 200 {
            ok = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(ok, "la clé des images n'a pas été apprise");
    let _ = std::fs::remove_dir_all(&b.data);
}

/// Faux Jellyfin : « admin » administre le serveur, « lecteur » non.
async fn fake_jellyfin() -> String {
    let token = |h: &HeaderMap| {
        h.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.split('"').nth(1)).unwrap_or_default().to_owned()
    };
    serve(
        Router::new()
            .route(
                "/System/Info",
                get(move |h: HeaderMap| async move {
                    if ["admin", "lecteur"].contains(&token(&h).as_str()) {
                        StatusCode::OK
                    } else {
                        StatusCode::UNAUTHORIZED
                    }
                }),
            )
            .route("/System/Info/Public", get(|| async { Json(json!({ "Id": "jf1" })) }))
            .route(
                "/Users",
                get(move |h: HeaderMap| async move {
                    if token(&h) != "admin" {
                        return StatusCode::FORBIDDEN.into_response();
                    }
                    Json(json!([{ "Id": "a1b2", "Name": "Admin" }, { "Id": "c3d4e5f6a7b8", "Name": "Lecteur" }])).into_response()
                }),
            )
            .route(
                "/Users/Me",
                get(move |h: HeaderMap| async move {
                    match token(&h).as_str() {
                        "admin" => Json(json!({ "Id": "a1b2", "Name": "Admin", "Policy": { "IsAdministrator": true } })).into_response(),
                        "lecteur" => Json(json!({ "Id": "c3d4e5f6a7b8", "Name": "Lecteur", "Policy": { "IsAdministrator": false } })).into_response(),
                        _ => StatusCode::UNAUTHORIZED.into_response(),
                    }
                }),
            ),
    )
    .await
}

#[tokio::test]
async fn jellyfin_administrateur_et_utilisateur() {
    let (jellyfin, (kavita, _seen)) = (fake_jellyfin().await, fake_kavita().await);
    let data = data_dir();
    let base = relay(&jellyfin, "http://127.0.0.1:9", &kavita, &data, "JELLYFIN_URL").await;
    let http = reqwest::Client::new();
    let me: Value = http.get(format!("{base}/maku/kavita")).header("X-Maku-Token", "admin").send().await.unwrap().json().await.unwrap();
    assert_eq!(me["admin"], true);
    assert_eq!(http.get(format!("{base}/maku/kavita")).header("X-Maku-Token", "lecteur").send().await.unwrap().status(), 403);
    // Les utilisateurs de Jellyfin, pour l'administration (sauf l'administrateur lui-même).
    let admin: Value = http.get(format!("{base}/maku/kavita/admin")).header("X-Maku-Token", "admin").send().await.unwrap().json().await.unwrap();
    assert_eq!(admin["members"], json!([{ "id": "jellyfin:c3d4e5f6a7b8", "name": "Lecteur" }]));
    let r = http
        .put(format!("{base}/maku/kavita/admin/users/jellyfin:c3d4e5f6a7b8"))
        .header("X-Maku-Token", "admin")
        .json(&json!({ "name": "Lecteur", "libraries": [1] }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let me: Value = http.get(format!("{base}/maku/kavita")).header("X-Maku-Token", "lecteur").send().await.unwrap().json().await.unwrap();
    assert_eq!(me, json!({ "key": "cle-ami", "admin": false }));
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn configuration_de_kavita() {
    let env = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
    // Kavita seul suffit ; sa clé est obligatoire, et propre.
    let c = Config::from_env(env(&[("PLEX_URL", "http://p"), ("KAVITA_URL", "http://k:5000/"), ("KAVITA_KEY", "abc-123")])).unwrap();
    assert_eq!(c.kavita, Some(("http://k:5000".into(), "abc-123".into())));
    assert_eq!(c.data, PathBuf::from("/data"));
    assert!(Config::from_env(env(&[("PLEX_URL", "http://p"), ("KAVITA_URL", "http://k")])).is_err());
    assert!(Config::from_env(env(&[("PLEX_URL", "http://p"), ("KAVITA_KEY", "abc")])).is_err());
    assert!(Config::from_env(env(&[("PLEX_URL", "http://p"), ("KAVITA_URL", "http://k"), ("KAVITA_KEY", "a&b")])).is_err());
    let c = Config::from_env(env(&[("PLEX_URL", "http://p"), ("SEERR_URL", "http://s"), ("MAKU_DATA", "/srv/maku")])).unwrap();
    assert_eq!((c.kavita, c.data), (None, PathBuf::from("/srv/maku")));
}

#[test]
fn noms_et_identifiants() {
    use crate::kavita::{username_for, valid_user_id};
    assert_eq!(username_for("Élodie Lectrice", "plex:42"), "Elodie-Lectrice.42");
    assert_eq!(username_for("  ", "jellyfin:0a1b-2c3d4e5f"), "lecteur.3d4e5f");
    assert_eq!(username_for("李 明", "emby:9"), "lecteur.9");
    assert!(valid_user_id(Kind::Plex, "plex:42"));
    assert!(valid_user_id(Kind::Jellyfin, "jellyfin:0a1b-2c3d"));
    assert!(!valid_user_id(Kind::Plex, "jellyfin:42"));
    assert!(!valid_user_id(Kind::Plex, "plex:"));
    assert!(!valid_user_id(Kind::Plex, "plex:4/2"));
    assert!(!valid_user_id(Kind::Plex, "plex:../x"));
}
