use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Configuration persistée dans le dossier de configuration de l'application.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Identifiant unique de ce client auprès de Plex (X-Plex-Client-Identifier).
    pub client_id: String,
    pub plex_token: Option<String>,
    /// Plex Home : se connecter au dernier profil utilisé sans passer par le choix du profil (et son
    /// code PIN), comme l'option « Connexion automatique » des applications Plex. Propre à l'appareil.
    pub auto_sign_in: bool,
    pub server: Option<SavedServer>,
    /// Serveurs utilisés : « plex », « jellyfin » ou « both » (écran de choix à chaque changement).
    /// Vide : fixé à la première connexion.
    pub server_kinds: Option<String>,
    /// Dernière connexion Jellyfin, gardée pour y revenir sans se reconnecter (30 jours au plus).
    pub jellyfin_saved: Option<SavedServer>,
    /// Date de la dernière utilisation de `jellyfin_saved` (secondes depuis 1970).
    pub jellyfin_saved_at: Option<u64>,
    /// Seerr s'ouvre avec le compte Plex de l'utilisateur, ou son compte Jellyfin.
    pub seerr_url: Option<String>,
    /// Cookies de session Seerr d'un compte Jellyfin (le mot de passe n'est jamais gardé).
    pub seerr_cookies: Option<String>,
    pub mpv_path: Option<String>,
    pub tracearr_url: Option<String>,
    /// Cookies de session Tracearr (« nom=valeur; … »), valables 30 jours.
    pub tracearr_cookies: Option<String>,
    /// Clé d'API TMDB de l'utilisateur (facultative) : logos des titres que Plex n'a pas.
    pub tmdb_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedServer {
    pub name: String,
    pub machine_id: String,
    pub uri: String,
    pub token: String,
    /// « jellyfin », ou vide pour Plex (les configurations d'avant Jellyfin n'ont pas ce champ).
    #[serde(default)]
    pub kind: String,
    /// Jellyfin : identifiant de l'utilisateur connecté.
    #[serde(default)]
    pub user_id: Option<String>,
}

impl SavedServer {
    pub fn is_jellyfin(&self) -> bool {
        self.kind == "jellyfin"
    }
}

impl Config {
    pub fn load(path: &Path) -> Self {
        let mut config: Config = match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            // Premier lancement : Seerr et Tracearr se configurent ensuite (à la main ou par invitation).
            Err(_) => Config::default(),
        };
        if config.client_id.is_empty() {
            config.client_id = uuid::Uuid::new_v4().to_string();
        }
        config
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
