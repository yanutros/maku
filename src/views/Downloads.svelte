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
  /**
   * Séries et musique, rangées dans un dossier par saison, par album ou par playlist : la page liste les dossiers, et
   * chacun s'ouvre sur son contenu. Sans cela, une playlist de cent titres ou trois saisons donnaient une liste sans fin.
   */
  type Group = { key: string; groupKey: string; title: string; season: number | null; music: boolean; items: Download[] };
  function groups(kind: "episode" | "track"): Group[] {
    const map = new Map<string, Download[]>();
    for (const d of offline.items.filter((x) => x.kind === kind)) {
      const of = d.groupKey ?? d.showTitle ?? d.ratingKey;
      // Une série : un dossier par saison.
      const key = kind === "episode" ? `${of}|s${d.parentIndex ?? 0}` : of;
      map.set(key, [...(map.get(key) ?? []), d]);
    }
    return [...map.entries()]
      .map(([key, items]) => ({
        key,
        groupKey: items[0].groupKey ?? items[0].ratingKey,
        title: (kind === "episode" ? items[0].showTitle : null) ?? items[0].groupTitle ?? items[0].showTitle ?? items[0].title,
        season: kind === "episode" ? (items[0].parentIndex ?? 0) : null,
        music: kind === "track",
        items: items.sort((a, b) => (a.parentIndex ?? 0) - (b.parentIndex ?? 0) || (a.index ?? 0) - (b.index ?? 0)),
      }))
      .sort((a, b) => a.title.localeCompare(b.title) || (a.season ?? 0) - (b.season ?? 0));
  }
  const shows = $derived(groups("episode"));
  const albums = $derived(groups("track"));

  /** Dossiers ouverts (tous fermés à l'arrivée sur la page). */
  let opened = $state<Record<string, boolean>>({});

  /** Sous le nom du dossier : ce qu'il contient et sa place, ou l'avancement tant que tout n'est pas arrivé. */
  function summary(g: Group): string {
    const ready = g.items.filter((d) => d.status === "done").length;
    const count = g.music ? t("music.detail.trackCount", { count: g.items.length }) : t("item.episodeCount", { count: g.items.length });
    if (ready < g.items.length) return `${count} · ${t("offline.groupProgress", { done: ready, total: g.items.length })}`;
    return `${count} · ${formatSize(g.items.reduce((s, d) => s + d.size, 0))}`;
  }

  async function removeGroup(g: Group) {
    for (const d of g.items) await act(() => removeDownload(d.id));
  }
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

    {#if shows.length || albums.length}
      <div class="groups">
        {#each [...shows, ...albums] as g (g.key)}
          {@const open = !!opened[g.key]}
          {@const cover = g.items.map(posterUrl).find((p) => p)}
          <section class="group" class:open data-group={g.key}>
            <div class="group-head">
              <!-- Le dossier : un appui l'ouvre sur son contenu, un autre le referme. -->
              <button class="group-toggle" aria-expanded={open} onclick={() => (opened[g.key] = !open)}>
                <span class="group-art" class:square={g.music}>{#if cover}<img src={cover} alt="" />{/if}</span>
                <span class="text">
                  <strong>{g.season !== null ? `${g.title} · ${t("offline.season", { season: g.season })}` : g.title}</strong>
                  <span class="muted small">{summary(g)}</span>
                </span>
                <Icon name={open ? "chevronUp" : "chevronDown"} size={20} />
              </button>
              {#if g.music}
                <button class="btn round primary" disabled={!g.items.some((d) => d.status === "done")} onclick={() => playAlbum(g.groupKey)} aria-label={t("item.playTitle", { title: g.title })}>
                  <Icon name="play" size={16} filled />
                </button>
              {/if}
            </div>
            {#if open}
              <div class="list">
                {#each g.items as d (d.id)}{@render row(d, !g.music, () => (g.music ? playAlbum(g.groupKey, d) : play(d)))}{/each}
              </div>
              <div class="group-foot">
                <button class="btn" onclick={() => removeGroup(g)}><Icon name="x" size={16} /> {t("offline.removeAll")}</button>
              </div>
            {/if}
          </section>
        {/each}
      </div>
    {/if}
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

  /* Dossiers : une saison, un album ou une playlist, ouverts d'un appui. */
  .groups {
    display: grid;
    gap: 10px;
    margin-top: 26px;
  }

  .group {
    border-radius: 16px;
    background: #ffffff06;
  }

  .group.open {
    padding-bottom: 10px;
  }

  .group-head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-right: 10px;
  }

  .group-toggle {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 14px;
    min-width: 0;
    padding: 10px;
    border: none;
    border-radius: 16px;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .group-art {
    flex: none;
    width: 56px;
    aspect-ratio: 2 / 3;
    overflow: hidden;
    border-radius: 8px;
    background: var(--bg-elev);
  }

  .group-art.square {
    aspect-ratio: 1;
  }

  .group-art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .group .list {
    margin: 4px 10px 0;
  }

  .group-foot {
    display: flex;
    justify-content: flex-end;
    margin: 10px 10px 0;
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
