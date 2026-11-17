// Barre du lecteur vidéo (Réglages → Interface) : la rangée minimaliste (par défaut), sans cadre, sur un dégradé en bas
// de l'image, ou la capsule complète. Le choix est gardé sur l'appareil.
export type PlayerBar = "full" | "minimal";

const KEY = "maku.playerBar";

function load(): PlayerBar {
  try {
    return localStorage.getItem(KEY) === "full" ? "full" : "minimal";
  } catch {
    return "minimal";
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
