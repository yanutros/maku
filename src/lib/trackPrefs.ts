// Son et sous-titres préférés. Les Réglages gardent une langue pour le son et un choix pour les sous-titres (tant
// qu'on n'y a pas touché : le son dans la langue de l'interface, aucun sous-titre) ; le Rust
// les lit dans la copie des préférences (config.json) et marque ces pistes à la place de celles du serveur, partout
// où il lit une fiche (src-tauri/src/tracks.rs). Un choix fait sur un titre, dans le lecteur ou sur la fiche, est
// retenu à part pour ce titre (une série entière compte pour un titre) et passe avant les Réglages.
import { invoke } from "@tauri-apps/api/core";
import { locale } from "./i18n";

const AUDIO = "maku.audioLang";
const SUBTITLES = "maku.subtitles";

/** Langues proposées dans les Réglages. */
export const TRACK_LANGUAGES = ["fr", "en", "es", "de", "nl", "it", "pt", "ja", "ko", "zh"];

/** Laisser le serveur choisir : une valeur à part entière, gardée comme les autres. */
export const SERVER = "server";

/** Le réglage gardé, ou `fallback` tant que l'utilisateur n'y a pas touché. */
function read(key: string, fallback: string): string {
  try {
    return localStorage.getItem(key) || fallback;
  } catch {
    return fallback;
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Stockage indisponible : la préférence ne peut pas être gardée.
  }
}

/** Langue préférée pour le son (« fr »), ou « server ». Par défaut : la langue de l'interface. */
export const audioLanguage = () => read(AUDIO, locale.split("-")[0]);
export const setAudioLanguage = (lang: string) => write(AUDIO, lang);

/**
 * Sous-titres : « off » (aucun), « auto » (si le son est dans une autre langue), une langue, ou « server ». Par
 * défaut : aucun.
 */
export const subtitlePreference = () => read(SUBTITLES, "off");
export const setSubtitlePreference = (value: string) => write(SUBTITLES, value);

/** Nom d'une langue dans celle de l'interface (« Français », « English »…). */
export function languageName(code: string, locale: string): string {
  try {
    const name = new Intl.DisplayNames([locale], { type: "language" }).of(code) ?? code;
    return name.charAt(0).toUpperCase() + name.slice(1);
  } catch {
    return code.toUpperCase();
  }
}

/**
 * L'utilisateur vient de choisir un son ou des sous-titres pour ce titre : Maku le retient. La piste est désignée par
 * son identifiant sur le serveur (`stream`, 0 : aucun sous-titre) ou par sa langue (`lang`, « off » : aucun) quand
 * le lecteur ne connaît que les pistes du fichier. Un échec ne gêne pas la lecture : le choix vaut pour cette fois.
 */
export function rememberTrack(ratingKey: string, kind: "audio" | "subtitle", choice: { stream?: number; lang?: string }) {
  if (!ratingKey) return;
  invoke("remember_track", { ratingKey, kind, stream: choice.stream ?? null, lang: choice.lang ?? null }).catch(() => {});
}
