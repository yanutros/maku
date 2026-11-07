// Navigation à la télécommande (Android TV) : les flèches déplacent le focus vers l'élément
// le plus proche dans la direction choisie, et « OK » l'active comme un clic.
// Chaque zone (`data-nav-zone` : menu, contenu) retient son dernier élément sélectionné, et
// les éléments `data-nav-low` (recherche, onglets) ne sont choisis qu'en dernier recours.
// Une page en rangées (`data-nav-rows`, comme les fiches) se parcourt comme dans Plex et Wholphin :
// haut / bas passent d'une rangée (`data-nav-row`) à l'autre, gauche / droite restent dans la rangée.
// Il y a toujours un élément sélectionné : à l'arrivée, après une lecture, quand il disparaît.

import { tick } from "svelte";

const FOCUSABLE = 'button:not(:disabled), a[href], input:not([type="hidden"]), select, textarea, [tabindex]:not([tabindex="-1"])';

type Dir = "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight";

function visible(el: HTMLElement): boolean {
  const r = el.getBoundingClientRect();
  if (r.width < 4 || r.height < 4) return false;
  const style = getComputedStyle(el);
  return style.visibility !== "hidden" && style.opacity !== "0" && !el.closest("[inert], [aria-hidden='true']");
}

function onScreen(el: HTMLElement): boolean {
  const r = el.getBoundingClientRect();
  return r.bottom > 0 && r.top < window.innerHeight && r.right > 0 && r.left < window.innerWidth;
}

const last = new WeakMap<Element, HTMLElement>();

function zone(el: Element | null): Element | null {
  return el?.closest("[data-nav-zone]") ?? null;
}

/** Zone où chercher : la fenêtre modale ouverte s'il y en a une, sinon toute la page. */
function scope(): ParentNode {
  const layers = document.querySelectorAll<HTMLElement>('[role="dialog"], [role="menu"]');
  return layers.length ? layers[layers.length - 1] : document;
}

function best(from: DOMRect, dir: Dir, candidates: HTMLElement[]): HTMLElement | null {
  const cx = from.left + from.width / 2;
  const cy = from.top + from.height / 2;
  let winner: HTMLElement | null = null;
  let score = Infinity;
  for (const el of candidates) {
    const r = el.getBoundingClientRect();
    const x = r.left + r.width / 2;
    const y = r.top + r.height / 2;
    // L'élément doit se trouver franchement dans la direction demandée.
    const ok =
      (dir === "ArrowRight" && r.left >= from.right - 8) ||
      (dir === "ArrowLeft" && r.right <= from.left + 8) ||
      (dir === "ArrowDown" && r.top >= from.bottom - 8) ||
      (dir === "ArrowUp" && r.bottom <= from.top + 8);
    if (!ok) continue;
    const horizontal = dir === "ArrowLeft" || dir === "ArrowRight";
    const main = horizontal ? Math.abs(x - cx) : Math.abs(y - cy);
    const side = horizontal ? Math.abs(y - cy) : Math.abs(x - cx);
    // On privilégie l'alignement : un léger écart latéral coûte cher.
    const s = main + side * 2.5 + (el.closest("[data-nav-low]") ? 1e6 : 0);
    if (s < score) {
      score = s;
      winner = el;
    }
  }
  return winner;
}

const rowLast = new WeakMap<Element, HTMLElement>();

/** Ancêtre qui défile dans ce sens (sinon la page). */
function scroller(el: HTMLElement, axis: "x" | "y"): HTMLElement | null {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const style = getComputedStyle(p);
    const overflow = axis === "y" ? style.overflowY : style.overflowX;
    const room = axis === "y" ? p.scrollHeight - p.clientHeight : p.scrollWidth - p.clientWidth;
    if ((overflow === "auto" || overflow === "scroll") && room > 1) return p;
  }
  return document.scrollingElement as HTMLElement | null;
}

/** Cadre de sélection (contour et son décalage) et marge laissée autour, comme dans Plex et Wholphin. */
const RING = 8;

/**
 * Fait défiler pour que l'élément sélectionné soit vu en entier, cadre compris, avec une marge par rapport
 * aux bords — `scrollIntoView` le collait au bord, et le cadre (voire le bas de l'élément) sortait de
 * l'écran. La zone visible est celle de l'écran réel (`visualViewport`), qui peut être plus petite que la
 * page sur un téléviseur. `center` : l'élément vient au milieu (rangées d'une fiche, grilles).
 */
export function reveal(el: HTMLElement, opts: { center?: boolean; inline?: "center" | "nearest"; y?: boolean } = {}) {
  const vv = window.visualViewport;
  const screen = {
    top: vv?.offsetTop ?? 0,
    bottom: vv ? vv.offsetTop + vv.height : window.innerHeight,
    left: vv?.offsetLeft ?? 0,
    right: vv ? vv.offsetLeft + vv.width : window.innerWidth,
  };
  const r = el.getBoundingClientRect();
  const delta = (start: number, end: number, from: number, to: number, center: boolean, margin: number) => {
    const lo = from + margin;
    const hi = to - margin;
    if (center) return (start + end) / 2 - (from + to) / 2;
    if (end - start > hi - lo) return start - lo;
    if (start < lo) return start - lo;
    if (end > hi) return end - hi;
    return 0;
  };
  const sy = scroller(el, "y");
  if (sy && opts.y !== false) {
    const box = sy === document.scrollingElement ? screen : sy.getBoundingClientRect();
    const top = Math.max(box.top, screen.top);
    const bottom = Math.min(box.bottom, screen.bottom);
    const dy = delta(r.top - RING, r.bottom + RING, top, bottom, !!opts.center, Math.min(64, (bottom - top) * 0.1));
    if (Math.abs(dy) > 1) sy.scrollBy({ top: dy, behavior: "smooth" });
  }
  const sx = scroller(el, "x");
  if (sx && sx !== document.scrollingElement) {
    const box = sx.getBoundingClientRect();
    const left = Math.max(box.left, screen.left);
    const right = Math.min(box.right, screen.right);
    const dx = delta(r.left - RING, r.right + RING, left, right, opts.inline === "center", Math.min(48, (right - left) * 0.05));
    if (Math.abs(dx) > 1) sx.scrollBy({ left: dx, behavior: "smooth" });
  }
}

/** Visible horizontalement (une rangée qui défile peut avoir son début hors de l'écran). */
function onScreenX(el: HTMLElement): boolean {
  const r = el.getBoundingClientRect();
  return r.left >= 0 && r.right <= window.innerWidth;
}

function focusables(root: ParentNode): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.tabIndex >= 0 && visible(el));
}

function show(el: HTMLElement, first: boolean) {
  el.focus({ preventScroll: true });
  // Première rangée : on remonte tout en haut, pour revoir le bandeau du titre.
  if (first) el.closest<HTMLElement>("[data-nav-rows]")?.closest(".content")?.scrollTo({ top: 0, behavior: "smooth" });
  else reveal(el, { center: true });
}

/** Page en rangées : haut / bas d'une rangée à l'autre. `false` : la flèche n'est pas pour elle. */
function moveInRows(current: HTMLElement, dir: Dir): boolean {
  const page = current.closest("[data-nav-rows]");
  const row = current.closest("[data-nav-row]");
  if (!page || !row) return false;
  const inRow = focusables(row);
  if (dir === "ArrowLeft" || dir === "ArrowRight") {
    // Dans la rangée ; au bord gauche, la flèche repart vers le menu (navigation ordinaire).
    const target = best(current.getBoundingClientRect(), dir, inRow.filter((el) => el !== current));
    if (target) {
      target.focus({ preventScroll: true });
      reveal(target, { inline: "center" });
      return true;
    }
    return dir === "ArrowRight";
  }
  // Une grille (`data-nav-row="grid"`, saisons) se parcourt d'abord ligne par ligne.
  if (row.getAttribute("data-nav-row") === "grid") {
    const target = best(current.getBoundingClientRect(), dir, inRow.filter((el) => el !== current));
    if (target) {
      show(target, false);
      return true;
    }
  }
  const rows = [...page.querySelectorAll("[data-nav-row]")].filter((r) => focusables(r).length);
  const i = rows.indexOf(row);
  const next = rows[i + (dir === "ArrowDown" ? 1 : -1)];
  // Tout en haut ou tout en bas : on reste où l'on est (jamais vers le menu ni la recherche).
  if (!next) return true;
  rowLast.set(row, current);
  const candidates = focusables(next);
  // L'élément quitté dans cette rangée, sinon son premier élément visible (comme Plex).
  const remembered = rowLast.get(next);
  const target =
    (remembered && candidates.includes(remembered) && remembered) || candidates.find(onScreenX) || candidates[0];
  show(target, rows.indexOf(next) === 0);
  return true;
}

/** Haut depuis la première rangée : l'onglet actif de la page (Recommandé, Bibliothèque…), s'il y en a. */
export function focusTabs(): boolean {
  const tab =
    document.querySelector<HTMLElement>('main [role="tablist"] [aria-selected="true"]') ??
    document.querySelector<HTMLElement>('main [role="tablist"] [role="tab"]');
  if (!tab || !visible(tab)) return false;
  tab.focus({ preventScroll: true });
  reveal(tab);
  return true;
}

/**
 * Changement d'onglet fait depuis la barre d'onglets : la barre d'onglets peut être redessinée ailleurs (sur
 * « Recommandé », elle fait partie des rangées TV), et le bouton sélectionné disparaît avec l'ancienne. On
 * rend alors la sélection au nouvel onglet actif, au lieu de la laisser partir sur la première jaquette.
 */
export function keepTabFocus() {
  if (document.activeElement?.getAttribute("role") !== "tab") return;
  tick().then(() => {
    const tab = document.querySelector<HTMLElement>('main [role="tablist"] [aria-selected="true"]');
    if (tab && tab !== document.activeElement) tab.focus({ preventScroll: true });
  });
}

/** Sans élément sélectionné : celui de la page (`data-nav-default`), sinon le premier du contenu. */
export function ensureFocus() {
  const current = document.activeElement as HTMLElement | null;
  if (current && current !== document.body && current.matches(FOCUSABLE) && visible(current)) return;
  const layer = scope();
  if (layer !== document) return focusables(layer)[0]?.focus({ preventScroll: true });
  // Lecteur vidéo, sinon l'élément par défaut de la page, sinon le premier du contenu. Tant que la
  // page charge ses données, rien : l'élément viendra avec elles (jamais le menu de gauche).
  const player = document.querySelector<HTMLElement>(".overlay");
  const def = document.querySelector<HTMLElement>("[data-nav-default]");
  const area = document.querySelector("[data-nav-rows]") ?? document.querySelector("main");
  const start =
    (player && focusables(player)[0]) || (def && visible(def) ? def : null) || (area ? focusables(area).find((el) => !el.closest("[data-nav-low], header")) : undefined);
  start?.focus({ preventScroll: true });
}

export function moveFocus(dir: Dir): boolean {
  const current = document.activeElement as HTMLElement | null;
  if (current && current !== document.body && moveInRows(current, dir)) return true;
  // Onglets (Recommandé, Bibliothèque…) : gauche / droite d'un onglet à l'autre ; au premier, la
  // flèche gauche rejoint le menu, au dernier la droite ne fait rien.
  const tabs = current?.closest('[role="tablist"]');
  if (current && tabs && (dir === "ArrowLeft" || dir === "ArrowRight")) {
    const list = focusables(tabs);
    const next = list[list.indexOf(current) + (dir === "ArrowRight" ? 1 : -1)];
    if (next) {
      next.focus({ preventScroll: true });
      return true;
    }
    if (dir === "ArrowRight") return true;
  }
  // Rien de sélectionné (premier appui, ou focus posé par le WebView sur un bloc de la page) :
  // on part de l'élément par défaut de la page.
  if (!current || current === document.body || !current.matches(FOCUSABLE)) {
    const start = document.querySelector<HTMLElement>("[data-nav-default]");
    if (start && visible(start)) {
      start.focus({ preventScroll: true });
      return true;
    }
  }
  const all = [...scope().querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (el) => el !== current && el.tabIndex >= 0 && visible(el),
  );
  const from =
    current && current !== document.body
      ? current.getBoundingClientRect()
      : new DOMRect(window.innerWidth / 2, 0, 0, 0);
  let target = all.length ? best(from, dir, all) : null;
  // Haut depuis le premier élément d'une page descendue : on remonte d'abord tout en haut, pour revoir
  // ce qui n'a rien à sélectionner (bandeau des statistiques…), avant de quitter le contenu.
  const content = document.querySelector<HTMLElement>(".content");
  if (dir === "ArrowUp" && current && content?.contains(current) && content.scrollTop > 0 && (!target || !content.contains(target))) {
    content.scrollTo({ top: 0, behavior: "smooth" });
    return true;
  }
  if (!target) return false;
  // En changeant de zone, on retrouve l'élément quitté s'il est encore à l'écran.
  const into = zone(target);
  if (into && into !== zone(current)) {
    // En entrant dans une zone : l'élément quitté, sinon celui de la page en cours (menu).
    const back = last.get(into) ?? into.querySelector<HTMLElement>('[aria-current="page"]');
    if (back && back.isConnected && visible(back) && onScreen(back)) target = back;
  }
  target.focus({ preventScroll: true });
  reveal(target);
  return true;
}

/** Active la navigation à la télécommande. `skip()` : true quand une autre partie gère les flèches. */
export function enableSpatialNavigation(skip: () => boolean) {
  window.addEventListener("focusin", (e) => {
    const el = e.target as HTMLElement;
    const z = zone(el);
    if (z && !el.closest("[data-nav-low]")) last.set(z, el);
  });
  window.addEventListener(
    "keydown",
    (e) => {
      if (!["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key) || skip()) return;
      const target = e.target as HTMLElement;
      // Les grilles `data-nav-grid` (accueil TV) gèrent elles-mêmes leurs flèches.
      if (target.closest?.("[data-nav-grid]")) return;
      // Dans un champ de saisie, gauche / droite déplacent le curseur.
      if (target.matches("input[type=text], input[type=search], textarea") && (e.key === "ArrowLeft" || e.key === "ArrowRight")) return;
      // Une liste déroulante s'ouvre avec OK ; dans une fiche, les flèches changent de rangée.
      if (target.matches("select") && !target.closest("[data-nav-row]")) return;
      if (moveFocus(e.key as Dir)) e.preventDefault();
    },
    true,
  );
  // Champs de saisie : les traverser aux flèches n'ouvre pas le clavier (ils passent en lecture
  // seule) ; OK les rend modifiables et ouvre le clavier. Avec `inputmode="none"`, Android garderait
  // la touche OK pour lui : la page ne la recevrait jamais.
  const TEXT = 'input:not([type="checkbox"], [type="radio"], [type="range"], [type="button"], [type="submit"]), textarea';
  const typing = new WeakSet<HTMLInputElement>();
  window.addEventListener("focusin", (e) => {
    const el = e.target as HTMLInputElement;
    if (!el.matches?.(TEXT) || typing.has(el) || (el.readOnly && !el.dataset.tvLock)) return;
    el.dataset.tvLock = "1";
    el.readOnly = true;
  });
  window.addEventListener("focusout", (e) => typing.delete(e.target as HTMLInputElement));
  window.addEventListener(
    "keydown",
    (e) => {
      const el = e.target as HTMLInputElement;
      if (e.key !== "Enter" || !el.dataset?.tvLock || typing.has(el)) return;
      e.preventDefault();
      el.readOnly = false;
      typing.add(el);
      // Android a déjà retiré la sélection du champ : on la lui rend, puis il ouvre le clavier.
      el.focus({ preventScroll: true });
      setTimeout(() => (window as { PlexrDevice?: { showKeyboard?: () => void } }).PlexrDevice?.showKeyboard?.(), 0);
    },
    true,
  );
  // Toujours un élément sélectionné et visible : au chargement des pages (les données arrivent
  // après coup), à la fin d'une lecture, au retour dans Maku.
  let pending = 0;
  const later = () => {
    cancelAnimationFrame(pending);
    pending = requestAnimationFrame(() => setTimeout(ensureFocus, 60));
  };
  new MutationObserver(later).observe(document.body, { childList: true, subtree: true });
  window.addEventListener("focusout", later);
  document.addEventListener("visibilitychange", () => document.visibilityState === "visible" && later());
}
