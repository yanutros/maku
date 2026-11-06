//! Couche commune aux serveurs de médias : Plex ou Jellyfin, vus par l'interface de la même façon
//! (bibliothèques, rangées, fiches, lecture…, dans les types de `plex.rs`). Les fonctions propres
//! à Plex (réglages du serveur, lecteur mpv, invitations) passent toujours par `PlexServer`.

use anyhow::{anyhow, Result};

use crate::jellyfin::JellyfinServer;
use crate::plex::{Adjacent, Genre, Hub, Item, ItemDetail, Marker, Page, PlayInfo, PlexServer, Previews, QueueTrack, Section, TrailerSource, WebStream};

#[derive(Clone)]
pub enum MediaServer {
    Plex(PlexServer),
    Jellyfin(JellyfinServer),
}

/// Fonction pas encore disponible avec Jellyfin (étape 2 du support de Jellyfin).
fn not_yet<T>() -> Result<T> {
    Err(anyhow!(crate::i18n::tr("jellyfin.notYet")))
}

/// Une méthode par fonction commune, faite par Plex et par Jellyfin (`both`), ou par Plex seulement
/// pour l'instant (`plex_only`).
macro_rules! common {
    (both { $( fn $name:ident(&self $(, $arg:ident : $ty:ty)* ) -> $ret:ty; )* }
     plex_only { $( fn $pname:ident(&self $(, $parg:ident : $pty:ty)* ) -> $pret:ty; )* }) => {
        impl MediaServer {
            $(
                pub async fn $name(&self $(, $arg: $ty)*) -> Result<$ret> {
                    match self {
                        Self::Plex(p) => p.$name($($arg),*).await,
                        Self::Jellyfin(j) => j.$name($($arg),*).await,
                    }
                }
            )*
            $(
                pub async fn $pname(&self $(, $parg: $pty)*) -> Result<$pret> {
                    match self {
                        Self::Plex(p) => p.$pname($($parg),*).await,
                        Self::Jellyfin(_) => not_yet(),
                    }
                }
            )*
        }
    };
}

common! {
    both {
        fn section_items(&self, key: &str, start: u32, size: u32, kind: Option<u32>, genre: Option<&str>, sort: Option<&str>) -> Page;
        fn on_deck(&self) -> Vec<Item>;
        fn recently_added(&self) -> Vec<Item>;
        fn genres(&self, key: &str) -> Vec<Genre>;
        fn section_hubs(&self, key: &str) -> Vec<Hub>;
        fn playlists(&self, section: &str) -> Vec<Item>;
        fn adjacent(&self, rating_key: &str) -> Adjacent;
        fn tracks(&self, rating_key: &str) -> Vec<Item>;
        fn queue(&self, rating_key: &str) -> Vec<QueueTrack>;
        fn metadata(&self, rating_key: &str) -> ItemDetail;
        fn collections(&self, key: &str) -> Vec<Item>;
        fn collection_items(&self, rating_key: &str) -> Vec<Item>;
        fn related(&self, rating_key: &str) -> Vec<Item>;
        fn children(&self, rating_key: &str) -> Vec<Item>;
        fn search(&self, query: &str) -> Vec<Item>;
        fn web_stream(&self, rating_key: &str, media: Option<usize>, audio: Option<u64>, subtitle: Option<u64>, max_kbps: Option<u32>, session: &str, copy_video: bool) -> WebStream;
        fn stop_transcode(&self, session: &str) -> ();
        fn play_info(&self, rating_key: &str, media: Option<usize>, audio: Option<u64>, subtitle: Option<u64>) -> PlayInfo;
        fn timeline(&self, rating_key: &str, state: &str, time_ms: u64, duration_ms: u64, session: &str) -> ();
        fn set_watched(&self, rating_key: &str, watched: bool) -> ();
        fn person_items(&self, id: &str) -> Vec<Item>;
        fn rate(&self, rating_key: &str, rating: Option<f32>) -> ();
        fn remove_from_continue(&self, rating_key: &str) -> ();
        fn markers(&self, rating_key: &str) -> Vec<Marker>;
        fn collection_choices(&self, rating_key: &str) -> Vec<Item>;
        fn add_to_collection(&self, rating_key: &str, collection: &str) -> ();
        fn create_collection(&self, rating_key: &str, title: &str) -> ();
        fn previews(&self, rating_key: &str, media: Option<usize>) -> Option<Previews>;
        fn trailer(&self, rating_key: &str) -> TrailerSource;
    }
    plex_only {}
}

impl MediaServer {
    pub async fn sections(&self) -> Result<Vec<Section>> {
        match self {
            Self::Plex(p) => p.sections().await,
            Self::Jellyfin(j) => j.sections().await,
        }
    }

    /// Le serveur Plex, pour ce qui n'existe que chez lui.
    pub fn plex(self) -> Result<PlexServer> {
        match self {
            Self::Plex(p) => Ok(p),
            Self::Jellyfin(_) => not_yet(),
        }
    }
}
