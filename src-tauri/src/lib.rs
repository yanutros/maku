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
mod plex_users;
mod seerr;
mod tracearr;
mod update;

use anyhow::anyhow;
use config::{Config, SavedServer};
use error::CmdResult;
use player::{Entry, PlayerControl, Source};
use plex::{Adjacent, Genre, Hub, Item, WebStream, ItemDetail, LoginPin, Page, PlexServer, QueueTrack, Section};
use plex_admin::{Account, EditInfo, FieldChange, ImageChoice, Pref, ServerSession};
use std::collections::HashMap;
use seerr::{Seerr, SeerrDetail, SeerrMedia, SeerrSeason, Slider};
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tracearr::{Dashboard, LoginState, Tracearr};

struct AppState {
    http: reqwest::Client,
    config_path: PathBuf,
    config: Mutex<Config>,
    seerr: tokio::sync::Mutex<Option<Arc<Seerr>>>,
    tracearr: tokio::sync::Mutex<Option<Arc<Tracearr>>>,
    /// Le serveur a changé : les sessions Seerr et Tracearr ouvertes sont celles de l'ancien.
    seerr_stale: std::sync::atomic::AtomicBool,
    tracearr_stale: std::sync::atomic::AtomicBool,
    player: Mutex<Option<PlayerControl>>,
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
        let s = c.server.ok_or_else(|| anyhow!(crate::i18n::tr("app.noServer")))?;
        Ok(if s.is_jellyfin() {
            media::MediaServer::Jellyfin(jellyfin::JellyfinServer {
                http: self.http.clone(),
                base: s.uri,
                token: s.token,
                user_id: s.user_id.unwrap_or_default(),
                client_id: c.client_id,
            })
        } else {
            media::MediaServer::Plex(PlexServer { http: self.http.clone(), base: s.uri, token: s.token, client_id: c.client_id })
        })
    }

    /// Serveur Plex, pour ce qui n'existe que chez lui (lecteur mpv, réglages du serveur, édition).
    fn plex(&self) -> anyhow::Result<PlexServer> {
        self.server()?.plex()
    }

    /// Le compte Plex connecté possède-t-il le serveur en cours ? D'après plex.tv (« owned ») : les
    /// membres d'un Plex Home lisent eux aussi les réglages du serveur, ce n'est donc pas un critère.
    async fn owns_plex_server(&self) -> bool {
        let c = self.cfg();
        let (Some(token), Some(saved)) = (c.plex_token, c.server) else { return false };
        if saved.is_jellyfin() {
            return false;
        }
        plex::servers(&self.http, &c.client_id, &token)
            .await
            .ok()
            .and_then(|list| list.into_iter().find(|r| r.client_identifier == saved.machine_id))
            .is_some_and(|r| r.owned)
    }

    /// Le serveur Plex, pour une action réservée à son propriétaire (fiches, collections, réglages…).
    async fn admin_plex(&self) -> anyhow::Result<PlexServer> {
        if !self.owns_plex_server().await {
            return Err(anyhow!(crate::i18n::tr("admin.forbidden")));
        }
        self.plex()
    }

    /// Jeton à joindre aux appels vers `url`, quand elle passe par le relais Maku Server du serveur.
    async fn relay_token(&self, url: &str) -> Option<String> {
        let server = self.cfg().server;
        self.relays.token_for(&self.http, url, server.as_ref()).await
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
        let relay = self.relay_token(&url).await;
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
        let relay = self.relay_token(&url).await;
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

mod commands;
use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
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
            tracearr_configure,
            relay_discover,
            tmdb_configure,
            tracearr_start_login,
            tracearr_poll_login,
            tracearr_dashboard,
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
        ])
        .build(tauri::generate_context!())
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
