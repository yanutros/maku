<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, formatClock, formatDuration, type Item, type ItemDetail } from "../lib/api";
  import { go, nav, toast } from "../lib/nav.svelte";
  import { current, player, playMusic } from "../lib/player.svelte";
  import PosterCard from "../components/PosterCard.svelte";
  import Icon from "../components/Icon.svelte";
  import { t } from "../lib/i18n";

  let { item }: { item: ItemDetail } = $props();

  let albums = $state<Item[]>([]);
  let tracks = $state<Item[] | null>(null);

  const isArtist = $derived(item.kind === "artist");
  const label: Record<string, string> = {
    artist: t("music.detail.kind.artist"),
    album: t("music.detail.kind.album"),
    playlist: t("music.detail.kind.playlist"),
    track: t("music.detail.kind.track"),
  };
  const totalMs = $derived(tracks?.reduce((sum, tr) => sum + (tr.durationMs ?? 0), 0) ?? 0);
  // Une playlist ou une compilation mélange les artistes : on les affiche sur chaque ligne.
  const showArtists = $derived(
    item.kind === "playlist" || (tracks ?? []).some((tr) => tr.subtitle && tr.subtitle !== item.showTitle),
  );
  const playingKey = $derived(current()?.ratingKey);

  function start(opts: { start?: string; shuffle?: boolean } = {}) {
    playMusic(item.ratingKey, opts).catch((e) => toast(errorText(e), true));
  }

  onMount(() => {
    if (item.kind === "artist") {
      api.children(item.ratingKey).then((a) => (albums = a), (e) => toast(errorText(e), true));
    } else {
      api.tracks(item.ratingKey).then((list) => (tracks = list), (e) => toast(errorText(e), true));
    }
  });
</script>

<div class="backdrop" style:background-image={item.poster ? `url("${item.poster}")` : undefined}></div>

<div class="page detail">
  <div class="head">
    <div class="cover" class:round={isArtist}>
      {#if item.poster}<img src={item.poster} alt="" />{:else}<Icon name="music" size={64} />{/if}
    </div>
    <div class="info">
      <div class="eyebrow">{label[item.kind] ?? ""}</div>
      <h1>{item.title}</h1>
      {#if item.showKey && item.showTitle}
        <button class="artist" onclick={() => go({ name: "item", ratingKey: item.showKey! })}>{item.showTitle}</button>
      {/if}
      <div class="meta">
        {#if item.year}<span>{item.year}</span>{/if}
        {#if isArtist && albums.length}<span>{t("music.detail.albumCount", { count: albums.length })}</span>{/if}
        {#if tracks?.length}<span>{t("music.detail.trackCount", { count: tracks.length })}</span>{/if}
        {#if totalMs}<span>{formatDuration(totalMs)}</span>{/if}
        {#each item.genres.slice(0, 3) as g}<span class="genre">{g}</span>{/each}
      </div>
      <div class="actions">
        <button class="btn primary big" onclick={() => start()}>
          <Icon name="play" size={18} filled /> {t("music.detail.play")}
        </button>
        {#if item.kind !== "track"}
          <button class="btn big" onclick={() => start({ shuffle: true })}>
            <Icon name="shuffle" size={18} /> {t("music.detail.shuffle")}
          </button>
        {/if}
        {#if nav.admin && item.kind !== "playlist"}
          <button class="btn big" onclick={() => (nav.editing = item)} title={t("music.detail.editInfo")}>
            <Icon name="edit" size={18} /> {t("music.detail.edit")}
          </button>
        {/if}
      </div>
      {#if item.summary}<p class="summary">{item.summary}</p>{/if}
    </div>
  </div>

  {#if isArtist}
    {#if albums.length}
      <h2 class="row-title">{t("music.detail.albums")}</h2>
      <div class="grid">
        {#each albums as album (album.ratingKey)}<PosterCard item={album} />{/each}
      </div>
    {/if}
  {:else if !tracks}
    <div class="skeleton" style:height="240px"></div>
  {:else}
    <div class="tracks">
      <div class="track header" class:artists={showArtists}>
        <span class="num">#</span>
        <span>{t("music.detail.columns.title")}</span>
        {#if showArtists}<span>{t("music.detail.columns.album")}</span>{/if}
        <span class="dur"><Icon name="clock" size={16} /></span>
      </div>
      {#each tracks as tr, i (tr.ratingKey)}
        <button
          class="track"
          class:artists={showArtists}
          class:playing={playingKey === tr.ratingKey}
          onclick={() => start({ start: tr.ratingKey })}
        >
          <span class="num">
            {#if playingKey === tr.ratingKey && !player.paused}
              <span class="eq"><i></i><i></i><i></i></span>
            {:else}
              <span class="n">{item.kind === "album" ? (tr.index ?? i + 1) : i + 1}</span>
              <span class="p"><Icon name="play" size={14} filled /></span>
            {/if}
          </span>
          <span class="name">
            {#if item.kind === "playlist" && tr.poster}<img src={tr.poster} alt="" loading="lazy" />{/if}
            <span class="text">
              <span class="t">{tr.title}</span>
              {#if showArtists && tr.subtitle}<span class="a">{tr.subtitle}</span>{/if}
            </span>
          </span>
          {#if showArtists}<span class="album">{tr.albumTitle ?? ""}</span>{/if}
          <span class="dur">{tr.durationMs ? formatClock(tr.durationMs / 1000) : ""}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0 0 0 var(--sidebar-w, 237px);
    z-index: 0;
    background-size: cover;
    background-position: center;
    filter: blur(60px) saturate(1.4);
    opacity: 0.35;
    mask-image: linear-gradient(#000 0%, transparent 70%);
    pointer-events: none;
  }

  .detail {
    position: relative;
    z-index: 1;
    padding-top: 24px;
  }

  .head {
    display: flex;
    gap: 36px;
    align-items: flex-end;
    margin-bottom: 32px;
  }

  .cover {
    display: grid;
    place-items: center;
    flex: none;
    width: 240px;
    aspect-ratio: 1;
    overflow: hidden;
    border-radius: var(--radius-lg);
    background: var(--bg-elev);
    color: var(--text-faint);
    box-shadow: 0 20px 50px #000c;
  }

  .cover.round {
    border-radius: 50%;
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .info {
    min-width: 0;
    max-width: 820px;
  }

  .eyebrow {
    color: var(--accent);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  h1 {
    margin: 4px 0 2px;
    font-size: 42px;
    line-height: 1.1;
  }

  .artist {
    padding: 0;
    border: none;
    background: none;
    font-size: 18px;
    font-weight: 600;
  }

  .artist:hover {
    color: var(--accent);
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    margin-top: 10px;
    color: var(--text-dim);
  }

  .genre {
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 12px;
    font-size: 13px;
  }

  .actions {
    display: flex;
    gap: 10px;
    margin-top: 20px;
  }

  .big {
    height: 44px;
    padding: 0 22px;
  }

  .summary {
    display: -webkit-box;
    margin: 18px 0 0;
    overflow: hidden;
    color: var(--text-dim);
    line-height: 1.6;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
  }

  .tracks {
    display: grid;
  }

  .track {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) 64px;
    align-items: center;
    gap: 12px;
    min-height: 52px;
    padding: 4px 12px;
    border: none;
    border-radius: var(--radius);
    background: none;
    text-align: left;
    color: var(--text);
  }

  .track.artists {
    grid-template-columns: 44px minmax(0, 3fr) minmax(0, 2fr) 64px;
  }

  .track.header {
    min-height: 36px;
    margin-bottom: 4px;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
    color: var(--text-faint);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  /* Télécommande : des lignes un peu écartées, et le cadre dessiné à l'intérieur de la ligne sélectionnée,
     pour ne jamais recouvrir la jaquette ou le texte des lignes voisines. */
  :global(html.tv) .tracks {
    gap: 6px;
  }

  :global(html.tv) button.track:focus {
    outline-offset: -3px;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  button.track:hover {
    background: #ffffff0f;
  }

  .track.playing .t {
    color: var(--accent);
  }

  .num {
    display: grid;
    place-items: center;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }

  .num .p {
    display: none;
  }

  button.track:hover .num .n {
    display: none;
  }

  button.track:hover .num .p {
    display: block;
    color: var(--text);
  }

  .name {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }

  .name img {
    width: 40px;
    height: 40px;
    border-radius: 4px;
    object-fit: cover;
  }

  .text {
    display: grid;
    min-width: 0;
  }

  .t,
  .a,
  .album {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .t {
    font-weight: 500;
  }

  .a,
  .album {
    color: var(--text-dim);
    font-size: 13px;
  }

  .dur {
    display: flex;
    justify-content: flex-end;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }

  /* Petit égaliseur animé sur le titre en cours. */
  .eq {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 14px;
  }

  .eq i {
    width: 3px;
    background: var(--accent);
    animation: eq 0.9s ease-in-out infinite;
  }

  .eq i:nth-child(2) {
    animation-delay: -0.3s;
  }

  .eq i:nth-child(3) {
    animation-delay: -0.6s;
  }

  @keyframes eq {
    0%,
    100% {
      height: 30%;
    }
    50% {
      height: 100%;
    }
  }

  /* ---------- Téléphone : pochette au-dessus, boutons en pleine largeur ---------- */

  :global(html.phone) .head {
    flex-direction: column;
    align-items: stretch;
    gap: 18px;
    margin-bottom: 20px;
  }

  :global(html.phone) .cover {
    align-self: center;
    width: 62%;
  }

  :global(html.phone) h1 {
    font-size: 26px;
  }

  :global(html.phone) .actions {
    flex-wrap: wrap;
  }

  :global(html.phone) .actions > :global(.btn) {
    flex: 1 1 auto;
  }
</style>
