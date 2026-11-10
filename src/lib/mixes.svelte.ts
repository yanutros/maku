// « Mix of Three » : trois playlists mélangées le temps d'une écoute. Le mix n'est pas une playlist du serveur :
// il est gardé sur cet appareil, et reste dans la rangée des playlists (onglet Recommandé de la musique)
// deux jours après sa dernière écoute, pour le relancer sans le recomposer, puis il disparaît.
import type { Item } from "./api";
import { playMusic } from "./player.svelte";

const KEY = "maku.mixes";
/** Deux jours après la dernière écoute. */
const LIFETIME = 2 * 24 * 60 * 60 * 1000;
const MAX = 12;

export interface MixPlaylist {
  key: string;
  title: string;
  poster: string | null;
}

export interface Mix {
  id: string;
  playlists: MixPlaylist[];
  /** Dernière écoute (ms depuis 1970). */
  at: number;
}

const alive = (m: Mix) => Date.now() - m.at < LIFETIME;

function load(): Mix[] {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(raw) ? raw.filter((m) => typeof m?.id === "string" && Array.isArray(m.playlists) && typeof m.at === "number").filter(alive) : [];
  } catch {
    return [];
  }
}

export const mixes = $state({ list: load() });

function save() {
  mixes.list = mixes.list.filter(alive).sort((a, b) => b.at - a.at).slice(0, MAX);
  try {
    localStorage.setItem(KEY, JSON.stringify(mixes.list));
  } catch {
    // Stockage indisponible : le mix ne vaut que pour cette session.
  }
}

/** Les mix encore en vie, du plus récemment écouté au plus ancien. */
export function liveMixes(): Mix[] {
  return mixes.list.filter(alive);
}

/** Un nouveau mix (les mêmes trois playlists, dans n'importe quel ordre, reprennent le mix existant). */
export function createMix(playlists: Item[]): Mix {
  const picked = playlists.map((p) => ({ key: p.ratingKey, title: p.title, poster: p.poster }));
  const same = (m: Mix) => m.playlists.length === picked.length && picked.every((p) => m.playlists.some((q) => q.key === p.key));
  const mix = mixes.list.find(same) ?? { id: crypto.randomUUID(), playlists: picked, at: Date.now() };
  mix.at = Date.now();
  mixes.list = [mix, ...mixes.list.filter((m) => m.id !== mix.id)];
  save();
  return mix;
}

/** Une écoute de plus : le mix reste deux jours de plus. */
export function touchMix(id: string) {
  const mix = mixes.list.find((m) => m.id === id);
  if (!mix) return;
  mix.at = Date.now();
  save();
}

export function removeMix(id: string) {
  mixes.list = mixes.list.filter((m) => m.id !== id);
  save();
}

/** Écoute un mix : les titres de ses playlists, mélangés (un titre présent dans deux playlists ne passe qu'une fois). */
export function playMix(mix: Mix) {
  touchMix(mix.id);
  return playMusic(`mix:${mix.id}`, { keys: mix.playlists.map((p) => p.key), shuffle: true });
}

/** « Écouter tout » : toutes les playlists d'un coup, en aléatoire, comme dans Plex. */
export function playAllPlaylists(playlists: Item[]) {
  return playMusic("playlists:all", { keys: playlists.map((p) => p.ratingKey), shuffle: true });
}
