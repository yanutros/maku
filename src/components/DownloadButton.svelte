<script lang="ts">
  // Bouton « Télécharger » des fiches (Android) : un film ou un épisode (qualité au choix), une saison entière,
  // un album ou une playlist. Il montre ensuite l'avancement, puis « Téléchargé ».
  import type { Item } from "../lib/api";
  import { t } from "../lib/i18n";
  import { toast } from "../lib/nav.svelte";
  import { errorText } from "../lib/api";
  import {
    downloadEpisodes,
    downloadItem,
    downloadMusic,
    offline,
    pauseDownload,
    removeDownload,
    resumeDownload,
    type Quality,
  } from "../lib/offline.svelte";
  import Icon from "./Icon.svelte";

  let { item, episodes = null, music = false }: { item: Item; episodes?: Item[] | null; music?: boolean } = $props();

  let open = $state(false);
  let busy = $state(false);

  // Téléchargements concernés : le titre lui-même, ou tous ceux de la saison, de l'album, de la playlist.
  const mine = $derived(
    episodes
      ? offline.items.filter((d) => episodes!.some((e) => e.ratingKey === d.ratingKey))
      : music
        ? offline.items.filter((d) => d.groupKey === item.ratingKey)
        : offline.items.filter((d) => d.ratingKey === item.ratingKey),
  );
  const total = $derived(episodes ? episodes.length : music ? Math.max(mine.length, 1) : 1);
  const done = $derived(mine.filter((d) => d.status === "done").length);
  const active = $derived(mine.filter((d) => d.status === "queued" || d.status === "downloading"));
  const failed = $derived(mine.find((d) => d.status === "failed"));
  const paused = $derived(mine.some((d) => d.status === "paused"));
  const percent = $derived.by(() => {
    const size = mine.reduce((s, d) => s + d.size, 0);
    return size ? Math.min(99, Math.floor((mine.reduce((s, d) => s + d.done, 0) / size) * 100)) : 0;
  });
  const phase = $derived(
    mine.length === 0 ? "none" : done === mine.length && mine.length >= total ? "done" : active.length ? "active" : failed ? "failed" : paused ? "paused" : "partial",
  );

  async function run(f: () => Promise<unknown>) {
    open = false;
    busy = true;
    try {
      await f();
    } catch (e) {
      toast(errorText(e), true);
    } finally {
      busy = false;
    }
  }

  function start(quality: Quality) {
    run(async () => {
      if (music) await downloadMusic(item);
      else if (episodes) await downloadEpisodes(episodes, quality);
      else await downloadItem(item, quality);
      toast(t("offline.started"));
    });
  }

  const pauseAll = () => run(() => Promise.all(active.map((d) => pauseDownload(d.id))));
  const resumeAll = () => run(() => Promise.all(mine.filter((d) => d.status !== "done").map((d) => resumeDownload(d.id))));
  const removeAll = () => run(() => Promise.all(mine.map((d) => removeDownload(d.id))));

  function click() {
    // La musique n'a qu'une qualité, celle d'origine : rien à choisir.
    if (phase === "none" && music) return start("original");
    open = !open;
  }

  const label = $derived(
    phase === "done"
      ? t("offline.downloaded")
      : phase === "active"
        ? t("offline.downloading", { percent })
        : phase === "paused" || phase === "partial"
          ? t("offline.resume")
          : phase === "failed"
            ? t("offline.failed")
            : episodes
              ? t("offline.downloadSeason")
              : t("offline.download"),
  );
</script>

<div class="download">
  <button class="btn big" class:on={phase === "done"} disabled={busy} onclick={click} aria-expanded={open}>
    <Icon name={phase === "done" ? "checkCircle" : "download"} size={18} />
    {label}
    {#if episodes && phase !== "none" && phase !== "done"}<span class="count">{done}/{total}</span>{/if}
  </button>
  {#if open}
    <!-- Choix sous le bouton ; un appui ailleurs le referme. -->
    <button class="scrim" aria-label={t("common.c1.close")} onclick={() => (open = false)}></button>
    <div class="menu" role="menu">
      {#if phase === "none"}
        <p class="hint">{t("offline.qualityHint")}</p>
        <button role="menuitem" onclick={() => start("original")}><strong>{t("offline.original")}</strong><span>{t("offline.originalHint")}</span></button>
        <button role="menuitem" onclick={() => start("1080")}><strong>1080p</strong><span>{t("offline.convertedHint", { size: "≈ 3,5 Go / h" })}</span></button>
        <button role="menuitem" onclick={() => start("720")}><strong>720p</strong><span>{t("offline.convertedHint", { size: "≈ 1,8 Go / h" })}</span></button>
      {:else}
        {#if failed?.error}<p class="hint error">{failed.error}</p>{/if}
        {#if phase === "active"}
          <button role="menuitem" onclick={pauseAll}><strong>{t("offline.pause")}</strong></button>
        {:else if phase !== "done"}
          <button role="menuitem" onclick={resumeAll}><strong>{t("offline.resume")}</strong></button>
        {/if}
        <button role="menuitem" class="danger" onclick={removeAll}><strong>{t("offline.remove")}</strong></button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .download {
    position: relative;
  }

  .btn.on :global(svg) {
    color: var(--ok);
  }

  .count {
    color: var(--text-dim);
    font-size: 13px;
  }

  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    border: none;
    background: transparent;
  }

  .menu {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    z-index: 41;
    display: grid;
    gap: 2px;
    width: max-content;
    min-width: 240px;
    max-width: min(320px, 86vw);
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--bg-elev);
    box-shadow: 0 16px 40px #000a;
  }

  .menu button {
    display: grid;
    gap: 2px;
    padding: 10px 12px;
    border: none;
    border-radius: 10px;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .menu button:hover,
  .menu button:focus-visible {
    background: #ffffff10;
  }

  .menu span {
    color: var(--text-dim);
    font-size: 12px;
  }

  .menu .danger strong {
    color: var(--danger);
  }

  .hint {
    margin: 4px 12px 6px;
    color: var(--text-dim);
    font-size: 12px;
  }

  .hint.error {
    color: var(--danger);
  }
</style>
