<!-- Saison : les épisodes de haut en bas, chacun avec ses informations à droite. La vignette (ou OK) le lance ;
     l'épisode ouvert est mis en avant. -->
<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  import Stars from "../../components/Stars.svelte";
  import { formatDuration, type Item } from "../../lib/api";
  import { nav } from "../../lib/nav.svelte";
  import { device } from "../../lib/device";
  import { menuable } from "../../lib/longpress";
  import { t } from "../../lib/i18n";

  /** `list` : la liste, pour que la fiche y retrouve la vignette d'un épisode (sélection, défilement). */
  let {
    episodes,
    target,
    onplay,
    onmenu,
    list = $bindable(),
  }: {
    episodes: Item[];
    target: string | null;
    onplay: (key: string) => void;
    onmenu: (e: MouseEvent, ep: Item) => void;
    list?: HTMLElement;
  } = $props();

  /** Temps restant d'un épisode commencé. */
  const left = (ep: Item) => (ep.viewOffsetMs && ep.durationMs ? ep.durationMs - ep.viewOffsetMs : null);
</script>

<h2 class="row-title">{t("item.episodes")}</h2>
<div class="episodes" bind:this={list}>
  {#each episodes as ep (ep.ratingKey)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="episode" class:target={ep.ratingKey === target} data-ep={ep.ratingKey} data-nav-row oncontextmenu={(e) => onmenu(e, ep)}>
      <button class="thumb" data-key="ep-{ep.ratingKey}" onclick={() => onplay(ep.ratingKey)} use:menuable={ep} aria-label={t("item.playTitle", { title: ep.title })}>
        {#if ep.thumb}<img src={ep.thumb} alt="" loading="lazy" />{/if}
        <span class="ep-badge">
          {#if ep.watched}<Icon name="check" size={13} />{/if}
          {t("item.episodeShort", { number: ep.index ?? "" })}
        </span>
        <span class="play"><Icon name="play" size={22} filled /></span>
        {#if ep.viewOffsetMs && ep.durationMs}
          <div class="progress"><div style:width="{(ep.viewOffsetMs / ep.durationMs) * 100}%"></div></div>
        {/if}
      </button>
      <div class="ep-body">
        <div class="ep-title">
          <span class="ep-num">{ep.index}</span>
          {ep.title}
          {#if ep.watched}<span class="badge ok">{t("item.watched")}</span>{/if}
        </div>
        <div class="ep-meta">
          {#if ep.durationMs}<span>{formatDuration(ep.durationMs)}</span>{/if}
          {#if left(ep)}<span class="left">{t("item.remaining", { time: formatDuration(left(ep)) })}</span>{/if}
          {#if ep.contentRating}<span class="badge outline">{ep.contentRating}</span>{/if}
          {#if ep.rating}<span class="rating"><Icon name="star" size={13} filled /> {ep.rating.toFixed(1)}</span>{/if}
          {#if nav.serverKind === "plex" && !device.tv}<Stars item={ep} size={14} />{/if}
        </div>
        {#if ep.summary}<p class="ep-summary">{ep.summary}</p>{/if}
      </div>
      {#if !device.tv}
        <button class="btn round ep-more" onclick={(e) => onmenu(e, ep)} title={t("trackGrid.moreActions")} aria-label={t("trackGrid.moreActions")}>
          <Icon name="more" size={18} />
        </button>
      {/if}
    </div>
  {/each}
</div>

<style>
  .rating {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--accent);
    font-weight: 700;
  }

  .episodes {
    display: grid;
    gap: 6px;
  }

  .episode {
    position: relative;
    display: grid;
    grid-template-columns: 280px minmax(0, 1fr) auto;
    align-items: start;
    gap: 20px;
    padding: 10px;
    border: 1px solid transparent;
    border-radius: var(--radius-lg);
    transition: background 0.15s, border-color 0.15s;
  }

  .episode:hover {
    background: #ffffff0a;
  }

  /* L'épisode ouvert (Continuer à regarder…) ou celui à reprendre : mis en avant dans la liste. */
  .episode.target {
    border-color: #ffffff1f;
    background: #ffffff0d;
  }

  /* Télécommande : toute la ligne de l'épisode sélectionné s'éclaire, cadre doré autour de la vignette. */
  :global(html.tv) .episode:focus-within {
    background: #ffffff17;
  }

  .thumb {
    position: relative;
    aspect-ratio: 16 / 9;
    padding: 0;
    overflow: hidden;
    border: none;
    border-radius: var(--radius);
    background: var(--bg-elev);
  }

  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .ep-badge {
    position: absolute;
    top: 6px;
    right: 6px;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 1px 7px;
    border-radius: 6px;
    background: #000000b0;
    color: #fff;
    font-size: 12px;
    font-weight: 700;
  }

  .progress {
    height: 4px;
    background: #000a;
  }

  .progress div {
    height: 100%;
    background: var(--accent);
  }

  .thumb .progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
  }

  /* Souris : ▶ au survol de la vignette. */
  .play {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: #0007;
    color: var(--accent);
    opacity: 0;
    transition: opacity 0.15s;
  }

  :global(html:not(.tv)) .thumb:hover .play {
    opacity: 1;
  }

  .ep-body {
    min-width: 0;
    padding-top: 4px;
  }

  .ep-title {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    font-size: 17px;
    font-weight: 600;
  }

  .ep-num {
    color: var(--text-faint);
  }

  .ep-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    margin-top: 4px;
    color: var(--text-dim);
    font-size: 13px;
  }

  .ep-meta .left {
    color: var(--accent);
    font-weight: 600;
  }

  .ep-summary {
    display: -webkit-box;
    margin: 10px 0 0;
    overflow: hidden;
    color: var(--text-dim);
    line-height: 1.55;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
  }

  /* ⋯ d'un épisode : visible au survol (et sur l'épisode mis en avant). */
  .ep-more {
    width: 36px;
    height: 36px;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .episode:hover .ep-more,
  .episode.target .ep-more,
  .ep-more:focus-visible {
    opacity: 1;
  }

  :global(html.phone) .episode {
    grid-template-columns: 132px minmax(0, 1fr);
    gap: 12px;
    padding: 6px;
  }

  :global(html.phone) .ep-more {
    display: none;
  }

  :global(html.phone) .ep-title {
    font-size: 15px;
  }

  :global(html.phone) .ep-summary {
    -webkit-line-clamp: 2;
    line-clamp: 2;
    font-size: 13px;
  }
</style>
