// Kavita : bibliothèques de livres, BD et mangas, mêlées à celles du serveur vidéo (sauf au téléviseur), et
// préférences du lecteur. Les appels passent par le moteur (kavita.rs), qui garde la clé d'API.
import { api, type BookLibrary, type BookSeries, type Section } from "./api";

export const books = $state({
  /** Bibliothèques de Kavita, telles que lues à la connexion. */
  libraries: [] as BookLibrary[],
  /** Chapitre ouvert dans le lecteur (par-dessus la page), avec la série d'où il a été ouvert et son format. */
  reading: null as { chapterId: number; seriesId: number; format: BookSeries["format"] } | null,
});

/** Clé d'une bibliothèque Kavita parmi les bibliothèques de Maku. */
export const bookKey = (id: number) => `kavita:${id}`;

/** Numéro Kavita d'une clé « kavita:<numéro> », ou null. */
export function bookLibraryId(key: string): number | null {
  const m = /^kavita:(\d+)$/.exec(key);
  return m ? Number(m[1]) : null;
}

/** Les bibliothèques de Kavita, comme des bibliothèques de Maku (vides si Kavita ne répond pas). */
export async function bookSections(): Promise<Section[]> {
  try {
    books.libraries = await api.kavitaLibraries();
  } catch {
    books.libraries = [];
  }
  return books.libraries.map((l) => ({ key: bookKey(l.id), title: l.name, kind: "book" }));
}

export const libraryKind = (id: number) => books.libraries.find((l) => l.id === id)?.kind ?? "comic";

// --- Sens de lecture, retenu par série (un manga se lit de droite à gauche par défaut) ---

export type Direction = "ltr" | "rtl" | "vertical";
const DIRECTION_KEY = "maku.readerDirection";

function directions(): Record<string, Direction> {
  try {
    const d = JSON.parse(localStorage.getItem(DIRECTION_KEY) ?? "{}");
    return d && typeof d === "object" ? d : {};
  } catch {
    return {};
  }
}

export function directionFor(seriesId: number, kind: BookLibrary["kind"]): Direction {
  const d = directions()[String(seriesId)];
  return d === "ltr" || d === "rtl" || d === "vertical" ? d : kind === "manga" ? "rtl" : "ltr";
}

export function setDirection(seriesId: number, direction: Direction) {
  const all = directions();
  all[String(seriesId)] = direction;
  // Les 300 dernières séries réglées suffisent.
  const kept = Object.fromEntries(Object.entries(all).slice(-300));
  try {
    localStorage.setItem(DIRECTION_KEY, JSON.stringify(kept));
  } catch {
    // Stockage indisponible : le choix vaut pour cette lecture.
  }
}

/** Adresse de la page `page` (à partir de 0) d'un chapitre. */
export const pageUrl = (template: string, page: number) => template.replace("{page}", String(page));

// --- Livres EPUB : réglages du lecteur, position dans une section, HTML nettoyé ---

export type EpubLayout = "paged" | "scroll";
export type EpubTheme = "dark" | "light" | "sepia";
export interface EpubSettings {
  layout: EpubLayout;
  /** Taille du texte, en % de celle du livre. */
  size: number;
  theme: EpubTheme;
}
const EPUB_KEY = "maku.epubReader";
export const EPUB_SIZES = { min: 70, max: 200, step: 10 };

export function epubSettings(): EpubSettings {
  const d: EpubSettings = { layout: "paged", size: 100, theme: "dark" };
  try {
    const s = JSON.parse(localStorage.getItem(EPUB_KEY) ?? "{}");
    return {
      layout: s.layout === "scroll" ? "scroll" : d.layout,
      size: typeof s.size === "number" ? Math.min(EPUB_SIZES.max, Math.max(EPUB_SIZES.min, s.size)) : d.size,
      theme: s.theme === "light" || s.theme === "sepia" ? s.theme : d.theme,
    };
  } catch {
    return d;
  }
}

export function saveEpubSettings(s: EpubSettings) {
  try {
    localStorage.setItem(EPUB_KEY, JSON.stringify(s));
  } catch {
    // Stockage indisponible : les réglages valent pour cette lecture.
  }
}

/** Kavita ne retient que la section d'un livre ; l'endroit dans la section (0 : début, 1 : fin) est gardé ici. */
const POSITION_KEY = "maku.epubPosition";
type Positions = Record<string, { page: number; at: number }>;

function positions(): Positions {
  try {
    const p = JSON.parse(localStorage.getItem(POSITION_KEY) ?? "{}");
    return p && typeof p === "object" ? p : {};
  } catch {
    return {};
  }
}

export function epubPosition(chapterId: number): { page: number; at: number } | null {
  const p = positions()[String(chapterId)];
  return p && typeof p.page === "number" && typeof p.at === "number" ? p : null;
}

export function saveEpubPosition(chapterId: number, page: number, at: number) {
  const all = positions();
  delete all[String(chapterId)];
  all[String(chapterId)] = { page, at: Math.round(Math.min(1, Math.max(0, at)) * 1000) / 1000 };
  try {
    localStorage.setItem(POSITION_KEY, JSON.stringify(Object.fromEntries(Object.entries(all).slice(-200))));
  } catch {
    // Stockage indisponible.
  }
}

/** Éléments retirés d'une page de livre : rien qui exécute, charge une autre page ou envoie un formulaire. */
const UNSAFE = "script, iframe, frame, frameset, object, embed, applet, form, base, meta, link, set, animate, animateTransform, animateMotion";

/**
 * Page de livre prête à afficher : le HTML de Kavita, sans script, cadre ni formulaire, sans attribut « on… » ni
 * adresse « javascript: ». Les liens internes de Kavita (`kavita-page`) restent, sans adresse : le lecteur les suit.
 */
export function cleanBookHtml(html: string): DocumentFragment {
  const tpl = document.createElement("template");
  tpl.innerHTML = html;
  const frag = tpl.content;
  for (const el of frag.querySelectorAll(UNSAFE)) el.remove();
  for (const el of frag.querySelectorAll("*")) {
    for (const attr of [...el.attributes]) {
      const name = attr.name.toLowerCase();
      const value = attr.value.replace(/[\s\u0000-\u001f]/g, "").toLowerCase();
      if (name.startsWith("on") || name === "srcdoc" || name === "formaction" || name === "autoplay") el.removeAttribute(attr.name);
      else if ((name === "href" || name === "src" || name === "xlink:href" || name === "action") && (value.startsWith("javascript:") || value.startsWith("vbscript:")))
        el.removeAttribute(attr.name);
    }
  }
  return frag;
}

// --- BD et mangas : pages simples ou doubles, page entière ou largeur, retenus par série ---

/** « auto » : deux pages côte à côte quand l'écran est plus large que haut. */
export type Spread = "auto" | "single" | "double";
/** « screen » : la page entière dans l'écran ; « width » : toute la largeur, la page défile. */
export type Fit = "screen" | "width";
export interface PageLayout {
  spread: Spread;
  fit: Fit;
}
const LAYOUT_KEY = "maku.readerLayout";

function layouts(): Record<string, Partial<PageLayout>> {
  try {
    const d = JSON.parse(localStorage.getItem(LAYOUT_KEY) ?? "{}");
    return d && typeof d === "object" ? d : {};
  } catch {
    return {};
  }
}

export function layoutFor(seriesId: number): PageLayout {
  const l = layouts()[String(seriesId)] ?? {};
  return {
    spread: l.spread === "single" || l.spread === "double" ? l.spread : "auto",
    fit: l.fit === "width" ? "width" : "screen",
  };
}

export function setLayout(seriesId: number, layout: PageLayout) {
  const all = layouts();
  delete all[String(seriesId)];
  all[String(seriesId)] = layout;
  try {
    localStorage.setItem(LAYOUT_KEY, JSON.stringify(Object.fromEntries(Object.entries(all).slice(-300))));
  } catch {
    // Stockage indisponible : le choix vaut pour cette lecture.
  }
}

/**
 * Pages groupées pour la lecture double page, comme Kavita : la couverture seule, une page en largeur seule (c'est
 * déjà une double page), les autres par deux ; après une page seule, une nouvelle paire commence.
 */
export function spreads(pages: number, wide: boolean[] = []): number[][] {
  const out: number[][] = [];
  let i = 0;
  while (i < pages) {
    if (i === 0 || wide[i] || i + 1 >= pages || wide[i + 1]) {
      out.push([i]);
      i += 1;
    } else {
      out.push([i, i + 1]);
      i += 2;
    }
  }
  return out;
}
