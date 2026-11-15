<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { episodeTitle, errorText, formatDuration, type Item } from "../lib/api";
  import { go, toast } from "../lib/nav.svelte";
  import { play } from "../lib/player.svelte";
  import Icon from "./Icon.svelte";
  import TitleLogo from "./TitleLogo.svelte";
  import { t } from "../lib/i18n";

  /**
   * Bandeau qui fait défiler jusqu'à 5 éléments, 5 s chacun (pause au survol).
   * `bleed` : pleine largeur sous l'en-tête (accueil) ; sinon carte arrondie dans la page.
   */
  let {
    items,
    eyebrow = null,
    bleed = false,
  }: { items: Item[]; eyebrow?: ((item: Item) => string | null) | null; bleed?: boolean } = $props();

  const SLIDE_MS = 5000;

  let slide = $state(0);
  let hovering = $state(false);
  /** Incrémenté à chaque relance, pour repartir de zéro la jauge de progression. */
  let cycle = $state(0);
  let timer: ReturnType<typeof setInterval> | undefined;

  const slides = $derived(items.slice(0, 5));
  const featured = $derived(slides[slide] ?? slides[0] ?? null);
  const remaining = $derived(
    featured?.durationMs && featured.viewOffsetMs ? featured.durationMs - featured.viewOffsetMs : null,
  );

  // Relance le compte à rebours à chaque changement manuel ; pause au survol.
  function restart() {
    cycle++;
    clearInterval(timer);
    timer = setInterval(() => {
      if (!hovering && slides.length > 1) slide = (slide + 1) % slides.length;
    }, SLIDE_MS);
  }

  function show(i: number) {
    slide = i;
    restart();
  }

  // Un épisode ou un film se lit directement ; une série ou une saison ouvre sa fiche.
  function start() {
    if (!featured) return;
    if (featured.playable) play(featured.ratingKey).catch((e) => toast(errorText(e), true));
    else go({ name: "item", ratingKey: featured.ratingKey });
  }

  onMount(restart);
  onDestroy(() => clearInterval(timer));
</script>

{#if featured}
  <section
    class="hero"
    class:bleed
    aria-label={t("hero.featured")}
    onmouseenter={() => (hovering = true)}
    onmouseleave={() => {
      hovering = false;
      restart();
    }}
  >
    {#each slides as s, i (s.ratingKey)}
      <div
        class="slide-bg"
        class:visible={i === slide}
        style:background-image={s.backdrop ? `url("${s.backdrop}")` : undefined}
      ></div>
    {/each}
    <div class="shade"></div>
    {#key featured.ratingKey}
      <div class="hero-body">
        {#if eyebrow?.(featured)}<div class="eyebrow">{eyebrow(featured)}</div>{/if}
        <h1><TitleLogo logo={featured.logo} guid={featured.logoGuid} genres={featured.genres} title={featured.showTitle ?? featured.title} /></h1>
        <div class="meta">
          {#if featured.kind === "episode"}
            <span>
              {episodeTitle(featured.title)
                ? t("hero.episodeLine", { season: featured.parentIndex ?? "", episode: featured.index ?? "", title: episodeTitle(featured.title) ?? "" })
                : t("item.seasonEpisode", { season: featured.parentIndex ?? "", episode: featured.index ?? "" })}
            </span>
          {:else if featured.year}
            <span>{featured.year}</span>
          {/if}
          {#if featured.genres.length}<span>{featured.genres.slice(0, 3).join(" · ")}</span>{/if}
          {#if featured.rating}<span class="rating"><Icon name="star" size={14} filled /> {featured.rating.toFixed(1)}</span>{/if}
          {#if remaining}<span>{t("common.c1.remaining", { time: formatDuration(remaining) })}</span>{/if}
        </div>
        {#if featured.summary}<p class="summary">{featured.summary}</p>{/if}
        <div class="actions">
          <button class="btn primary" onclick={start}>
            <Icon name="play" size={18} filled />
            {featured.viewOffsetMs ? t("common.c1.resume") : featured.playable ? t("common.c1.play") : t("hero.view")}
          </button>
          <button class="btn" onclick={() => go({ name: "item", ratingKey: featured!.ratingKey })}>{t("common.c1.moreInfo")}</button>
        </div>
      </div>
    {/key}
    {#if slides.length > 1}
      <div class="dots">
        {#each slides as s, i (s.ratingKey)}
          <button class:active={i === slide} onclick={() => show(i)} aria-label={t("hero.showSlide", { title: s.showTitle ?? s.title })}>
            {#if i === slide}
              {#key cycle + slide * 1000}
                <span class="fill" class:paused={hovering} style:animation-duration="{SLIDE_MS}ms"></span>
              {/key}
            {/if}
          </button>
        {/each}
      </div>
    {/if}
  </section>
{/if}

<style>
  .hero {
    position: relative;
    height: min(52vh, 460px);
    overflow: hidden;
    border-radius: var(--radius-lg);
    box-shadow: 0 20px 50px #0008;
  }

  .hero.bleed {
    height: min(62vh, 560px);
    margin-top: -68px;
    border-radius: 0;
    box-shadow: none;
  }

  /* Les fonds sont empilés et se fondent l'un dans l'autre. */
  .slide-bg {
    position: absolute;
    inset: 0;
    background-size: cover;
    background-position: center 20%;
    opacity: 0;
    transform: scale(1.04);
    transition:
      opacity 0.9s ease,
      transform 6s ease-out;
  }

  .slide-bg.visible {
    opacity: 1;
    transform: scale(1);
  }

  .shade {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, color-mix(in srgb, var(--bg) 95%, transparent) 0%, color-mix(in srgb, var(--bg) 75%, transparent) 32%, color-mix(in srgb, var(--bg) 19%, transparent) 65%, transparent 100%),
      linear-gradient(0deg, color-mix(in srgb, var(--bg) 82%, transparent) 0%, transparent 50%);
  }

  .bleed .shade {
    background:
      linear-gradient(90deg, var(--bg) 0%, color-mix(in srgb, var(--bg) 85%, transparent) 30%, color-mix(in srgb, var(--bg) 25%, transparent) 65%, transparent 100%),
      linear-gradient(0deg, var(--bg) 0%, transparent 45%);
  }

  .hero-body {
    position: absolute;
    left: 40px;
    bottom: 36px;
    max-width: 560px;
    animation: slide-in 0.6s var(--ease);
  }

  @keyframes slide-in {
    from {
      opacity: 0;
      transform: translateX(-16px);
    }
  }

  .eyebrow {
    color: var(--accent);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  h1 {
    margin: 6px 0 8px;
    font-size: 40px;
    line-height: 1.05;
    text-shadow: 0 2px 16px #0008;
  }

  .bleed h1 {
    font-size: 44px;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    color: #fffc;
    font-weight: 500;
  }

  .rating {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--accent);
  }

  .summary {
    display: -webkit-box;
    margin: 12px 0 0;
    overflow: hidden;
    color: #ffffffc0;
    line-height: 1.6;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
  }

  .actions {
    display: flex;
    gap: 10px;
    margin-top: 22px;
  }

  .actions .btn {
    height: 44px;
    padding: 0 22px;
  }

  .dots {
    position: absolute;
    right: 32px;
    bottom: 36px;
    display: flex;
    gap: 8px;
  }

  .dots button {
    position: relative;
    width: 28px;
    height: 4px;
    padding: 0;
    overflow: hidden;
    border: none;
    border-radius: 2px;
    background: #ffffff40;
    transition: width 0.3s var(--ease);
  }

  .dots button.active {
    width: 48px;
  }

  .dots button:hover {
    background: #ffffff70;
  }

  /* Progression jusqu'à l'élément suivant. */
  .fill {
    position: absolute;
    inset: 0;
    background: var(--accent);
    transform-origin: left;
    animation: fill linear forwards;
  }

  .fill.paused {
    animation-play-state: paused;
  }

  @keyframes fill {
    from {
      transform: scaleX(0);
    }
  }

  /* ---------- Téléphone : bandeau à la largeur de l'écran, sous l'en-tête opaque ---------- */

  :global(html.phone) .hero {
    height: min(58vh, 500px);
  }

  :global(html.phone) .hero.bleed {
    margin-top: 0;
  }

  :global(html.phone) .hero-body {
    right: 16px;
    bottom: 44px;
    left: 16px;
    max-width: none;
  }

  :global(html.phone) h1 {
    font-size: 28px;
  }

  :global(html.phone) .summary {
    -webkit-line-clamp: 2;
    line-clamp: 2;
    font-size: 13px;
  }

  :global(html.phone) .actions {
    margin-top: 16px;
  }

  :global(html.phone) .dots {
    right: 16px;
    bottom: 18px;
  }

  /* Logo du titre dans le bandeau. */
  h1 {
    --logo-h: 130px;
    --logo-w: min(440px, 90%);
  }

  :global(html.phone) h1 {
    --logo-h: 84px;
  }
</style>
