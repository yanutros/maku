import { invoke } from "@tauri-apps/api/core";
import { notePlay } from "./playlistPlays";
import { listen } from "@tauri-apps/api/event";
import { untrack } from "svelte";
import type { QueueTrack, Track } from "./api";
import { t } from "./i18n";
import { device } from "./device";

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
  repeat: false,
  /** Vue « En cours de lecture » ouverte. */
  expanded: false,
  /** Incrémenté à chaque saut dans le morceau (pour la notification Android). */
  seeks: 0,
});

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
};
const bridge = (globalThis as { PlexrDevice?: VideoBridge }).PlexrDevice;
const nativeVideo = !!bridge?.videoOpen;

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
    expanded: false,
  });
}

export async function play(ratingKey: string, opts: PlayOptions = {}) {
  reset(true);
  player.ratingKey = ratingKey;
  // Épisodes voisins et repères (intro, générique), chargés à part : la lecture démarre sans les attendre.
  invoke<{ prev: string | null; next: string | null }>("plex_adjacent", { ratingKey }).then(
    (a) => {
      if (player.ratingKey === ratingKey) Object.assign(player, { prevKey: a.prev, nextKey: a.next });
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
  const { ratingKey, prevKey, nextKey, markers, mediaIndex, previews } = player;
  const startMs = Math.round(player.position * 1000);
  const done = play(ratingKey, { ...streams, startMs, mediaIndex });
  // Les épisodes voisins, les repères et les aperçus ne changent pas : pas de clignotement.
  Object.assign(player, { prevKey, nextKey, markers, previews });
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
  opts: { start?: string; shuffle?: boolean; keys?: string[] } = {},
) {
  const wasExpanded = player.expanded && !player.video;
  notePlay(ratingKey);
  reset(false);
  // Téléviseur : le lecteur plein écran s'ouvre, comme dans Plex.
  player.expanded = wasExpanded || device.tv;
  try {
    const q = await invoke<{ session: string; tracks: QueueTrack[]; index: number }>(
      player.web ? "music_queue" : "play_music",
      {
        ratingKey,
        keys: opts.keys ?? null,
        start: opts.start ?? null,
        shuffle: opts.shuffle ?? false,
      },
    );
    player.session = q.session;
    player.queue = q.tracks;
    player.index = q.index;
    player.duration = (q.tracks[q.index]?.durationMs ?? 0) / 1000;
    if (player.web) web.playTrack(q.index);
  } catch (e) {
    player.active = false;
    throw e;
  }
}

/** Commande au lecteur, au format mpv (par ex. `["cycle", "pause"]`), quel que soit le moteur. */
export function mpv(...args: unknown[]) {
  if (player.web) return Promise.resolve(web.command(args));
  return invoke<void>("player_command", { args }).catch(() => {});
}

export function stop() {
  endTranscode();
  if (player.web) web.teardown();
  else invoke("player_stop");
  player.active = false;
  player.expanded = false;
}

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
  // Lecteur natif : pistes demandées (pour le secours) et conversion du serveur en cours.
  let nativeStreams: { audioStream: number | null; subtitleStream: number | null } = { audioStream: null, subtitleStream: null };
  let fallbackTried = false;
  /** Le secours en cours ne convertit que le son (la vidéo d'origine est copiée). */
  let copyTried = false;
  let transcoding = false;

  const key = () => (player.video ? player.ratingKey : (player.queue[player.index]?.ratingKey ?? ""));

  function report(state: "playing" | "paused" | "stopped", ratingKey = key()) {
    if (!ratingKey) return;
    invoke("report_timeline", {
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
      ["timeupdate", () => ((player.position = el.currentTime), (player.buffering = false))],
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
      stop();
      return;
    }
    if (player.index < player.queue.length - 1) playTrack(player.index + 1);
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
  };

  (globalThis as { __plexrVideo?: (e: NativeEvent) => void }).__plexrVideo = (e) => {
    if (!player.native) return;
    if (e.ended) return stop();
    if (e.unsupported !== undefined || e.error !== undefined) {
      // Codec ou format que l'appareil ne lit pas : le serveur convertit. Si seul le son pose problème,
      // d'abord en gardant la vidéo d'origine ; en cas d'échec, la conversion complète, comme avant.
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
    } catch {
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
      if (reportedKey) report("stopped", reportedKey);
      reportedKey = "";
      clearInterval(reportTimer);
      unbind?.();
      unbind = null;
      hls?.destroy();
      hls = null;
      pending = null;
      for (const el of [videoEl, audioEl]) {
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
