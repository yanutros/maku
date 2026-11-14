// Chromecast (cast.rs) : appareils du réseau, état de la diffusion, commandes traduites pour l'appareil, et
// événements qu'il renvoie. La bascule de la lecture locale vers l'appareil (castTo) reste dans player.svelte.ts.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { device } from "../device";
import { t } from "../i18n";
import { toast } from "../nav.svelte";
import { player } from "./state.svelte";

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

export async function findCastDevices() {
  if (cast.searching) return;
  cast.searching = true;
  try {
    cast.devices = await invoke<CastDevice[]>("cast_devices");
  } finally {
    cast.searching = false;
  }
}

export async function sendToCast(target: CastDevice, index: number, position: number) {
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

/** Répétition de la file ou du titre, appliquée par l'appareil Chromecast lui-même. */
export function castRepeat() {
  const mode = player.repeatOne ? "REPEAT_SINGLE" : player.repeat ? "REPEAT_ALL" : "REPEAT_OFF";
  invoke("cast_repeat", { mode }).catch(() => {});
}

/** Les commandes au format mpv, traduites pour l'appareil. */
export function castCommand(args: unknown[]) {
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
