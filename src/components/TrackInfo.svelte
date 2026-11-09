<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, formatClock, type MediaInfo } from "../lib/api";
  import { nav, toast } from "../lib/nav.svelte";
  import { formatSize } from "../lib/offline.svelte";
  import Icon from "./Icon.svelte";
  import { locale, t } from "../lib/i18n";

  /**
   * « Voir les informations » d'un titre, comme dans Plex : son format et son fichier. Le chemin du
   * fichier sur le serveur n'est montré qu'à l'administrateur.
   */
  const item = nav.infoFor!;
  let info = $state<MediaInfo | null>(null);

  onMount(() => {
    api.mediaInfo(item.ratingKey).then(
      (i) => (info = i),
      (e) => {
        toast(errorText(e), true);
        close();
      },
    );
  });

  function close() {
    nav.infoFor = null;
  }

  const channels = (n: number) => (n === 1 ? t("trackInfo.mono") : n === 2 ? t("trackInfo.stereo") : t("trackInfo.channels", { count: n }));

  const rows = $derived.by(() => {
    if (!info) return [];
    const i = info;
    const r: [string, string][] = [];
    if (item.subtitle) r.push([t("trackInfo.artist"), item.subtitle]);
    if (item.albumTitle) r.push([t("trackInfo.album"), item.albumTitle]);
    if (i.durationMs) r.push([t("trackInfo.duration"), formatClock(i.durationMs / 1000)]);
    if (i.container) r.push([t("trackInfo.container"), i.container.toUpperCase()]);
    if (i.codec) r.push([t("trackInfo.codec"), i.codec.toUpperCase()]);
    if (i.bitrateKbps) r.push([t("trackInfo.bitrate"), `${i.bitrateKbps.toLocaleString(locale)} kb/s`]);
    if (i.channels) r.push([t("trackInfo.audio"), channels(i.channels)]);
    if (i.samplingRate) r.push([t("trackInfo.samplingRate"), `${(i.samplingRate / 1000).toLocaleString(locale)} kHz`]);
    if (i.bitDepth) r.push([t("trackInfo.bitDepth"), `${i.bitDepth} bits`]);
    if (i.sizeBytes) r.push([t("trackInfo.size"), formatSize(i.sizeBytes)]);
    if (i.playCount != null) r.push([t("trackInfo.plays"), String(i.playCount)]);
    if (i.addedAt) r.push([t("trackInfo.added"), new Date(i.addedAt).toLocaleDateString(locale, { dateStyle: "long" })]);
    if (nav.admin && i.file) r.push([t("trackInfo.file"), i.file]);
    return r;
  });
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={close}></div>
<div class="dialog" role="dialog" aria-modal="true" aria-label={t("trackInfo.title")}>
  <div class="top">
    <div class="head">
      {#if item.poster}<img src={item.poster} alt="" />{/if}
      <div>
        <div class="eyebrow">{t("trackInfo.title")}</div>
        <h2>{item.title}</h2>
      </div>
    </div>
    <button class="close" onclick={close} aria-label={t("trackInfo.close")}><Icon name="x" size={20} /></button>
  </div>
  {#if !info}
    <p class="muted">{t("common.c1.loading")}</p>
  {:else}
    <dl>
      {#each rows as [label, value] (label)}
        <dt>{label}</dt>
        <dd class:path={label === t("trackInfo.file")}>{value}</dd>
      {/each}
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
    background: #1c1e23f7;
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
    width: 56px;
    height: 56px;
    border-radius: 8px;
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

  dd.path {
    font-family: ui-monospace, monospace;
    font-size: 12px;
  }
</style>
