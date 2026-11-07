<script lang="ts">
  import { seerrStatusLabel, type SeerrMedia } from "../lib/api";
  import { go, nav } from "../lib/nav.svelte";
  import { t } from "../lib/i18n";

  let { media }: { media: SeerrMedia } = $props();

  const label = $derived(seerrStatusLabel(media.status));
  // Une série partiellement disponible peut encore recevoir des demandes de saisons.
  const requestable = $derived(media.status === 0 || media.status === 1 || (media.mediaType === "tv" && media.status === 4));

  function open() {
    go({ name: "seerr", mediaType: media.mediaType, tmdbId: media.tmdbId });
  }
</script>

<div class="card">
  <div class="img">
    {#if media.poster}<img src={media.poster} alt="" loading="lazy" />{/if}
    <span class="type">{media.mediaType === "movie" ? t("common.c1.movie") : t("common.c1.show")}</span>
    {#if label}
      <span class="badge status" class:ok={media.status === 5} class:info={media.status !== 5}>{label}</span>
    {/if}
    <!-- Au survol, comme sur Seerr : année, titre et début du synopsis. -->
    <button class="open" onclick={open} aria-label={t("seerrCard.viewTitle", { title: media.title })}>
      <span class="hover-info" class:pad={requestable}>
        {#if media.year}<span class="year">{media.year}</span>{/if}
        <span class="name">{media.title}</span>
        {#if media.overview}<span class="overview">{media.overview}</span>{/if}
      </span>
    </button>
    {#if requestable}
      <button class="btn primary request" onclick={() => (nav.requesting = media)}>{t("seerrCard.request")}</button>
    {/if}
  </div>
  <button class="meta" onclick={open} tabindex="-1">
    <span class="title">{media.title}</span>
    <span class="sub muted">{media.year ?? ""}</span>
  </button>
</div>

<style>
  .card {
    min-width: 0;
  }

  .img {
    position: relative;
    aspect-ratio: 2 / 3;
    background: var(--bg-elev);
    border-radius: var(--radius);
    overflow: hidden;
    box-shadow: 0 4px 14px #0006;
    transition: transform 0.25s var(--ease), box-shadow 0.25s var(--ease);
  }

  .card:hover .img,
  .card:focus-within .img {
    transform: translateY(-4px);
    box-shadow: 0 0 0 2px var(--accent), 0 14px 28px #000a;
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .type {
    position: absolute;
    top: 6px;
    left: 6px;
    font-size: 11px;
    background: #000b;
    border-radius: 4px;
    padding: 1px 6px;
  }

  .status {
    position: absolute;
    top: 6px;
    right: 6px;
    max-width: calc(100% - 60px);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    backdrop-filter: blur(6px);
  }

  /* Toute la jaquette ouvre la fiche ; au survol, le synopsis apparaît en bas. */
  .open {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: flex-end;
    padding: 0;
    border: none;
    background: linear-gradient(transparent 20%, #000000f0 85%);
    text-align: left;
    opacity: 0;
    transition: opacity 0.2s;
  }

  .card:hover .open,
  .open:focus-visible {
    opacity: 1;
  }

  /* Téléviseur : le bouton qui ouvre la fiche reste « visible » (transparent) : à opacité nulle, la télécommande
     l'ignorait et les rangées de jaquettes Seerr ne se sélectionnaient pas. Le synopsis n'apparaît qu'à la sélection. */
  :global(html.tv) .open {
    opacity: 1;
    background: none;
  }

  :global(html.tv) .open .hover-info {
    visibility: hidden;
  }

  :global(html.tv) .open:focus {
    outline: none;
    background: linear-gradient(transparent 20%, #000000f0 85%);
  }

  :global(html.tv) .open:focus .hover-info {
    visibility: visible;
  }

  .hover-info {
    display: grid;
    gap: 2px;
    width: 100%;
    padding: 12px;
  }

  /* Place pour le bouton « Demander ». */
  .hover-info.pad {
    padding-bottom: 56px;
  }

  .year {
    color: #fffc;
    font-size: 12px;
    font-weight: 600;
  }

  .name {
    font-size: 16px;
    font-weight: 700;
    line-height: 1.2;
  }

  .overview {
    display: -webkit-box;
    margin-top: 4px;
    overflow: hidden;
    color: #ffffffd0;
    font-size: 12px;
    line-height: 1.4;
    -webkit-line-clamp: 5;
    line-clamp: 5;
    -webkit-box-orient: vertical;
  }

  .request {
    position: absolute;
    left: 8px;
    right: 8px;
    bottom: 8px;
    justify-content: center;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .card:hover .request,
  .request:focus-visible {
    opacity: 1;
  }

  .meta {
    display: grid;
    width: 100%;
    padding: 0;
    border: none;
    background: none;
    text-align: left;
  }

  .title,
  .sub {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title {
    margin-top: 10px;
    font-weight: 600;
  }

  .sub {
    font-size: 13px;
  }
</style>
