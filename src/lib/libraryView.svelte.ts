// Filtre et tri choisis dans l'onglet Bibliothèque d'une bibliothèque de films ou de séries, gardés d'une
// visite à l'autre comme dans Plex (une valeur par bibliothèque, dans le stockage local).

export interface LibraryView {
  /** Filtre à cocher (« unwatched », « inProgress », « hdr »…), ou aucun (« Tout »). */
  flag: string | null;
  /** Filtre sur une valeur : un genre, une année, un acteur… */
  value: { field: string; id: string; title: string } | null;
  /** Ordre de tri (clé de Plex : « titleSort », « year », « addedAt »…), celui du serveur sinon. */
  sort: string | null;
  desc: boolean;
}

const KEY = "maku.libraryView";

function all(): Record<string, LibraryView> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}") ?? {};
  } catch {
    return {};
  }
}

export function loadView(section: string): LibraryView {
  const empty: LibraryView = { flag: null, value: null, sort: null, desc: false };
  return { ...empty, ...all()[section] };
}

export function saveView(section: string, view: LibraryView) {
  try {
    localStorage.setItem(KEY, JSON.stringify({ ...all(), [section]: view }));
  } catch {
    // Stockage indisponible : le choix vaut pour cette visite seulement.
  }
}

/** Les filtres à envoyer au serveur, comme le fait Plex Web (« unwatched=1 », « genre=… »). */
export function viewFilters(view: LibraryView): [string, string][] {
  return [...(view.flag ? [[view.flag, "1"] as [string, string]] : []), ...(view.value ? [[view.value.field, view.value.id] as [string, string]] : [])];
}

/** L'ordre à envoyer au serveur (« year:desc »), ou aucun : celui du serveur. */
export function viewSort(view: LibraryView): string | null {
  return view.sort ? (view.desc ? `${view.sort}:desc` : view.sort) : null;
}
