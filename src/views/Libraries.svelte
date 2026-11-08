<script lang="ts">
  import { showDownloads } from "../lib/offline.svelte";
  import type { Section } from "../lib/api";
  import { go } from "../lib/nav.svelte";
  import { ordered, sidebar } from "../lib/sidebar.svelte";
  import { t } from "../lib/i18n";
  import Icon, { type IconName } from "../components/Icon.svelte";

  /** Onglet « Bibliothèques » du téléphone : les bibliothèques du serveur, dans l'ordre du menu. */
  let { sections }: { sections: Section[] } = $props();

  const list = $derived(ordered(sections).filter((s) => !sidebar.hidden.includes(s.key)));

  function icon(kind: string): IconName {
    return kind === "movie" ? "film" : kind === "artist" ? "music" : "tv";
  }
</script>

<div class="page">
  <h1 class="row-title page-title">{t("phone.libraries")}</h1>
  {#if !list.length}
    <p class="muted">{t("phone.noLibraries")}</p>
  {/if}
  <div class="list">
    <!-- Téléphone Android : les téléchargements, lisibles hors ligne. -->
    {#if showDownloads()}
      <button class="library" onclick={() => go({ name: "downloads" })}>
        <span class="icon"><Icon name="download" size={22} /></span>
        <span class="name">{t("offline.title")}</span>
        <Icon name="chevronRight" size={18} />
      </button>
    {/if}
    {#each list as s (s.key)}
      <button class="library" onclick={() => go({ name: "section", key: s.key, title: s.title, kind: s.kind })}>
        <span class="icon"><Icon name={icon(s.kind)} size={22} /></span>
        <span class="name">{s.title}</span>
        <Icon name="chevronRight" size={18} />
      </button>
    {/each}
  </div>
</div>

<style>
  .list {
    display: grid;
    gap: 8px;
  }

  .library {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-elev);
    color: var(--text-dim);
    text-align: left;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    border-radius: 12px;
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    color: var(--accent);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    color: var(--text);
    font-size: 16px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
