// Notification de musique d'Android (MediaService.kt) et lecteur MPRIS de Linux : ce qui joue, tenu à jour.

import { invoke } from "@tauri-apps/api/core";
import { untrack } from "svelte";
import { current, player } from "./state.svelte";

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
