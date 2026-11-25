<script lang="ts">
  // « Détails » d'un volume ou d'un chapitre Kavita, comme dans Kavita : pages, mots (livres), temps de lecture,
  // taille, date d'ajout et fichiers (leur nom seulement). Même allure que « Voir les informations » (TrackInfo).
  import { onMount } from "svelte";
  import { api, errorText, type BookDetails } from "../lib/api";
  import { toast } from "../lib/nav.svelte";
  import { formatSize } from "../lib/offline.svelte";
  import Icon from "./Icon.svelte";
  import { locale, t } from "../lib/i18n";

  let {
    title,
    cover,
    volumeId,
    chapterId,
    onclose,
  }: { title: string; cover: string; volumeId?: number; chapterId?: number; onclose: () => void } = $props();

  let info = $state<BookDetails | null>(null);

  onMount(() => {
    api.kavitaDetails({ volumeId, chapterId }).then(
      (d) => (info = d),
      (e) => {
        toast(errorText(e), true);
        onclose();
      },
    );
  });

  /** Date de Kavita, en heure universelle même sans « Z ». */
  const date = (iso: string) => {
    const d = new Date(/[zZ]|[+-]\d\d:\d\d$/.test(iso) ? iso : `${iso}Z`);
    return Number.isNaN(d.getTime()) ? null : d.toLocaleDateString(locale, { dateStyle: "long" });
  };

  const rows = $derived.by(() => {
    if (!info) return [];
    const r: [string, string][] = [[t("books.details.pages"), t("books.pages", { count: info.pages })]];
    if (info.words > 0) r.push([t("books.details.words"), info.words.toLocaleString(locale)]);
    if (info.hours > 0) r.push([t("books.details.readingTime"), info.hours < 1 ? t("books.details.underHour") : t("books.details.hours", { count: Math.round(info.hours) })]);
    if (info.bytes > 0) r.push([t("books.details.size"), formatSize(info.bytes)]);
    const added = info.added ? date(info.added) : null;
    if (added) r.push([t("books.details.added"), added]);
    return r;
  });
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}></div>
<div class="dialog" role="dialog" aria-modal="true" aria-label={t("books.details.title")}>
  <div class="top">
    <div class="head">
      <img src={cover} alt="" />
      <div>
        <div class="eyebrow">{t("books.details.title")}</div>
        <h2>{title}</h2>
      </div>
    </div>
    <button class="close" onclick={onclose} aria-label={t("trackInfo.close")}><Icon name="x" size={20} /></button>
  </div>
  {#if !info}
    <p class="muted">{t("common.c1.loading")}</p>
  {:else}
    <dl>
      {#each rows as [label, value] (label)}
        <dt>{label}</dt>
        <dd>{value}</dd>
      {/each}
      {#if info.files.length}
        <dt>{t("books.details.files", { count: info.files.length })}</dt>
        <dd class="files">{#each info.files as f, i (i)}<span>{f}</span>{/each}</dd>
      {/if}
    </dl>
  {/if}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 70;
    background: #000a;
  }

  .dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    z-index: 71;
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: min(520px, calc(100vw - 32px));
    max-height: min(700px, calc(100vh - 48px));
    padding: 22px;
    overflow-y: auto;
    border: 1px solid #ffffff1a;
    border-radius: 18px;
    background: color-mix(in srgb, var(--bg-elev) 97%, transparent);
    box-shadow: 0 24px 60px #000c;
    transform: translate(-50%, -50%);
  }

  .top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }

  .head img {
    width: 48px;
    height: 72px;
    border-radius: 6px;
    background: var(--bg);
    object-fit: cover;
  }

  .eyebrow {
    color: var(--accent);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  h2 {
    margin: 2px 0 0;
    font-size: 20px;
    overflow-wrap: anywhere;
  }

  .close {
    display: grid;
    flex: none;
    place-items: center;
    width: 36px;
    height: 36px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--text-dim);
  }

  .close:hover,
  .close:focus-visible {
    background: #ffffff1a;
    color: var(--text);
  }

  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 8px 18px;
    margin: 0;
  }

  dt {
    color: var(--text-dim);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .files {
    display: grid;
    gap: 4px;
    font-family: ui-monospace, monospace;
    font-size: 12px;
  }
</style>
