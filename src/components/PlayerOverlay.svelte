<script lang="ts" module>
  import { onBack } from "../lib/nav.svelte";

  /** Carte « Épisode suivant » affichée : le bouton Retour l'annule (la vidéo continue). */
  let cancelNextCard: (() => boolean) | null = null;
  onBack(() => cancelNextCard?.() ?? false);
</script>

<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { errorText } from "../lib/api";
  import { toast } from "../lib/nav.svelte";
  import {
    QUALITIES,
    attachVideo,
    autoNext,
    detachVideo,
    mpv,
    play,
    player,
    qualityLabel,
    setQuality,
    setServerStream,
    stop,
    trackLabel,
  } from "../lib/player.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n";
  import { device } from "../lib/device";

  // Téléphone : la vidéo passe en paysage et en plein écran (barres système masquées), comme Plex.
  const android = (window as { PlexrDevice?: { setVideoMode?(on: boolean): void } }).PlexrDevice;

  const appWindow = getCurrentWindow();

  let visible = $state(true);
  let fullscreen = $state(false);
  let menu = $state<"audio" | "sub" | "quality" | null>(null);
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
  // Vidéo convertie par le serveur : les pistes sont celles du serveur, choisies en relançant la conversion.
  const serverAudioId = $derived(player.streams.audioStream ?? player.serverAudio.find((t) => t.selected)?.id ?? null);
  const serverSubId = $derived(player.streams.subtitleStream ?? player.serverSubs.find((t) => t.selected)?.id ?? 0);
  const audioLabel = $derived(
    player.transcoded
      ? (player.serverAudio.find((t) => t.id === serverAudioId)?.label ?? t("player.audio"))
      : currentAudio
        ? trackLabel(currentAudio)
        : t("player.audio"),
  );
  const subLabel = $derived(
    player.transcoded
      ? (player.serverSubs.find((t) => t.id === serverSubId)?.label ?? t("player.none"))
      : currentSub
        ? trackLabel(currentSub)
        : t("player.none"),
  );
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

  // Aperçu au-dessus de la barre : la miniature du serveur la plus proche de l'instant survolé.
  const PREVIEW_W = 240;
  const preview = $derived.by(() => {
    const p = player.previews;
    if (!p || hoverTime === null || !bar) return null;
    const ms = Math.max(0, hoverTime * 1000);
    const width = PREVIEW_W;
    const height = Math.round((PREVIEW_W * (p.height || 9)) / (p.width || 16));
    // Centré sur la tête de lecture, sans dépasser des bords de la barre.
    const left = Math.min(Math.max(hoverX, width / 2), bar.clientWidth - width / 2);
    if (p.kind === "frame") {
      const at = Math.round(ms / p.intervalMs) * p.intervalMs;
      return { left, width, height, style: `background-image: url("${p.url.replace("{ms}", String(at))}"); background-size: cover` };
    }
    // Planches de vignettes (Jellyfin) : la bonne planche, puis la bonne case.
    const per = p.columns * p.rows;
    const i = Math.min(Math.floor(ms / p.intervalMs), Math.max(0, p.count - 1));
    const sheet = Math.floor(i / per);
    const cell = i % per;
    const x = (cell % p.columns) * width;
    const y = Math.floor(cell / p.columns) * height;
    return {
      left,
      width,
      height,
      style:
        `background-image: url("${p.url.replace("{index}", String(sheet))}");` +
        ` background-size: ${p.columns * width}px ${p.rows * height}px; background-position: -${x}px -${y}px`,
    };
  });

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

  function selectServerTrack(kind: "audio" | "subtitle", id: number) {
    menu = null;
    if (id === (kind === "audio" ? serverAudioId : serverSubId)) return;
    setServerStream(kind, id)?.catch((e) => toast(errorText(e), true));
  }

  function selectQuality(kbps: number) {
    menu = null;
    setQuality(kbps)?.catch((e) => toast(errorText(e), true));
  }

  // Épisode suivant : sa carte apparaît au début du générique (repère du serveur, s'il est dans la seconde
  // moitié), sinon 30 s avant la fin, avec un compte à rebours réglé dans Réglages → Lecteur. La pause
  // l'arrête ; « Annuler » ou Retour masquent la carte et la vidéo s'arrête à la fin, comme avant.
  // Hauteur des commandes du bas : la carte et « Passer » se posent juste au-dessus quand elles sont affichées.
  let controlsHeight = $state(0);
  let nextCancelled = $state(false);
  let nextShownAt = $state<number | null>(null);
  let nextLaunched = false;
  $effect(() => {
    void player.ratingKey;
    nextCancelled = false;
    nextShownAt = null;
    nextLaunched = false;
  });
  const creditsAt = $derived.by(() => {
    const credits = player.markers.filter((m) => m.kind === "credits" && m.startMs / 1000 > player.duration / 2)[0];
    return credits ? credits.startMs / 1000 : player.duration - 30;
  });
  const showNext = $derived(
    player.video && !!player.nextKey && !!player.upNext && autoNext.delay > 0 && !nextCancelled && player.duration > 120 && player.position >= creditsAt,
  );
  const nextLeft = $derived(nextShownAt === null ? autoNext.delay : Math.max(0, autoNext.delay - (player.position - nextShownAt)));
  let nextEl = $state<HTMLButtonElement>();
  $effect(() => {
    player.autoNext = showNext;
    if (!showNext) {
      nextShownAt = null;
      return;
    }
    untrack(() => {
      if (nextShownAt === null) nextShownAt = player.position;
    });
  });
  $effect(() => {
    if (showNext && nextShownAt !== null && nextLeft <= 0 && !nextLaunched) {
      nextLaunched = true;
      goTo(player.nextKey);
    }
  });
  $effect(() => {
    if (showNext && nextEl && document.documentElement.classList.contains("tv")) nextEl.focus();
  });
  function cancelNext() {
    nextCancelled = true;
    player.autoNext = false;
  }
  cancelNextCard = () => {
    if (!showNext) return false;
    cancelNext();
    return true;
  };
  onDestroy(() => (cancelNextCard = null));

  // Repère (intro, générique) sous la tête de lecture : le bouton « Passer », qui prend le focus sur
  // téléviseur pour qu'un simple OK suffise. Pendant la carte « Épisode suivant », elle le remplace.
  const skip = $derived(
    player.markers.find((m) => {
      const ms = player.position * 1000;
      return (m.kind === "intro" || (m.kind === "credits" && !showNext)) && ms >= m.startMs && ms < m.endMs - 1500;
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
        else if (showNext) cancelNext();
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
<div class="overlay" class:hidden={!visible} style:--controls="{controlsHeight}px" class:remote={!player.embedded} class:web={player.web && !player.native} class:loading={!player.web && player.embedded && !player.videoReady} onpointermove={poke}>
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

  <!-- Coins de l'image : fermer à gauche ; volume et plein écran à droite, en boutons ronds translucides.
       Pas sur téléviseur : la télécommande a ses touches Retour et volume, et l'image y est déjà plein écran. -->
  {#if !device.tv}
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
  {/if}

  <!-- Repère en cours (intro, générique) : un bouton pour le passer, comme dans Plex et Netflix. -->
  {#if skip}
    <button class="btn skip" bind:this={skipEl} onclick={() => mpv("seek", skip.endMs / 1000, "absolute")}>
      {skip.kind === "credits" ? t("player.skipCredits") : t("player.skipIntro")}
      <Icon name="skipNext" size={18} filled />
    </button>
  {/if}

  <!-- Épisode suivant, au générique : vignette, titre, « Lire » qui se remplit pendant le compte à rebours. -->
  {#if showNext && player.upNext}
    <div class="next-card" role="dialog" aria-label={t("player.nextEpisode")}>
      <div class="next-thumb">{#if player.upNext.thumb}<img src={player.upNext.thumb} alt="" />{/if}</div>
      <div class="next-info">
        <span class="next-eyebrow">{t("player.nextEpisode")}{#if player.upNext.subtitle} · {player.upNext.subtitle}{/if}</span>
        <strong class="next-title">{player.upNext.title}</strong>
        <div class="next-actions">
          <button class="btn next-play" bind:this={nextEl} onclick={() => goTo(player.nextKey)}>
            <span class="next-fill" style:width="{(1 - nextLeft / autoNext.delay) * 100}%"></span>
            <span class="next-label"><Icon name="play" size={16} filled /> {t("player.playNextIn", { count: Math.ceil(nextLeft) })}</span>
          </button>
          <button class="btn next-cancel" onclick={cancelNext}>{t("player.cancelNext")}</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Capsule flottante, façon Apple TV : titre et pistes, progression, puis commandes de lecture. -->
  <div class="bottom" bind:clientHeight={controlsHeight}>
    <div class="capsule">
      <div class="head">
        <div class="titles">
          {#if player.subtitle}<div class="subtitle">{player.subtitle}</div>{/if}
          <div class="title">{player.title}</div>
        </div>
        <!-- Qualité, sous-titres et audio. Une vidéo convertie par le serveur change de piste ou de qualité
             en relançant la conversion au même endroit. -->
        <div class="pills">
            <div class="menu-anchor">
              <button
                class="pill"
                class:on={menu === "quality"}
                onclick={() => (menu = menu === "quality" ? null : "quality")}
                aria-label={t("player.quality")}
                title={t("player.quality")}
              >
                <Icon name="sliders" size={16} />
                <span>{qualityLabel(player.quality)}</span>
              </button>
              {#if menu === "quality"}
                <div class="menu">
                  <div class="menu-title">{t("player.quality")}</div>
                  {#each QUALITIES as kbps (kbps)}
                    <button class:selected={player.quality === kbps} onclick={() => selectQuality(kbps)}>
                      <span class="tick">{#if player.quality === kbps}<Icon name="check" size={16} />{/if}</span>
                      {qualityLabel(kbps)}
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
            <div class="menu-anchor">
              <button
                class="pill"
                class:on={menu === "sub"}
                onclick={() => (menu = menu === "sub" ? null : "sub")}
                aria-label={t("player.subtitles")}
                title={t("player.subtitles")}
              >
                <Icon name="subtitles" size={16} />
                <span>{subLabel}</span>
              </button>
              {#if menu === "sub" && player.transcoded}
                <div class="menu">
                  <div class="menu-title">{t("player.subtitles")}</div>
                  <button class:selected={serverSubId === 0} onclick={() => selectServerTrack("subtitle", 0)}>
                    <span class="tick">{#if serverSubId === 0}<Icon name="check" size={16} />{/if}</span>
                    {t("player.none")}
                  </button>
                  {#each player.serverSubs as track (track.id)}
                    <button class:selected={serverSubId === track.id} onclick={() => selectServerTrack("subtitle", track.id)}>
                      <span class="tick">{#if serverSubId === track.id}<Icon name="check" size={16} />{/if}</span>
                      {track.label}
                    </button>
                  {/each}
                </div>
              {:else if menu === "sub"}
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
                <span>{audioLabel}</span>
              </button>
              {#if menu === "audio" && player.transcoded}
                <div class="menu">
                  <div class="menu-title">{t("player.audio")}</div>
                  {#each player.serverAudio as track (track.id)}
                    <button class:selected={serverAudioId === track.id} onclick={() => selectServerTrack("audio", track.id)}>
                      <span class="tick">{#if serverAudioId === track.id}<Icon name="check" size={16} />{/if}</span>
                      {track.label}
                    </button>
                  {/each}
                </div>
              {:else if menu === "audio"}
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
        {#if preview}
          <div class="preview" style:left="{preview.left}px" style:width="{preview.width}px" style:height="{preview.height}px" style={preview.style}>
            <span>{fmt(hoverTime ?? 0)}</span>
          </div>
        {:else if hoverTime !== null}
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
            <Icon name={player.paused ? "play" : "pause"} size={26} filled />
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

  /* Ordinateur, avant la première image de mpv : fond noir, la roue de chargement au milieu. */
  .overlay.loading {
    background: #000;
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
    right: var(--edge);
    bottom: var(--above);
    transition: bottom 0.25s ease;
    height: 46px;
    padding: 0 20px;
    border-color: #ffffff55;
    background: #000000aa;
    font-size: 15px;
    font-weight: 600;
    backdrop-filter: blur(8px);
  }

  /* Carte et « Passer » : alignées sur le bord droit de la capsule (1100 px au plus, centrée), juste au-dessus
     des commandes quand elles sont affichées, en bas de l image sinon. */
  .overlay {
    --edge: max(28px, calc((100% - 1100px) / 2));
    --above: calc(var(--controls) + 12px);
  }

  .overlay.hidden {
    --above: 28px;
  }

  .next-card {
    position: absolute;
    right: var(--edge);
    bottom: var(--above);
    transition: bottom 0.25s ease;
    display: flex;
    gap: 14px;
    width: min(440px, calc(100% - 28px));
    padding: 12px;
    border: 1px solid #ffffff22;
    border-radius: 18px;
    background: #0b0c10e6;
    color: #fff;
    backdrop-filter: blur(12px);
  }

  .next-thumb {
    flex: none;
    width: 160px;
    aspect-ratio: 16 / 9;
    overflow: hidden;
    border-radius: 10px;
    background: #ffffff14;
  }

  .next-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .next-info {
    display: grid;
    align-content: center;
    gap: 6px;
    min-width: 0;
  }

  .next-eyebrow {
    color: #ffffffaa;
    font-size: 12px;
  }

  .next-title {
    overflow: hidden;
    font-size: 16px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .next-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 2px;
  }

  .next-play {
    position: relative;
    overflow: hidden;
    border-color: transparent;
    background: var(--accent);
    color: var(--accent-text);
    font-weight: 700;
  }

  .next-fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: #ffffff40;
    transition: width 0.5s linear;
  }

  .next-label {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .next-cancel {
    border-color: #ffffff33;
    background: #ffffff14;
    color: #fff;
  }

  :global(html.tv) .next-cancel:focus {
    background: #fff;
    color: #111;
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

  /* Miniature d'aperçu, l'heure posée en bas, comme dans Plex. */
  .preview {
    position: absolute;
    bottom: 24px;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    overflow: hidden;
    border: 2px solid #fff;
    border-radius: 10px;
    background-color: #000;
    background-repeat: no-repeat;
    box-shadow: 0 10px 30px #000a;
    transform: translateX(-50%);
    pointer-events: none;
  }

  .preview span {
    margin-bottom: 6px;
    padding: 2px 8px;
    border-radius: 6px;
    background: #000c;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
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

    .overlay {
      --edge: 10px;
    }

    .next-thumb {
      width: 110px;
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

  }
</style>
