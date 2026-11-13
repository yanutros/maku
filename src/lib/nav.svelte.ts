import type { Genre, Item, SeerrMedia } from "./api";
import { reveal } from "./spatial";
import { device } from "./device";

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
      /** Filtres d'une collection intelligente à modifier (onglet Bibliothèque, filtres avancés ouverts). */
      smart?: { ratingKey: string; title: string; content: string } | null;
    }
  | { name: "item"; ratingKey: string }
  /** Fiche d'un « Mix of Three » (lib/mixes.svelte.ts). */
  | { name: "mix"; id: string }
  | { name: "search"; query: string }
  | { name: "discover" }
  | { name: "seerr"; mediaType: "movie" | "tv"; tmdbId: number }
  | { name: "stats" }
  /** Téléchargements de l'appareil, lisibles hors ligne (Android). */
  | { name: "downloads" }
  /** `seerr` : acteur connu par Seerr (TMDB) : ses films et séries, à demander, plutôt que ceux du serveur. */
  | { name: "person"; id: string; personName: string; photo: string | null; seerr?: boolean }
  | { name: "settings"; page?: string }
  /** Catégorie Admin (administrateur, sur ordinateur) : Dashboard ou Réglages du serveur, et leur section. */
  | { name: "admin"; page: "dashboard" | "settings"; tab?: string };

export const nav = $state({
  view: { name: "home" } as View,
  history: [] as View[],
  /** Média Seerr dont la fenêtre de demande est ouverte. */
  requesting: null as SeerrMedia | null,
  /** Lecteur vidéo : ferme le menu ouvert (qualité, sous-titres, audio) ; Retour le ferme avant de quitter le film. */
  closePlayerMenu: null as (() => void) | null,
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
  /** « Ajouter à une collection » pour tous les titres de la bibliothèque filtrée (`keys` les rassemble). */
  collectionBatch: null as { sectionKey: string; kind: string; count: number; keys: () => Promise<string[]> } | null,
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
/**
 * Arrête la remise en place en cours (restoreSpot). Une touche du clavier le fait déjà ; la touche Retour
 * d'Android n'en est pas une pour la page : sans cela, une page qui finit de charger pourrait rendre la
 * sélection à la jaquette quittée juste après qu'un second Retour l'a passée au menu.
 */
let cancelRestore: (() => void) | null = null;

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
  cancelRestore?.();
  const stop = () => {
    clearTimeout(timer);
    removeEventListener("keydown", stop, true);
    removeEventListener("pointerdown", stop, true);
    if (cancelRestore === stop) cancelRestore = null;
  };
  cancelRestore = stop;
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

/**
 * Destination du menu latéral (ou des onglets du téléphone) sur téléviseur et mobile : comme dans Plex, c'est une
 * page de premier niveau, qui n'empile pas la précédente. Retour y ramène au menu, puis à l'accueil.
 */
export function goTop(view: View) {
  if (!device.mobile && !device.tv) return go(view);
  nav.history = [];
  spots.length = 0;
  pendingSpot = null;
  nav.view = view;
}

/** Remplace la vue affichée sans l'ajouter à l'historique (la page quittée n'existe plus : collection convertie…). */
export function replace(view: View) {
  nav.view = view;
}

/** Recharge la vue affichée (après une modification). */
export function reload() {
  pendingSpot = spot();
  nav.view = { ...nav.view };
}

/** Touche Retour d'Android prise en charge par la page (et non par l'historique du WebView). */
let androidBack = false;

export function back() {
  if (androidBack) popView();
  else if (nav.history.length) history.back();
}

const backHandlers: (() => boolean)[] = [];

/**
 * Le gestionnaire renvoie true quand il a fermé quelque chose (lecteur, menu…) : on reste sur la page.
 * Renvoie de quoi le retirer.
 */
export function onBack(handler: () => boolean) {
  backHandlers.push(handler);
  return () => {
    const i = backHandlers.indexOf(handler);
    if (i >= 0) backHandlers.splice(i, 1);
  };
}

/** Page précédente, à l'endroit quitté (jaquette sélectionnée, défilement). False : pas de page avant. */
function popView(): boolean {
  const previous = nav.history.pop();
  if (!previous) return false;
  pendingSpot = spots.pop() ?? null;
  nav.view = previous;
  return true;
}

const sidebarItem = (home = false) =>
  document.querySelector<HTMLElement>(home ? "aside [data-nav-home]" : 'aside [aria-current="page"]') ??
  document.querySelector<HTMLElement>("aside [data-nav-home]");

/**
 * Retour sur une page de premier niveau (accueil, bibliothèque ouverte depuis le menu…), comme dans Plex :
 * sur téléviseur, la sélection passe d'abord au menu de gauche ; depuis le menu (ou sur mobile), on revient à
 * l'accueil ; depuis l'accueil, on quitte. Renvoie ce qui a été fait, ou null : il faut quitter l'application.
 */
function rootBack(): string | null {
  const inMenu = !!document.activeElement?.closest("aside");
  if (device.tv && !inMenu) {
    const item = sidebarItem();
    if (item) {
      item.focus({ preventScroll: true });
      return "menu de gauche";
    }
  }
  if (nav.view.name !== "home") {
    goTop({ name: "home" });
    if (device.tv) setTimeout(() => sidebarItem(true)?.focus({ preventScroll: true }));
    return "accueil";
  }
  return null;
}

/**
 * Ce que fait Retour : fermer ce qui est ouvert, sinon la page précédente, sinon le menu puis l'accueil.
 * Renvoie ce qui a été fait (pour le journal), ou null quand il faut quitter.
 */
function handleBack(): string | null {
  cancelRestore?.();
  if (backHandlers.some((h) => h())) return "fermeture de ce qui était ouvert";
  if (popView()) return "page précédente";
  return rootBack();
}

/** Journal de diagnostic (Android) : l'élément sélectionné, en quelques mots. */
const diagLog = (message: string) => (globalThis as { PlexrDevice?: { diagLog?(m: string): void } }).PlexrDevice?.diagLog?.(message);
function describe(el: Element | null): string {
  if (!el || el === document.body) return "rien";
  const label = (el.getAttribute("aria-label") ?? el.getAttribute("title") ?? el.textContent ?? "").trim().replace(/\s+/g, " ").slice(0, 40);
  return `${el.closest("aside") ? "menu" : el.closest("main") ? "page" : "autre"} : ${el.tagName.toLowerCase()} « ${label} »`;
}

/** Après un Retour, note dans le journal ce qui reprend la sélection pendant 1,5 s (et d'où, s'il ne s'agit pas de nous). */
let watching: (() => void) | null = null;
function watchFocus() {
  watching?.();
  const onFocus = (e: FocusEvent) => diagLog(`Sélection après Retour : ${describe(e.target as Element)}`);
  addEventListener("focusin", onFocus, true);
  const timer = setTimeout(() => watching?.(), 1500);
  watching = () => {
    removeEventListener("focusin", onFocus, true);
    clearTimeout(timer);
    watching = null;
  };
}

export function setupBackButton() {
  // Android : la page traite elle-même la touche Retour. Sans cela, Tauri recule dans l'historique du WebView,
  // qui peut sauter les entrées ajoutées sans geste de l'utilisateur et, à court d'entrées, ferme l'application
  // en pleine navigation. Ici, l'application n'est quittée (mise en arrière-plan) que depuis l'accueil.
  if (device.os === "android") {
    androidBack = true;
    import("@tauri-apps/api/app")
      .then(({ onBackButtonPress }) =>
        onBackButtonPress(() => {
          const before = describe(document.activeElement);
          watchFocus();
          const done = handleBack();
          diagLog(`Retour (${nav.view.name}, sélection ${before}) : ${done ?? "Maku passe en arrière-plan"}`);
          if (!done) (globalThis as { PlexrDevice?: { leave?(): void } }).PlexrDevice?.leave?.();
        }),
      )
      .catch(() => (androidBack = false));
  }
  // Ailleurs (bouton précédent de la souris, essais dans un navigateur) : une entrée de garde, pour que le
  // premier Retour ferme ce qui est ouvert au lieu de quitter la page.
  history.replaceState({ plexr: 0 }, "");
  history.pushState({ plexr: 0 }, "");
  window.addEventListener("popstate", () => {
    cancelRestore?.();
    if (backHandlers.some((h) => h())) {
      history.pushState({ plexr: nav.history.length }, "");
      return;
    }
    if (popView()) return;
    if (rootBack()) history.pushState({ plexr: 0 }, "");
  });
}

let toastTimer: ReturnType<typeof setTimeout> | undefined;

export function toast(text: string, error = false) {
  nav.toast = { text, error };
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (nav.toast = null), 4000);
}
