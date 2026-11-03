<script lang="ts">
  import { current, player } from "../lib/player.svelte";
  import Icon from "./Icon.svelte";

  /**
   * Téléviseur : rappel discret du titre en cours pendant qu'on parcourt les pages (la barre musique de
   * l'ordinateur, faite pour la souris, n'a pas sa place ici). Rien à sélectionner : le lecteur s'ouvre
   * par l'entrée « En cours de lecture » du menu, et les touches multimédia de la télécommande agissent.
   */
  const track = $derived(current());
  const progress = $derived(player.duration ? Math.min(100, (player.position / player.duration) * 100) : 0);
</script>

{#if track}
  <div class="tv-mini" aria-hidden="true">
    <span class="cover">
      {#if track.cover}<img src={track.cover} alt="" />{/if}
      <span class="state"><Icon name={player.paused ? "pause" : "play"} size={16} filled /></span>
    </span>
    <span class="text">
      <span class="title">{track.title}</span>
      <span class="sub">{track.artist ?? ""}</span>
    </span>
    <span class="progress"><span style:width="{progress}%"></span></span>
  </div>
{/if}

<style>
  .tv-mini {
    position: fixed;
    right: 24px;
    bottom: 20px;
    z-index: 20;
    display: flex;
    align-items: center;
    gap: 12px;
    width: 300px;
    padding: 8px 14px 10px 8px;
    overflow: hidden;
    border: 1px solid #ffffff14;
    border-radius: 14px;
    background: #16171ce6;
    box-shadow: 0 10px 30px #000a;
    pointer-events: none;
  }

  .cover {
    position: relative;
    flex: none;
    width: 48px;
    height: 48px;
    overflow: hidden;
    border-radius: 8px;
    background: #ffffff14;
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .state {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: #0007;
    color: #fff;
  }

  .text {
    display: grid;
    min-width: 0;
  }

  .title,
  .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    font-size: 15px;
    font-weight: 600;
  }

  .sub {
    color: var(--text-dim);
    font-size: 13px;
  }

  .progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    background: #ffffff1f;
  }

  .progress span {
    display: block;
    height: 100%;
    background: var(--accent);
  }
</style>
