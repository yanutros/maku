// Téléviseur : fond flouté aux couleurs d'une image (pochette sélectionnée dans la musique, titre le plus
// regardé dans les statistiques), posé par App.svelte derrière toute la fenêtre, menu compris. Les pages
// ne peuvent pas le dessiner elles-mêmes sur TV : elles glissent vers la droite quand le menu se déplie.

export const ambient = $state<{ image: string | null }>({ image: null });

/** Pose le fond pendant la vie d'une page ; renvoie la fonction qui le retire. */
export function useAmbient(image: string | null) {
  ambient.image = image;
  return () => {
    if (ambient.image === image) ambient.image = null;
  };
}
