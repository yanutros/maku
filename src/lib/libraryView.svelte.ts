// Filtre et tri choisis dans l'onglet Bibliothèque d'une bibliothèque de films ou de séries, gardés d'une
// visite à l'autre comme dans Plex (une valeur par bibliothèque, dans le stockage local).

/** Filtre avancé : un champ, un opérateur de Plex (« = », « != », « >>= »…) et une valeur. */
export interface Rule {
  field: string;
  op: string;
  value: string;
}

/** Filtres avancés, comme dans Plex : des règles qui doivent toutes (ou une seule, `any`) être vraies. */
export interface SmartFilter {
  any: boolean;
  rules: Rule[];
  /** Nombre de titres au plus (dans l'ordre de tri). */
  limit: number | null;
}

export interface LibraryView {
  /** Filtre à cocher (« unwatched », « inProgress », « hdr »…), ou aucun (« Tout »). */
  flag: string | null;
  /** Filtre sur une valeur : un genre, une année, un acteur… */
  value: { field: string; id: string; title: string } | null;
  /** Ordre de tri (clé de Plex : « titleSort », « year », « addedAt »…), celui du serveur sinon. */
  sort: string | null;
  desc: boolean;
  /** Filtres avancés : quand il y en a, ils remplacent le filtre et la valeur ci-dessus. */
  advanced?: SmartFilter | null;
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
  // L'aperçu d'une collection intelligente ne se garde pas d'une visite à l'autre.
  return { ...empty, ...all()[section], advanced: null };
}

export function saveView(section: string, view: LibraryView) {
  try {
    localStorage.setItem(KEY, JSON.stringify({ ...all(), [section]: { ...view, advanced: null } }));
  } catch {
    // Stockage indisponible : le choix vaut pour cette visite seulement.
  }
}

/** Les filtres à envoyer au serveur, comme le fait Plex Web (« unwatched=1 », « genre=… »). */
export function viewFilters(view: LibraryView): [string, string][] {
  if (view.advanced) return smartPairs(view.advanced);
  return [...(view.flag ? [[view.flag, "1"] as [string, string]] : []), ...(view.value ? [[view.value.field, view.value.id] as [string, string]] : [])];
}

/** L'ordre à envoyer au serveur (« year:desc »), ou aucun : celui du serveur. */
export function viewSort(view: LibraryView): string | null {
  return view.sort ? (view.desc ? `${view.sort}:desc` : view.sort) : null;
}

/** Règles complètes (champ, opérateur et valeur). */
export function completeRules(f: SmartFilter): Rule[] {
  return f.rules.filter((r) => r.field && r.op && r.value !== "");
}

/**
 * Les filtres avancés tels que Plex les écrit : « title=nuit » (contient), « title!=aube », « year>>=2000 »…
 * L'opérateur passe dans le nom (sans son « = » final) ; « l'un des » les regroupe entre push et pop, séparés par or.
 */
export function smartPairs(f: SmartFilter): [string, string][] {
  const rules = completeRules(f);
  const pair = (r: Rule): [string, string] => [r.field + r.op.slice(0, -1), r.value];
  const out: [string, string][] =
    f.any && rules.length > 1
      ? [["push", "1"], ...rules.flatMap((r, i) => (i ? [["or", "1"] as [string, string], pair(r)] : [pair(r)])), ["pop", "1"]]
      : rules.map(pair);
  if (f.limit) out.push(["limit", String(f.limit)]);
  return out;
}

/** Recherche d'une collection intelligente (« /library/sections/1/all?type=1&sort=year:desc&push=1&… ») en règles. */
export function parseSmart(content: string): { filter: SmartFilter; sort: string | null; desc: boolean } {
  const filter: SmartFilter = { any: false, rules: [], limit: null };
  let sort: string | null = null;
  let desc = false;
  for (const part of (content.split("?")[1] ?? "").split("&")) {
    const m = part.match(/^([A-Za-z0-9._]+)([!<>=]*)=(.*)$/);
    if (!m) continue;
    const [, field, op, raw] = m;
    const value = decodeURIComponent(raw.replace(/\+/g, " "));
    if (field === "type" || field === "push" || field === "pop" || field === "includeGuids") continue;
    if (field === "or") filter.any = true;
    else if (field === "sort") [sort, desc] = [value.split(":")[0], value.endsWith(":desc")];
    else if (field === "limit") filter.limit = Number(value) || null;
    else filter.rules.push({ field, op: op + "=", value });
  }
  return { filter, sort, desc };
}
