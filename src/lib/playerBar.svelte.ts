// Barre du lecteur vidéo (Réglages → Interface) : la capsule complète (par défaut), ou la rangée minimaliste,
// sans cadre, sur un dégradé en bas de l'image. Le choix est gardé sur l'appareil.
export type PlayerBar = "full" | "minimal";

const KEY = "maku.playerBar";

function load(): PlayerBar {
  try {
    return localStorage.getItem(KEY) === "minimal" ? "minimal" : "full";
  } catch {
    return "full";
  }
}

export const playerBar = $state({ style: load() });

export function setPlayerBar(style: PlayerBar) {
  playerBar.style = style;
  try {
    localStorage.setItem(KEY, style);
  } catch {
    // Stockage indisponible : le choix vaut jusqu'à la fermeture.
  }
}
