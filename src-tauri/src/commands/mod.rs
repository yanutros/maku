//! Commandes Tauri appelées par l'interface, rangées par domaine (elles étaient toutes dans lib.rs). Tout est
//! réexporté ici : lib.rs, et `generate_handler!`, les nomment comme avant.

mod session;
mod account;
mod library;
mod actions;
mod collections;
mod admin;
mod users;
mod playback;
mod casting;
mod downloads;
mod requests;
mod stats;
mod services;
mod app;
mod invite;

pub(crate) use session::*;
pub(crate) use account::*;
pub(crate) use library::*;
pub(crate) use actions::*;
pub(crate) use collections::*;
pub(crate) use admin::*;
pub(crate) use users::*;
pub(crate) use playback::*;
pub(crate) use casting::*;
pub(crate) use downloads::*;
pub(crate) use requests::*;
pub(crate) use stats::*;
pub(crate) use services::*;
pub(crate) use app::*;
pub(crate) use invite::*;
