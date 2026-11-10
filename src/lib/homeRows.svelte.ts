// Rangées de l'accueil, gardées sur cet appareil : leur ordre, et celles qui s'affichent.
// « continue », « recent » et « trending » sont celles de Maku ; « lib:<clé> » épingle
// les derniers ajouts d'une bibliothèque.
import { ordered } from "./sidebar.svelte";
const KEY = "maku.home";

export interface HomeRow {
  id: string;
  on: boolean;
}

/**
 * Au départ (et après « Rétablir l'accueil ») : Continuer à regarder, les derniers ajouts de la première
 * bibliothèque de films et de la première de séries (dans l'ordre du menu), et les tendances à demander.
 * Les ajouts récents de tout le serveur, les autres bibliothèques et la musique restent à cocher.
 */
export function shownByDefault(sections: { key: string; kind: string }[]): string[] {
  const list = ordered(sections);
  const first = (kind: string) => list.find((s) => s.kind === kind);
  return ["continue", ...[first("movie"), first("show")].filter((s) => !!s).map((s) => `lib:${s!.key}`), "trending"];
}

function load(): HomeRow[] {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(raw) ? raw.filter((r) => typeof r?.id === "string" && typeof r?.on === "boolean") : [];
  } catch {
    return [];
  }
}

export const homeRows = $state({ saved: load() });

/**
 * Les rangées disponibles, dans l'ordre choisi : celles déjà rangées d'abord, les nouvelles (une
 * bibliothèque ajoutée sur le serveur) ensuite, affichées ou non selon leur type.
 */
export function arrange(available: string[], shown: string[]): HomeRow[] {
  const known = homeRows.saved.filter((r) => available.includes(r.id));
  const fresh = available.filter((id) => !known.some((r) => r.id === id)).map((id) => ({ id, on: shown.includes(id) }));
  return [...known, ...fresh];
}

export function saveHome(rows: HomeRow[]) {
  homeRows.saved = rows.map(({ id, on }) => ({ id, on }));
  try {
    localStorage.setItem(KEY, JSON.stringify(homeRows.saved));
  } catch {
    // Stockage indisponible : le choix ne vaut que pour cette session.
  }
}

export function resetHome() {
  homeRows.saved = [];
  try {
    localStorage.removeItem(KEY);
  } catch {
    // Rien à effacer.
  }
}

/** Identifiants des rangées, d'après les bibliothèques du serveur. */
export function availableRows(sections: { key: string; kind: string }[]): string[] {
  return ["continue", "recent", ...sections.map((s) => `lib:${s.key}`), "trending"];
}
