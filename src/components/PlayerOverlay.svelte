<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { errorText } from "../lib/api";
  import { toast } from "../lib/nav.svelte";
  import { attachVideo, detachVideo, mpv, play, player, stop, trackLabel } from "../lib/player.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n";
  import { device } from "../lib/device";

  // Téléphone : la vidéo passe en paysage et en plein écran (barres système masquées), comme Plex.
  const android = (window as { PlexrDevice?: { setVideoMode?(on: boolean): void } }).PlexrDevice;

  const appWindow = getCurrentWindow();

  let visible = $state(true);
  let fullscreen = $state(false);
  let menu = $state<"audio" | "sub" | null>(null);
  let hoverTime = $state<number | null>(null);
  let hoverX = $state(0);
  let dragging = $state(false);
  let bar = $state<HTMLElement>();
  // Sur mobile et TV, la vidéo est lue par cet élément plutôt que par mpv.
  let videoEl = $state<HTMLVideoElement>();
  $effect(() => {
    if (!videoEl) return;
    attachVideo(videoEl);
    return detachVideo;
  });
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let clickTimer: ReturnType<typeof setTimeout> | undefined;

  const audioTracks = $derived(player.tracks.filter((t) => t.type === "audio"));
  const subTracks = $derived(player.tracks.filter((t) => t.type === "sub"));
  const currentAudio = $derived(audioTracks.find((t) => t.id === player.aid));
  const currentSub = $derived(subTracks.find((t) => t.id === player.sid));
  const progress = $derived(player.duration ? (player.position / player.duration) * 100 : 0);
  const buffered = $derived(player.duration ? Math.min(100, (player.bufferedUntil / player.duration) * 100) : 0);

  function fmt(s: number) {
    s = Math.max(0, Math.floor(s));
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = String(s % 60).padStart(2, "0");
    return h ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
  }

  // Les contrôles s'effacent après 3 s sans mouvement, sauf en pause ou menu ouvert.
  function poke() {
    visible = true;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      // Avec une fenêtre mpv séparée, cette page sert de télécommande : elle reste affichée.
      if (!player.paused && !menu && !dragging && player.embedded) visible = false;
    }, 3000);
  }

  $effect(() => {
    if (player.paused) visible = true;
    else poke();
  });

  async function toggleFullscreen() {
    try {
      fullscreen = !(await appWindow.isFullscreen());
      await appWindow.setFullscreen(fullscreen);
    } catch {
      // Sur mobile, l'application occupe déjà tout l'écran.
    }
  }

  async function close() {
    if (fullscreen) await appWindow.setFullscreen(false);
    stop();
    player.active = false;
  }

  // Épisode précédent ou suivant de la série.
  function goTo(key: string | null) {
    if (key) play(key).catch((e) => toast(errorText(e), true));
  }

  // Un clic sur la vidéo met en pause ; un double clic bascule le plein écran.
  function onStageClick() {
    if (menu) {
      menu = null;
      return;
    }
    clearTimeout(clickTimer);
    clickTimer = setTimeout(() => mpv("cycle", "pause"), 220);
  }

  function onStageDblClick() {
    clearTimeout(clickTimer);
    toggleFullscreen();
  }

  function timeAt(clientX: number) {
    const rect = bar!.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
    hoverX = ratio * rect.width;
    return ratio * player.duration;
  }

  function onBarDown(e: PointerEvent) {
    dragging = true;
    bar!.setPointerCapture(e.pointerId);
    player.position = timeAt(e.clientX);
  }

  function onBarMove(e: PointerEvent) {
    hoverTime = timeAt(e.clientX);
    if (dragging) player.position = hoverTime;
  }

  function onBarUp(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    mpv("seek", timeAt(e.clientX), "absolute");
  }

  function setVolume(e: Event) {
    const v = Number((e.target as HTMLInputElement).value);
    mpv("set_property", "volume", v);
    if (player.muted && v > 0) mpv("set_property", "mute", false);
  }

  function selectTrack(kind: "aid" | "sid", id: number | "no") {
    mpv("set_property", kind, id);
    menu = null;
  }

  // Repère (intro, générique) sous la tête de lecture : le bouton « Passer », qui prend le focus sur
  // téléviseur pour qu'un simple OK suffise.
  const skip = $derived(
    player.markers.find((m) => {
      const ms = player.position * 1000;
      return (m.kind === "intro" || m.kind === "credits") && ms >= m.startMs && ms < m.endMs - 1500;
    }),
  );
  let skipEl = $state<HTMLButtonElement>();
  $effect(() => {
    if (skip && skipEl && document.documentElement.classList.contains("tv")) skipEl.focus();
  });

  function onKey(e: KeyboardEvent) {
    poke();
    // Sur téléviseur, les flèches de la télécommande servent à passer d'un bouton à l'autre.
    if (document.documentElement.classList.contains("tv") && e.key.startsWith("Arrow")) return;
    switch (e.key) {
      case "MediaPlayPause":
        mpv("cycle", "pause");
        break;
      case "MediaTrackNext":
        if (player.nextKey) goTo(player.nextKey);
        break;
      case "MediaTrackPrevious":
        if (player.prevKey) goTo(player.prevKey);
        break;
      case " ":
      case "k":
        mpv("cycle", "pause");
        break;
      case "ArrowLeft":
        mpv("seek", -10, "relative");
        break;
      case "ArrowRight":
        mpv("seek", 10, "relative");
        break;
      case "ArrowUp":
        mpv("add", "volume", 5);
        break;
      case "ArrowDown":
        mpv("add", "volume", -5);
        break;
      case "m":
        mpv("cycle", "mute");
        break;
      case "f":
        toggleFullscreen();
        break;
      case "Escape":
        if (menu) menu = null;
        else if (fullscreen) toggleFullscreen();
        else close();
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  onMount(async () => {
    poke();
    // À la télécommande, le focus part du bouton lecture.
    document.querySelector<HTMLElement>(".overlay .icon-btn.main")?.focus();
    if (device.phone) android?.setVideoMode?.(true);
    fullscreen = await appWindow.isFullscreen().catch(() => false);
  });

  onDestroy(() => {
    if (device.phone) android?.setVideoMode?.(false);
    clearTimeout(hideTimer);
    clearTimeout(clickTimer);
  });
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" class:hidden={!visible} class:remote={!player.embedded} class:web={player.web && !player.native} onpointermove={poke}>
  <!-- Android : la vidéo est dessinée sous la page par le lecteur natif ; sinon par cet élément. -->
  {#if player.web && !player.native}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video class="web-video" bind:this={videoEl} playsinline></video>
  {/if}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="stage" onclick={onStageClick} ondblclick={onStageDblClick}></div>

  {#if !player.embedded}
    <div class="center remote-note">
      <Icon name="fullscreen" size={40} />
      <p>{t("player.remoteNote")}<br />{t("player.remoteNote2")}</p>
    </div>
  {:else if player.buffering}
    <div class="center"><div class="spinner"></div></div>
  {:else if player.paused}
    <div class="center"><div class="big-play"><Icon name="play" size={76} filled /></div></div>
  {/if}

  <!-- Coins de l'image : fermer à gauche ; volume et plein écran à droite, en boutons ronds translucides. -->
  <div class="top">
    <button class="icon-btn glass" onclick={close} aria-label={t("player.close")} title={t("player.close")}>
      <Icon name="arrowLeft" size={22} />
    </button>
    <div class="corner">
      <div class="volume glass">
        <input
          type="range"
          min="0"
          max="130"
          value={player.muted ? 0 : player.volume}
          oninput={setVolume}
          aria-label={t("player.volume")}
        />
        <button class="icon-btn" onclick={() => mpv("cycle", "mute")} aria-label={t("player.mute")}>
          <Icon name={player.muted || player.volume === 0 ? "mute" : "volume"} size={20} />
        </button>
      </div>
      <button class="icon-btn glass" onclick={toggleFullscreen} aria-label={t("player.fullscreen")}>
        <Icon name={fullscreen ? "exitFullscreen" : "fullscreen"} size={20} />
      </button>
    </div>
  </div>

  <!-- Repère en cours (intro, générique) : un bouton pour le passer, comme dans Plex et Netflix. -->
  {#if skip}
    <button class="btn skip" bind:this={skipEl} onclick={() => mpv("seek", skip.endMs / 1000, "absolute")}>
      {skip.kind === "credits" ? t("player.skipCredits") : t("player.skipIntro")}
      <Icon name="skipNext" size={18} filled />
    </button>
  {/if}

  <!-- Capsule flottante, façon Apple TV : titre et pistes, progression, puis commandes de lecture. -->
  <div class="bottom">
    <div class="capsule">
      <div class="head">
        <div class="titles">
          {#if player.subtitle}<div class="subtitle">{player.subtitle}</div>{/if}
          <div class="title">{player.title}</div>
        </div>
        <!-- Avec un flux converti (navigateur), l'audio et les sous-titres se choisissent sur la fiche. -->
        {#if !player.web || player.native}
          <div class="pills">
            <div class="menu-anchor">
              <button
                class="pill"
                class:on={menu === "sub"}
                onclick={() => (menu = menu === "sub" ? null : "sub")}
                aria-label={t("player.subtitles")}
                title={t("player.subtitles")}
              >
                <Icon name="subtitles" size={16} />
                <span>{currentSub ? trackLabel(currentSub) : t("player.none")}</span>
              </button>
              {#if menu === "sub"}
                <div class="menu">
                  <div class="menu-title">{t("player.subtitles")}</div>
                  <button class:selected={player.sid === null} onclick={() => selectTrack("sid", "no")}>
                    <span class="tick">{#if player.sid === null}<Icon name="check" size={16} />{/if}</span>
                    {t("player.none")}
                  </button>
                  {#each subTracks as track (track.id)}
                    <button class:selected={player.sid === track.id} onclick={() => selectTrack("sid", track.id)}>
                      <span class="tick">{#if player.sid === track.id}<Icon name="check" size={16} />{/if}</span>
                      {trackLabel(track)}
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
            <div class="menu-anchor">
              <button
                class="pill"
                class:on={menu === "audio"}
                onclick={() => (menu = menu === "audio" ? null : "audio")}
                aria-label={t("player.audioTracks")}
                title={t("player.audioTracks")}
              >
                <Icon name="audio" size={16} />
                <span>{currentAudio ? trackLabel(currentAudio) : t("player.audio")}</span>
              </button>
              {#if menu === "audio"}
                <div class="menu">
                  <div class="menu-title">{t("player.audio")}</div>
                  {#each audioTracks as track (track.id)}
                    <button class:selected={player.aid === track.id} onclick={() => selectTrack("aid", track.id)}>
                      <span class="tick">{#if player.aid === track.id}<Icon name="check" size={16} />{/if}</span>
                      {trackLabel(track)}
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
        {/if}
      </div>

      <div
        class="bar"
        bind:this={bar}
        onpointerdown={onBarDown}
        onpointermove={onBarMove}
        onpointerup={onBarUp}
        onpointerleave={() => (hoverTime = null)}
        role="slider"
        tabindex="-1"
        aria-label={t("player.position")}
        aria-valuemin={0}
        aria-valuemax={player.duration}
        aria-valuenow={player.position}
      >
        <div class="track">
          <div class="buffered" style:width="{buffered}%"></div>
          <div class="played" style:width="{progress}%"></div>
        </div>
        <div class="thumb" style:left="{progress}%"></div>
        {#if hoverTime !== null}
          <div class="hover-time" style:left="{hoverX}px">{fmt(hoverTime)}</div>
        {/if}
      </div>

      <div class="controls">
        <span class="time">{fmt(player.position)}</span>
        <div class="transport">
          <!-- Contrepoids du bouton stop, pour garder lecture/pause au centre. -->
          <span class="spacer" aria-hidden="true"></span>
          {#if player.prevKey || player.nextKey}
            <button class="icon-btn" disabled={!player.prevKey} onclick={() => goTo(player.prevKey)} aria-label={t("player.prevEpisode")} title={t("player.prevEpisode")}>
              <Icon name="skipPrev" size={20} filled />
            </button>
          {/if}
          <button class="icon-btn" onclick={() => mpv("seek", -10, "relative")} aria-label={t("player.back10")} title={t("player.back10")}>
            <Icon name="replay10" size={24} />
          </button>
          <button class="icon-btn main" class:paused={player.paused} onclick={() => mpv("cycle", "pause")} aria-label={player.paused ? t("player.play") : t("player.pause")}>
            <Icon name={player.paused ? "play" : "pause"} size={36} filled />
          </button>
          <button class="icon-btn" onclick={() => mpv("seek", 10, "relative")} aria-label={t("player.forward10")} title={t("player.forward10")}>
            <Icon name="forward10" size={24} />
          </button>
          {#if player.prevKey || player.nextKey}
            <button class="icon-btn" disabled={!player.nextKey} onclick={() => goTo(player.nextKey)} aria-label={t("player.nextEpisode")} title={t("player.nextEpisode")}>
              <Icon name="skipNext" size={20} filled />
            </button>
          {/if}
          <!-- Arrêter : la lecture s'arrête et le lecteur se ferme (la reprise reste enregistrée). -->
          <button class="icon-btn" onclick={close} aria-label={t("common.c1.stop")} title={t("common.c1.stop")}>
            <Icon name="stop" size={28} filled />
          </button>
        </div>
        <!-- Temps restant, comme sur Apple TV. -->
        <span class="time end">−{fmt(player.duration - player.position)}</span>
      </div>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 40;
    color: #fff;
    user-select: none;
  }

  /* Fenêtre mpv séparée : fond opaque, la page sert de télécommande. */
  .overlay.remote {
    background:
      radial-gradient(60% 60% at 50% 40%, color-mix(in srgb, var(--accent) 10%, transparent), transparent 70%),
      var(--bg);
  }

  .remote-note {
    display: grid;
    gap: 12px;
    justify-items: center;
    align-content: center;
    color: var(--text-dim);
    text-align: center;
    font-size: 15px;
  }

  .remote-note p {
    margin: 0;
  }

  /* Lecteur du système : la vidéo est dans la page, sur fond noir. */
  .overlay.web {
    background: #000;
  }

  .web-video {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    background: #000;
  }

  .overlay.hidden {
    cursor: none;
  }

  .stage {
    position: absolute;
    inset: 0;
  }

  .top,
  .bottom {
    position: absolute;
    left: 0;
    right: 0;
    transition: opacity 0.25s ease, transform 0.25s ease;
  }

  .skip {
    position: absolute;
    right: 40px;
    bottom: 190px;
    height: 46px;
    padding: 0 20px;
    border-color: #ffffff55;
    background: #000000aa;
    font-size: 15px;
    font-weight: 600;
    backdrop-filter: blur(8px);
  }

  .skip:hover,
  :global(html.tv) .skip:focus {
    background: var(--accent);
    color: var(--accent-text);
  }

  .hidden .top,
  .hidden .bottom {
    opacity: 0;
    pointer-events: none;
  }

  .hidden .bottom {
    transform: translateY(12px);
  }

  .top {
    top: 0;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 28px;
  }

  .corner {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  /* Boutons ronds translucides des coins de l'image. */
  .glass,
  .icon-btn.glass {
    background: #1c1f24b3;
    border: 1px solid #ffffff1f;
    backdrop-filter: blur(16px);
  }

  .icon-btn.glass:hover {
    background: #2a2e35d9;
  }

  .bottom {
    bottom: 0;
    padding: 0 28px 22px;
  }

  /* La capsule : une carte translucide qui flotte au-dessus du bas de l'image. */
  .capsule {
    max-width: 1100px;
    margin: 0 auto;
    padding: 16px 22px 12px;
    border-radius: 22px;
    background: #1c1f24d9;
    border: 1px solid #ffffff1f;
    backdrop-filter: blur(24px) saturate(1.4);
    box-shadow: 0 20px 50px #0009;
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px;
    margin-bottom: 8px;
  }

  .titles {
    min-width: 0;
  }

  .title {
    font-size: 19px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .subtitle {
    color: #fff9;
    font-size: 13px;
  }

  .pills {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  /* Piste audio et sous-titres en cours, lisibles sans ouvrir de menu. */
  .pill {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 200px;
    height: 32px;
    padding: 0 12px;
    border: none;
    border-radius: 999px;
    background: #ffffff1f;
    color: #fffe;
    font-size: 13px;
    transition: background 0.15s;
  }

  .pill span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pill:hover,
  .pill.on {
    background: #ffffff38;
  }

  .bar {
    position: relative;
    height: 22px;
    display: flex;
    align-items: center;
    cursor: pointer;
    touch-action: none;
  }

  .track {
    position: relative;
    width: 100%;
    height: 5px;
    border-radius: 3px;
    background: #ffffff2e;
    overflow: hidden;
    transition: height 0.15s;
  }

  .bar:hover .track {
    height: 8px;
  }

  .buffered,
  .played {
    position: absolute;
    inset: 0 auto 0 0;
  }

  .buffered {
    background: #ffffff33;
  }

  .played {
    background: #fff;
  }

  .thumb {
    position: absolute;
    width: 14px;
    height: 14px;
    margin-left: -7px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 2px 8px #0008;
    transform: scale(0);
    transition: transform 0.15s;
  }

  .bar:hover .thumb {
    transform: scale(1);
  }

  .hover-time {
    position: absolute;
    bottom: 24px;
    transform: translateX(-50%);
    background: #000c;
    padding: 3px 8px;
    border-radius: 6px;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    pointer-events: none;
  }

  .controls {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    margin-top: 2px;
  }

  .transport {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .spacer {
    width: 42px;
  }

  .time {
    color: #fffb;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }

  .time.end {
    text-align: right;
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    border: none;
    border-radius: 50%;
    background: none;
    color: #fff;
    transition: background 0.15s, color 0.15s;
  }

  .icon-btn:hover {
    background: #ffffff1f;
  }

  .icon-btn:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .icon-btn:disabled:hover {
    background: none;
  }

  /* Bouton lecture principal : plus grand, sans pastille. */
  .icon-btn.main {
    width: 54px;
    height: 54px;
  }

  /* Le focus donné au bouton à l'ouverture (pour la télécommande) n'a pas à l'entourer à la souris. */
  :global(html:not(.tv)) .icon-btn.main:focus {
    outline: none;
  }

  /* Volume : le curseur se déplie vers la gauche au survol. */
  .volume {
    display: flex;
    align-items: center;
    border-radius: 999px;
  }

  .volume input {
    width: 0;
    margin: 0;
    padding: 0;
    border: none;
    opacity: 0;
    transition: width 0.2s, opacity 0.2s, margin 0.2s;
    accent-color: #fff;
  }

  .volume:hover input,
  .volume input:focus {
    width: 100px;
    margin-left: 14px;
    opacity: 1;
  }

  .menu-anchor {
    position: relative;
  }

  .menu {
    position: absolute;
    right: 0;
    bottom: 44px;
    min-width: 280px;
    max-height: 50vh;
    overflow-y: auto;
    padding: 8px;
    border-radius: 14px;
    background: #16181ce8;
    backdrop-filter: blur(16px);
    border: 1px solid #ffffff1a;
    box-shadow: 0 16px 40px #000a;
  }
  .menu-title {
    padding: 6px 10px 8px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: #fff9;
  }

  .menu button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 9px 10px;
    border: none;
    border-radius: 8px;
    background: none;
    text-align: left;
    color: #fffd;
  }

  .menu button:hover {
    background: #ffffff14;
  }

  .menu button.selected {
    color: var(--accent);
  }

  .tick {
    display: inline-grid;
    width: 16px;
  }

  .center {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
  }

  /* Triangle seul, façon Netflix : une ombre douce suffit à le détacher de l'image. */
  .big-play {
    display: grid;
    /* L'icône place déjà le triangle un peu à droite ; on n'en garde que la moitié, pour l'œil. */
    translate: -2px 0;
    filter: drop-shadow(0 2px 12px #000a);
  }

  .spinner {
    width: 56px;
    height: 56px;
    border: 4px solid #ffffff33;
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.9s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Téléphone : capsule plus serrée, pistes sous le titre. */
  @media (max-width: 640px) {
    .top {
      padding: 12px 14px;
    }

    .bottom {
      padding: 0 10px 10px;
    }

    .capsule {
      padding: 12px 14px 8px;
      border-radius: 18px;
    }

    .head {
      flex-direction: column;
      align-items: stretch;
      gap: 8px;
    }

    .pill {
      max-width: 150px;
    }

    .skip {
      right: 14px;
      bottom: 210px;
    }
  }
</style>
