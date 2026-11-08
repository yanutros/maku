// Lecture hors ligne (Android, téléphone et tablette) : films, épisodes et musique téléchargés sur l'appareil.
// Le téléchargement lui-même se fait côté Rust (src-tauri/src/offline.rs) ; ici, la liste, son suivi en direct
// et les demandes (un titre, une saison, un album ou une playlist).
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Item, QueueTrack } from "./api";
import { device } from "./device";

export type Quality = "original" | "1080" | "720";
export type Status = "queued" | "downloading" | "paused" | "done" | "failed";

export interface Download {
  id: string;
  server: string;
  ratingKey: string;
  kind: "movie" | "episode" | "track";
  title: string;
  subtitle: string | null;
  showTitle: string | null;
  groupKey: string | null;
  groupTitle: string | null;
  index: number | null;
  parentIndex: number | null;
  quality: Quality;
  file: string;
  poster: string | null;
  size: number;
  done: number;
  status: Status;
  error: string | null;
  durationMs: number;
  positionMs: number;
  watched: boolean;
  pending: boolean;
  added: number;
}

/** `allowed` : le compte peut télécharger sur le serveur en cours (réglé par son administrateur). */
export const offline = $state({ items: [] as Download[], dir: "", loaded: false, allowed: false });

const ALLOWED_KEY = "maku.offlineAllowed";
try {
  offline.allowed = localStorage.getItem(ALLOWED_KEY) === "1";
} catch {
  // Stockage indisponible : le droit sera redemandé au serveur.
}

/** Redemande le droit au serveur ; sans réponse (hors ligne), la dernière connue est gardée. */
export async function checkOfflineAllowed() {
  if (!offlineAvailable()) return;
  try {
    offline.allowed = await invoke<boolean>("offline_allowed");
    localStorage.setItem(ALLOWED_KEY, offline.allowed ? "1" : "0");
  } catch {
    // Serveur injoignable : rien ne change.
  }
}

/** Bouton « Télécharger » : appareil Android et droit donné par l'administrateur. */
export const canDownload = () => offlineAvailable() && offline.allowed;

/** Page des téléchargements : avec le droit, ou s'il reste des titres téléchargés à regarder. */
export const showDownloads = () => offlineAvailable() && (offline.allowed || offline.items.length > 0);

/** Téléphones et tablettes Android (le téléviseur reste branché au réseau). */
export function offlineAvailable() {
  return device.os === "android" && !device.tv;
}

export async function loadOffline() {
  if (!offlineAvailable()) return;
  const r = await invoke<{ dir: string; items: Download[] }>("offline_list");
  offline.dir = r.dir;
  offline.items = r.items;
  offline.loaded = true;
}

// Avancement en direct, envoyé par le téléchargement en cours.
listen<{ id: string; done: number; size: number; status: Status; error: string | null }>("offline-progress", (e) => {
  const d = offline.items.find((x) => x.id === e.payload.id);
  if (d) Object.assign(d, e.payload);
  else loadOffline().catch(() => {});
}).catch(() => {});

export const filePath = (d: Download) => `${offline.dir}/${d.file}`;
export const fileUrl = (d: Download) => convertFileSrc(filePath(d));
export const posterUrl = (d: Download) => (d.poster ? convertFileSrc(`${offline.dir}/${d.poster}`) : null);

/** Téléchargements d'un titre (une qualité, en principe). */
export const downloadsOf = (ratingKey: string) => offline.items.filter((d) => d.ratingKey === ratingKey);

type Request = {
  ratingKey: string;
  kind: string;
  title: string;
  subtitle: string | null;
  showTitle: string | null;
  groupKey: string | null;
  groupTitle: string | null;
  index: number | null;
  parentIndex: number | null;
  quality: Quality;
  poster: string | null;
  durationMs: number;
  url: string | null;
};

async function add(requests: Request[]) {
  await invoke<Download[]>("offline_add", { requests });
  await loadOffline();
}

function request(item: Item, quality: Quality): Request {
  const episode = item.kind === "episode";
  return {
    ratingKey: item.ratingKey,
    kind: item.kind,
    title: item.title,
    subtitle: item.subtitle,
    showTitle: item.showTitle,
    groupKey: episode ? item.showKey : null,
    groupTitle: episode ? item.showTitle : null,
    index: item.index,
    parentIndex: item.parentIndex,
    quality,
    poster: (episode ? item.thumb : null) ?? item.poster,
    durationMs: item.durationMs ?? 0,
    url: null,
  };
}

/** Un film ou un épisode. */
export const downloadItem = (item: Item, quality: Quality) => add([request(item, quality)]);

/** Tous les épisodes d'une saison. */
export const downloadEpisodes = (episodes: Item[], quality: Quality) => add(episodes.map((e) => request(e, quality)));

/** Un album ou une playlist : chacun de ses titres, dans le fichier d'origine. */
export async function downloadMusic(item: Item) {
  const q = await invoke<{ tracks: QueueTrack[] }>("music_queue", { ratingKey: item.ratingKey, keys: null, start: null, shuffle: false });
  await add(
    q.tracks.map((t, i) => ({
      ratingKey: t.ratingKey,
      kind: "track",
      title: t.title,
      subtitle: t.album,
      showTitle: t.artist,
      groupKey: item.ratingKey,
      groupTitle: item.title,
      index: i + 1,
      parentIndex: null,
      quality: "original",
      poster: item.poster ?? t.cover,
      durationMs: t.durationMs,
      url: t.url,
    })),
  );
}

export async function pauseDownload(id: string) {
  await invoke("offline_pause", { id });
  await loadOffline();
}

export async function resumeDownload(id: string) {
  await invoke("offline_resume", { id });
  await loadOffline();
}

export async function removeDownload(id: string) {
  await invoke("offline_remove", { id });
  await loadOffline();
}

/** Serveur joignable : progression jouée hors ligne renvoyée, téléchargements en attente relancés. */
export async function syncOffline() {
  if (!offlineAvailable()) return;
  await invoke("offline_sync").catch(() => {});
  await loadOffline().catch(() => {});
}

/** File de musique d'un album ou d'une playlist téléchargés, à partir des titres déjà sur l'appareil. */
export function offlineQueue(groupKey: string): { tracks: QueueTrack[]; ids: string[] } {
  const done = offline.items.filter((d) => d.groupKey === groupKey && d.kind === "track" && d.status === "done").sort((a, b) => (a.index ?? 0) - (b.index ?? 0));
  return {
    ids: done.map((d) => d.id),
    tracks: done.map((d) => ({
      ratingKey: d.ratingKey,
      title: d.title,
      artist: d.showTitle,
      artistKey: null,
      album: d.subtitle,
      albumKey: null,
      cover: posterUrl(d),
      durationMs: d.durationMs,
      url: fileUrl(d),
    })),
  };
}

/** « 1,2 Go », « 640 Mo ». */
export function formatSize(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toLocaleString(undefined, { maximumFractionDigits: 1 })} Go`;
  return `${Math.max(1, Math.round(bytes / 1e6)).toLocaleString()} Mo`;
}
