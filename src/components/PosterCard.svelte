<script lang="ts">
  import { episodeTitle, errorText, isMusic, type Item } from "../lib/api";
  import { go, nav, toast } from "../lib/nav.svelte";
  import { play, playMusic } from "../lib/player.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n";

  let { item, wide = false }: { item: Item; wide?: boolean } = $props();

  const image = $derived(wide ? (item.thumb ?? item.backdrop ?? item.poster) : item.poster);
  const progress = $derived(
    item.viewOffsetMs && item.durationMs ? Math.min(100, (item.viewOffsetMs / item.durationMs) * 100) : 0,
  );
  const title = $derived(item.kind === "episode" ? (item.showTitle ?? item.title) : item.title);
  const music = $derived(isMusic(item.kind));
  const sub = $derived(
    music
      ? (item.subtitle ?? "")
      : item.kind === "episode"
      ? `${t("posterCard.episodeCode", { season: item.parentIndex ?? 0, episode: item.index ?? 0 })}${wide && episodeTitle(item.title) ? ` — ${episodeTitle(item.title)}` : ""}`
      : item.kind === "season"
        ? (item.showTitle ?? "")
        : item.kind === "collection"
        ? (item.subtitle ?? "")
        : String(item.year ?? ""),
  );

  // Séries et saisons n'ont pas de lecture directe : le clic ouvre leur fiche.
  const canPlay = $derived(music || item.playable);

  // La jaquette ouvre la fiche (un titre seul ouvre son album).
  function open() {
    const key = item.kind === "track" ? (item.albumKey ?? null) : item.ratingKey;
    if (key) go({ name: "item", ratingKey: key });
    else playNow();
  }

  // Menu « ⋮ » (ou clic droit) : lecture, vu / non vu, modification…
  function openMenu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    nav.menu = { item, x: e.clientX, y: e.clientY };
  }

  // Le bouton lecture lance tout de suite, sans passer par la fiche.
  function playNow() {
    const started = music ? playMusic(item.ratingKey) : play(item.ratingKey);
    started.catch((e) => toast(errorText(e), true));
  }
</script>

<div class="card" class:wide class:square={music} class:round={item.kind === "artist"} oncontextmenu={openMenu} role="group" data-key={item.ratingKey}>
  <div class="img">
    {#if image}<img src={image} alt="" loading="lazy" />{/if}
    {#if item.unwatchedCount}
      <span class="corner">{item.unwatchedCount}</span>
    {:else if item.watched}
      <span class="corner seen"><Icon name="check" size={12} /></span>
    {/if}
    {#if progress > 0}<div class="progress"><div style:width="{progress}%"></div></div>{/if}
    <button class="open" onclick={open} title={item.title} aria-label={t("posterCard.openTitle", { title: item.title })}></button>
    {#if canPlay}
      <button class="play" onclick={playNow} aria-label={t("common.c1.playTitle", { title: item.title })}>
        <Icon name="play" size={22} filled />
      </button>
    {/if}
    <button class="more" onclick={openMenu} aria-label={t("common.c1.moreActions")}><Icon name="more" size={18} /></button>
  </div>
  <button class="meta" onclick={open} tabindex="-1">
    <span class="title">{title}</span>
    {#if sub}<span class="sub">{sub}</span>{/if}
  </button>
</div>

<style>
  .card {
    min-width: 0;
  }

  .img {
    position: relative;
    aspect-ratio: 2 / 3;
    overflow: hidden;
    border-radius: var(--radius);
    background: var(--bg-elev);
    box-shadow: 0 4px 14px #0006;
    transition: transform 0.25s var(--ease), box-shadow 0.25s var(--ease);
  }

  .wide .img {
    aspect-ratio: 16 / 9;
  }

  .square .img {
    aspect-ratio: 1;
  }

  .round .img {
    border-radius: 50%;
  }

  .round .title,
  .round .sub {
    text-align: center;
  }

  .card:hover .img,
  .card:focus-within .img {
    transform: translateY(-4px);
    box-shadow: 0 0 0 2px var(--accent), 0 14px 28px #000a;
  }

  img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  /* Toute la jaquette ouvre la fiche ; elle s'assombrit au survol. */
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

  /* Bouton lecture au centre, au-dessus de la zone « ouvrir ». */
  .play {
    position: absolute;
    top: 50%;
    left: 50%;
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    margin: -24px 0 0 -24px;
    padding: 0 0 0 3px;
    border: none;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-text);
    opacity: 0;
    transform: scale(0.85);
    transition:
      opacity 0.2s,
      transform 0.2s var(--ease),
      background 0.15s;
  }

  .card:hover .play,
  .play:focus-visible {
    opacity: 1;
    transform: scale(1);
  }

  .play:hover {
    background: var(--accent-hover);
    transform: scale(1.12) !important;
  }

  .more {
    position: absolute;
    right: 6px;
    bottom: 8px;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
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

  .round .more {
    right: 14%;
    bottom: 10%;
  }

  .card:hover .more,
  .more:focus-visible {
    opacity: 1;
  }

  .more:hover {
    background: var(--accent);
    color: var(--accent-text);
  }

  .meta {
    display: grid;
    width: 100%;
    padding: 0;
    border: none;
    background: none;
    text-align: left;
  }

  .corner {
    position: absolute;
    top: 0;
    right: 0;
    display: grid;
    place-items: center;
    min-width: 24px;
    height: 24px;
    padding: 0 7px;
    border-bottom-left-radius: var(--radius);
    background: var(--accent);
    color: var(--accent-text);
    font-size: 12px;
    font-weight: 700;
  }

  .corner.seen {
    background: #000b;
    color: var(--accent);
  }

  .progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 4px;
    background: #000a;
  }

  .progress div {
    height: 100%;
    background: var(--accent);
  }

  .title,
  .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    margin-top: 10px;
    font-weight: 600;
  }

  .sub {
    color: var(--text-dim);
    font-size: 13px;
  }
</style>
