import type { Genre, Item, SeerrMedia } from "./api";
import { reveal } from "./spatial";

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
  /** Téléchargements de l'appareil, lisibles hors ligne (Android). */
  | { name: "downloads" }
  /** `seerr` : acteur connu par Seerr (TMDB) : ses films et séries, à demander, plutôt que ceux du serveur. */
  | { name: "person"; id: string; personName: string; photo: string | null; seerr?: boolean }
  | { name: "settings"; page?: string };

export const nav = $state({
  view: { name: "home" } as View,
  history: [] as View[],
  /** Média Seerr dont la fenêtre de demande est ouverte. */
  requesting: null as SeerrMedia | null,
  /** Menu « ⋮ » ouvert sur une jaquette, à la position du clic. */
  menu: null as { item: Item; x: number; y: number; here?: boolean } | null,
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
  /** Film ou série à ranger dans une collection (fenêtre « Ajouter à la collection » ouverte). */
  collectionFor: null as Item | null,
  /** Musique à ranger dans une playlist (fenêtre « Ajouter à une playlist » ouverte). */
  playlistFor: null as Item | null,
  /** Titre dont la fenêtre « Informations » est ouverte. */
  infoFor: null as Item | null,
  /** Serveur connecté : Plex note de 1 à 5 étoiles, Jellyfin et Emby seulement « J'aime ». */
  serverKind: "plex" as "plex" | "jellyfin",
  /** Écran « Qui regarde ? » ouvert depuis la carte du profil (Plex Home). */
  profiles: false,
  /** Le Plex Home du compte a plusieurs profils : la carte du profil ouvre leur choix. */
  homeProfiles: 0,
});

// Chaque page ouverte ajoute une entrée à l'historique du navigateur : le bouton Retour d'Android
// (et les boutons précédent de la souris) reviennent ainsi à la page d'avant.
/**
 * Où l'on était sur une page : défilement et élément sélectionné (`data-key` du titre, `data-cell` de sa
 * place dans les rangées TV). On y revient après une action qui recharge la page, et au retour d'une autre.
 */
/** `index` : à défaut de `data-key`, la place de l'élément parmi ceux qu'on peut sélectionner dans la page. */
export type Spot = { scroll: number; key?: string; cell?: string; index?: number };

const SELECTABLE = "button:not(:disabled), a[href], [tabindex]:not([tabindex=\"-1\"])";

// En parallèle de `nav.history` : l'endroit quitté sur chaque page de l'historique.
const spots: Spot[] = [];
let pendingSpot: Spot | null = null;

function spot(): Spot {
  const el = document.activeElement as HTMLElement | null;
  const content = document.querySelector(".content");
  const index = el && content?.contains(el) ? [...content.querySelectorAll(SELECTABLE)].indexOf(el) : -1;
  return {
    index: index >= 0 ? index : undefined,
    scroll: document.querySelector(".content")?.scrollTop ?? 0,
    key: el?.closest<HTMLElement>("[data-key]")?.dataset.key,
    cell: el?.closest<HTMLElement>("[data-cell]")?.dataset.cell,
  };
}

/** L'endroit où reprendre la vue qui s'affiche (une seule fois), ou null pour repartir du haut. */
export function takeSpot(): Spot | null {
  const s = pendingSpot;
  pendingSpot = null;
  return s;
}

/**
 * Ramène la page rechargée à l'endroit quitté. Ses données arrivent après coup : on attend (5 s au plus)
 * que la page soit assez haute et que l'élément réapparaisse. Une touche ou un clic de l'utilisateur arrête tout.
 */
export function restoreSpot(content: HTMLElement, s: Spot) {
  const started = Date.now();
  let timer: ReturnType<typeof setTimeout>;
  const stop = () => {
    clearTimeout(timer);
    removeEventListener("keydown", stop, true);
    removeEventListener("pointerdown", stop, true);
  };
  addEventListener("keydown", stop, true);
  addEventListener("pointerdown", stop, true);
  const target = (): HTMLElement | null => {
    const same = s.key ? [...content.querySelectorAll<HTMLElement>("[data-key]")].filter((el) => el.dataset.key === s.key) : [];
    const atCell = (el: HTMLElement) => el.closest<HTMLElement>("[data-cell]")?.dataset.cell === s.cell;
    // Le même titre à la même place, sinon ailleurs ; sinon (retiré de sa rangée) celui qui l'a remplacé.
    const holder =
      same.find(atCell) ?? same[0] ?? (s.cell ? [...content.querySelectorAll<HTMLElement>("[data-cell]")].find((el) => el.dataset.cell === s.cell) : undefined);
    if (!holder) return !s.key && !s.cell && s.index !== undefined ? (content.querySelectorAll<HTMLElement>(SELECTABLE)[s.index] ?? null) : null;
    return holder.matches("button, a[href], [tabindex]") ? holder : holder.querySelector<HTMLElement>("button, a[href], [tabindex]");
  };
  const attempt = () => {
    const el = target();
    const tall = content.scrollHeight - content.clientHeight >= s.scroll;
    const late = Date.now() - started > 5000;
    if (tall || late) content.scrollTop = s.scroll;
    // Sur téléviseur, la sélection compte autant que le défilement.
    const tv = document.documentElement.classList.contains("tv");
    if (tall && (el || !tv || !(s.key || s.cell || s.index !== undefined))) {
      if (el && tv) {
        el.focus({ preventScroll: true });
        // Rangée qui défile de côté (distribution…) : l'élément revient à l'écran.
        reveal(el, { y: false });
      }
      return stop();
    }
    if (late) return stop();
    timer = setTimeout(attempt, 100);
  };
  attempt();
}

export function go(view: View) {
  spots.push(spot());
  nav.history.push(nav.view);
  nav.view = view;
  history.pushState({ plexr: nav.history.length }, "");
}

/** Recharge la vue affichée (après une modification). */
export function reload() {
  pendingSpot = spot();
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
    if (previous) {
      pendingSpot = spots.pop() ?? null;
      nav.view = previous;
    }
  });
}

let toastTimer: ReturnType<typeof setTimeout> | undefined;

export function toast(text: string, error = false) {
  nav.toast = { text, error };
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (nav.toast = null), 4000);
}
