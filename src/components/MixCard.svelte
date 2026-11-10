<script lang="ts">
  import { errorText } from "../lib/api";
  import { go, toast } from "../lib/nav.svelte";
  import { playMix, removeMix, type Mix } from "../lib/mixes.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n";

  /**
   * Un « Mix of Three » dans la rangée des playlists : les pochettes de ses trois playlists en mosaïque.
   * Comme une playlist, la pochette ouvre sa fiche (ses titres) et ▶ le lance, mélangé ; la croix l'oublie
   * avant ses deux jours.
   */
  let { mix }: { mix: Mix } = $props();

  const names = $derived(mix.playlists.map((p) => p.title).join(" · "));

  function start() {
    playMix(mix).catch((e) => toast(errorText(e), true));
  }

  const open = () => go({ name: "mix", id: mix.id });
</script>

<div class="card" role="group" data-key="mix-{mix.id}">
  <div class="img">
    <div class="mosaic">
      {#each mix.playlists as p, i (p.key)}
        <div class="tile" class:big={i === 0}>{#if p.poster}<img src={p.poster} alt="" loading="lazy" />{/if}</div>
      {/each}
    </div>
    <span class="tag"><Icon name="shuffle" size={13} /> {t("mix.tag")}</span>
    <button class="open" onclick={open} title={names} aria-label={t("mix.openTitle", { names })}></button>
    <button class="play" onclick={start} aria-label={t("mix.playTitle", { names })}><Icon name="play" size={40} filled /></button>
    <button class="forget" onclick={() => removeMix(mix.id)} title={t("mix.forget")} aria-label={t("mix.forget")}><Icon name="x" size={15} /></button>
  </div>
  <button class="meta" onclick={open} tabindex="-1">
    <span class="title">{t("mix.title")}</span>
    <span class="sub">{names}</span>
  </button>
</div>

<style>
  .card {
    min-width: 0;
  }

  .img {
    position: relative;
    aspect-ratio: 1;
    overflow: hidden;
    border-radius: var(--radius);
    background: var(--bg-elev);
    box-shadow: 0 4px 14px #0006;
    transition: transform 0.25s var(--ease), box-shadow 0.25s var(--ease);
  }

  .card:hover .img,
  .card:focus-within .img {
    transform: translateY(-4px);
    box-shadow: 0 0 0 2px var(--accent), 0 14px 28px #000a;
  }

  /* La première playlist en grand à gauche, les deux autres l'une sur l'autre à droite. */
  .mosaic {
    display: grid;
    grid-template-columns: 2fr 1fr;
    grid-template-rows: 1fr 1fr;
    gap: 2px;
    width: 100%;
    height: 100%;
  }

  .tile {
    overflow: hidden;
    background: #ffffff12;
  }

  .tile.big {
    grid-row: 1 / 3;
  }

  .tile img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .tag {
    position: absolute;
    top: 8px;
    left: 8px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: 6px;
    background: var(--accent);
    color: var(--accent-text);
    font-size: 11px;
    font-weight: 800;
  }

  .open {
    position: absolute;
    inset: 0;
    padding: 0;
    border: none;
    background: #0000;
    transition: background 0.2s;
  }

  .card:hover .open {
    background: #0006;
  }

  .play {
    position: absolute;
    top: 50%;
    left: 50%;
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    margin: -24px 0 0 -24px;
    padding: 0;
    border: none;
    background: none;
    color: #fff;
    filter: drop-shadow(0 2px 10px #000c);
    opacity: 0;
    transform: scale(0.85);
    transition:
      opacity 0.2s,
      transform 0.2s var(--ease);
  }

  .card:hover .play,
  .play:focus-visible {
    opacity: 1;
    transform: scale(1);
  }

  .play:hover {
    color: var(--accent);
  }

  .forget {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: #000a;
    color: #fff;
    opacity: 0;
    transition:
      opacity 0.2s,
      background 0.15s;
  }

  .card:hover .forget,
  .forget:focus-visible {
    opacity: 1;
  }

  .forget:hover {
    background: var(--accent);
    color: var(--accent-text);
  }

  .meta {
    display: grid;
    width: 100%;
    margin-top: 10px;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
  }

  .title {
    overflow: hidden;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    overflow: hidden;
    color: var(--text-dim);
    font-size: 13px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
