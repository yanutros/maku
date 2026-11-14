// État du lecteur, partagé par toute l'interface : ce qui joue, la file, les pistes, les réglages de lecture.
// Ce module ne dépend d'aucune autre partie du lecteur (player.svelte.ts le réexporte).

import { invoke } from "@tauri-apps/api/core";
import type { QueueTrack, Track } from "../api";
import { t } from "../i18n";

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
  /** « S1 · E2 » (sans accent, comme partout). */
  number: string | null;
  summary: string | null;
  durationMs: number | null;
  /** Jaquette de la saison (sinon de la série), mieux composée qu'une image de l'épisode. */
  poster: string | null;
  /** Image de l'épisode : en secours sans jaquette, et en fond flouté. */
  thumb: string | null;
  backdrop: string | null;
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
export function loadUpNext(current: string, next: string) {
  type Meta = {
    title: string;
    showTitle: string | null;
    summary: string | null;
    durationMs: number | null;
    index: number | null;
    parentIndex: number | null;
    thumb: string | null;
    poster: string | null;
    backdrop: string | null;
    seasonKey: string | null;
  };
  const fresh = () => player.ratingKey === current && player.nextKey === next;
  invoke<Meta>("plex_metadata", { ratingKey: next }).then(
    async (m) => {
      if (!fresh()) return;
      const number = m.parentIndex != null && m.index != null ? `S${m.parentIndex} · E${m.index}` : null;
      // L'affiche d'un épisode est celle de la série : la jaquette de la saison se lit sur sa propre fiche.
      let poster = m.poster;
      if (m.seasonKey) {
        try {
          poster = (await invoke<Meta>("plex_metadata", { ratingKey: m.seasonKey })).poster ?? poster;
        } catch {
          // La jaquette de la série fera l'affaire.
        }
      }
      if (!fresh()) return;
      player.upNext = { title: m.title, showTitle: m.showTitle, number, summary: m.summary, durationMs: m.durationMs, poster, thumb: m.thumb, backdrop: m.backdrop ?? m.thumb };
    },
    () => {},
  );
}

/** Écran de fin possible : un épisode qui a une suite. */
export const hasEndScreen = () => player.video && !!player.nextKey && !!player.upNext;

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

export interface NativeInfo {
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

export function trackLabel(track: MpvTrack): string {
  const parts = [track.title, track.lang?.toUpperCase()].filter(Boolean);
  if (track.type === "audio" && track["demux-channel-count"])
    parts.push(t("player.channels", { channels: track["demux-channel-count"] }));
  if (track.forced) parts.push(t("player.forced"));
  return parts.length ? parts.join(" · ") : t("player.trackNumber", { id: track.id });
}
