use serde::{Serialize, Serializer};

/// Erreur renvoyée aux commandes Tauri : sérialisée en simple message pour l'interface.
#[derive(Debug)]
pub struct AppError(anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(e: E) -> Self {
        AppError(e.into())
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // Le message s'affiche, se copie et part dans le journal de diagnostic : sans jeton.
        s.serialize_str(&crate::net::redact(&format!("{:#}", self.0)))
    }
}

pub type CmdResult<T> = Result<T, AppError>;
