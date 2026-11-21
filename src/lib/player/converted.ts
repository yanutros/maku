// Fichiers que cet appareil ne lit pas tels quels : format refusé par le lecteur, mémoire de l'appareil qui s'effondre
// pendant la lecture, ou conversion demandée parce que l'image se figeait. La fois suivante, la vidéo part directement
// en conversion par le serveur, sans tentative ratée ni coupure. La liste est propre à l'appareil (stockage du
// navigateur, recopié dans la configuration de Maku : prefs.ts) et ne garde que les derniers fichiers.

/** Ce que le serveur convertit : le son seulement (l'image d'origine est gardée), ou tout. */
export type Conversion = "audio" | "full";

const KEY = "maku.converted";
export const MAX_CONVERTED = 200;

/** Un fichier : le serveur, l'élément, et la version lue. */
export function fileKey(serverId: string, ratingKey: string, mediaIndex: number): string {
  return `${serverId}|${ratingKey}|${mediaIndex}`;
}

function load(): [string, Conversion][] {
  try {
    const list = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    if (!Array.isArray(list)) return [];
    return list.filter((e): e is [string, Conversion] => Array.isArray(e) && typeof e[0] === "string" && (e[1] === "audio" || e[1] === "full"));
  } catch {
    return [];
  }
}

function save(list: [string, Conversion][]) {
  try {
    if (list.length) localStorage.setItem(KEY, JSON.stringify(list));
    else localStorage.removeItem(KEY);
  } catch {
    // Stockage indisponible : rien n'est retenu.
  }
}

/** La conversion retenue pour ce fichier, ou null : il se lit tel quel. */
export function conversionFor(key: string): Conversion | null {
  return load().find(([k]) => k === key)?.[1] ?? null;
}

/**
 * Retient la conversion d'un fichier. Une conversion complète remplace celle du son seul ; jamais l'inverse (le son
 * seul a déjà échoué). Le fichier passe en dernier, et les plus anciens sortent au-delà de MAX_CONVERTED.
 */
export function rememberConversion(key: string, mode: Conversion) {
  const list = load();
  const before = list.find(([k]) => k === key)?.[1];
  const kept = before === "full" ? "full" : mode;
  const next = [...list.filter(([k]) => k !== key), [key, kept] as [string, Conversion]];
  save(next.slice(-MAX_CONVERTED));
}

/** Le fichier se relit tel quel (« Lire le fichier d'origine »). */
export function forgetConversion(key: string) {
  const list = load();
  if (list.some(([k]) => k === key)) save(list.filter(([k]) => k !== key));
}
