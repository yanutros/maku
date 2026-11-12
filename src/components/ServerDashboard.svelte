<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, errorText, formatClock, type Section, type ServerSession } from "../lib/api";
  import { toast } from "../lib/nav.svelte";
  import Icon from "./Icon.svelte";
  import ServerStats from "./ServerStats.svelte";
  import { t } from "../lib/i18n";

  // Tableau de bord du serveur : lectures en cours de tous les utilisateurs et entretien des bibliothèques.
  let sessions = $state<ServerSession[] | null>(null);
  let sections = $state<Section[]>([]);
  let error = $state<string | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;

  async function load() {
    try {
      sessions = await api.sessions();
      error = null;
    } catch (e) {
      error = errorText(e);
    }
  }

  async function act(action: () => Promise<void>, done: string) {
    try {
      await action();
      toast(done);
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  // « Arrêter la lecture », comme dans Plex : un message s'affiche chez celui qui regarde (Plex Pass).
  let stopping = $state<ServerSession | null>(null);
  let reason = $state("");
  let stopBusy = $state(false);

  async function stopSession() {
    const s = stopping;
    if (!s) return;
    stopBusy = true;
    try {
      await api.stopSession(s.sessionId, reason.trim());
      toast(t("dashboard.stopped", { user: s.user }));
      stopping = null;
      reason = "";
      setTimeout(load, 1500);
    } catch (e) {
      toast(errorText(e), true);
    } finally {
      stopBusy = false;
    }
  }

  onMount(() => {
    load();
    api.sections().then((s) => (sections = s), () => {});
    timer = setInterval(load, 10000);
  });

  onDestroy(() => clearInterval(timer));
</script>

<h3 class="sub">{t("dashboard.nowPlaying")}</h3>
{#if error}
  <p class="error">{error}</p>
{:else if !sessions}
  <div class="skeleton" style:height="120px"></div>
{:else if !sessions.length}
  <p class="muted">{t("dashboard.nobody")}</p>
{:else}
  <div class="sessions">
    {#each sessions as s, i (i)}
      <div class="session">
        <div class="poster">{#if s.poster}<img src={s.poster} alt="" />{/if}</div>
        <div class="body">
          <div class="who">
            {#if s.userThumb}<img class="avatar" src={s.userThumb} alt="" />{/if}
            <span>{s.user}</span>
            <span class="state" class:paused={s.state === "paused"}>{s.state === "paused" ? t("dashboard.paused") : s.state === "buffering" ? t("dashboard.buffering") : t("dashboard.playing")}</span>
          </div>
          <div class="title">{s.title}</div>
          {#if s.subtitle}<div class="muted small">{s.subtitle}</div>{/if}
          <div class="bar"><div style:width="{s.durationMs ? (s.progressMs / s.durationMs) * 100 : 0}%"></div></div>
          <div class="meta">
            <span>{formatClock(s.progressMs / 1000)} / {formatClock(s.durationMs / 1000)}</span>
            <span>
              {s.player} · {s.local ? t("dashboard.local") : t("dashboard.remote")} · {s.transcode ? t("dashboard.transcode") : t("dashboard.directPlay")}
              {#if s.bandwidthKbps}· {t("dashboard.bandwidth", { rate: (s.bandwidthKbps / 1000).toFixed(1) })}{/if}
            </span>
          </div>
          {#if s.sessionId}
            <button class="btn stop" onclick={() => ((stopping = s), (reason = ""))}><Icon name="x" size={15} /> {t("dashboard.stop")}</button>
          {/if}
        </div>
      </div>
    {/each}
  </div>
{/if}

<ServerStats />

<h3 class="sub">{t("dashboard.libraries")}</h3>
<div class="libs">
  {#each sections as s (s.key)}
    <div class="lib">
      <Icon name={s.kind === "movie" ? "film" : s.kind === "artist" ? "music" : "tv"} />
      <span class="lib-name">{s.title}</span>
      <button class="btn" onclick={() => act(() => api.scanSection(s.key), t("dashboard.scanStarted", { title: s.title }))}>
        <Icon name="search" size={16} /> {t("dashboard.scan")}
      </button>
      <button class="btn" onclick={() => act(() => api.emptyTrash(s.key), t("dashboard.trashEmptied", { title: s.title }))} title={t("dashboard.emptyTrashHint")}>
        <Icon name="x" size={16} /> {t("dashboard.emptyTrash")}
      </button>
    </div>
  {/each}
</div>

{#if stopping}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="stop-backdrop" onclick={() => (stopping = null)}></div>
  <div class="stop-dialog" role="dialog" aria-modal="true" aria-label={t("dashboard.stopTitle", { title: stopping.title })}>
    <h3>{t("dashboard.stopTitle", { title: stopping.title })}</h3>
    <div class="stop-who">
      {#if stopping.userThumb}<img class="avatar big" src={stopping.userThumb} alt="" />{/if}
      <p>{t("dashboard.stopQuestion", { user: stopping.user })}</p>
    </div>
    <label class="stop-label" for="stop-reason">{t("dashboard.stopMessage", { user: stopping.user })}</label>
    <textarea id="stop-reason" bind:value={reason} rows="3" maxlength="300" placeholder={t("dashboard.stopPlaceholder")}></textarea>
    <div class="stop-actions">
      <button class="btn" onclick={() => (stopping = null)}>{t("common.c1.cancel")}</button>
      <button class="btn primary" disabled={stopBusy} onclick={stopSession}>{t("dashboard.stopConfirm")}</button>
    </div>
  </div>
{/if}

<style>
  .btn.stop {
    justify-self: start;
    margin-top: 8px;
    background: #c0392b33;
    color: #ff8a80;
  }

  .btn.stop:hover {
    background: #c0392b;
    color: #fff;
  }

  .stop-backdrop {
    position: fixed;
    inset: 0;
    z-index: 70;
    background: #000a;
  }

  .stop-dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    z-index: 71;
    display: grid;
    gap: 12px;
    width: min(480px, calc(100vw - 32px));
    padding: 22px;
    border: 1px solid #ffffff1a;
    border-radius: 18px;
    background: #1c1e23f7;
    box-shadow: 0 24px 60px #000c;
    transform: translate(-50%, -50%);
  }

  .stop-dialog h3 {
    margin: 0;
    font-size: 19px;
  }

  .stop-who {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .stop-who p {
    margin: 0;
  }

  .avatar.big {
    width: 56px;
    height: 56px;
  }

  .stop-label {
    color: var(--text-dim);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  textarea {
    resize: vertical;
    padding: 10px 12px;
    font: inherit;
  }

  .stop-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .sub {
    margin: 8px 0 14px;
    font-size: 17px;
  }

  .sub:not(:first-child) {
    margin-top: 32px;
  }

  .small {
    font-size: 13px;
  }

  .sessions {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(420px, 1fr));
    gap: 14px;
  }

  .session {
    display: flex;
    gap: 16px;
    padding: 14px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: #ffffff06;
  }

  .poster {
    flex: none;
    width: 64px;
    aspect-ratio: 2 / 3;
    overflow: hidden;
    border-radius: 8px;
    background: var(--bg-hover);
  }

  .poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .body {
    flex: 1;
    min-width: 0;
  }

  .who {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-dim);
    font-size: 13px;
  }

  .avatar {
    width: 20px;
    height: 20px;
    border-radius: 50%;
  }

  .state {
    margin-left: auto;
    padding: 2px 8px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--ok) 18%, transparent);
    color: var(--ok);
    font-size: 11px;
    font-weight: 700;
  }

  .state.paused {
    background: #ffffff14;
    color: var(--text-dim);
  }

  .title {
    margin-top: 4px;
    font-weight: 700;
  }

  .bar {
    height: 4px;
    margin: 10px 0 6px;
    overflow: hidden;
    border-radius: 2px;
    background: #ffffff1a;
  }

  .bar div {
    height: 100%;
    background: var(--accent);
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 4px 10px;
    color: var(--text-faint);
    font-size: 12px;
  }

  .libs {
    display: grid;
    gap: 8px;
  }

  .lib {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border: 1px solid var(--border);
    border-radius: 12px;
  }

  .lib-name {
    flex: 1;
    min-width: 140px;
    font-weight: 600;
  }

  .lib .btn {
    height: 34px;
    padding: 0 12px;
    font-size: 13px;
  }
</style>
