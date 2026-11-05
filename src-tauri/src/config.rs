use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    /// Dernière connexion Emby, gardée à part : passer de Jellyfin à Emby ne fait perdre ni l'une ni l'autre.
    pub emby_saved: Option<SavedServer>,
    pub emby_saved_at: Option<u64>,
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
    /// Seerr et Tracearr de chacun des autres serveurs (clé : voir `SavedServer::key`). Les champs
    /// `seerr_*` et `tracearr_*` ci-dessus sont ceux du serveur en cours ; ils sont échangés avec
    /// cette liste quand on change de serveur (voir `switch_services`).
    pub services: HashMap<String, Services>,
}

/// Seerr et Tracearr d'un serveur : chacun a les siens.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Services {
    pub seerr_url: Option<String>,
    pub seerr_cookies: Option<String>,
    pub tracearr_url: Option<String>,
    pub tracearr_cookies: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedServer {
    pub name: String,
    pub machine_id: String,
    pub uri: String,
    pub token: String,
    /// « jellyfin », « emby », ou vide pour Plex (les configurations d'avant Jellyfin n'ont pas ce champ).
    #[serde(default)]
    pub kind: String,
    /// Jellyfin : identifiant de l'utilisateur connecté.
    #[serde(default)]
    pub user_id: Option<String>,
}

impl SavedServer {
    /// Serveur Jellyfin ou Emby : même API, même client (`jellyfin.rs`).
    pub fn is_jellyfin(&self) -> bool {
        matches!(self.kind.as_str(), "jellyfin" | "emby")
    }

    pub fn is_emby(&self) -> bool {
        self.kind == "emby"
    }

    /// Identité du serveur, pour lui garder ses propres Seerr et Tracearr.
    pub fn key(&self) -> String {
        let kind = if self.kind.is_empty() { "plex" } else { &self.kind };
        format!("{kind}:{}", self.machine_id)
    }
}

impl Config {
    /// Serveur en cours, au sens de ses Seerr et Tracearr.
    pub fn services_key(&self) -> Option<String> {
        self.server.as_ref().map(SavedServer::key)
    }

    fn take_services(&mut self) -> Services {
        Services {
            seerr_url: self.seerr_url.take(),
            seerr_cookies: self.seerr_cookies.take(),
            tracearr_url: self.tracearr_url.take(),
            tracearr_cookies: self.tracearr_cookies.take(),
        }
    }

    fn put_services(&mut self, s: Services) {
        self.seerr_url = s.seerr_url;
        self.seerr_cookies = s.seerr_cookies;
        self.tracearr_url = s.tracearr_url;
        self.tracearr_cookies = s.tracearr_cookies;
    }

    /// Le serveur en cours a changé (`old` : celui d'avant) : ses Seerr et Tracearr sont rangés, ceux
    /// du nouveau serveur reviennent. Sans serveur avant (première connexion, configuration d'une version
    /// précédente, réglages faits par invitation), le nouveau serveur garde ceux en place s'il n'en a pas.
    pub fn switch_services(&mut self, old: Option<String>) {
        let new = self.services_key();
        if old == new {
            return;
        }
        let current = self.take_services();
        let saved = new.as_ref().and_then(|k| self.services.remove(k));
        match old {
            Some(k) => {
                if current != Services::default() {
                    self.services.insert(k, current);
                }
                self.put_services(saved.unwrap_or_default());
            }
            None => self.put_services(saved.unwrap_or(current)),
        }
    }

    pub fn load(path: &Path) -> Self {
        let mut config: Config = match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            // Premier lancement : Seerr et Tracearr se configurent ensuite (à la main ou par invitation).
            Err(_) => Config::default(),
        };
        // Versions précédentes : une connexion Emby gardée à la place de celle de Jellyfin.
        if config.jellyfin_saved.as_ref().is_some_and(SavedServer::is_emby) && config.emby_saved.is_none() {
            config.emby_saved = config.jellyfin_saved.take();
            config.emby_saved_at = config.jellyfin_saved_at.take();
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn server(kind: &str, id: &str) -> SavedServer {
        SavedServer { name: id.into(), machine_id: id.into(), uri: String::new(), token: String::new(), kind: kind.into(), user_id: None }
    }

    fn switch(c: &mut Config, to: Option<SavedServer>) {
        let old = c.services_key();
        c.server = to;
        c.switch_services(old);
    }

    #[test]
    fn each_server_keeps_its_own_services() {
        let mut c = Config { server: Some(server("", "plex1")), seerr_url: Some("https://seerr-a".into()), tracearr_url: Some("https://tr-a".into()), ..Default::default() };
        // Emby : rien de configuré.
        switch(&mut c, Some(server("emby", "emby1")));
        assert_eq!((c.seerr_url.as_deref(), c.tracearr_url.as_deref()), (None, None));
        c.seerr_url = Some("https://seerr-b".into());
        c.seerr_cookies = Some("s=1".into());
        // Retour sur Plex : ses réglages reviennent.
        switch(&mut c, Some(server("", "plex1")));
        assert_eq!((c.seerr_url.as_deref(), c.tracearr_url.as_deref(), c.seerr_cookies.as_deref()), (Some("https://seerr-a"), Some("https://tr-a"), None));
        // Et ceux d'Emby aussi, en passant par l'écran de choix (aucun serveur).
        switch(&mut c, None);
        assert_eq!(c.seerr_url, None);
        switch(&mut c, Some(server("emby", "emby1")));
        assert_eq!((c.seerr_url.as_deref(), c.seerr_cookies.as_deref()), (Some("https://seerr-b"), Some("s=1")));
    }

    #[test]
    fn first_server_adopts_existing_services() {
        // Configuration d'une version précédente, ou invitation appliquée avant de choisir un serveur.
        let mut c = Config { seerr_url: Some("https://seerr".into()), ..Default::default() };
        switch(&mut c, Some(server("", "plex1")));
        assert_eq!(c.seerr_url.as_deref(), Some("https://seerr"));
    }
}
