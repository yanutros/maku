<script lang="ts">
  // Téléchargements de l'appareil (Android) : films, séries et musique, lisibles sans réseau.
  import { onMount } from "svelte";
  import Icon from "../components/Icon.svelte";
  import { errorText, formatDuration } from "../lib/api";
  import { t } from "../lib/i18n";
  import { toast } from "../lib/nav.svelte";
  import {
    filePath,
    formatSize,
    loadOffline,
    offline,
    offlineQueue,
    pauseDownload,
    posterUrl,
    removeDownload,
    resumeDownload,
    type Download,
  } from "../lib/offline.svelte";
  import { playOffline, playOfflineMusic } from "../lib/player.svelte";

  onMount(() => {
    loadOffline().catch((e) => toast(errorText(e), true));
  });

  const movies = $derived(offline.items.filter((d) => d.kind === "movie").sort((a, b) => b.added - a.added));
  // Séries et musique, regroupées par série, album ou playlist.
  function groups(kind: Download["kind"]) {
    const map = new Map<string, Download[]>();
    for (const d of offline.items.filter((x) => x.kind === kind)) {
      const key = d.groupKey ?? d.ratingKey;
      map.set(key, [...(map.get(key) ?? []), d]);
    }
    return [...map.entries()].map(([key, items]) => ({
      key,
      title: items[0].groupTitle ?? items[0].showTitle ?? items[0].title,
      items: items.sort((a, b) => (a.parentIndex ?? 0) - (b.parentIndex ?? 0) || (a.index ?? 0) - (b.index ?? 0)),
    }));
  }
  const shows = $derived(groups("episode"));
  const albums = $derived(groups("track"));
  const used = $derived(offline.items.reduce((s, d) => s + d.done, 0));

  function play(d: Download) {
    playOffline(d, filePath(d));
  }

  function playAlbum(groupKey: string, from?: Download) {
    const q = offlineQueue(groupKey);
    if (!q.tracks.length) return;
    playOfflineMusic(q.tracks, Math.max(0, from ? q.ids.indexOf(from.id) : 0));
  }

  async function act(f: () => Promise<unknown>) {
    try {
      await f();
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  function status(d: Download): string {
    if (d.status === "done") return formatSize(d.size);
    if (d.status === "downloading") return d.size ? t("offline.progress", { done: formatSize(d.done), size: formatSize(d.size) }) : t("offline.starting");
    if (d.status === "queued") return t("offline.queued");
    if (d.status === "paused") return t("offline.paused");
    return d.error ?? t("offline.failed");
  }

  const qualityLabel = (d: Download) => (d.quality === "original" ? t("offline.original") : `${d.quality}p`);
</script>

{#snippet row(d: Download, wide: boolean, onplay: () => void)}
  <div class="row" class:wide>
    <button class="art" disabled={d.status !== "done"} onclick={onplay} aria-label={t("item.playTitle", { title: d.title })}>
      {#if posterUrl(d)}<img src={posterUrl(d)} alt="" />{/if}
      {#if d.status === "done"}<span class="play"><Icon name="play" size={18} filled /></span>{/if}
    </button>
    <div class="text">
      <strong>
        {#if d.kind === "episode"}{t("offline.episodeCode", { season: d.parentIndex ?? 0, episode: d.index ?? 0 })} · {/if}{d.title}
      </strong>
      <span class="muted small">
        {#if d.kind !== "track"}{qualityLabel(d)} · {/if}{#if d.durationMs}{formatDuration(d.durationMs)} · {/if}{status(d)}
      </span>
      {#if d.status === "downloading" && d.size}
        <div class="bar"><div style:width="{Math.min(100, (d.done / d.size) * 100)}%"></div></div>
      {/if}
    </div>
    <div class="actions">
      {#if d.status === "downloading" || d.status === "queued"}
        <button class="btn round" onclick={() => act(() => pauseDownload(d.id))} aria-label={t("offline.pause")} title={t("offline.pause")}><Icon name="pause" size={16} filled /></button>
      {:else if d.status !== "done"}
        <button class="btn round" onclick={() => act(() => resumeDownload(d.id))} aria-label={t("offline.resume")} title={t("offline.resume")}><Icon name="restart" size={16} /></button>
      {/if}
      <button class="btn round" onclick={() => act(() => removeDownload(d.id))} aria-label={t("offline.remove")} title={t("offline.remove")}><Icon name="x" size={16} /></button>
    </div>
  </div>
{/snippet}

<div class="page downloads">
  <h1 class="title">{t("offline.title")}</h1>
  {#if offline.loaded && !offline.items.length}
    <div class="empty">
      <Icon name="download" size={40} />
      <p>{t("offline.empty")}</p>
    </div>
  {:else}
    <p class="muted">{t("offline.used", { size: formatSize(used) })}</p>

    {#if movies.length}
      <h2 class="section">{t("offline.movies")}</h2>
      <div class="list">
        {#each movies as d (d.id)}{@render row(d, false, () => play(d))}{/each}
      </div>
    {/if}

    {#each shows as g (g.key)}
      <h2 class="section">{g.title}</h2>
      <div class="list">
        {#each g.items as d (d.id)}{@render row(d, true, () => play(d))}{/each}
      </div>
    {/each}

    {#each albums as g (g.key)}
      <div class="album-head">
        <h2 class="section">{g.title}</h2>
        <button class="btn primary" disabled={!g.items.some((d) => d.status === "done")} onclick={() => playAlbum(g.key)}>
          <Icon name="play" size={16} filled /> {t("item.play")}
        </button>
      </div>
      <div class="list">
        {#each g.items as d (d.id)}{@render row(d, false, () => playAlbum(g.key, d))}{/each}
      </div>
    {/each}
  {/if}
</div>

<style>
  .title {
    margin: 4px 0 6px;
    font-size: 30px;
  }

  .section {
    margin: 26px 0 10px;
    font-size: 18px;
  }

  .album-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .list {
    display: grid;
    gap: 8px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px;
    border-radius: 14px;
    background: #ffffff06;
  }

  .art {
    position: relative;
    display: grid;
    flex: none;
    place-items: center;
    width: 56px;
    aspect-ratio: 2 / 3;
    overflow: hidden;
    padding: 0;
    border: none;
    border-radius: 8px;
    background: var(--bg-elev);
  }

  .wide .art {
    width: 112px;
    aspect-ratio: 16 / 9;
  }

  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .play {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: #0006;
    color: #fff;
  }

  .text {
    display: grid;
    flex: 1;
    gap: 4px;
    min-width: 0;
  }

  .text strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .bar {
    height: 4px;
    overflow: hidden;
    border-radius: 2px;
    background: #ffffff1a;
  }

  .bar div {
    height: 100%;
    background: var(--accent);
  }

  .actions {
    display: flex;
    gap: 6px;
  }

  .empty {
    display: grid;
    justify-items: center;
    gap: 12px;
    margin-top: 80px;
    color: var(--text-dim);
    text-align: center;
  }

  .empty p {
    max-width: 360px;
  }
</style>
