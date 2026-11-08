<script lang="ts">
  // Écran de fin d'un épisode qui a une suite, comme la « Lecture du suivant » de Plex : la vignette de l'épisode
  // suivant avec le compte à rebours (Réglages → Lecteur), son titre et son résumé, « Lire » ou « Fermer ».
  // Il s'ouvre avec « Passer le générique » ou à la fin de l'épisode (player.svelte.ts).
  import { onDestroy, onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import { errorText } from "../lib/api";
  import { t } from "../lib/i18n";
  import { toast } from "../lib/nav.svelte";
  import { autoNext, play, player, stop } from "../lib/player.svelte";

  const next = player.upNext!;
  const delay = autoNext.delay;
  let left = $state(delay);
  let playEl = $state<HTMLButtonElement>();
  let timer: ReturnType<typeof setInterval> | undefined;

  // Anneau du compte à rebours : r = 44, circonférence ≈ 276,5.
  const CIRCLE = 2 * Math.PI * 44;

  function go() {
    clearInterval(timer);
    if (player.nextKey) play(player.nextKey).catch((e) => toast(errorText(e), true));
  }

  function close() {
    clearInterval(timer);
    stop();
  }

  onMount(() => {
    playEl?.focus();
    if (!delay) return;
    const start = Date.now();
    timer = setInterval(() => {
      left = Math.max(0, delay - (Date.now() - start) / 1000);
      if (left <= 0) go();
    }, 200);
  });
  onDestroy(() => clearInterval(timer));

  const minutes = next.durationMs ? Math.round(next.durationMs / 60000) : null;
</script>

<div class="end" role="dialog" aria-label={t("player.nextEpisode")}>
  {#if next.thumb}<div class="backdrop" style:background-image="url({next.thumb})"></div>{/if}
  <div class="content">
    <div class="left">
      <span class="eyebrow">{t("player.nextEpisode")}</span>
      <button class="thumb" onclick={go} tabindex="-1" aria-label={t("player.playNext")}>
        {#if next.thumb}<img src={next.thumb} alt="" />{/if}
        {#if delay}
          <span class="ring">
            <svg viewBox="0 0 100 100" aria-hidden="true">
              <circle cx="50" cy="50" r="44" class="track" />
              <circle cx="50" cy="50" r="44" class="bar" style:stroke-dashoffset={CIRCLE * (1 - left / delay)} style:stroke-dasharray={CIRCLE} />
            </svg>
            <span class="count">{Math.ceil(left)}</span>
          </span>
        {:else}
          <span class="ring play"><Icon name="play" size={34} filled /></span>
        {/if}
      </button>
    </div>
    <div class="info">
      {#if next.showTitle}<h2>{next.showTitle}</h2>{/if}
      <strong class="episode">{next.number ? `${next.number} — ` : ""}{next.title}</strong>
      {#if minutes}<span class="muted">{t("player.minutes", { count: minutes })}</span>{/if}
      {#if next.summary}<p class="summary">{next.summary}</p>{/if}
      <div class="actions">
        <button class="btn primary" bind:this={playEl} onclick={go}><Icon name="play" size={18} filled /> {t("player.playNext")}</button>
        <button class="btn" onclick={close}>{t("player.closeEnd")}</button>
      </div>
    </div>
  </div>
</div>

<style>
  .end {
    position: absolute;
    inset: 0;
    z-index: 20;
    display: grid;
    place-items: center;
    overflow: hidden;
    background: #0b0c10;
    color: #fff;
  }

  .backdrop {
    position: absolute;
    inset: -40px;
    background-position: center;
    background-size: cover;
    filter: blur(40px) brightness(0.35);
  }

  .content {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 0.9fr) minmax(0, 1.1fr);
    gap: 48px;
    align-items: center;
    width: min(1200px, calc(100% - 96px));
  }

  .left {
    display: grid;
    gap: 14px;
  }

  .eyebrow {
    color: #ffffffb3;
    font-size: 14px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .thumb {
    position: relative;
    display: grid;
    place-items: center;
    aspect-ratio: 16 / 9;
    overflow: hidden;
    padding: 0;
    border: none;
    border-radius: 16px;
    background: #ffffff14;
    cursor: pointer;
  }

  .thumb img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .ring {
    position: relative;
    display: grid;
    place-items: center;
    width: 96px;
    height: 96px;
    border-radius: 50%;
    background: #000000a6;
    color: #fff;
  }

  .ring svg {
    position: absolute;
    inset: 0;
    transform: rotate(-90deg);
  }

  .ring circle {
    fill: none;
    stroke-width: 6;
  }

  .track {
    stroke: #ffffff33;
  }

  .bar {
    stroke: var(--accent);
    stroke-linecap: round;
    transition: stroke-dashoffset 0.2s linear;
  }

  .count {
    position: relative;
    font-size: 32px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .info {
    display: grid;
    gap: 10px;
    min-width: 0;
  }

  h2 {
    margin: 0;
    font-size: 30px;
  }

  .episode {
    font-size: 19px;
  }

  .muted {
    color: #ffffffa6;
  }

  .summary {
    display: -webkit-box;
    margin: 6px 0 0;
    overflow: hidden;
    color: #ffffffd9;
    line-height: 1.5;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 4;
    line-clamp: 4;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 12px;
  }

  .actions .btn {
    height: 46px;
    padding: 0 22px;
    font-size: 15px;
  }

  .actions .btn:not(.primary) {
    border-color: #ffffff40;
    background: #ffffff14;
    color: #fff;
  }

  :global(html.tv) .actions .btn:not(.primary):focus {
    background: #fff;
    color: #111;
  }

  /* Téléphone : la vignette au-dessus, le texte dessous. */
  @media (max-width: 760px) {
    .content {
      grid-template-columns: minmax(0, 1fr);
      gap: 18px;
      width: calc(100% - 32px);
    }

    h2 {
      font-size: 22px;
    }

    .summary {
      -webkit-line-clamp: 2;
      line-clamp: 2;
    }

    .ring {
      width: 72px;
      height: 72px;
    }

    .count {
      font-size: 24px;
    }
  }
</style>
