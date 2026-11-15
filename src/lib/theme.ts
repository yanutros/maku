// Couleur de l'interface (et de l'icône de l'application), gardée sur cet appareil.
import { invoke } from "@tauri-apps/api/core";

export const THEMES = ["jaune", "rouge", "vert", "violet"] as const;
export type Theme = (typeof THEMES)[number];

/** Teinte principale de chaque thème, pour les pastilles du choix (le reste est dans app.css). */
export const THEME_COLORS: Record<Theme, string> = {
  jaune: "#e5a00d",
  rouge: "#e50914",
  vert: "#1db954",
  violet: "#aa5cc3",
};

const KEY = "plexr.theme";

export function savedTheme(): Theme {
  try {
    const t = localStorage.getItem(KEY) as Theme | null;
    return t && THEMES.includes(t) ? t : "jaune";
  } catch {
    return "jaune";
  }
}

/**
 * Applique le thème à l'interface et à l'icône : fenêtre et barre des tâches sur ordinateur,
 * icône du lanceur sur Android (voir MainActivity.kt).
 */
export function applyTheme(theme: Theme, save = false) {
  document.documentElement.dataset.theme = theme;
  if (save) {
    try {
      localStorage.setItem(KEY, theme);
    } catch {
      // Stockage indisponible : le thème ne vaut que pour cette session.
    }
  }
  const android = (window as { PlexrDevice?: { setIconTheme(theme: string): void } }).PlexrDevice;
  if (android) android.setIconTheme(theme);
  else invoke("set_app_icon", { theme }).catch(() => {});
}

// Fond de l'interface, gardé sur cet appareil : la nuit de Maku (par défaut), un gris neutre, ou un gris bleuté.
export const BACKGROUNDS = ["nuit", "graphite", "ardoise"] as const;
export type Background = (typeof BACKGROUNDS)[number];

/** Teinte de chaque fond, pour les pastilles du choix (les nuances dérivées sont dans app.css). */
export const BACKGROUND_COLORS: Record<Background, string> = {
  nuit: "#141518",
  graphite: "#1a1a1a",
  ardoise: "#2f383e",
};

const BACKGROUND_KEY = "plexr.background";

export function savedBackground(): Background {
  try {
    const b = localStorage.getItem(BACKGROUND_KEY) as Background | null;
    return b && BACKGROUNDS.includes(b) ? b : "nuit";
  } catch {
    return "nuit";
  }
}

export function applyBackground(background: Background, save = false) {
  if (background === "nuit") delete document.documentElement.dataset.background;
  else document.documentElement.dataset.background = background;
  if (save) {
    try {
      localStorage.setItem(BACKGROUND_KEY, background);
    } catch {
      // Stockage indisponible : le fond ne vaut que pour cette session.
    }
  }
}
