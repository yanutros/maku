// Dernières lectures des playlists lancées dans Maku, gardées sur l'appareil : le serveur ne note pas
// toujours la lecture d'une playlist (Jellyfin, Emby, ou Plex quand Maku enchaîne ses titres).
// Avec la date du serveur (`lastPlayed`, lectures faites ailleurs), elles ordonnent la rangée Playlists.

import type { Item } from "./api";

const KEY = "maku.playlistPlays";
const MAX = 200;

function load(): Record<string, number> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}") ?? {};
  } catch {
    return {};
  }
}

/** À chaque lecture de musique : la date de l'élément lancé (seules les playlists s'en servent). */
export function notePlay(ratingKey: string) {
  try {
    const plays = load();
    plays[ratingKey] = Date.now();
    const kept = Object.entries(plays).sort((a, b) => b[1] - a[1]).slice(0, MAX);
    localStorage.setItem(KEY, JSON.stringify(Object.fromEntries(kept)));
  } catch {
    // Stockage indisponible : l'ordre du serveur reste.
  }
}

/** Playlists de la plus récemment lue à la plus ancienne ; jamais lues : dans l'ordre du serveur, à la fin. */
export function byLastPlay(items: Item[]): Item[] {
  const plays = load();
  const at = (i: Item) => Math.max(plays[i.ratingKey] ?? 0, i.lastPlayed ?? 0);
  return items
    .map((item, index) => ({ item, index, at: at(item) }))
    .sort((a, b) => b.at - a.at || a.index - b.index)
    .map((e) => e.item);
}
