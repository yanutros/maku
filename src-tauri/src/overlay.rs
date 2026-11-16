//! Linux : la vidéo dans la fenêtre de Maku (réglage expérimental, désactivé par défaut).
//!
//! Sous Windows, mpv dessine dans la fenêtre de Maku (`--wid`), sous la page devenue transparente. Sous Linux, un
//! programme ne dessine pas dans la fenêtre d'un autre de cette façon : mpv garde donc sa fenêtre, en plein écran et
//! sans ses propres commandes, et c'est la fenêtre de Maku qui passe par-dessus, en plein écran elle aussi, avec un
//! fond transparent. On voit le film, avec la barre et les menus de Maku au-dessus.
//!
//! Une fenêtre ne peut être transparente que si elle a été créée ainsi : le réglage est donc lu au démarrage, avant
//! la création de la fenêtre, directement dans le fichier de configuration. Le changer demande de relancer Maku.

use std::path::PathBuf;

/// Préférence de l'interface (copiée dans `ui`, voir `src/lib/prefs.ts`) : « 1 » quand le réglage est activé.
pub const PREF: &str = "maku.linuxEmbed";

/// Le fichier de configuration de Maku sous Linux, tel que Tauri le placera (`$XDG_CONFIG_HOME`, sinon `~/.config`).
pub fn config_file(identifier: &str, xdg: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
    let base = match xdg.map(str::trim).filter(|x| x.starts_with('/')) {
        Some(x) => PathBuf::from(x),
        None => PathBuf::from(home.map(str::trim).filter(|h| !h.is_empty())?).join(".config"),
    };
    Some(base.join(identifier).join("config.json"))
}

/// Le réglage est-il activé dans ce fichier de configuration (son texte) ? Illisible ou absent : non.
pub fn enabled_in(config: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(config).is_ok_and(|v| v["ui"][PREF] == "1")
}

/// Faut-il créer la fenêtre transparente ? Sous Linux seulement, et si l'utilisateur a activé le réglage.
pub fn wanted(identifier: &str) -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }
    let (xdg, home) = (std::env::var("XDG_CONFIG_HOME").ok(), std::env::var("HOME").ok());
    config_file(identifier, xdg.as_deref(), home.as_deref()).and_then(|path| std::fs::read_to_string(path).ok()).is_some_and(|text| enabled_in(&text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fichier_de_configuration_sous_linux() {
        let id = "io.exemple.maku";
        assert_eq!(config_file(id, None, Some("/home/moi")), Some(PathBuf::from("/home/moi/.config/io.exemple.maku/config.json")));
        assert_eq!(config_file(id, Some("/data/conf"), Some("/home/moi")), Some(PathBuf::from("/data/conf/io.exemple.maku/config.json")));
        // Un XDG_CONFIG_HOME vide ou relatif n'est pas suivi (comme le veut sa spécification).
        assert_eq!(config_file(id, Some(""), Some("/home/moi")), Some(PathBuf::from("/home/moi/.config/io.exemple.maku/config.json")));
        assert_eq!(config_file(id, Some("relatif"), Some("/home/moi")), Some(PathBuf::from("/home/moi/.config/io.exemple.maku/config.json")));
        assert_eq!(config_file(id, None, None), None);
        assert_eq!(config_file(id, None, Some(" ")), None);
    }

    #[test]
    fn reglage_lu_dans_la_configuration() {
        assert!(enabled_in(r#"{ "client_id": "x", "ui": { "maku.linuxEmbed": "1", "maku.audioLang": "fr" } }"#));
        // Désactivé, absent, d'un autre type, ou fichier illisible : la fenêtre reste comme avant.
        for config in [r#"{ "ui": { "maku.linuxEmbed": "0" } }"#, r#"{ "ui": {} }"#, r#"{}"#, r#"{ "ui": { "maku.linuxEmbed": 1 } }"#, r#"{ "ui": { "maku.linuxEmbed": true } }"#, "", "pas du json"] {
            assert!(!enabled_in(config), "{config}");
        }
    }
}
