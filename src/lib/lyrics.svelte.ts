// Paroles du morceau en cours, depuis lrclib.net (src-tauri/src/lyrics.rs) : synchronisées (format LRC) quand
// c'est possible, sinon le texte seul. Une recherche par morceau, gardée en mémoire le temps de la session.
import { invoke } from "@tauri-apps/api/core";
import type { QueueTrack } from "./api";

export interface Line {
  /** Début de la ligne, en secondes. */
  at: number;
  text: string;
}

export type LyricsState =
  | { status: "loading" }
  | { status: "none" }
  | { status: "instrumental" }
  | { status: "plain"; text: string }
  | { status: "synced"; lines: Line[] };

const KEY = "maku.lyrics";

/** Recherche des paroles (Réglages → Lecteur), activée par défaut. */
export const lyricsPrefs = $state({ enabled: load() });

function load(): boolean {
  try {
    return localStorage.getItem(KEY) !== "0";
  } catch {
    return true;
  }
}

export function setLyricsEnabled(on: boolean) {
  lyricsPrefs.enabled = on;
  try {
    localStorage.setItem(KEY, on ? "1" : "0");
  } catch {
    // Stockage indisponible : réglage gardé jusqu'à la fermeture.
  }
}

/** Paroles du morceau affiché ; `key` : son identifiant. */
export const lyrics = $state<{ key: string; state: LyricsState }>({ key: "", state: { status: "none" } });

const cache = new Map<string, LyricsState>();

/** `[01:23.45]`, `[1:23]`, `[01:23:45]` (centièmes après deux-points, chez certains). */
const STAMP = /\[(\d+):(\d{1,2})(?:[.:](\d{1,3}))?\]/g;

/** Paroles LRC → lignes triées ; les balises d'en-tête (`[ar:…]`, `[length:…]`) sont ignorées. */
export function parseLrc(lrc: string): Line[] {
  const lines: Line[] = [];
  for (const raw of lrc.split(/\r?\n/)) {
    const stamps = [...raw.matchAll(STAMP)];
    if (!stamps.length) continue;
    const text = raw.replace(STAMP, "").trim();
    for (const m of stamps) {
      const frac = m[3] ? Number(m[3]) / 10 ** m[3].length : 0;
      lines.push({ at: Number(m[1]) * 60 + Number(m[2]) + frac, text });
    }
  }
  return lines.sort((a, b) => a.at - b.at);
}

/** Ligne chantée à `position` (secondes) : la dernière commencée, -1 avant la première. */
export function lineAt(lines: Line[], position: number): number {
  let lo = 0;
  let hi = lines.length - 1;
  let found = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid].at <= position + 0.15) {
      found = mid;
      lo = mid + 1;
    } else hi = mid - 1;
  }
  return found;
}

/** Charge les paroles d'un morceau (une fois par morceau et par session). */
export async function loadLyrics(track: QueueTrack | null | undefined) {
  if (!track || !lyricsPrefs.enabled) return;
  const key = track.ratingKey;
  if (lyrics.key === key) return;
  const cached = cache.get(key);
  lyrics.key = key;
  if (cached) {
    lyrics.state = cached;
    return;
  }
  if (!track.artist || !track.title) {
    lyrics.state = { status: "none" };
    return;
  }
  lyrics.state = { status: "loading" };
  let result: LyricsState;
  try {
    const r = await invoke<{ synced: string | null; plain: string | null; instrumental: boolean }>("track_lyrics", {
      artist: track.artist,
      title: track.title,
      album: track.album,
      duration: track.durationMs / 1000,
    });
    const lines = r.synced ? parseLrc(r.synced) : [];
    result = lines.length
      ? { status: "synced", lines }
      : r.plain
        ? { status: "plain", text: r.plain }
        : r.instrumental
          ? { status: "instrumental" }
          : { status: "none" };
    cache.set(key, result);
  } catch {
    // Hors ligne ou lrclib injoignable : on réessaiera au prochain passage sur ce morceau.
    result = { status: "none" };
  }
  if (lyrics.key === key) lyrics.state = result;
}
