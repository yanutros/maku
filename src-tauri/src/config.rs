use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
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
    /// Clé publique du Maku Server de chaque serveur (clé : voir `SavedServer::key`), retenue à la première
    /// rencontre : le jeton ne part ensuite qu'au relais qui sait signer avec elle (voir `maku_server`).
    pub relay_keys: HashMap<String, String>,
    /// Préférences de l'interface (couleur, fond, notes, accueil, menu…), copie de celles du navigateur intégré :
    /// elles y sont relues au démarrage, et ne se perdent ni quand ce stockage est vidé ou n'a pas été écrit à
    /// temps, ni entre deux versions de Maku (installée et de développement) qui ne le partagent pas.
    pub ui: BTreeMap<String, String>,
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

    /// Nouvelle adresse du serveur Plex `machine_id`, s'il est toujours le serveur en cours. Renvoie vrai si elle a été
    /// enregistrée : un autre serveur (Plex, Jellyfin ou Emby) choisi entre-temps ne reçoit pas l'adresse de l'ancien.
    pub fn set_plex_uri(&mut self, machine_id: &str, uri: String) -> bool {
        match self.server.as_mut() {
            Some(s) if !s.is_jellyfin() && s.machine_id == machine_id => {
                s.uri = uri;
                true
            }
            _ => false,
        }
    }

    pub fn load(path: &Path) -> Self {
        let mut config: Config = match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| {
                // Fichier illisible (écriture coupée, version plus récente) : gardé à côté, pour ne pas perdre
                // les connexions qu'il contient quand la configuration neuve le remplacera.
                let _ = std::fs::copy(path, unreadable(path));
                Config::default()
            }),
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
        // Écrit à côté puis renommé : une coupure pendant l'écriture laisse l'ancien fichier entier.
        let tmp = path.with_extension("json.tmp");
        write_private(&tmp, serde_json::to_string_pretty(self)?.as_bytes())?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

/// Copie d'une configuration illisible (`config.json.bad`).
fn unreadable(path: &Path) -> std::path::PathBuf {
    path.with_extension("json.bad")
}

/// Le fichier contient des jetons : lisible par son seul propriétaire (Linux, macOS, Android).
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
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
    fn plex_address_only_for_the_same_server() {
        let mut c = Config { server: Some(server("", "plex1")), ..Default::default() };
        assert!(c.set_plex_uri("plex1", "https://nouvelle".into()));
        assert_eq!(c.server.as_ref().unwrap().uri, "https://nouvelle");
        // Passé sur un autre serveur Plex, ou sur Jellyfin (même identifiant ou non), pendant le tour des adresses.
        for other in [server("", "plex2"), server("jellyfin", "plex1"), server("emby", "e1")] {
            let mut c = Config { server: Some(SavedServer { uri: "http://la-sienne".into(), ..other }), ..Default::default() };
            assert!(!c.set_plex_uri("plex1", "https://ancienne".into()));
            assert_eq!(c.server.as_ref().unwrap().uri, "http://la-sienne");
        }
        assert!(!Config::default().set_plex_uri("plex1", "https://x".into()));
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("maku-config-{name}-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn unreadable_config_is_kept_aside() {
        let dir = temp_dir("bad");
        let path = dir.join("config.json");
        // Écriture coupée en plein milieu.
        let cut = r#"{"client_id":"abc","plex_token":"jet"#;
        std::fs::write(&path, cut).unwrap();
        let loaded = Config::load(&path);
        assert!(loaded.plex_token.is_none());
        loaded.save(&path).unwrap();
        // L'ancien contenu n'est pas perdu : il reste à côté.
        assert_eq!(std::fs::read_to_string(dir.join("config.json.bad")).unwrap(), cut);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn save_replaces_the_file_in_one_step() {
        let dir = temp_dir("save");
        let path = dir.join("config.json");
        let c = Config { client_id: "abc".into(), plex_token: Some("jeton".into()), ..Default::default() };
        c.save(&path).unwrap();
        c.save(&path).unwrap();
        let back = Config::load(&path);
        assert_eq!((back.client_id.as_str(), back.plex_token.as_deref()), ("abc", Some("jeton")));
        // Ni fichier de travail oublié, ni copie « illisible » d'un fichier sain.
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        assert_eq!(names, ["config.json"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn first_server_adopts_existing_services() {
        // Configuration d'une version précédente, ou invitation appliquée avant de choisir un serveur.
        let mut c = Config { seerr_url: Some("https://seerr".into()), ..Default::default() };
        switch(&mut c, Some(server("", "plex1")));
        assert_eq!(c.seerr_url.as_deref(), Some("https://seerr"));
    }
}
