// Une commande Tauri a un paramètre par valeur que la page lui passe : certaines en ont huit ou neuf.
#![allow(clippy::too_many_arguments)]

mod cast;
mod config;
mod error;
mod i18n;
mod jellyfin;
mod allocine;
mod lyrics;
mod maku_server;
mod media;
mod net;
#[cfg(target_os = "linux")]
mod mpris;
mod player;
mod plex;
mod plex_admin;
mod relay;
mod offline;
mod overlay;
mod plex_users;
mod seerr;
mod tracearr;
mod tracks;
mod update;

use anyhow::anyhow;
use config::{Config, SavedServer};
use error::CmdResult;
use player::{Entry, PlayerControl, Source};
use plex::{Adjacent, Genre, Hub, Item, WebStream, ItemDetail, LoginPin, Page, PlexServer, QueueTrack, Section};
use plex_admin::{Account, EditInfo, FieldChange, ImageChoice, MatchResult, Pref, ServerSession};
use std::collections::HashMap;
use seerr::{ArrOptions, ArrServer, RequestChanges, RequestCounts, RequestPage, Seerr, SeerrDetail, SeerrMedia, SeerrSeason, Slider};
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tracearr::{Dashboard, LoginState, Tracearr};

struct AppState {
    /// Linux : la fenêtre a été créée transparente, et passe par-dessus celle de mpv pendant un film (voir `overlay`).
    overlay: bool,
    http: reqwest::Client,
    config_path: PathBuf,
    config: Mutex<Config>,
    seerr: tokio::sync::Mutex<Option<Arc<Seerr>>>,
    tracearr: tokio::sync::Mutex<Option<Arc<Tracearr>>>,
    /// Le serveur a changé : les sessions Seerr et Tracearr ouvertes sont celles de l'ancien.
    seerr_stale: std::sync::atomic::AtomicBool,
    tracearr_stale: std::sync::atomic::AtomicBool,
    player: Mutex<Option<PlayerControl>>,
    /// Le compte Plex connecté possède-t-il le serveur en cours ? Dernière réponse de plex.tv.
    owner: Mutex<OwnerCache>,
    /// Lecture en préparation : une lecture arrêtée ou remplacée pendant l'attente du serveur ne démarre pas.
    play_turn: commands::PlayTurn,
    relay: Mutex<Option<relay::Relay>>,
    /// Logos trouvés (ou non) sur le service en ligne de Plex, par identifiant.
    remote_logos: Mutex<std::collections::HashMap<String, Option<String>>>,
    /// Requêtes de logos en ligne simultanées.
    logo_slots: tokio::sync::Semaphore,
    /// « Changer de serveur » : retour à l'écran de choix Plex / Jellyfin, connexions gardées.
    choosing: std::sync::atomic::AtomicBool,
    /// Téléchargements pour la lecture hors ligne (offline.rs).
    offline: Arc<offline::Offline>,
    /// Téléchargements longs : pas de délai maximal pour la requête entière, contrairement à `http`.
    download_http: reqwest::Client,
    /// Diffusion en cours sur un appareil Chromecast (cast.rs).
    cast: std::sync::Mutex<Option<cast::Session>>,
    /// Relais Maku Server déjà interrogés (maku_server.rs).
    relays: maku_server::Known,
}

impl AppState {
    fn cfg(&self) -> Config {
        self.config.lock().unwrap().clone()
    }

    fn update_cfg(&self, f: impl FnOnce(&mut Config)) -> anyhow::Result<()> {
        let mut config = self.config.lock().unwrap();
        let before = config.services_key();
        f(&mut config);
        // Chaque serveur a ses propres Seerr et Tracearr.
        if config.services_key() != before {
            config.switch_services(before);
            self.seerr_stale.store(true, std::sync::atomic::Ordering::Relaxed);
            self.tracearr_stale.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        config.save(&self.config_path)
    }

    /// Serveur de médias choisi : Plex ou Jellyfin (voir media.rs).
    fn server(&self) -> anyhow::Result<media::MediaServer> {
        let c = self.cfg();
        // Son et sous-titres préférés : appliqués par le client du serveur, là où il lit une fiche.
        let tracks = std::sync::Arc::new(tracks::Prefs::from_config(&c, crate::i18n::language()));
        let s = c.server.ok_or_else(|| anyhow!(crate::i18n::tr("app.noServer")))?;
        Ok(if s.is_jellyfin() {
            media::MediaServer::Jellyfin(jellyfin::JellyfinServer {
                http: self.http.clone(),
                base: s.uri,
                token: s.token,
                user_id: s.user_id.unwrap_or_default(),
                client_id: c.client_id,
                tracks,
            })
        } else {
            media::MediaServer::Plex(PlexServer { http: self.http.clone(), base: s.uri, token: s.token, client_id: c.client_id, tracks })
        })
    }

    /// Serveur Plex, pour ce qui n'existe que chez lui (lecteur mpv, réglages du serveur, édition).
    fn plex(&self) -> anyhow::Result<PlexServer> {
        self.server()?.plex()
    }

    /// Le compte Plex connecté possède-t-il le serveur en cours ? D'après plex.tv (« owned ») : les
    /// membres d'un Plex Home lisent eux aussi les réglages du serveur, ce n'est donc pas un critère.
    ///
    /// La réponse est gardée cinq minutes par compte et par serveur : chaque action d'administration, et le tableau
    /// de bord toutes les trois secondes, redemandaient la liste des serveurs à plex.tv. Si plex.tv ne répond pas,
    /// la dernière réponse connue vaut encore : un serveur local reste administrable sans Internet.
    async fn owns_plex_server(&self) -> bool {
        let c = self.cfg();
        let (Some(token), Some(saved)) = (c.plex_token, c.server) else { return false };
        if saved.is_jellyfin() {
            return false;
        }
        let key = format!("{}|{token}", saved.machine_id);
        if let Some(owned) = self.owner.lock().unwrap().fresh(&key, std::time::Instant::now()) {
            return owned;
        }
        match plex::servers(&self.http, &c.client_id, &token).await {
            Ok(list) => {
                let owned = list.into_iter().find(|r| r.client_identifier == saved.machine_id).is_some_and(|r| r.owned);
                self.owner.lock().unwrap().put(key, owned, std::time::Instant::now());
                owned
            }
            Err(_) => self.owner.lock().unwrap().last(&key).unwrap_or(false),
        }
    }

    /// Le serveur Plex, pour une action réservée à son propriétaire (fiches, collections, réglages…).
    async fn admin_plex(&self) -> anyhow::Result<PlexServer> {
        if !self.owns_plex_server().await {
            return Err(anyhow!(crate::i18n::tr("admin.forbidden")));
        }
        self.plex()
    }

    /// Ce qu'il faut joindre aux appels vers `url`, quand elle passe par le relais du serveur (Maku Server ou le
    /// plugin Maku de Jellyfin).
    ///
    /// Erreur si un relais répond à cette adresse sans être celui du serveur (sa clé n'est pas celle retenue) : le
    /// jeton n'y part pas, et l'appel non plus.
    async fn relay_token(&self, url: &str) -> anyhow::Result<Option<maku_server::Relay>> {
        let c = self.cfg();
        let client_id = c.client_id.clone();
        let server_key = c.server.as_ref().map(config::SavedServer::key);
        let pinned = server_key.as_ref().and_then(|k| c.relay_keys.get(k)).map(String::as_str);
        let decision = self
            .relays
            .token_for(&self.http, url, c.server.as_ref(), pinned, |token| jellyfin::authorization(&client_id, Some(token)))
            .await;
        match decision {
            maku_server::Decision::Direct => Ok(None),
            maku_server::Decision::Send(relay, pin) => {
                if let (Some(key), Some(server_key)) = (pin, server_key) {
                    self.update_cfg(|c| void(c.relay_keys.insert(server_key, key)))?;
                }
                Ok(Some(relay))
            }
            maku_server::Decision::Refused => Err(anyhow!(crate::i18n::tr("app.relayUntrusted"))),
        }
    }

    /// L'utilisateur vient de donner ou de retirer lui-même une adresse de Seerr ou de Tracearr : la clé retenue pour
    /// le relais de ce serveur est oubliée, et le prochain relais rencontré sera retenu à sa place (relais réinstallé
    /// avec une autre clé).
    fn forget_relay_key(&self) -> anyhow::Result<()> {
        self.relays.forget();
        let Some(key) = self.cfg().server.as_ref().map(config::SavedServer::key) else { return Ok(()) };
        if self.cfg().relay_keys.contains_key(&key) {
            self.update_cfg(|c| void(c.relay_keys.remove(&key)))?;
        }
        Ok(())
    }

    /// Session Seerr ouverte à la demande, puis réutilisée.
    async fn seerr(&self) -> anyhow::Result<Arc<Seerr>> {
        let mut slot = self.seerr.lock().await;
        if self.seerr_stale.swap(false, std::sync::atomic::Ordering::Relaxed) {
            *slot = None;
        }
        if let Some(s) = slot.as_ref() {
            return Ok(s.clone());
        }
        let c = self.cfg();
        let url = c.seerr_url.ok_or_else(|| anyhow!(crate::i18n::tr("app.seerrNotConfigured")))?;
        // La session Seerr d'un compte Jellyfin ne sert qu'avec le serveur Jellyfin.
        let cookies = c.seerr_cookies.as_deref().filter(|_| c.server.as_ref().is_some_and(|s| s.is_jellyfin()));
        let relay = self.relay_token(&url).await?;
        let s = Arc::new(Seerr::connect(&url, c.plex_token.clone(), cookies, relay).await?);
        *slot = Some(s.clone());
        Ok(s)
    }

    /// Client Tracearr, recréé depuis la configuration (adresse et cookies de session).
    async fn tracearr(&self) -> anyhow::Result<Arc<Tracearr>> {
        let mut slot = self.tracearr.lock().await;
        if self.tracearr_stale.swap(false, std::sync::atomic::Ordering::Relaxed) {
            *slot = None;
        }
        if let Some(t) = slot.as_ref() {
            return Ok(t.clone());
        }
        let c = self.cfg();
        let url = c.tracearr_url.ok_or_else(|| anyhow!(crate::i18n::tr("app.tracearrNotConfigured")))?;
        let relay = self.relay_token(&url).await?;
        let t = Arc::new(Tracearr::new(&url, c.tracearr_cookies.as_deref(), relay)?);
        *slot = Some(t.clone());
        Ok(t)
    }

    /// Conserve les cookies de session Tracearr (renouvelés au fil des requêtes).
    fn save_tracearr_cookies(&self, t: &Tracearr) -> anyhow::Result<()> {
        let cookies = t.saved_cookies();
        if cookies != self.cfg().tracearr_cookies {
            self.update_cfg(|c| c.tracearr_cookies = cookies)?;
        }
        Ok(())
    }
}

/// Pour une fermeture qui ne rend rien, autour d'un appel qui rend quelque chose.
fn void<T>(_: T) {}

/// Dernière réponse de plex.tv à « ce compte possède-t-il ce serveur ? », pour un compte et un serveur.
#[derive(Default)]
struct OwnerCache(Option<(String, bool, std::time::Instant)>);

impl OwnerCache {
    const FRESH_FOR: Duration = Duration::from_secs(300);

    /// La réponse, si elle a moins de cinq minutes et vaut pour ce compte et ce serveur.
    fn fresh(&self, key: &str, now: std::time::Instant) -> Option<bool> {
        self.0.as_ref().filter(|(k, _, at)| k == key && now.duration_since(*at) < Self::FRESH_FOR).map(|(_, owned, _)| *owned)
    }

    /// La dernière réponse pour ce compte et ce serveur, quel que soit son âge (plex.tv injoignable).
    fn last(&self, key: &str) -> Option<bool> {
        self.0.as_ref().filter(|(k, _, _)| k == key).map(|(_, owned, _)| *owned)
    }

    fn put(&mut self, key: String, owned: bool, now: std::time::Instant) {
        self.0 = Some((key, owned, now));
    }
}

mod commands;
use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Linux, réglage « vidéo dans Maku » activé : la fenêtre est créée transparente, pour passer par-dessus celle de
    // mpv pendant un film (voir `overlay`).
    let mut context = tauri::generate_context!();
    let overlay = overlay::wanted(&context.config().identifier);
    if overlay {
        if let Some(window) = context.config_mut().app.windows.first_mut() {
            window.transparent = true;
        }
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let config_path = app.path().app_config_dir()?.join("config.json");
            let config = Config::load(&config_path);
            config.save(&config_path)?;
            app.manage(AppState {
                http: reqwest::Client::builder().timeout(Duration::from_secs(30)).redirect(net::same_host()).build()?,
                config_path,
                config: Mutex::new(config),
                seerr: tokio::sync::Mutex::new(None),
                tracearr: tokio::sync::Mutex::new(None),
                seerr_stale: Default::default(),
                tracearr_stale: Default::default(),
                player: Mutex::new(None),
                owner: Default::default(),
                play_turn: Default::default(),
                relay: Mutex::new(None),
                remote_logos: Mutex::new(Default::default()),
                logo_slots: tokio::sync::Semaphore::new(4),
                choosing: Default::default(),
                offline: Arc::new(offline::Offline::new(app.path().app_local_data_dir()?.join("offline"))),
                download_http: reqwest::Client::builder()
                    .connect_timeout(Duration::from_secs(20))
                    .read_timeout(Duration::from_secs(90))
                    .redirect(net::same_host())
                    .build()?,
                cast: std::sync::Mutex::new(None),
                relays: maku_server::Known::default(),
                overlay,
            });
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = refresh_connection(&handle.state::<AppState>(), false).await;
            });
            // Linux : les touches multimédia du clavier passent par MPRIS (voir mpris.rs).
            #[cfg(target_os = "linux")]
            {
                use tauri::Emitter;
                let handle = app.handle().clone();
                let keys = mpris::start(move |action| {
                    let _ = handle.emit("media-command", action);
                });
                app.manage(MediaKeys(keys));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_session,
            open_url,
            set_mpv_path,
            plex_start_login,
            jellyfin_login,
            jellyfin_quick_start,
            jellyfin_quick_poll,
            plex_poll_login,
            plex_logout,
            plex_forget_server,
            plex_switch_account,
            resume_plex,
            resume_jellyfin,
            set_server_kinds,
            plex_servers,
            plex_select_server,
            plex_sections,
            plex_section_items,
            plex_first_characters,
            plex_section_filters,
            plex_filter_values,
            plex_create_smart_collection,
            plex_update_smart_collection,
            plex_smart_collection,
            plex_set_collection_sort,
            plex_bandwidth,
            plex_media_stats,
            plex_history,
            plex_stop_session,
            plex_move_in_collection,
            plex_convert_collection,
            plex_add_many_to_collection,
            plex_create_collection_with,
            plex_section_hubs,
            plex_genres,
            plex_adjacent,
            plex_markers,
            video_previews,
            collection_choices,
            add_to_collection,
            create_collection,
            plex_trailer,
            media_update,
            plex_playlists,
            plex_tracks,
            plex_home,
            plex_metadata,
            plex_children,
            plex_related,
            plex_rate,
            plex_remove_from_continue,
            plex_watchlisted,
            plex_set_watchlist,
            plex_collections,
            plex_collection_items,
            plex_person_items,
            plex_remote_logo,
            plex_search,
            plex_set_watched,
            plex_is_admin,
            plex_home_users,
            reconnect_server,
            plex_switch_user,
            set_auto_sign_in,
            plex_account,
            plex_edit_info,
            plex_edit,
            plex_images,
            plex_set_image,
            plex_upload_image,
            plex_refresh_metadata,
            plex_analyze,
            plex_matches,
            plex_fix_match,
            plex_unmatch,
            plex_prefs,
            plex_set_prefs,
            plex_sessions,
            plex_scan_section,
            plex_empty_trash,
            play,
            play_music,
            platform,
            music_queue,
            video_stream,
            native_video,
            remember_track,
            stop_transcode,
            report_timeline,
            player_command,
            player_stop,
            seerr_configure,
            seerr_configure_jellyfin,
            seerr_search,
            seerr_trending,
            seerr_tv_seasons,
            seerr_request,
            seerr_details,
            seerr_related,
            seerr_slider,
            seerr_person_credits,
            seerr_recent_requests,
            seerr_watchlist,
            seerr_can_manage,
            seerr_requests,
            seerr_request_counts,
            seerr_request_status,
            seerr_request_retry,
            seerr_request_delete,
            seerr_request_update,
            seerr_arr_servers,
            seerr_arr_options,
            tracearr_configure,
            relay_discover,
            tmdb_configure,
            tracearr_start_login,
            tracearr_poll_login,
            tracearr_dashboard,
            tracearr_jellyfin_login,
            tracearr_logout,
            update_check,
            release_notes,
            update_install,
            relay_start,
            relay_status,
            relay_stop,
            relay_journal,
            offline_list,
            plex_users,
            track_lyrics,
            allocine_ratings,
            cast_devices,
            cast_start,
            cast_command,
            cast_stop,
            cast_queue_insert,
            cast_queue_move,
            cast_queue_order,
            cast_repeat,
            player_queue_order,
            music_tracks,
            player_queue_insert,
            player_queue_move,
            playlist_choices,
            add_to_playlist,
            create_playlist,
            media_info,
            plex_server_libraries,
            plex_user_libraries,
            plex_user_permissions,
            plex_user_remove,
            plex_user_invite,
            offline_allowed,
            offline_add,
            offline_pause,
            offline_resume,
            offline_remove,
            offline_progress,
            offline_sync,
            set_app_icon,
            set_language,
            ui_prefs,
            ui_pref_set,
        ])
        .build(context)
        .expect("erreur au lancement de Maku")
        .run(|app, event| {
            // Fermeture de Maku : mpv (vidéo ou musique) s'arrête avec lui.
            if let tauri::RunEvent::Exit = event {
                if let Some(player) = app.state::<AppState>().player.lock().unwrap().take() {
                    player.kill_now();
                }
            }
        });
}

#[cfg(test)]
mod owner_cache_tests {
    use super::OwnerCache;
    use std::time::{Duration, Instant};

    #[test]
    fn droit_d_administration_garde_cinq_minutes() {
        let mut cache = OwnerCache::default();
        let t0 = Instant::now();
        assert_eq!(cache.fresh("serveur|compte", t0), None);
        cache.put("serveur|compte".into(), true, t0);
        // Pendant cinq minutes : pas de nouvelle demande à plex.tv.
        assert_eq!(cache.fresh("serveur|compte", t0 + Duration::from_secs(299)), Some(true));
        assert_eq!(cache.fresh("serveur|compte", t0 + Duration::from_secs(301)), None);
        // plex.tv injoignable : la dernière réponse vaut encore, même ancienne.
        assert_eq!(cache.last("serveur|compte"), Some(true));
        // Un autre compte ou un autre serveur n'hérite de rien.
        assert_eq!(cache.fresh("serveur|autre-compte", t0), None);
        assert_eq!(cache.last("autre-serveur|compte"), None);
        // Un refus est gardé aussi.
        cache.put("serveur|invite".into(), false, t0);
        assert_eq!(cache.fresh("serveur|invite", t0), Some(false));
        assert_eq!(cache.last("serveur|compte"), None);
    }
}

#[cfg(test)]
mod local_art_tests {
    use super::commands::local_art;

    /// Faux serveur : une image à l'adresse `/art`, une erreur ailleurs ; compte les demandes.
    async fn fake_server() -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = hits.clone();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = listener.accept().await {
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).await.unwrap_or(0);
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let found = String::from_utf8_lossy(&buf[..n]).starts_with("GET /art");
                let resp = if found { "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nIMAGE" } else { "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n" };
                let _ = s.write_all(resp.as_bytes()).await;
            }
        });
        (base, hits)
    }

    #[tokio::test]
    async fn pochette_publiee_sans_adresse_ni_jeton() {
        let (base, hits) = fake_server().await;
        let dir = std::env::temp_dir().join(format!("maku-art-{}", uuid::Uuid::new_v4().simple()));
        let http = reqwest::Client::new();
        let url = format!("{base}/art?X-Plex-Token=SECRET");
        let published = local_art(&http, &dir, &url).await.unwrap();
        // Un fichier de l'appareil : ni le serveur ni son jeton.
        assert!(published.starts_with("file://"), "{published}");
        assert!(!published.contains("SECRET") && !published.contains("127.0.0.1"), "{published}");
        let file = reqwest::Url::parse(&published).unwrap().to_file_path().unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"IMAGE");
        // Même pochette : pas de nouveau téléchargement.
        assert_eq!(local_art(&http, &dir, &url).await.unwrap(), published);
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
        // Titre suivant : sa pochette remplace la précédente.
        let next = local_art(&http, &dir, &format!("{base}/absent?X-Plex-Token=SECRET")).await;
        assert!(next.is_none());
        let other = local_art(&http, &dir, &format!("{base}/art?X-Plex-Token=SECRET&v=2")).await.unwrap();
        assert_ne!(other, published);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        // Rien à publier sans image, et jamais une adresse qui n'est pas celle d'un serveur.
        assert!(local_art(&http, &dir, "").await.is_none());
        assert!(local_art(&http, &dir, "file:///etc/passwd").await.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod play_turn_tests {
    use super::commands::PlayTurn;

    #[test]
    fn une_lecture_remplacee_ou_arretee_ne_demarre_pas() {
        let turns = PlayTurn::default();
        // Seule : elle démarre.
        let a = turns.begin();
        assert!(turns.is_current(a));
        // Une autre lecture demandée pendant l'attente : la première ne démarre plus, la seconde oui.
        let b = turns.begin();
        assert!(!turns.is_current(a));
        assert!(turns.is_current(b));
        // Arrêt pendant l'attente : plus rien ne démarre.
        turns.cancel();
        assert!(!turns.is_current(b));
        // La lecture suivante repart normalement.
        let c = turns.begin();
        assert!(turns.is_current(c));
    }
}

#[cfg(test)]
mod player_command_tests {
    use super::allowed_player_command;
    use serde_json::json;

    fn ok(v: serde_json::Value) -> bool {
        allowed_player_command(v.as_array().unwrap())
    }

    #[test]
    fn accepts_playback_commands() {
        assert!(ok(json!(["cycle", "pause"])));
        assert!(ok(json!(["cycle", "mute"])));
        assert!(ok(json!(["seek", 10, "relative"])));
        assert!(ok(json!(["seek", 12.5, "absolute"])));
        assert!(ok(json!(["add", "volume", -5])));
        assert!(ok(json!(["playlist-next"])));
        assert!(ok(json!(["set_property", "volume", 40])));
        assert!(ok(json!(["set_property", "loop-playlist", "inf"])));
        assert!(ok(json!(["set_property", "sid", "no"])));
        assert!(ok(json!(["set_property", "mute", false])));
    }

    #[test]
    fn rejects_everything_else() {
        assert!(!ok(json!(["run", "id"])));
        assert!(!ok(json!(["subprocess", "bash", "-c", "whoami"])));
        assert!(!ok(json!(["loadfile", "/etc/passwd"])));
        assert!(!ok(json!(["write-watch-later-config"])));
        assert!(!ok(json!(["set_property", "script-opts", "x"])));
        assert!(!ok(json!(["set_property", "volume", ["run", "id"]])));
        assert!(!ok(json!(["seek", "10", "relative"])));
        assert!(!ok(json!([])));
    }
}
