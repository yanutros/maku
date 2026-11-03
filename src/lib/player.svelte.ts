import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { untrack } from "svelte";
import type { ItemDetail, QueueTrack } from "./api";
import { t } from "./i18n";

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
  bufferedUntil: 0,
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

export const current = () => (player.video ? null : (player.queue[player.index] ?? null));

// Sur Android et iOS, mpv n'existe pas : le lecteur du système prend le relais.
invoke<string | null>("platform").then(
  (os) => (player.web = os === "android" || os === "ios"),
  () => {},
);

/** Lecteur vidéo natif d'Android (NativeVideo.kt), quand l'application le fournit. */
type VideoBridge = { videoOpen?(json: string): void; videoCommand?(name: string, value: string): void; videoClose?(): void };
const bridge = (globalThis as { PlexrDevice?: VideoBridge }).PlexrDevice;
const nativeVideo = !!bridge?.videoOpen;

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

export interface PlayOptions {
  /** Identifiants de pistes Plex ; `subtitleStream: 0` désactive les sous-titres. */
  audioStream?: number | null;
  subtitleStream?: number | null;
  fromStart?: boolean;
}

function reset(video: boolean) {
  web.teardown();
  Object.assign(player, {
    active: true,
    video,
    embedded: true,
    native: false,
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
    bufferedUntil: 0,
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
  try {
    // Android : le fichier d'origine, lu par le lecteur natif (le serveur ne convertit qu'en secours).
    if (player.web && nativeVideo) {
      const s = await invoke<NativeInfo>("native_video", { ratingKey, ...streams, fromStart: opts.fromStart ?? false });
      Object.assign(player, { session: s.session, title: s.title, subtitle: s.subtitle, duration: s.durationMs / 1000, native: true });
      web.startNative(s, streams);
      return;
    }
    if (player.web) {
      const s = await invoke<{
        session: string;
        url: string;
        title: string;
        subtitle: string | null;
        offsetMs: number;
        durationMs: number;
      }>("video_stream", { ratingKey, ...streams });
      Object.assign(player, { session: s.session, title: s.title, subtitle: s.subtitle, duration: s.durationMs / 1000 });
      web.startVideo(s.url, opts.fromStart ? 0 : s.offsetMs / 1000);
      // Pistes de la fiche (identifiants Plex), pour en changer pendant la lecture (iPad, navigateur).
      api_metadata(ratingKey).then(
        (d) => {
          if (player.ratingKey !== ratingKey || player.native) return;
          player.tracks = [
            ...d.audioTracks.map((tr) => ({ id: tr.id, type: "audio" as const, title: tr.label })),
            ...d.subtitleTracks.map((tr) => ({ id: tr.id, type: "sub" as const, title: tr.label })),
          ];
          player.aid = streams.audioStream ?? d.audioTracks.find((tr) => tr.selected)?.id ?? null;
          // Sous-titres : 0 demande « aucun » ; sans choix, ceux que Plex retient pour ce titre.
          player.sid =
            streams.subtitleStream === 0 ? null : (streams.subtitleStream ?? d.subtitleTracks.find((tr) => tr.selected)?.id ?? null);
        },
        () => {},
      );
      return;
    }
    const now = await invoke<{ session: string; title: string; subtitle: string | null; embedded: boolean }>("play", {
      ratingKey,
      ...streams,
      fromStart: opts.fromStart ?? false,
    });
    Object.assign(player, now);
  } catch (e) {
    player.active = false;
    throw e;
  }
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
  reset(false);
  player.expanded = wasExpanded;
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

const api_metadata = (ratingKey: string) => invoke<ItemDetail>("plex_metadata", { ratingKey });

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
  let transcoding = false;

  const key = () => (player.video ? player.ratingKey : (player.queue[player.index]?.ratingKey ?? ""));

  function report(state: "playing" | "paused" | "stopped", ratingKey = key()) {
    if (!ratingKey) return;
    invoke("report_timeline", {
      ratingKey,
      playback: state,
      timeMs: Math.round(player.position * 1000),
      durationMs: Math.round(player.duration * 1000),
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
      // Codec ou format que l'appareil ne lit pas : le serveur convertit, comme avant.
      if (!fallbackTried) fallback();
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

  async function fallback() {
    fallbackTried = true;
    const key = player.ratingKey;
    try {
      const s = await invoke<{ session: string; url: string }>("video_stream", { ratingKey: key, ...nativeStreams });
      if (player.ratingKey !== key || !player.native) return;
      player.session = s.session;
      transcoding = true;
      bridge!.videoOpen!(JSON.stringify({ url: s.url, offsetMs: Math.round(player.position * 1000), aid: null, sid: "auto", subFiles: [] }));
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

  /**
   * Flux converti : une autre piste audio ou d'autres sous-titres demandent un nouveau flux au serveur,
   * repris au même moment (les sous-titres y sont incrustés). L'ancienne conversion est libérée.
   */
  async function switchStream(kind: "aid" | "sid", value: unknown) {
    const key = player.ratingKey;
    const at = player.position;
    const audioStream = kind === "aid" ? Number(value) : player.aid;
    const subtitleStream = kind === "sid" ? (value === "no" ? 0 : Number(value)) : (player.sid ?? 0);
    const previous = player.session;
    try {
      const s = await invoke<{ session: string; url: string }>("video_stream", { ratingKey: key, audioStream, subtitleStream });
      if (player.ratingKey !== key || !videoEl) return;
      invoke("stop_transcode", { session: previous }).catch(() => {});
      Object.assign(player, { session: s.session, aid: audioStream, sid: subtitleStream || null, buffering: true });
      hls?.destroy();
      hls = null;
      load(videoEl, s.url, at);
    } catch {
      // Le flux en cours continue tel quel.
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
      transcoding = false;
      bridge!.videoOpen!(JSON.stringify({ url: s.url, offsetMs: s.offsetMs, aid: s.aid, sid: s.sid, subFiles: s.subFiles }));
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
          else if ((a === "aid" || a === "sid") && player.video) switchStream(a, b);
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
  }
});

// Une lecture remplacée signale sa fin après le démarrage de la suivante : on l'ignore.
listen<string>("player-stopped", ({ payload }) => {
  if (payload === player.session) {
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
