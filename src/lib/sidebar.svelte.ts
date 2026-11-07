// Préférences de la barre latérale, gardées sur cet ordinateur : repliée ou non,
// ordre des bibliothèques et bibliothèques masquées (rangées sous « Plus »).
const KEY = "plexr.sidebar";

interface Saved {
  collapsed: boolean;
  /** Téléviseur : barre simplifiée, une pilule d'icônes qui ne se déplie jamais. */
  simple: boolean;
  order: string[];
  hidden: string[];
}

function load(): Saved {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return {
      // Par défaut, menu replié sur les petits écrans (tablette, téléviseur).
      collapsed: typeof raw.collapsed === "boolean" ? raw.collapsed : window.innerWidth < 1100,
      simple: raw.simple === true,
      order: Array.isArray(raw.order) ? raw.order : [],
      hidden: Array.isArray(raw.hidden) ? raw.hidden : [],
    };
  } catch {
    return { collapsed: window.innerWidth < 1100, simple: false, order: [], hidden: [] };
  }
}

export const sidebar = $state({ ...load(), editing: false, showHidden: false });

/** Téléviseur : menu replié au départ, quelle que soit la préférence enregistrée. */
export function collapseForTv() {
  sidebar.collapsed = true;
}

export function saveSidebar() {
  try {
    const { collapsed, simple, order, hidden } = sidebar;
    localStorage.setItem(KEY, JSON.stringify({ collapsed, simple, order, hidden }));
  } catch {
    // Stockage indisponible : les préférences ne valent que pour cette session.
  }
}

/** Trie des éléments selon l'ordre enregistré ; les nouveaux arrivent à la fin. */
export function ordered<T extends { key: string }>(list: T[]): T[] {
  const rank = (k: string) => {
    const i = sidebar.order.indexOf(k);
    return i === -1 ? Number.MAX_SAFE_INTEGER : i;
  };
  return [...list].sort((a, b) => rank(a.key) - rank(b.key));
}
