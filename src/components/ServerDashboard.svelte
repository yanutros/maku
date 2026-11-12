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
  /** Jaquettes qui n'ont pas pu se charger : on montre une icône à la place. */
  let broken = $state<Record<string, boolean>>({});

  // En direct, comme Plex Web : les lectures sont relues toutes les 3 s, et la position avance d'elle-même chaque
  // seconde pour celles en cours de lecture. Le lecteur ne donne sa position au serveur que toutes les ~10 s :
  // entre deux, le serveur renvoie une position en retard. On garde donc notre propre repère (position, instant)
  // par lecture, recalé seulement quand le serveur est en avance, ou nettement différent (retour en arrière,
  // saut dans la lecture), pause ou reprise : le temps affiché ne recule plus de quelques secondes.
  type Anchor = { pos: number; at: number; state: string };
  let anchors = new Map<string, Anchor>();
  let now = $state(Date.now());
  let clock: ReturnType<typeof setInterval> | undefined;
  const keyOf = (s: ServerSession, i: number) => s.sessionId || [i, s.user, s.title].join(":");
  const estimate = (a: Anchor, at: number) => (a.state === "playing" ? a.pos + (at - a.at) : a.pos);

  function reanchor(list: ServerSession[]) {
    const at = Date.now();
    const next = new Map<string, Anchor>();
    list.forEach((s, i) => {
      const key = keyOf(s, i);
      const old = anchors.get(key);
      const guess = old ? estimate(old, at) : NaN;
      const keep = old && old.state === s.state && s.state === "playing" && s.progressMs <= guess && guess - s.progressMs < 15000;
      next.set(key, keep ? { pos: guess, at, state: s.state } : { pos: s.progressMs, at, state: s.state });
    });
    anchors = next;
  }

  function progressOf(s: ServerSession, i: number) {
    const a = anchors.get(keyOf(s, i));
    const pos = a ? estimate(a, now) : s.progressMs;
    return Math.max(0, Math.min(s.durationMs || Infinity, pos));
  }

  async function load() {
    try {
      const list = await api.sessions();
      reanchor(list);
      sessions = list;
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
    timer = setInterval(load, 3000);
    clock = setInterval(() => (now = Date.now()), 1000);
  });

  onDestroy(() => (clearInterval(timer), clearInterval(clock)));
</script>

<div class="dash">
<div class="block wide">
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
        <!-- Jaquette à ses vraies proportions : affiche (2:3) pour un film ou une série, carrée pour la musique. -->
        <div class="poster" class:square={s.music}>
          {#if s.poster && !broken[s.poster]}
            <img src={s.poster} alt="" onerror={() => (broken[s.poster ?? ""] = true)} />
          {:else}
            <Icon name={s.music ? "music" : "film"} size={28} />
          {/if}
        </div>
        <div class="body">
          <div class="who">
            {#if s.userThumb}<img class="avatar" src={s.userThumb} alt="" />{/if}
            <span>{s.user}</span>
            <span class="state" class:paused={s.state === "paused"}>{s.state === "paused" ? t("dashboard.paused") : s.state === "buffering" ? t("dashboard.buffering") : t("dashboard.playing")}</span>
          </div>
          <div class="title">{s.title}</div>
          {#if s.subtitle}<div class="muted small">{s.subtitle}</div>{/if}
          <div class="bar"><div style:width="{s.durationMs ? (progressOf(s, i) / s.durationMs) * 100 : 0}%"></div></div>
          <div class="meta">
            <span>{formatClock(progressOf(s, i) / 1000)} / {formatClock(s.durationMs / 1000)}</span>
            {#if s.bandwidthKbps}<span>{t("dashboard.bandwidth", { rate: (s.bandwidthKbps / 1000).toFixed(1) })}</span>{/if}
          </div>
          <!-- L'appareil et le mode de lecture, en entier ; le bouton d'arrêt en pied de carte, toujours en bas. -->
          <div class="chips">
            <span class="chip">{s.player}</span>
            <span class="chip">{s.local ? t("dashboard.local") : t("dashboard.remote")}</span>
            <span class="chip">{s.transcode ? t("dashboard.transcode") : t("dashboard.directPlay")}</span>
          </div>
          {#if s.sessionId}
            <div class="foot">
              <button class="btn stop" onclick={() => ((stopping = s), (reason = ""))}><Icon name="x" size={15} /> {t("dashboard.stop")}</button>
            </div>
          {/if}
        </div>
      </div>
    {/each}
  </div>
{/if}

</div>

<ServerStats />

<div class="block wide">
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

</div>
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
  /* Tableau de bord en tuiles, façon Netflix : débit et historique côte à côte, le reste sur toute la largeur. */
  .dash {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    align-items: start;
    gap: 20px;
  }

  .dash > :global(.wide) {
    grid-column: 1 / -1;
  }

  @media (max-width: 1280px) {
    .dash {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  .block {
    min-width: 0;
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: #ffffff05;
  }

  .block .sub {
    margin: 0 0 14px;
  }

  .btn.stop {
    flex: none;
    height: 34px;
    padding: 0 12px;
    font-size: 13px;
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

  /* align-self : sans lui, la jaquette s'étire sur toute la hauteur de la carte et paraît tassée. */
  .poster {
    display: grid;
    flex: none;
    place-items: center;
    align-self: flex-start;
    width: 96px;
    aspect-ratio: 2 / 3;
    color: var(--text-faint);
    overflow: hidden;
    border-radius: 8px;
    background: var(--bg-hover);
  }

  .poster.square {
    width: 96px;
    aspect-ratio: 1;
  }

  .poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  /* Le texte en colonne, le pied de carte poussé en bas : toutes les cartes d'une rangée ont la même hauteur,
     leurs boutons tombent donc sur la même ligne. */
  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }

  .title,
  .body > .muted {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .foot {
    display: flex;
    justify-content: flex-end;
    margin-top: auto;
    padding-top: 14px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 10px;
  }

  .chip {
    padding: 3px 9px;
    border-radius: 10px;
    background: #ffffff0f;
    color: var(--text-dim);
    font-size: 12px;
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

  /* La barre avance en continu entre deux secondes. */
  .bar div {
    height: 100%;
    background: var(--accent);
    transition: width 1s linear;
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
