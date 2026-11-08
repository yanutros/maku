<script lang="ts" module>
  export interface Panel {
    key: string;
    /** Petite ligne en capitales au-dessus du titre (« NOUVEL ALBUM », « FILMS SORTIS RÉCEMMENT »…). */
    eyebrow: string;
    title: string;
    subtitle?: string | null;
    /** Visuel large ; à défaut, la pochette floutée sert de fond. */
    image?: string | null;
    /** Affiche ou pochette, en vignette dans le coin. */
    cover?: string | null;
    caption?: string | null;
    square?: boolean;
    onclick: () => void;
    onplay?: () => void;
  }
</script>

<script lang="ts">
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n";

  // Grands panneaux de mise en avant, à la manière d'Apple Music : titre au-dessus, visuel en grand
  // format, légende en bas et vignette de l'affiche ou de la pochette dans le coin.
  let { panels }: { panels: Panel[] } = $props();

  // Dès qu'un panneau a un sous-titre, tous gardent sa ligne : les visuels restent alignés.
  const withSubtitle = $derived(panels.some((p) => p.subtitle));

  let scroller = $state<HTMLElement>();
  let atStart = $state(true);
  let atEnd = $state(false);

  function update() {
    if (!scroller) return;
    atStart = scroller.scrollLeft <= 4;
    atEnd = scroller.scrollLeft + scroller.clientWidth >= scroller.scrollWidth - 4;
  }

  function scrollBy(dir: number) {
    scroller?.scrollBy({ left: dir * scroller.clientWidth * 0.9, behavior: "smooth" });
  }

  $effect(() => {
    void panels;
    update();
  });
</script>

{#if panels.length}
  <section>
    {#if !atStart}
      <button class="nav left" onclick={() => scrollBy(-1)} aria-label={t("common.c1.previous")}><Icon name="chevronLeft" size={22} /></button>
    {/if}
    {#if !atEnd}
      <button class="nav right" onclick={() => scrollBy(1)} aria-label={t("common.c1.next")}><Icon name="chevronRight" size={22} /></button>
    {/if}
    <div class="track" bind:this={scroller} onscroll={update}>
      {#each panels as p (p.key)}
        <article class="panel">
          <button class="head" onclick={p.onclick}>
            <span class="eyebrow">{p.eyebrow}</span>
            <span class="title">{p.title}</span>
            {#if withSubtitle}<span class="subtitle">{p.subtitle || " "}</span>{/if}
          </button>
          <div class="frame">
            <button class="art" onclick={p.onclick} aria-label={p.title}>
              {#if p.image}
                <img class="image" src={p.image} alt="" loading="lazy" />
              {:else if p.cover}
                <!-- Sans visuel large : la pochette floutée en fond, et en grand au centre. -->
                <span class="blur" style:background-image={`url("${p.cover}")`}></span>
                <img class="center-cover" class:square={p.square} src={p.cover} alt="" loading="lazy" />
              {/if}
              <span class="shade"></span>
              {#if p.caption}<span class="caption">{p.caption}</span>{/if}
              {#if p.cover && p.image}
                <img class="badge" class:square={p.square} src={p.cover} alt="" loading="lazy" />
              {/if}
            </button>
            {#if p.onplay}
              <button class="play" onclick={p.onplay} aria-label={t("common.c1.playTitle", { title: p.title })}><Icon name="play" size={40} filled /></button>
            {/if}
          </div>
        </article>
      {/each}
    </div>
  </section>
{/if}

<style>
  section {
    position: relative;
    margin-top: 8px;
  }

  /* Flèches posées sur les bords, à hauteur des visuels. */
  .nav {
    position: absolute;
    z-index: 2;
    top: 58%;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    padding: 0;
    border: 1px solid #ffffff2a;
    border-radius: 50%;
    background: #14151899;
    backdrop-filter: blur(12px);
    color: #fff;
    box-shadow: 0 8px 20px #0008;
    opacity: 0;
    transition:
      opacity 0.2s,
      background 0.15s;
  }

  section:hover .nav,
  .nav:focus-visible {
    opacity: 1;
  }

  .nav:hover {
    background: var(--accent);
    color: var(--accent-text);
  }

  .nav.left {
    left: -14px;
  }

  .nav.right {
    right: -14px;
  }

  /* Trois panneaux par ligne sur grand écran, au moins 380 px chacun. */
  .track {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: max(380px, calc((100% - 40px) / 3));
    gap: 20px;
    overflow-x: auto;
    padding: 4px 6px 12px;
    margin: -4px -6px 0;
    scroll-snap-type: x mandatory;
    scroll-padding: 0 6px;
    scrollbar-width: none;
  }

  .track::-webkit-scrollbar {
    display: none;
  }

  .panel {
    min-width: 0;
    scroll-snap-align: start;
  }

  .head {
    display: grid;
    width: 100%;
    margin-bottom: 10px;
    padding: 0;
    border: none;
    background: none;
    text-align: left;
  }

  .eyebrow {
    overflow: hidden;
    color: var(--text-faint);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-overflow: ellipsis;
    text-transform: uppercase;
    white-space: nowrap;
  }

  .title,
  .subtitle {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    margin-top: 2px;
    font-size: 17px;
    font-weight: 600;
  }

  .subtitle {
    color: var(--text-dim);
    font-size: 15px;
  }

  .frame {
    position: relative;
  }

  .art {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9.2;
    padding: 0;
    overflow: hidden;
    border: none;
    border-radius: 14px;
    background: var(--bg-elev);
    box-shadow: 0 14px 34px #0008;
  }

  .image {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.6s var(--ease);
  }

  .panel:hover .image {
    transform: scale(1.04);
  }

  .blur {
    position: absolute;
    inset: -40px;
    /* Safari (iPad, Mac) ne coupe pas un calque flouté selon les coins arrondis de son cadre : il le découpe
       lui-même, au contour exact du cadre (son débord, plus le bord de 1 px s'il y en a un). */
    clip-path: inset(40px round 14px);
    background-size: cover;
    background-position: center;
    filter: blur(40px) saturate(1.6) brightness(0.8);
  }

  .center-cover {
    position: absolute;
    left: 50%;
    top: 50%;
    height: 78%;
    aspect-ratio: 2 / 3;
    object-fit: cover;
    border-radius: 10px;
    box-shadow: 0 18px 40px #000a;
    transform: translate(-50%, -54%);
    transition: transform 0.5s var(--ease);
  }

  .center-cover.square {
    aspect-ratio: 1;
  }

  .panel:hover .center-cover {
    transform: translate(-50%, -56%) scale(1.03);
  }

  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, #000000c0 0%, #00000040 28%, transparent 50%);
  }

  .caption {
    position: absolute;
    left: 16px;
    right: 92px;
    bottom: 14px;
    display: -webkit-box;
    overflow: hidden;
    color: #fff;
    font-size: 13px;
    font-weight: 500;
    line-height: 1.4;
    text-align: left;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .badge {
    position: absolute;
    right: 14px;
    bottom: 14px;
    width: 52px;
    aspect-ratio: 2 / 3;
    object-fit: cover;
    border-radius: 6px;
    box-shadow: 0 6px 16px #000a;
  }

  .badge.square {
    width: 60px;
    aspect-ratio: 1;
  }

  .play {
    position: absolute;
    left: 16px;
    top: 16px;
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    /* Triangle seul, comme dans le lecteur, détaché de l'image par une ombre douce. */
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

  .panel:hover .play,
  .play:focus-visible {
    opacity: 1;
    transform: scale(1);
  }

  .play:hover {
    color: var(--accent);
    transform: scale(1.1) !important;
  }
</style>
