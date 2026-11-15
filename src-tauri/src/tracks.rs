//! Pistes préférées : la langue du son et les sous-titres que l'utilisateur veut, à la place du choix du serveur.
//!
//! Deux sources, la première l'emportant sur la seconde :
//! - le dernier choix fait sur ce titre (une série entière compte pour un titre), retenu par Maku ;
//! - la préférence des Réglages (langue du son, sous-titres). Tant que l'utilisateur n'y a pas touché : le son dans
//!   la langue de l'interface, et aucun sous-titre.
//!
//! Avec « Choix du serveur » dans les Réglages, et sans choix retenu, la piste que le serveur a marquée est gardée. Le choix se fait là où Maku
//! lit la fiche du serveur, en déplaçant cette marque : la fiche, le lecteur et la conversion suivent tous.

use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Sous-titres : aucun.
pub const OFF: &str = "off";
/// Sous-titres : seulement quand le son n'est pas dans la langue préférée.
pub const AUTO: &str = "auto";
/// Dans les Réglages : laisser le serveur choisir.
pub const SERVER: &str = "server";
/// Titres dont le choix est retenu : au-delà, les plus anciens sont oubliés.
const KEPT: usize = 400;

/// Le choix retenu pour un titre.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Memory {
    /// Langue du son (« fr »).
    pub audio: Option<String>,
    /// Langue des sous-titres, ou « off ».
    pub subtitle: Option<String>,
    /// Date du choix (secondes depuis 1970).
    pub at: u64,
}

/// Une piste d'un fichier, telle que le serveur la décrit.
#[derive(Debug, Clone, PartialEq)]
pub struct Stream {
    pub id: u64,
    pub lang: Option<String>,
    /// Sous-titres « forcés » : seulement les passages dans une autre langue que celle du son.
    pub forced: bool,
    /// Piste que le serveur lirait.
    pub selected: bool,
}

/// Les pistes à lire. `None` : garder celle du serveur. `subtitle: Some(None)` : aucun sous-titre.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Choice {
    pub audio: Option<u64>,
    pub subtitle: Option<Option<u64>>,
}

/// Les préférences qui valent pour le serveur en cours.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Prefs {
    pub audio: Option<String>,
    pub subtitle: Option<String>,
    /// Choix retenus, par titre.
    pub memory: HashMap<String, Memory>,
}

/// Langue en deux lettres (« fr ») d'un code de piste : « fra », « fre », « fr », « fr-FR », « FRE »…
pub fn lang(code: &str) -> Option<String> {
    let code = code.trim().to_ascii_lowercase();
    let code = code.split(['-', '_']).next().unwrap_or_default();
    let two = match code {
        "fra" | "fre" => "fr",
        "eng" => "en",
        "spa" => "es",
        "deu" | "ger" => "de",
        "nld" | "dut" => "nl",
        "ita" => "it",
        "por" => "pt",
        "jpn" => "ja",
        "kor" => "ko",
        "zho" | "chi" => "zh",
        "rus" => "ru",
        "ara" => "ar",
        "pol" => "pl",
        "tur" => "tr",
        "swe" => "sv",
        "dan" => "da",
        "nor" | "nob" => "no",
        "fin" => "fi",
        "hin" => "hi",
        // Indéterminée : pas une langue.
        "und" | "zxx" | "mul" | "" => return None,
        other => other,
    };
    (two.len() >= 2 && two.len() <= 3 && two.chars().all(|c| c.is_ascii_lowercase())).then(|| two.to_owned())
}

/// Une préférence de sous-titres lisible : « off », « auto » ou une langue.
fn subtitle_pref(value: &str) -> Option<String> {
    match value.trim() {
        OFF => Some(OFF.into()),
        AUTO => Some(AUTO.into()),
        other => lang(other),
    }
}

impl Prefs {
    /// Préférences des Réglages (copiées de l'interface dans `ui`) et choix retenus pour le serveur en cours.
    /// `language` : la langue de l'interface.
    ///
    /// Un réglage jamais touché (ou illisible) vaut : le son dans la langue de l'interface, aucun sous-titre. C'est ce
    /// que veut presque tout le monde, et ce que le serveur ne donne pas de lui-même (version originale, sous-titres
    /// affichés). « Choix du serveur » est donc une valeur à part entière, et non l'absence de réglage.
    pub fn from_config(c: &Config, language: &str) -> Self {
        let prefix = c.server.as_ref().map(|s| format!("{}/", s.key()));
        let memory = match &prefix {
            Some(prefix) => c.track_memory.iter().filter_map(|(k, m)| Some((k.strip_prefix(prefix.as_str())?.to_owned(), m.clone()))).collect(),
            None => HashMap::new(),
        };
        Prefs {
            audio: match c.ui.get("maku.audioLang").map(String::as_str) {
                Some(SERVER) => None,
                saved => saved.and_then(lang).or_else(|| lang(language)),
            },
            subtitle: match c.ui.get("maku.subtitles").map(String::as_str) {
                Some(SERVER) => None,
                saved => saved.and_then(subtitle_pref).or_else(|| Some(OFF.into())),
            },
            memory,
        }
    }

    /// Rien à appliquer : le choix du serveur est gardé partout.
    pub fn is_empty(&self) -> bool {
        self.audio.is_none() && self.subtitle.is_none() && self.memory.is_empty()
    }

    /// Les pistes à lire pour le titre `key`, parmi celles d'un fichier.
    pub fn choose(&self, key: &str, audio: &[Stream], subtitles: &[Stream]) -> Choice {
        let memory = self.memory.get(key);
        let wanted_audio = memory.and_then(|m| m.audio.as_deref()).or(self.audio.as_deref());
        let wanted_sub = memory.and_then(|m| m.subtitle.as_deref()).or(self.subtitle.as_deref());

        // Le son : la première piste dans la langue voulue. S'il n'y en a pas (film en version originale seulement),
        // celle du serveur reste.
        let chosen = wanted_audio.and_then(|l| audio.iter().find(|s| s.lang.as_deref() == Some(l)));
        // La langue qui sera entendue.
        let heard = chosen.or_else(|| audio.iter().find(|s| s.selected)).or_else(|| audio.first()).and_then(|s| s.lang.as_deref());

        let full = |l: &str| subtitles.iter().find(|s| !s.forced && s.lang.as_deref() == Some(l));
        // Son dans ma langue, en « automatiques » : restent les sous-titres qui ne traduisent que les passages dans une
        // autre langue.
        let foreign_parts = || Some(heard.and_then(|l| subtitles.iter().find(|s| s.forced && s.lang.as_deref() == Some(l))).map(|s| s.id));
        let subtitle = match wanted_sub {
            None => None,
            // « Aucun » veut dire aucun : pas même ceux-là.
            Some(OFF) => Some(None),
            Some(AUTO) => match wanted_audio {
                // Sans langue préférée pour le son, « une autre langue » ne veut rien dire : le serveur décide.
                None => None,
                Some(mine) if heard == Some(mine) => foreign_parts(),
                Some(mine) => Some(full(mine).map(|s| s.id)),
            },
            Some(l) => Some(full(l).or_else(|| subtitles.iter().find(|s| s.lang.as_deref() == Some(l))).map(|s| s.id)),
        };
        Choice { audio: chosen.map(|s| s.id), subtitle }
    }
}

/// Ce que l'utilisateur vient de choisir dans le lecteur ou sur la fiche.
#[derive(Debug, Clone, PartialEq)]
pub enum Picked {
    Audio(String),
    /// Langue des sous-titres, ou « off ».
    Subtitle(String),
}

/// Retient un choix pour le titre `key` (clé complète : serveur et titre). Au-delà de `KEPT` titres, les plus anciens
/// sont oubliés.
pub fn remember(memory: &mut HashMap<String, Memory>, key: String, picked: Picked, now: u64) {
    let entry = memory.entry(key).or_default();
    match picked {
        Picked::Audio(l) => entry.audio = Some(l),
        Picked::Subtitle(s) => entry.subtitle = Some(s),
    }
    entry.at = now;
    while memory.len() > KEPT {
        let Some(oldest) = memory.iter().min_by_key(|(k, m)| (m.at, k.as_str())).map(|(k, _)| k.clone()) else { break };
        memory.remove(&oldest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: u64, lang: &str, forced: bool, selected: bool) -> Stream {
        Stream { id, lang: super::lang(lang), forced, selected }
    }

    fn prefs(audio: Option<&str>, subtitle: Option<&str>) -> Prefs {
        Prefs { audio: audio.map(str::to_owned), subtitle: subtitle.map(str::to_owned), memory: HashMap::new() }
    }

    /// Film avec une version originale anglaise (celle du serveur) et une version française.
    fn film() -> (Vec<Stream>, Vec<Stream>) {
        (
            vec![s(1, "eng", false, true), s(2, "fra", false, false)],
            vec![s(10, "eng", false, false), s(11, "fre", false, true), s(12, "fre", true, false), s(13, "eng", true, false)],
        )
    }

    #[test]
    fn codes_de_langue() {
        for (code, two) in [("fra", "fr"), ("fre", "fr"), ("FRE", "fr"), ("fr", "fr"), ("fr-FR", "fr"), ("eng", "en"), ("ger", "de"), ("jpn", "ja"), ("cat", "cat")] {
            assert_eq!(lang(code).as_deref(), Some(two), "{code}");
        }
        for code in ["", "und", "zxx", "  ", "français", "f"] {
            assert_eq!(lang(code), None, "{code}");
        }
    }

    #[test]
    fn sans_preference_le_serveur_decide() {
        let (audio, subs) = film();
        assert!(Prefs::default().is_empty());
        assert_eq!(Prefs::default().choose("1", &audio, &subs), Choice::default());
    }

    #[test]
    fn son_en_francais_sans_sous_titres() {
        let (audio, subs) = film();
        // Le son passe en français, et aucun sous-titre ne reste : ni les complets, ni les « forcés » de cette langue.
        let c = prefs(Some("fr"), Some(OFF)).choose("1", &audio, &subs);
        assert_eq!(c, Choice { audio: Some(2), subtitle: Some(None) });
        // Pareil sans langue préférée pour le son.
        assert_eq!(prefs(None, Some(OFF)).choose("1", &audio, &subs), Choice { audio: None, subtitle: Some(None) });
    }

    #[test]
    fn langue_absente_le_son_du_serveur_reste() {
        // Version originale seulement : rien à choisir pour le son.
        let audio = vec![s(1, "eng", false, true)];
        let subs = vec![s(10, "eng", false, false), s(11, "fra", false, false)];
        assert_eq!(prefs(Some("fr"), None).choose("1", &audio, &subs), Choice { audio: None, subtitle: None });
        // « Aucun » reste aucun, même si le son n'est pas dans ma langue.
        assert_eq!(prefs(Some("fr"), Some(OFF)).choose("1", &audio, &subs).subtitle, Some(None));
        // « Automatiques » : le son est dans une autre langue, les sous-titres viennent dans la mienne.
        assert_eq!(prefs(Some("fr"), Some(AUTO)).choose("1", &audio, &subs).subtitle, Some(Some(11)));
    }

    #[test]
    fn sous_titres_automatiques() {
        let (audio, subs) = film();
        // Le son est dans ma langue : pas de sous-titres complets.
        assert_eq!(prefs(Some("fr"), Some(AUTO)).choose("1", &audio, &subs), Choice { audio: Some(2), subtitle: Some(Some(12)) });
        // Sans langue préférée pour le son, le serveur décide.
        assert_eq!(prefs(None, Some(AUTO)).choose("1", &audio, &subs).subtitle, None);
        // Son étranger, et aucun sous-titre dans ma langue : aucun.
        assert_eq!(prefs(Some("de"), Some(AUTO)).choose("1", &audio, &subs), Choice { audio: None, subtitle: Some(None) });
    }

    #[test]
    fn sous_titres_toujours_dans_une_langue() {
        let (audio, subs) = film();
        // Les sous-titres complets d'abord, jamais les « forcés » s'il y a mieux.
        assert_eq!(prefs(None, Some("en")).choose("1", &audio, &subs), Choice { audio: None, subtitle: Some(Some(10)) });
        assert_eq!(prefs(None, Some("fr")).choose("1", &audio, &[subs[2].clone(), subs[1].clone()]).subtitle, Some(Some(11)));
        // Seulement des « forcés » dans cette langue : ceux-là.
        assert_eq!(prefs(None, Some("fr")).choose("1", &audio, &subs[2..]).subtitle, Some(Some(12)));
        // Aucun dans cette langue : aucun.
        assert_eq!(prefs(None, Some("ja")).choose("1", &audio, &subs).subtitle, Some(None));
    }

    #[test]
    fn le_choix_fait_sur_un_titre_passe_avant_les_reglages() {
        let (audio, subs) = film();
        let mut p = prefs(Some("fr"), Some(OFF));
        // Sur cette série, version originale et sous-titres français.
        remember(&mut p.memory, "serie".into(), Picked::Audio("en".into()), 1);
        remember(&mut p.memory, "serie".into(), Picked::Subtitle("fr".into()), 2);
        assert_eq!(p.choose("serie", &audio, &subs), Choice { audio: Some(1), subtitle: Some(Some(11)) });
        // Les autres titres suivent les Réglages.
        assert_eq!(p.choose("film", &audio, &subs), Choice { audio: Some(2), subtitle: Some(None) });
        // Un seul des deux choisi sur le titre : l'autre vient des Réglages.
        remember(&mut p.memory, "autre".into(), Picked::Subtitle("en".into()), 3);
        assert_eq!(p.choose("autre", &audio, &subs), Choice { audio: Some(2), subtitle: Some(Some(10)) });
        // Sans Réglages, le choix retenu suffit.
        let only = Prefs { memory: p.memory.clone(), ..Prefs::default() };
        assert!(!only.is_empty());
        assert_eq!(only.choose("serie", &audio, &subs), Choice { audio: Some(1), subtitle: Some(Some(11)) });
        assert_eq!(only.choose("film", &audio, &subs), Choice::default());
    }

    #[test]
    fn les_choix_les_plus_anciens_sont_oublies() {
        let mut memory = HashMap::new();
        for i in 0..KEPT as u64 + 25 {
            remember(&mut memory, format!("t{i}"), Picked::Audio("fr".into()), i);
        }
        assert_eq!(memory.len(), KEPT);
        assert!(!memory.contains_key("t0") && !memory.contains_key("t24"));
        assert!(memory.contains_key("t25") && memory.contains_key(&format!("t{}", KEPT + 24)));
        // Un titre revu redevient récent.
        remember(&mut memory, "t25".into(), Picked::Subtitle(OFF.into()), 9999);
        remember(&mut memory, "neuf".into(), Picked::Audio("fr".into()), 10000);
        assert!(memory.contains_key("t25") && !memory.contains_key("t26"));
        assert_eq!(memory["t25"], Memory { audio: Some("fr".into()), subtitle: Some(OFF.into()), at: 9999 });
    }

    #[test]
    fn preferences_lues_dans_la_configuration() {
        let mut c = Config::default();
        // Réglages jamais touchés : le son dans la langue de l'interface, aucun sous-titre.
        let p = Prefs::from_config(&c, "fr");
        assert_eq!((p.audio.as_deref(), p.subtitle.as_deref()), (Some("fr"), Some("off")));
        assert_eq!(Prefs::from_config(&c, "nl").audio.as_deref(), Some("nl"));
        // « Choix du serveur », choisi dans les Réglages : rien n'est appliqué.
        c.ui.insert("maku.audioLang".into(), "server".into());
        c.ui.insert("maku.subtitles".into(), "server".into());
        assert!(Prefs::from_config(&c, "fr").is_empty());
        // L'un sans l'autre.
        c.ui.remove("maku.subtitles");
        let p = Prefs::from_config(&c, "fr");
        assert_eq!((p.audio.as_deref(), p.subtitle.as_deref()), (None, Some("off")));
        c.ui.insert("maku.audioLang".into(), "en".into());
        c.ui.insert("maku.subtitles".into(), "auto".into());
        c.server = Some(crate::config::SavedServer { name: "s".into(), machine_id: "abc".into(), uri: "http://h".into(), token: "t".into(), kind: String::new(), user_id: None });
        // Les choix retenus sont ceux du serveur en cours : le même numéro de titre, sur un autre, n'en hérite pas.
        c.track_memory.insert("plex:abc/42".into(), Memory { audio: Some("en".into()), subtitle: None, at: 1 });
        c.track_memory.insert("plex:autre/42".into(), Memory { audio: Some("de".into()), subtitle: None, at: 1 });
        let p = Prefs::from_config(&c, "fr");
        assert_eq!((p.audio.as_deref(), p.subtitle.as_deref()), (Some("en"), Some("auto")));
        assert_eq!(p.memory.len(), 1);
        assert_eq!(p.memory["42"].audio.as_deref(), Some("en"));
        // Valeurs illisibles : comme un réglage jamais touché.
        c.ui.insert("maku.audioLang".into(), "n'importe quoi".into());
        c.ui.insert("maku.subtitles".into(), "".into());
        let p = Prefs::from_config(&c, "fr");
        assert_eq!((p.audio.as_deref(), p.subtitle.as_deref()), (Some("fr"), Some("off")));
    }
}
