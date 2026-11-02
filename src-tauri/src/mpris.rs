//! Linux : Maku s'annonce comme lecteur MPRIS sur D-Bus. GNOME et KDE gardent pour eux les touches
//! multimédia du clavier (lecture / pause, suivant, précédent) et ne les transmettent qu'aux lecteurs
//! MPRIS : elles reviennent ainsi à Maku, et ce qui joue apparaît dans les commandes du bureau.
//! Sous Windows, ces touches arrivent directement à la page.

use mpris_server::{Metadata, PlaybackStatus, Player, Time};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

/// Ce qui joue, envoyé par l'interface (le même JSON que la notification de musique d'Android).
#[derive(serde::Deserialize, Default)]
pub struct State {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub art: String,
    #[serde(default)]
    pub playing: bool,
    /// Durée en millisecondes.
    #[serde(default)]
    pub duration: u64,
}

pub enum Update {
    State(State),
    /// Plus rien ne joue.
    Stop,
}

/// Démarre le lecteur MPRIS sur son propre fil (la connexion D-Bus n'est pas `Send`). Chaque commande
/// reçue du bureau est transmise à `emit` : `play_pause`, `play`, `pause`, `next`, `previous`, `stop`.
pub fn start(emit: impl Fn(&str) + Clone + Send + 'static) -> UnboundedSender<Update> {
    let (tx, mut rx) = unbounded_channel::<Update>();
    let spawned = std::thread::Builder::new().name("mpris".into()).spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return };
        let local = tokio::task::LocalSet::new();
        local.block_on(&rt, async move {
            let player = match Player::builder("io.github.yanutros.maku")
                .identity("Maku")
                .can_play(true)
                .can_pause(true)
                .can_go_next(true)
                .can_go_previous(true)
                .can_control(true)
                .can_seek(false)
                .can_raise(false)
                .can_quit(false)
                .build()
                .await
            {
                Ok(p) => p,
                // Sans bus de session (pas de bureau), Maku fonctionne simplement sans.
                Err(e) => {
                    eprintln!("MPRIS indisponible : {e}");
                    return;
                }
            };
            let e = emit.clone();
            player.connect_play_pause(move |_| e("play_pause"));
            let e = emit.clone();
            player.connect_play(move |_| e("play"));
            let e = emit.clone();
            player.connect_pause(move |_| e("pause"));
            let e = emit.clone();
            player.connect_next(move |_| e("next"));
            let e = emit.clone();
            player.connect_previous(move |_| e("previous"));
            let e = emit.clone();
            player.connect_stop(move |_| e("stop"));
            tokio::task::spawn_local(player.run());

            while let Some(update) = rx.recv().await {
                match update {
                    Update::Stop => {
                        let _ = player.set_playback_status(PlaybackStatus::Stopped).await;
                        let _ = player.set_metadata(Metadata::new()).await;
                    }
                    Update::State(s) => {
                        let mut md = Metadata::builder().title(s.title).length(Time::from_millis(s.duration as i64));
                        if !s.artist.is_empty() {
                            md = md.artist([s.artist]);
                        }
                        if !s.album.is_empty() {
                            md = md.album(s.album);
                        }
                        if !s.art.is_empty() {
                            md = md.art_url(s.art);
                        }
                        let _ = player.set_metadata(md.build()).await;
                        let status = if s.playing { PlaybackStatus::Playing } else { PlaybackStatus::Paused };
                        let _ = player.set_playback_status(status).await;
                    }
                }
            }
        });
    });
    if spawned.is_err() {
        eprintln!("MPRIS : impossible de démarrer le fil");
    }
    tx
}
