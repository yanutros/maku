import { invoke } from "@tauri-apps/api/core";
import { notePlay } from "./playlistPlays";
import { listen } from "@tauri-apps/api/event";
import { untrack } from "svelte";
import type { QueueTrack, Track } from "./api";
import { t } from "./i18n";
import { device } from "./device";
import { toast } from "./nav.svelte";

export interface MpvTrack {
  id: number;
  type: "video" | "audio" | "sub";
  title?: string;
  lang?: string;
  codec?: string;
  "demux-channel-count"?: number;
  external?: boolean;
  forced?: boolean;
}

/**
 * État du lecteur. Sur ordinateur, il est alimenté par les propriétés que mpv publie ;
 * sur mobile et TV (`web`), par le lecteur vidéo / audio du système, piloté ici même.
 */
export const player = $state({
  /** Une lecture est en cours (vidéo ou musique). */
  active: false,
  /** Vidéo plein cadre, sinon musique avec la barre du bas. */
  video: false,
  /** Vidéo dessinée dans la fenêtre de Maku (Windows) ; sinon dans une fenêtre mpv séparée. */
  embedded: true,
  /** Lecture par le lecteur intégré du système (Android, iOS) plutôt que par mpv. */
  web: false,
  /** Vidéo lue par le lecteur natif d'Android (ExoPlayer, sous la page) plutôt que par <video>. */
  native: false,
  /** Identifiant de la lecture : les événements d'une lecture remplacée sont ignorés. */
  session: "",
  /** Qualité choisie dans le lecteur, en kb/s ; 0 : le fichier d'origine, sans conversion. */
  quality: loadQuality(),
  /** La vidéo en cours est convertie par le serveur (qualité réduite, ou format non lu). */
  transcoded: false,
  /** Pistes du serveur demandées pour la vidéo en cours (identifiants Plex ou Jellyfin). */
  streams: { audioStream: null as number | null, subtitleStream: null as number | null },
  /** Version lue (voir `ItemDetail.versions`) ; 0 : la première. */
  mediaIndex: 0,
  /** Pistes du serveur, proposées quand la vidéo est convertie (mpv n'y voit qu'une piste). */
  serverAudio: [] as Track[],
  serverSubs: [] as Track[],
  /** Vidéo en cours et épisodes voisins (pour les boutons précédent / suivant). */
  ratingKey: "",
  prevKey: null as string | null,
  nextKey: null as string | null,
  /** Épisode suivant, pour l'écran de fin (EndScreen.svelte). */
  upNext: null as UpNext | null,
  /** Écran de fin affiché : « Passer le générique » ou fin d'un épisode qui a une suite. */
  endScreen: false,
  /** Android : la vidéo est figée (prête, chargée, mais l'image n'avance plus) ; position en s, et son envoyé à l'ampli. */
  stalled: null as { at: number; passthrough: boolean } | null,
  title: "",
  subtitle: null as string | null,
  position: 0,
  duration: 0,
  /** Repères de l'épisode (intro, générique), pour le bouton « Passer ». */
  markers: [] as { kind: string; startMs: number; endMs: number }[],
  /** Miniatures d'aperçu, si le serveur en a pour cette vidéo. */
  previews: null as Previews | null,
  bufferedUntil: 0,
  /** Ordinateur : mpv affiche déjà l'image sous la page (sinon, la page reste opaque). */
  videoReady: false,
  paused: false,
  buffering: true,
  volume: 100,
  muted: false,
  tracks: [] as MpvTrack[],
  aid: null as number | null,
  sid: null as number | null,
  /** File de lecture musicale et titre en cours. */
  queue: [] as QueueTrack[],
  index: 0,
  /** Répéter la file (mpv : loop-playlist). */
  repeat: false,
  /** Répéter le titre en cours (mpv : loop-file). */
  repeatOne: false,
  /** Ordre aléatoire activé sur la file en cours (bouton Aléatoire du lecteur). */
  shuffled: false,
  /** Vue « En cours de lecture » ouverte. */
  expanded: false,
  /** Incrémenté à chaque saut dans le morceau (pour la notification Android). */
  seeks: 0,
  /** Lecture d'un téléchargement (lib/offline.svelte.ts) : son identifiant ; la progression reste sur l'appareil. */
  offline: "",
});

/** Durées proposées pour le compte à rebours de l'épisode suivant (secondes ; 0 : désactivé). */
export const NEXT_DELAYS = [0, 5, 10, 15, 30];

export interface UpNext {
  title: string;
  showTitle: string | null;
  /** « S1 · É2 ». */
  number: string | null;
  summary: string | null;
  durationMs: number | null;
  thumb: string | null;
}

/** Compte à rebours de l'écran de fin, réglé dans Réglages → Lecteur (30 s par défaut ; 0 : pas de compte à rebours). */
export const autoNext = $state({ delay: loadNextDelay() });

function loadNextDelay(): number {
  try {
    const v = localStorage.getItem("maku.nextDelay");
    return v !== null && NEXT_DELAYS.includes(Number(v)) ? Number(v) : 30;
  } catch {
    return 30;
  }
}

export function setNextDelay(s: number) {
  autoNext.delay = s;
  try {
    localStorage.setItem("maku.nextDelay", String(s));
  } catch {
    // Stockage indisponible : réglage gardé jusqu'à la fermeture.
  }
}

/** Titre, numéro et vignette de l'épisode suivant, pour sa carte. */
function loadUpNext(current: string, next: string) {
  type Meta = { title: string; showTitle: string | null; summary: string | null; durationMs: number | null; index: number | null; parentIndex: number | null; thumb: string | null; poster: string | null };
  invoke<Meta>("plex_metadata", { ratingKey: next }).then(
    (m) => {
      if (player.ratingKey !== current || player.nextKey !== next) return;
      const number = m.parentIndex != null && m.index != null ? `S${m.parentIndex} · É${m.index}` : null;
      player.upNext = { title: m.title, showTitle: m.showTitle, number, summary: m.summary, durationMs: m.durationMs, thumb: m.thumb ?? m.poster };
    },
    () => {},
  );
}

/** Écran de fin possible : un épisode qui a une suite. */
export const hasEndScreen = () => player.video && !!player.nextKey && !!player.upNext;

/** « Passer le générique » d'un épisode qui a une suite : la lecture s'interrompt sur l'écran de fin. */
export function openEndScreen() {
  if (!player.paused) mpv("cycle", "pause");
  player.endScreen = true;
}

/** Fin d'une vidéo : l'écran de fin s'il y a un épisode suivant, sinon l'arrêt. */
function videoEnded() {
  if (hasEndScreen()) player.endScreen = true;
  else stop();
}

/** Qualités proposées, comme dans les applications Plex (kb/s ; 0 : originale). */
export const QUALITIES = [0, 20000, 12000, 8000, 4000, 2000, 1500];

function loadQuality(): number {
  try {
    const q = Number(localStorage.getItem("maku.quality"));
    return QUALITIES.includes(q) ? q : 0;
  } catch {
    return 0;
  }
}

/** « 8 Mb/s · 1080p », d'après le débit (même règle que le serveur, voir plex::resolution_for). */
export function qualityLabel(kbps: number): string {
  if (!kbps) return t("player.qualityOriginal");
  const rate = kbps >= 1000 ? `${(kbps / 1000).toLocaleString()} Mb/s` : `${kbps} kb/s`;
  return `${rate} · ${kbps >= 8000 ? "1080p" : kbps >= 2000 ? "720p" : "480p"}`;
}

// Rien après un arrêt : la file reste en mémoire, mais plus aucun titre n'est « en cours ».
export const current = () => (player.video || !player.active ? null : (player.queue[player.index] ?? null));

// Sur Android et iOS, mpv n'existe pas : le lecteur du système prend le relais.
invoke<string | null>("platform").then(
  (os) => (player.web = os === "android" || os === "ios"),
  () => {},
);

/** Lecteur vidéo natif d'Android (NativeVideo.kt), quand l'application le fournit. */
type VideoBridge = {
  videoOpen?(json: string): void;
  videoCommand?(name: string, value: string): void;
  videoClose?(): void;
  audioPassthrough?(): string;
  diagLog?(message: string): void;
  diagText?(): string;
  diagShare?(): void;
};
const bridge = (globalThis as { PlexrDevice?: VideoBridge }).PlexrDevice;
const nativeVideo = !!bridge?.videoOpen;

/** Journal de diagnostic d'Android (DiagLog.kt) : lignes de la page, à transmettre en cas de blocage. */
export const diagAvailable = !!bridge?.diagText;
export function diag(message: string) {
  try {
    bridge?.diagLog?.(message);
  } catch {
    // Journal indisponible : la lecture continue.
  }
}
export const diagText = () => bridge?.diagText?.() ?? "";
export const diagShare = () => bridge?.diagShare?.();
if (diagAvailable) {
  addEventListener("error", (e) => diag(`Erreur de la page : ${e.message} (${e.filename?.split("/").pop()}:${e.lineno})`));
  addEventListener("unhandledrejection", (e) => diag(`Erreur de la page : ${String(e.reason).slice(0, 300)}`));
}

/** Lecteur natif d'Android : le son peut être envoyé tel quel à l'ampli (réglage de l'appareil). */
export const passthroughAvailable = nativeVideo && !!bridge?.audioPassthrough;
const PASSTHROUGH_KEY = "maku.passthrough";
export function passthroughMode(): "auto" | "off" {
  try {
    return localStorage.getItem(PASSTHROUGH_KEY) === "off" ? "off" : "auto";
  } catch {
    return "auto";
  }
}
export function setPassthroughMode(mode: "auto" | "off") {
  try {
    localStorage.setItem(PASSTHROUGH_KEY, mode);
  } catch {
    // Stockage indisponible : le mode automatique reste.
  }
}
/** Formats que la sortie audio accepte tels quels, et son nombre de canaux. */
export function passthroughFormats(): { formats: string[]; channels: number } | null {
  try {
    return bridge?.audioPassthrough ? JSON.parse(bridge.audioPassthrough()) : null;
  } catch {
    return null;
  }
}

interface NativeInfo {
  session: string;
  url: string;
  title: string;
  subtitle: string | null;
  offsetMs: number;
  durationMs: number;
  aid: number | null;
  sid: string;
  subFiles: { url: string; codec: string; language: string | null; title: string | null }[];
}

/** Miniatures d'aperçu de la barre de progression (voir `plex::Previews`). */
export interface Previews {
  kind: "frame" | "tiles";
  url: string;
  intervalMs: number;
  width: number;
  height: number;
  columns: number;
  rows: number;
  count: number;
}

export interface PlayOptions {
  /** Identifiants de pistes Plex ; `subtitleStream: 0` désactive les sous-titres. */
  audioStream?: number | null;
  subtitleStream?: number | null;
  fromStart?: boolean;
  /** Position de départ imposée (ms), pour reprendre au même endroit après un changement de qualité. */
  startMs?: number;
  /** Version à lire (4K, 1080p…), la première par défaut. */
  mediaIndex?: number;
}

// ---------- Fin d'une lecture vidéo par le lecteur du système (Android, iOS, TV) ----------
// Sur ordinateur, mpv signale sa fermeture par l'événement `player-stopped`. Le lecteur du système n'en émet pas :
// sans ce signal, la saison ou l'accueil gardaient l'épisode tout juste terminé « à voir » jusqu'à la réouverture
// de la page.
const playbackEndedListeners = new Set<() => void>();

/** Appelle `cb` à la fin d'une vidéo lue par le lecteur du système ; renvoie de quoi se désabonner. */
export function onPlaybackEnded(cb: () => void): () => void {
  playbackEndedListeners.add(cb);
  return () => void playbackEndedListeners.delete(cb);
}

function notifyPlaybackEnded() {
  for (const cb of playbackEndedListeners) cb();
}

/** Libère la conversion du serveur lancée pour mpv (qualité réduite sur ordinateur). */
function endTranscode() {
  if (!player.web && player.transcoded && player.session) invoke("stop_transcode", { session: player.session }).catch(() => {});
}

function reset(video: boolean) {
  endTranscode();
  web.teardown();
  Object.assign(player, {
    active: true,
    video,
    embedded: true,
    native: false,
    transcoded: false,
    serverAudio: [],
    serverSubs: [],
    session: "",
    ratingKey: "",
    prevKey: null,
    nextKey: null,
    upNext: null,
    endScreen: false,
    stalled: null,
    title: "",
    subtitle: null,
    buffering: true,
    position: 0,
    duration: 0,
    markers: [],
    previews: null,
    bufferedUntil: 0,
    videoReady: false,
    paused: false,
    tracks: [],
    queue: [],
    index: 0,
    repeat: false,
    repeatOne: false,
    shuffled: false,
    expanded: false,
    offline: "",
  });
  unshuffled = null;
}

/** Session de la musique jouée depuis les téléchargements. */
export const OFFLINE_MUSIC = "offline-music";

/** Vidéo téléchargée (Android) : le fichier de l'appareil, lu par le lecteur natif, sans le serveur. */
export function playOffline(d: { id: string; ratingKey: string; title: string; subtitle: string | null; showTitle: string | null; durationMs: number; positionMs: number; watched: boolean }, path: string) {
  reset(true);
  Object.assign(player, {
    ratingKey: d.ratingKey,
    title: d.showTitle ?? d.title,
    subtitle: d.showTitle ? d.title : d.subtitle,
    duration: d.durationMs / 1000,
    native: true,
    offline: d.id,
  });
  // Vu jusqu'au bout : on repart du début.
  const offsetMs = d.watched || d.positionMs > d.durationMs * 0.95 ? 0 : d.positionMs;
  web.startNative({ session: "", url: "file://" + path, title: player.title, subtitle: player.subtitle, offsetMs, durationMs: d.durationMs, aid: null, sid: "auto", subFiles: [] }, { audioStream: null, subtitleStream: null });
}

/** Musique téléchargée : une file de titres lus depuis l'appareil (protocole « asset » de Tauri). */
export function playOfflineMusic(tracks: QueueTrack[], index: number) {
  reset(false);
  player.session = OFFLINE_MUSIC;
  player.queue = tracks;
  player.index = index;
  player.duration = (tracks[index]?.durationMs ?? 0) / 1000;
  web.playTrack(index);
}

export async function play(ratingKey: string, opts: PlayOptions = {}) {
  // Une vidéo se regarde ici : la musique diffusée sur un appareil s'arrête.
  if (cast.device) {
    invoke("cast_stop").catch(() => {});
    cast.device = null;
  }
  reset(true);
  player.ratingKey = ratingKey;
  // Épisodes voisins et repères (intro, générique), chargés à part : la lecture démarre sans les attendre.
  invoke<{ prev: string | null; next: string | null }>("plex_adjacent", { ratingKey }).then(
    (a) => {
      if (player.ratingKey !== ratingKey) return;
      Object.assign(player, { prevKey: a.prev, nextKey: a.next });
      if (a.next) loadUpNext(ratingKey, a.next);
    },
    () => {},
  );
  invoke<{ kind: string; startMs: number; endMs: number }[]>("plex_markers", { ratingKey }).then(
    (m) => {
      if (player.ratingKey === ratingKey) player.markers = m;
    },
    () => {},
  );
  const streams = {
    audioStream: opts.audioStream ?? null,
    subtitleStream: opts.subtitleStream ?? null,
  };
  player.streams = streams;
  const mediaIndex = opts.mediaIndex ?? 0;
  player.mediaIndex = mediaIndex;
  invoke<Previews | null>("video_previews", { ratingKey, mediaIndex }).then(
    (p) => {
      if (player.ratingKey === ratingKey && player.mediaIndex === mediaIndex) player.previews = p;
    },
    () => {},
  );
  const maxKbps = player.quality || null;
  const startMs = opts.startMs ?? null;
  try {
    // Qualité réduite : le serveur convertit, et le lecteur natif d'Android lit son flux.
    if (player.web && nativeVideo && maxKbps) {
      const s = await invoke<WebStreamInfo>("video_stream", { ratingKey, ...streams, maxKbps, mediaIndex });
      Object.assign(player, { session: s.session, title: s.title, subtitle: s.subtitle, duration: s.durationMs / 1000, native: true });
      markTranscoded();
      diag(`Maku : qualité réduite choisie (${maxKbps} kb/s), le serveur convertit`);
      web.startNativeStream(s.url, startMs ?? (opts.fromStart ? 0 : s.offsetMs), streams);
      return;
    }
    // Android : le fichier d'origine, lu par le lecteur natif (le serveur ne convertit qu'en secours).
    if (player.web && nativeVideo) {
      const s = await invoke<NativeInfo>("native_video", { ratingKey, ...streams, fromStart: opts.fromStart ?? false, startMs, mediaIndex });
      Object.assign(player, { session: s.session, title: s.title, subtitle: s.subtitle, duration: s.durationMs / 1000, native: true });
      web.startNative(s, streams);
      return;
    }
    if (player.web) {
      const s = await invoke<WebStreamInfo>("video_stream", { ratingKey, ...streams, maxKbps, mediaIndex });
      Object.assign(player, { session: s.session, title: s.title, subtitle: s.subtitle, duration: s.durationMs / 1000 });
      markTranscoded();
      web.startVideo(s.url, (startMs ?? (opts.fromStart ? 0 : s.offsetMs)) / 1000);
      return;
    }
    const now = await invoke<{ session: string; title: string; subtitle: string | null; embedded: boolean }>("play", {
      ratingKey,
      ...streams,
      fromStart: opts.fromStart ?? false,
      maxKbps,
      startMs,
      mediaIndex,
    });
    Object.assign(player, now);
    if (maxKbps) markTranscoded();
  } catch (e) {
    player.active = false;
    throw e;
  }
}

interface WebStreamInfo {
  session: string;
  url: string;
  title: string;
  subtitle: string | null;
  offsetMs: number;
  durationMs: number;
}

/** Vidéo convertie : ses pistes se choisissent parmi celles du serveur, en relançant la conversion. */
function markTranscoded() {
  player.transcoded = true;
  const key = player.ratingKey;
  const index = player.mediaIndex;
  invoke<{ audioTracks: Track[]; subtitleTracks: Track[]; versions: { audioTracks: Track[]; subtitleTracks: Track[] }[] }>(
    "plex_metadata",
    { ratingKey: key },
  ).then(
    (d) => {
      if (player.ratingKey !== key) return;
      const v = d.versions[index] ?? d;
      player.serverAudio = v.audioTracks;
      player.serverSubs = v.subtitleTracks;
    },
    () => {},
  );
}

/** Relance la vidéo en cours au même endroit, avec d'autres pistes ou une autre qualité. */
function restart(streams = player.streams) {
  if (!player.video || !player.ratingKey) return;
  const { ratingKey, prevKey, nextKey, upNext, markers, mediaIndex, previews } = player;
  const startMs = Math.round(player.position * 1000);
  const done = play(ratingKey, { ...streams, startMs, mediaIndex });
  // Les épisodes voisins, les repères et les aperçus ne changent pas : pas de clignotement.
  Object.assign(player, { prevKey, nextKey, upNext, markers, previews });
  return done;
}

/** Change la qualité de lecture (retenue pour les vidéos suivantes) et relance la vidéo en cours. */
export function setQuality(kbps: number) {
  if (kbps === player.quality) return;
  player.quality = kbps;
  try {
    localStorage.setItem("maku.quality", String(kbps));
  } catch {
    // Stockage indisponible : le choix vaut pour cette session.
  }
  return restart();
}

/** Vidéo convertie : change de piste audio ou de sous-titres (0 : aucun) en relançant la conversion. */
export function setServerStream(kind: "audio" | "subtitle", id: number) {
  const streams = { ...player.streams };
  if (kind === "audio") streams.audioStream = id;
  else streams.subtitleStream = id;
  return restart(streams);
}

/**
 * Joue un artiste, un album, une playlist ou un titre ; `start` : titre par lequel commencer ;
 * `keys` : une sélection de titres à enchaîner (par ex. « Titres du moment »).
 */
export async function playMusic(
  ratingKey: string,
  opts: { start?: string; shuffle?: boolean; keys?: string[]; radio?: boolean } = {},
) {
  const wasExpanded = player.expanded && !player.video;
  notePlay(ratingKey);
  reset(false);
  player.shuffled = false;
  unshuffled = null;
  // Téléviseur : le lecteur plein écran s'ouvre, comme dans Plex.
  player.expanded = wasExpanded || device.tv;
  try {
    const q = await invoke<{ session: string; tracks: QueueTrack[]; index: number }>(
      player.web || cast.device ? "music_queue" : "play_music",
      {
        ratingKey,
        keys: opts.keys ?? null,
        start: opts.start ?? null,
        shuffle: opts.shuffle ?? false,
        radio: opts.radio ?? false,
      },
    );
    player.session = q.session;
    player.queue = q.tracks;
    player.index = q.index;
    player.duration = (q.tracks[q.index]?.durationMs ?? 0) / 1000;
    if (cast.device) await sendToCast(cast.device, q.index, 0);
    else if (player.web) web.playTrack(q.index);
  } catch (e) {
    player.active = false;
    throw e;
  }
}

/**
 * Bouton Répéter, comme dans Plex : un appui répète la file, un deuxième le titre en cours, un troisième
 * arrête de répéter.
 */
export function cycleRepeat() {
  const all = !player.repeat && !player.repeatOne;
  const one = player.repeat && !player.repeatOne;
  // Le bouton change tout de suite ; mpv (ou l'appareil) confirme ensuite le même état.
  player.repeat = all;
  player.repeatOne = one;
  mpv("set_property", "loop-playlist", all ? "inf" : "no");
  mpv("set_property", "loop-file", one ? "inf" : "no");
}

/** Libellé du bouton Répéter : ce qu'il fait en ce moment. */
export const repeatLabel = () => t(player.repeatOne ? "musicBar.repeatOne" : player.repeat ? "musicBar.repeatQueue" : "musicBar.repeat");

/** Ordre de la file avant l'aléatoire (clés des titres), pour le retrouver en le désactivant. */
let unshuffled: string[] | null = null;

/**
 * Bouton Aléatoire du lecteur : les titres à venir passent dans un ordre au hasard (le titre en cours et ceux
 * déjà écoutés restent à leur place) ; un second appui remet les titres à venir dans l'ordre d'origine.
 */
export function toggleShuffle() {
  const queue = player.queue;
  const head = player.index + 1;
  if (queue.length - head < 2 && !player.shuffled) {
    player.shuffled = !player.shuffled;
    return;
  }
  const rest = [...Array(queue.length - head).keys()].map((i) => i + head);
  if (!player.shuffled) {
    unshuffled = queue.map((t) => t.ratingKey);
    for (let i = rest.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [rest[i], rest[j]] = [rest[j], rest[i]];
    }
  } else {
    // Les titres à venir reprennent leur ordre d'avant ; ceux ajoutés depuis restent après, dans l'ordre actuel.
    const rank = new Map<string, number>();
    (unshuffled ?? []).forEach((k, i) => rank.has(k) || rank.set(k, i));
    const at = (i: number) => rank.get(queue[i].ratingKey) ?? Number.MAX_SAFE_INTEGER;
    rest.sort((a, b) => at(a) - at(b) || a - b);
    unshuffled = null;
  }
  reorderQueue([...Array(head).keys(), ...rest]);
  player.shuffled = !player.shuffled;
}

/** Nouvel ordre de la file (`order[k]` : position d'avant du titre placé en `k`), le titre en cours ne bougeant pas. */
function reorderQueue(order: number[]) {
  const old = player.queue.slice();
  player.queue = order.map((i) => old[i]);
  if (cast.device) invoke("cast_queue_order", { order }).catch(() => {});
  else if (!player.web) invoke("player_queue_order", { order }).catch(() => {});
}

/** Une musique est en cours (ou en pause) : la file peut s'allonger sans l'interrompre. */
export const musicPlaying = () => player.active && !player.video && player.queue.length > 0;

/**
 * « Lire ensuite » (`next`) ou « Ajouter à la file d'attente » : les titres d'un titre, d'un album, d'un
 * artiste ou d'une playlist entrent dans la file sans couper le titre en cours. Sans musique en cours,
 * la lecture démarre avec eux.
 */
export async function addToQueue(ratingKey: string, next: boolean) {
  if (!musicPlaying()) {
    await playMusic(ratingKey);
    return;
  }
  const at = next ? player.index + 1 : player.queue.length;
  let tracks: QueueTrack[];
  if (!player.web && !cast.device) {
    tracks = await invoke<QueueTrack[]>("player_queue_insert", { ratingKey, at });
  } else {
    tracks = await invoke<QueueTrack[]>("music_tracks", { ratingKey });
    if (cast.device) {
      const items = tracks.map((q) => ({ url: q.url, title: q.title, artist: q.artist, album: q.album, cover: q.cover }));
      await invoke("cast_queue_insert", { tracks: items, at });
    }
  }
  player.queue.splice(Math.min(at, player.queue.length), 0, ...tracks);
  toast(next ? t("queue.playNextDone", { count: tracks.length }) : t("queue.addedDone", { count: tracks.length }));
}

/** Glisser-déposer dans la file d'attente : le titre `from` passe à la position `to`. */
export function moveInQueue(from: number, to: number) {
  const len = player.queue.length;
  if (from === to || from < 0 || to < 0 || from >= len || to >= len) return;
  const [moved] = player.queue.splice(from, 1);
  player.queue.splice(to, 0, moved);
  const i = player.index;
  player.index = i === from ? to : from < i && to >= i ? i - 1 : from > i && to <= i ? i + 1 : i;
  if (cast.device) invoke("cast_queue_move", { from, to }).catch(() => {});
  else if (!player.web) invoke("player_queue_move", { from, to }).catch(() => {});
}

/** Commande au lecteur, au format mpv (par ex. `["cycle", "pause"]`), quel que soit le moteur. */
export function mpv(...args: unknown[]) {
  if (cast.device && !player.video) return Promise.resolve(castCommand(args));
  if (player.web) return Promise.resolve(web.command(args));
  return invoke<void>("player_command", { args }).catch(() => {});
}

/** Vidéo figée (Android) : relancer au même endroit, ou décoder le son ici plutôt que l'envoyer à l'ampli. */
export function unstall(how: "retry" | "decodeAudio") {
  player.stalled = null;
  if (player.native) bridge?.videoCommand?.(how, "");
}

export function stop() {
  if (cast.device) {
    invoke("cast_stop").catch(() => {});
    cast.device = null;
  }
  endTranscode();
  if (player.web) web.teardown();
  else invoke("player_stop");
  player.active = false;
  player.expanded = false;
}

// ---------- Chromecast (cast.rs) et AirPlay ----------

export interface CastDevice {
  id: string;
  name: string;
  host: string;
  port: number;
}

/** Appareils trouvés sur le réseau, et celui qui joue la musique en ce moment (null : cet appareil). */
export const cast = $state({ devices: [] as CastDevice[], searching: false, device: null as CastDevice | null, connecting: false });

/** Chromecast : ordinateur, téléphone et tablette Android (pas le téléviseur, ni iOS qui a AirPlay). */
export const castAvailable = () => !device.tv && device.os !== "ios";

/** AirPlay : iPad et iPhone, par le sélecteur du système (le Mac suit la sortie son de macOS). */
export const airPlayAvailable = () => device.os === "ios" && player.web;

export function showAirPlay() {
  web.airPlay();
}

export async function findCastDevices() {
  if (cast.searching) return;
  cast.searching = true;
  try {
    cast.devices = await invoke<CastDevice[]>("cast_devices");
  } finally {
    cast.searching = false;
  }
}

async function sendToCast(target: CastDevice, index: number, position: number) {
  cast.connecting = true;
  try {
    await invoke("cast_start", {
      device: target,
      tracks: player.queue.map((q) => ({ url: q.url, title: q.title, artist: q.artist, album: q.album, cover: q.cover })),
      index,
      position,
    });
    cast.device = target;
    Object.assign(player, { index, position, buffering: true });
  } finally {
    cast.connecting = false;
  }
}

/**
 * Envoie la musique en cours sur un appareil Chromecast, au même titre et au même endroit ; avec `null`,
 * la reprend sur cet appareil. La lecture locale s'arrête sans fermer le lecteur.
 */
export async function castTo(target: CastDevice | null) {
  const { index, position } = player;
  if (target) {
    if (!player.active || player.video || cast.device?.id === target.id) return;
    const wasCasting = !!cast.device;
    if (!wasCasting) stopLocal();
    await sendToCast(target, index, position);
    return;
  }
  if (!cast.device) return;
  invoke("cast_stop").catch(() => {});
  cast.device = null;
  if (!player.active || !player.queue.length) return;
  if (player.web) {
    web.playTrack(index);
    web.command(["seek", position, "absolute"]);
    return;
  }
  // Ordinateur : mpv repart avec la même file, sur le titre en cours.
  const q = await invoke<{ session: string; tracks: QueueTrack[]; index: number }>("play_music", {
    ratingKey: "",
    keys: player.queue.map((x) => x.ratingKey),
    start: player.queue[index]?.ratingKey ?? null,
    shuffle: false,
    radio: false,
  });
  Object.assign(player, { session: q.session, queue: q.tracks, index: q.index });
  setTimeout(() => invoke("player_command", { args: ["seek", position, "absolute"] }).catch(() => {}), 900);
}

/** Arrête la lecture locale sans fermer le lecteur (la diffusion prend le relais). */
function stopLocal() {
  endTranscode();
  if (player.web) web.teardown();
  else {
    // mpv signale sa fermeture : une session vide la fait ignorer.
    player.session = "";
    invoke("player_stop").catch(() => {});
  }
}

/** Répétition de la file ou du titre, appliquée par l'appareil Chromecast lui-même. */
function castRepeat() {
  const mode = player.repeatOne ? "REPEAT_SINGLE" : player.repeat ? "REPEAT_ALL" : "REPEAT_OFF";
  invoke("cast_repeat", { mode }).catch(() => {});
}

/** Les commandes au format mpv, traduites pour l'appareil. */
function castCommand(args: unknown[]) {
  const send = (action: string, value?: number) => invoke("cast_command", { action, value: value ?? null }).catch(() => {});
  const [cmd, a, b] = args as [string, unknown, unknown];
  switch (cmd) {
    case "cycle":
      if (a === "pause") {
        player.paused = !player.paused;
        send("toggle");
      } else if (a === "mute") send("mute", (player.muted = !player.muted) ? 1 : 0);
      break;
    case "seek": {
      const to = Math.max(0, b === "absolute" ? Number(a) : player.position + Number(a));
      player.position = to;
      send("seek", to);
      break;
    }
    case "add":
      if (a === "volume") send("volume", (player.volume = Math.min(100, Math.max(0, player.volume + Number(b)))));
      break;
    case "set_property":
      if (a === "volume") send("volume", (player.volume = Math.min(100, Number(b))));
      else if (a === "mute") send("mute", (player.muted = !!b) ? 1 : 0);
      else if (a === "playlist-pos") send("jump", Number(b));
      else if (a === "loop-playlist") {
        player.repeat = b !== "no";
        castRepeat();
      } else if (a === "loop-file") {
        player.repeatOne = b !== "no";
        castRepeat();
      }
      break;
    case "playlist-next":
      send("next");
      break;
    case "playlist-prev":
      send("previous");
      break;
  }
}

listen<{ state: string; position: number; duration: number; index: number | null; volume: number | null; muted: boolean | null }>("cast-status", ({ payload: s }) => {
  if (!cast.device || player.video) return;
  if (s.state) {
    player.paused = s.state === "PAUSED";
    player.buffering = s.state === "BUFFERING";
  }
  if (s.index !== null && s.index >= 0 && s.index < player.queue.length && s.index !== player.index) player.index = s.index;
  if (s.state && s.state !== "IDLE") player.position = s.position;
  if (s.duration > 0) player.duration = s.duration;
  if (s.volume !== null) player.volume = Math.round(s.volume * 100);
  if (s.muted !== null) player.muted = s.muted;
}).catch(() => {});

/** Fin de la diffusion : file terminée, appareil repris par une autre application, ou connexion perdue. */
listen<string>("cast-ended", ({ payload: reason }) => {
  if (!cast.device) return;
  cast.device = null;
  if (player.video) return;
  player.active = false;
  player.expanded = false;
  if (reason === "disconnected" || reason === "closed") toast(t("cast.ended"), true);
}).catch(() => {});

listen<string>("cast-error", ({ payload }) => toast(payload, true)).catch(() => {});

/** Titre précédent, ou retour au début du titre s'il joue depuis plus de 3 s. */
export function previous() {
  if (player.position > 3 || player.index === 0) mpv("seek", 0, "absolute");
  else mpv("playlist-prev");
}

export function next() {
  mpv("playlist-next");
}

export function trackLabel(track: MpvTrack): string {
  const parts = [track.title, track.lang?.toUpperCase()].filter(Boolean);
  if (track.type === "audio" && track["demux-channel-count"])
    parts.push(t("player.channels", { channels: track["demux-channel-count"] }));
  if (track.forced) parts.push(t("player.forced"));
  return parts.length ? parts.join(" · ") : t("player.trackNumber", { id: track.id });
}

// ---------- Lecteur intégré du système (mobile, TV) ----------

/** Lecture par les éléments <video> / <audio> de la page, avec le suivi Plex. */
const web = (() => {
  let media: HTMLMediaElement | null = null;
  let videoEl: HTMLVideoElement | null = null;
  let audioEl: HTMLAudioElement | null = null;
  let hls: { destroy(): void } | null = null;
  let pending: { url: string; offset: number } | null = null;
  let reportTimer: ReturnType<typeof setInterval> | undefined;
  let reportedKey = "";
  let unbind: (() => void) | null = null;
  // Musique sans blanc : le titre suivant est chargé dans un second élément pendant la fin du titre en cours,
  // puis lancé juste avant la dernière milliseconde (le passage d'un élément à l'autre ne laisse pas de trou).
  let nextEl: HTMLAudioElement | null = null;
  let nextIndex = -1;
  let handoff: ReturnType<typeof setTimeout> | undefined;

  const followingIndex = () =>
    player.repeatOne ? -1 : player.index < player.queue.length - 1 ? player.index + 1 : player.repeat && player.queue.length > 1 ? 0 : -1;

  /** Pendant les 20 dernières secondes d'un titre : le suivant se charge, et l'enchaînement se prépare. */
  function prepareNext(el: HTMLMediaElement) {
    if (player.video || el !== audioEl || !Number.isFinite(el.duration)) return;
    const left = el.duration - el.currentTime;
    if (left > 20) return;
    const ni = followingIndex();
    if (ni < 0) return;
    // La file a pu changer (titre ajouté ou déplacé) : le suivant préparé doit être le bon.
    if (nextIndex !== ni || !nextEl || nextEl.src !== player.queue[ni].url) {
      nextEl ??= new Audio();
      nextEl.preload = "auto";
      nextEl.src = player.queue[ni].url;
      nextEl.load();
      nextIndex = ni;
    }
    // Dernier quart de seconde, titre suivant prêt : on le lance à l'instant où celui-ci finit.
    if (left < 0.35 && !handoff && !el.paused && nextEl.readyState >= 3) {
      const from = player.index;
      handoff = setTimeout(() => {
        handoff = undefined;
        if (player.index === from && !player.video) playTrack(ni);
      }, Math.max(0, (left - 0.03) * 1000));
    }
  }
  // Lecteur natif : pistes demandées (pour le secours) et conversion du serveur en cours.
  let nativeStreams: { audioStream: number | null; subtitleStream: number | null } = { audioStream: null, subtitleStream: null };
  let fallbackTried = false;
  /** Le secours en cours ne convertit que le son (la vidéo d'origine est copiée). */
  let copyTried = false;
  let transcoding = false;

  const key = () => (player.video ? player.ratingKey : (player.queue[player.index]?.ratingKey ?? ""));

  function report(state: "playing" | "paused" | "stopped", ratingKey = key()): Promise<unknown> {
    if (!ratingKey) return Promise.resolve();
    // Téléchargement : la progression est gardée sur l'appareil et renvoyée au serveur plus tard (offline.rs).
    // La musique hors ligne ne la note pas.
    if (player.session === OFFLINE_MUSIC) return Promise.resolve();
    if (player.offline) {
      const ms = Math.round(player.position * 1000);
      const watched = player.duration > 0 && player.position / player.duration > 0.9;
      return invoke("offline_progress", { id: player.offline, positionMs: ms, watched }).catch(() => {});
    }
    return invoke("report_timeline", {
      ratingKey,
      playback: state,
      timeMs: Math.round(player.position * 1000),
      durationMs: Math.round(player.duration * 1000),
      session: player.session || null,
    }).catch(() => {});
  }

  function bind(el: HTMLMediaElement) {
    unbind?.();
    const on: [string, () => void][] = [
      ["timeupdate", () => ((player.position = el.currentTime), (player.buffering = false), prepareNext(el))],
      ["durationchange", () => Number.isFinite(el.duration) && el.duration > 0 && (player.duration = el.duration)],
      ["play", () => (player.paused = false)],
      ["pause", () => ((player.paused = true), report("paused"))],
      ["waiting", () => (player.buffering = true)],
      ["playing", () => (player.buffering = false)],
      ["volumechange", () => ((player.volume = Math.round(el.volume * 100)), (player.muted = el.muted))],
      ["progress", () => el.buffered.length && (player.bufferedUntil = el.buffered.end(el.buffered.length - 1))],
      ["seeked", () => player.seeks++],
      ["ended", ended],
    ];
    for (const [ev, fn] of on) el.addEventListener(ev, fn);
    unbind = () => on.forEach(([ev, fn]) => el.removeEventListener(ev, fn));
    media = el;
    clearInterval(reportTimer);
    reportTimer = setInterval(() => report(player.paused ? "paused" : "playing"), 10000);
  }

  function ended() {
    if (player.video) {
      videoEnded();
      return;
    }
    if (player.repeatOne) playTrack(player.index);
    else if (player.index < player.queue.length - 1) playTrack(player.index + 1);
    else if (player.repeat) playTrack(0);
    else stop();
  }

  /** État publié par le lecteur natif (toutes les 500 ms, et à chaque changement). */
  type NativeEvent = {
    position?: number;
    duration?: number;
    buffered?: number;
    paused?: boolean;
    buffering?: boolean;
    volume?: number;
    tracks?: MpvTrack[];
    aid?: number | null;
    sid?: number | null;
    ended?: boolean;
    unsupported?: string;
    error?: string;
    stalled?: { at: number; passthrough: boolean } | false;
  };

  (globalThis as { __plexrVideo?: (e: NativeEvent) => void }).__plexrVideo = (e) => {
    if (!player.native) return;
    if (e.stalled !== undefined) player.stalled = e.stalled ? { at: e.stalled.at / 1000, passthrough: e.stalled.passthrough } : null;
    if (e.ended) return videoEnded();
    if (e.unsupported !== undefined || e.error !== undefined) {
      // Codec ou format que l'appareil ne lit pas : le serveur convertit. Si seul le son pose problème,
      // d'abord en gardant la vidéo d'origine ; en cas d'échec, la conversion complète, comme avant.
      diag(`Maku : ${e.unsupported !== undefined ? `format non lisible ici (${e.unsupported})` : `erreur de lecture (${e.error})`}`);
      // Fichier téléchargé : pas de serveur à qui demander une conversion.
      if (player.offline) return stop();
      if (!fallbackTried) fallback(e.unsupported === "audio" && !player.quality);
      else if (copyTried) fallback(false);
      else stop();
      return;
    }
    if (e.position !== undefined) player.position = e.position / 1000;
    if (e.duration) player.duration = e.duration / 1000;
    if (e.buffered !== undefined) player.bufferedUntil = e.buffered / 1000;
    if (e.paused !== undefined && e.paused !== player.paused) {
      player.paused = e.paused;
      if (e.paused) report("paused");
    }
    if (e.buffering !== undefined) player.buffering = e.buffering;
    if (e.volume !== undefined) player.volume = e.volume;
    if (e.tracks) player.tracks = e.tracks;
    if (e.aid !== undefined) player.aid = e.aid;
    if (e.sid !== undefined) player.sid = e.sid;
  };

  async function fallback(copyVideo: boolean) {
    // Une conversion « son seulement » qui a échoué : on l'arrête avant la conversion complète.
    if (copyTried && player.session) invoke("stop_transcode", { session: player.session }).catch(() => {});
    fallbackTried = true;
    copyTried = copyVideo;
    diag(copyVideo ? "Maku : secours, le serveur garde l'image et convertit le son" : "Maku : secours, le serveur convertit l'image et le son");
    const key = player.ratingKey;
    try {
      const s = await invoke<{ session: string; url: string }>("video_stream", {
        ratingKey: key,
        ...nativeStreams,
        maxKbps: player.quality || null,
        mediaIndex: player.mediaIndex,
        copyVideo,
      });
      if (player.ratingKey !== key || !player.native) return;
      player.session = s.session;
      transcoding = true;
      markTranscoded();
      bridge!.videoOpen!(JSON.stringify({ passthrough: passthroughMode(),  url: s.url, offsetMs: Math.round(player.position * 1000), aid: null, sid: "auto", subFiles: [] }));
    } catch (e) {
      diag(`Maku : secours impossible (${String(e).slice(0, 300)})`);
      stop();
    }
  }

  function nativeCommand(args: unknown[]) {
    const send = (name: string, value: unknown = "") => bridge!.videoCommand!(name, String(value));
    const [cmd, a, b] = args as [string, unknown, unknown];
    switch (cmd) {
      case "cycle":
        if (a === "pause") send("toggle");
        else if (a === "mute") send("mute", (player.muted = !player.muted) ? 1 : 0);
        break;
      case "seek":
        send(b === "absolute" ? "seek" : "seekBy", Math.round(Number(a) * 1000));
        player.seeks++;
        break;
      case "add":
        if (a === "volume") send("volume", Math.min(100, Math.max(0, player.volume + Number(b))));
        break;
      case "set_property":
        if (a === "volume") send("volume", Math.min(100, Number(b)));
        else if (a === "mute") send("mute", (player.muted = !!b) ? 1 : 0);
        else if (a === "aid" || a === "sid") send(a, b);
        break;
    }
  }

  async function load(el: HTMLVideoElement, url: string, offset: number) {
    el.addEventListener("loadedmetadata", () => offset > 0 && (el.currentTime = offset), { once: true });
    // Le lecteur d'Android et d'iOS lit le HLS nativement ; sinon hls.js s'en charge.
    if (el.canPlayType("application/vnd.apple.mpegurl")) {
      el.src = url;
    } else {
      const { default: Hls } = await import("hls.js");
      const h = new Hls();
      h.loadSource(url);
      h.attachMedia(el);
      hls = h;
    }
    el.play().catch(() => {});
  }

  return {
    /** L'élément <video> du lecteur plein écran s'enregistre ici à son affichage. */
    attachVideo(el: HTMLVideoElement) {
      videoEl = el;
      bind(el);
      if (pending) {
        load(el, pending.url, pending.offset);
        pending = null;
      }
    },

    startNative(s: NativeInfo, streams: { audioStream: number | null; subtitleStream: number | null }) {
      reportedKey = player.ratingKey;
      nativeStreams = streams;
      fallbackTried = false;
      copyTried = false;
      transcoding = false;
      bridge!.videoOpen!(JSON.stringify({ passthrough: passthroughMode(),  url: s.url, offsetMs: s.offsetMs, aid: s.aid, sid: s.sid, subFiles: s.subFiles }));
      clearInterval(reportTimer);
      reportTimer = setInterval(() => report(player.paused ? "paused" : "playing"), 10000);
    },

    /** Flux converti par le serveur (qualité réduite), lu par le lecteur natif d'Android. */
    startNativeStream(url: string, offsetMs: number, streams: { audioStream: number | null; subtitleStream: number | null }) {
      reportedKey = player.ratingKey;
      nativeStreams = streams;
      fallbackTried = true;
      copyTried = false;
      transcoding = true;
      bridge!.videoOpen!(JSON.stringify({ passthrough: passthroughMode(),  url, offsetMs, aid: null, sid: "auto", subFiles: [] }));
      clearInterval(reportTimer);
      reportTimer = setInterval(() => report(player.paused ? "paused" : "playing"), 10000);
    },

    startVideo(url: string, offset: number) {
      reportedKey = player.ratingKey;
      if (videoEl) {
        bind(videoEl);
        load(videoEl, url, offset);
      } else {
        pending = { url, offset };
      }
    },

    playTrack(index: number) {
      if (reportedKey && reportedKey !== player.queue[index]?.ratingKey) report("stopped", reportedKey);
      const t = player.queue[index];
      if (!t) return;
      Object.assign(player, { index, position: 0, duration: t.durationMs / 1000, buffering: true });
      reportedKey = t.ratingKey;
      clearTimeout(handoff);
      handoff = undefined;
      if (nextEl && nextIndex === index && nextEl.src === t.url) {
        // Titre déjà chargé : les deux éléments échangent leur rôle, le son part aussitôt.
        const done = audioEl;
        audioEl = nextEl;
        if (done) {
          audioEl.volume = done.volume;
          audioEl.muted = done.muted;
        }
        bind(audioEl);
        audioEl.play().catch(() => {});
        done?.pause();
        nextEl = done;
        nextIndex = -1;
        return;
      }
      audioEl ??= new Audio();
      if (media !== audioEl) bind(audioEl);
      audioEl.src = t.url;
      audioEl.play().catch(() => {});
    },

    /** Traduit les commandes au format mpv pour l'élément <video> / <audio>. */
    command(args: unknown[]) {
      if (player.native) return nativeCommand(args);
      const el = media;
      const [cmd, a, b] = args as [string, unknown, unknown];
      if (!el) return;
      switch (cmd) {
        case "cycle":
          if (a === "pause") (el.paused ? el.play().catch(() => {}) : el.pause());
          else if (a === "mute") el.muted = !el.muted;
          break;
        case "seek":
          el.currentTime = Math.max(0, b === "absolute" ? Number(a) : el.currentTime + Number(a));
          break;
        case "add":
          if (a === "volume") el.volume = Math.min(1, Math.max(0, el.volume + Number(b) / 100));
          break;
        case "set_property":
          if (a === "volume") el.volume = Math.min(1, Number(b) / 100);
          else if (a === "mute") el.muted = !!b;
          else if (a === "playlist-pos") playTrack(Number(b));
          else if (a === "loop-playlist") player.repeat = b !== "no";
          else if (a === "loop-file") player.repeatOne = b !== "no";
          break;
        case "playlist-next":
          if (player.index < player.queue.length - 1) playTrack(player.index + 1);
          else if (player.repeat) playTrack(0);
          break;
        case "playlist-prev":
          if (player.index > 0) playTrack(player.index - 1);
          break;
      }
    },

    /** Arrête tout : dernière progression envoyée, conversion du serveur libérée. */
    teardown() {
      if (reportedKey) {
        const wasVideo = player.video;
        const sent = report("stopped", reportedKey);
        // Une vidéo terminée : les pages relisent l'état « vu » et la reprise, une fois le serveur prévenu.
        if (wasVideo) sent.finally(notifyPlaybackEnded);
      }
      reportedKey = "";
      clearInterval(reportTimer);
      unbind?.();
      unbind = null;
      hls?.destroy();
      hls = null;
      pending = null;
      clearTimeout(handoff);
      handoff = undefined;
      nextIndex = -1;
      for (const el of [videoEl, audioEl, nextEl]) {
        if (!el) continue;
        el.pause();
        el.removeAttribute("src");
        el.load();
      }
      if (player.native) {
        bridge?.videoClose?.();
        if (transcoding) invoke("stop_transcode", { session: player.session }).catch(() => {});
        transcoding = false;
      } else if (player.web && player.video && player.session) {
        invoke("stop_transcode", { session: player.session }).catch(() => {});
      }
      media = null;
    },

    detachVideo() {
      videoEl = null;
    },

    airPlay() {
      (audioEl as (HTMLAudioElement & { webkitShowPlaybackTargetPicker?: () => void }) | null)?.webkitShowPlaybackTargetPicker?.();
    },
  };

  function playTrack(index: number) {
    web.playTrack(index);
  }
})();

export const attachVideo = (el: HTMLVideoElement) => web.attachVideo(el);
export const detachVideo = () => web.detachVideo();

// ---------- Événements de mpv (ordinateur) ----------

listen<{ name: string; data: unknown }>("player-prop", ({ payload: { name, data } }) => {
  switch (name) {
    case "time-pos":
      if (typeof data === "number") {
        player.position = data;
        player.buffering = false;
      }
      break;
    case "duration":
      if (typeof data === "number") player.duration = data;
      break;
    case "pause":
      player.paused = !!data;
      break;
    case "paused-for-cache":
      player.buffering = !!data;
      break;
    case "demuxer-cache-time":
      player.bufferedUntil = (data as number) ?? 0;
      break;
    case "volume":
      player.volume = (data as number) ?? 100;
      break;
    case "mute":
      player.muted = !!data;
      break;
    case "track-list":
      player.tracks = (data as MpvTrack[]) ?? [];
      break;
    case "aid":
      player.aid = typeof data === "number" ? data : null;
      break;
    case "sid":
      player.sid = typeof data === "number" ? data : null;
      break;
    case "playlist-pos":
      if (typeof data === "number" && data >= 0 && data < player.queue.length && data !== player.index) {
        player.index = data;
        player.position = 0;
        player.duration = player.queue[data].durationMs / 1000;
      }
      break;
    case "loop-playlist":
      player.repeat = data !== false && data !== "no";
      break;
    case "loop-file":
      player.repeatOne = data !== false && data !== "no";
      break;
    case "vo-configured":
      // Un court délai laisse mpv peindre sa première image avant que la page ne s'efface.
      if (data === true) setTimeout(() => player.active && player.video && (player.videoReady = true), 150);
      else player.videoReady = false;
      break;
  }
});

// Une lecture remplacée signale sa fin après le démarrage de la suivante : on l'ignore.
listen<string>("player-stopped", ({ payload }) => {
  if (payload === player.session) {
    // mpv se ferme à la fin du fichier (et pas quand on l'arrête, `active` est alors déjà faux) : l'écran de fin.
    if (player.active && hasEndScreen() && player.position >= player.duration - 10) {
      endTranscode();
      player.endScreen = true;
      return;
    }
    endTranscode();
    player.transcoded = false;
    player.active = false;
    player.expanded = false;
  }
});

// ---------- Commandes venues de l'extérieur ----------

/**
 * Commande reçue d'ailleurs que la page : la notification de musique d'Android (MediaService.kt),
 * ou les touches multimédia du clavier sous Linux, que le bureau transmet par MPRIS (mpris.rs).
 */
export function mediaCommand(action: string) {
  if (!player.active) return;
  if (action === "play_pause") mpv("cycle", "pause");
  else if (action === "play" || action === "pause") {
    if (player.paused === (action === "play")) mpv("cycle", "pause");
  } else if (action === "next") {
    if (player.video) {
      if (player.nextKey) play(player.nextKey);
    } else next();
  } else if (action === "previous") {
    if (player.video) {
      if (player.prevKey) play(player.prevKey);
    } else previous();
  } else if (action === "stop") stop();
  else if (action.startsWith("seek:")) mpv("seek", Number(action.slice(5)) / 1000, "absolute");
}

listen<string>("media-command", (e) => mediaCommand(e.payload)).catch(() => {});

/**
 * Ce qui joue, pour les commandes extérieures : sur Android, la notification (pochette, titre,
 * précédent / lecture / suivant) qui garde aussi la musique en vie en arrière-plan ; sous Linux, le
 * lecteur MPRIS du bureau. Les commandes reviennent par `mediaCommand`.
 */
const android = (globalThis as { PlexrDevice?: { mediaUpdate?(json: string): void; mediaStop?(): void } }).PlexrDevice;
$effect.root(() => {
  $effect(() => {
    void player.seeks;
    const track = current();
    // Android : la musique jouée dans la page seulement ; ailleurs, aussi la vidéo.
    const nothing = !player.active || (android ? !player.web || !track : !track && !player.title);
    if (nothing) {
      if (android) android.mediaStop?.();
      else invoke("media_update", { json: null }).catch(() => {});
      return;
    }
    const json = JSON.stringify({
      title: track?.title ?? player.title,
      artist: track?.artist ?? player.subtitle ?? "",
      album: track?.album ?? "",
      art: track?.cover ?? "",
      playing: !player.paused,
      position: Math.round(untrack(() => player.position) * 1000),
      duration: Math.round(player.duration * 1000),
    });
    if (android?.mediaUpdate) android.mediaUpdate(json);
    else invoke("media_update", { json }).catch(() => {});
  });
});

if (android?.mediaUpdate) {
  (globalThis as { __plexrMedia?: (action: string) => void }).__plexrMedia = (action) => {
    mediaCommand(action);
  };
}
