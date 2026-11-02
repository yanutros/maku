import type { Genre, Item, SeerrMedia } from "./api";

export type View =
  | { name: "home" }
  | { name: "libraries" }
  | {
      name: "section";
      key: string;
      title: string;
      kind: string;
      tab?: "recommended" | "library" | "collections" | "categories";
      /** Catégorie ouverte (la bibliothèque filtrée sur ce genre). */
      genre?: Genre | null;
      /** Pour revenir au même endroit : jaquette sélectionnée, défilement, nombre de titres chargés. */
      focusKey?: string | null;
      scroll?: number;
      loaded?: number;
    }
  | { name: "item"; ratingKey: string }
  | { name: "search"; query: string }
  | { name: "discover" }
  | { name: "seerr"; mediaType: "movie" | "tv"; tmdbId: number }
  | { name: "stats" }
  | { name: "person"; id: string; personName: string; photo: string | null }
  | { name: "settings" };

export const nav = $state({
  view: { name: "home" } as View,
  history: [] as View[],
  /** Média Seerr dont la fenêtre de demande est ouverte. */
  requesting: null as SeerrMedia | null,
  /** Menu « ⋮ » ouvert sur une jaquette, à la position du clic. */
  menu: null as { item: Item; x: number; y: number } | null,
  /** Téléviseur : menu d'un titre (appui long sur OK ou touche Menu), en panneau à droite. */
  tvMenu: null as { item: Item } | null,
  /** Élément Plex dont la fenêtre de modification est ouverte. */
  editing: null as Item | null,
  /** Le compte connecté administre le serveur (édition, réglages). */
  admin: false,
  /** Bande-annonce YouTube ouverte par-dessus la page (identifiant de la vidéo). */
  trailer: null as string | null,
  /** Téléviseur : lien d'autorisation Tracearr, montré en QR code à scanner avec un téléphone. */
  authUrl: null as string | null,
  toast: null as { text: string; error: boolean } | null,
});

// Chaque page ouverte ajoute une entrée à l'historique du navigateur : le bouton Retour d'Android
// (et les boutons précédent de la souris) reviennent ainsi à la page d'avant.
export function go(view: View) {
  nav.history.push(nav.view);
  nav.view = view;
  history.pushState({ plexr: nav.history.length }, "");
}

/** Recharge la vue affichée (après une modification). */
export function reload() {
  nav.view = { ...nav.view };
}

export function back() {
  if (nav.history.length) history.back();
}

const backHandlers: (() => boolean)[] = [];

/** Le gestionnaire renvoie true quand il a fermé quelque chose (lecteur, menu…) : on reste sur la page. */
export function onBack(handler: () => boolean) {
  backHandlers.push(handler);
}

export function setupBackButton() {
  // Une entrée de garde : le premier Retour ferme ce qui est ouvert au lieu de quitter l'application.
  history.replaceState({ plexr: 0 }, "");
  history.pushState({ plexr: 0 }, "");
  window.addEventListener("popstate", () => {
    if (backHandlers.some((h) => h())) {
      history.pushState({ plexr: nav.history.length }, "");
      return;
    }
    const previous = nav.history.pop();
    if (previous) nav.view = previous;
  });
}

let toastTimer: ReturnType<typeof setTimeout> | undefined;

export function toast(text: string, error = false) {
  nav.toast = { text, error };
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (nav.toast = null), 4000);
}
