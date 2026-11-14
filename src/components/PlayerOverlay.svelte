<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { errorText } from "../lib/api";
  import { nav, toast } from "../lib/nav.svelte";
  import {
    QUALITIES,
    attachVideo,
    hasEndScreen,
    openEndScreen,
    detachVideo,
    mpv,
    play,
    player,
    qualityLabel,
    setQuality,
    setServerStream,
    stop,
    trackLabel,
    unstall,
  } from "../lib/player.svelte";
  import Icon from "./Icon.svelte";
  import EndScreen from "./EndScreen.svelte";
  import { t } from "../lib/i18n";
  import { device } from "../lib/device";
  import { playerBar } from "../lib/playerBar.svelte";

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

  /*
   * Comme dans Plex, les commandes s'effacent pour laisser toute la place à l'image :
   * - pendant la lecture, après 4 s sans bouger la souris ni toucher la télécommande ;
   * - en pause, sur ordinateur et téléphone, elles restent tant que la souris est dans la fenêtre (elles partent
   *   quand elle en sort) ; à la télécommande, elles partent après 8 s sans rien toucher (le temps d'aller
   *   chercher à boire, l'image reste seule).
   * Jamais avec un menu ouvert, pendant qu'on glisse la tête de lecture, ni avec une fenêtre mpv séparée (cette
   * page lui sert alors de télécommande).
   */
  const HIDE_PLAYING = 4000;
  const HIDE_PAUSED_TV = 8000;

  function canHide() {
    return !menu && !dragging && player.embedded && !player.stalled;
  }

  function poke() {
    visible = true;
    clearTimeout(hideTimer);
    if (player.paused && !device.tv) return;
    hideTimer = setTimeout(() => {
      if (canHide()) visible = false;
    }, player.paused ? HIDE_PAUSED_TV : HIDE_PLAYING);
  }

  // Retour (télécommande, Android) avec un menu de pistes ouvert : il se ferme, la sélection revient à son bouton, et
  // le film continue (sinon Retour quittait le film).
  $effect(() => {
    if (!menu) {
      nav.closePlayerMenu = null;
      return;
    }
    nav.closePlayerMenu = () => {
      const opener = document.querySelector<HTMLElement>(".overlay .pill.on");
      menu = null;
      opener?.focus({ preventScroll: true });
      poke();
    };
    return () => (nav.closePlayerMenu = null);
  });

  /** Ordinateur : la souris sort de la fenêtre, les commandes partent aussitôt (en pause comme en lecture). */
  function onPointerLeave(e: PointerEvent) {
    if (device.tv || e.pointerType === "touch") return;
    clearTimeout(hideTimer);
    if (canHide()) visible = false;
  }

  // Pause ou reprise : les commandes reviennent, puis repartent selon le nouvel état.
  $effect(() => {
    void player.paused;
    poke();
  });

  /**
   * Télécommande, commandes effacées : le premier appui (OK ou une flèche) ne fait que les réafficher, comme dans
   * Plex. OK ne met donc pas en pause par surprise, et l'on voit le temps restant sans arrêter le film. Le bouton
   * « Passer » (intro, générique), lui, reste à un seul OK.
   */
  function onKeyCapture(e: KeyboardEvent) {
    if (!device.tv || visible || player.endScreen || player.stalled) return;
    if (e.key !== "Enter" && e.key !== " " && !e.key.startsWith("Arrow")) return;
    if (skipEl && document.activeElement === skipEl && e.key === "Enter") return;
    e.preventDefault();
    e.stopImmediatePropagation();
    poke();
  }

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

  // Hauteur des commandes du bas : « Passer » se pose juste au-dessus quand elles sont affichées.
  let controlsHeight = $state(0);

  // Repère (intro, générique) sous la tête de lecture : le bouton « Passer », qui prend le focus sur
  // téléviseur pour qu'un simple OK suffise. Le générique d'un épisode qui a une suite mène à l'écran de fin.
  const skip = $derived(
    player.markers.find((m) => {
      const ms = player.position * 1000;
      return (m.kind === "intro" || m.kind === "credits") && ms >= m.startMs && ms < m.endMs - 1500;
    }),
  );
  let skipEl = $state<HTMLButtonElement>();

  // Vidéo figée : les commandes restent affichées, et sur téléviseur la sélection va sur « Réessayer ».
  let stalledEl = $state<HTMLDivElement>();
  $effect(() => {
    if (!player.stalled || !stalledEl) return;
    poke();
    if (device.tv) stalledEl.querySelector<HTMLButtonElement>("button")?.focus();
  });
  $effect(() => {
    if (skip && skipEl && document.documentElement.classList.contains("tv")) skipEl.focus();
  });

  function onKey(e: KeyboardEvent) {
    poke();
    // Écran de fin : ses boutons seulement ; Échap le ferme (fin de la lecture).
    if (player.endScreen) {
      if (e.key === "Escape") {
        e.preventDefault();
        close();
      }
      return;
    }
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

<svelte:window onkeydown={onKey} onkeydowncapture={onKeyCapture} />

<!-- Qualité, sous-titres et audio : en pastilles avec la piste en cours (barre complète), ou en icônes seules
     (barre minimaliste, la piste dans l'infobulle). -->
{#snippet pickers(compact: boolean)}
    <div class="menu-anchor">
      <button
        class="pill"
        class:compact
        class:on={menu === "quality"}
        onclick={() => (menu = menu === "quality" ? null : "quality")}
        aria-label={t("player.quality")}
        title={t("player.quality")}
      >
        <Icon name="sliders" size={16} />
        {#if !compact}<span>{qualityLabel(player.quality)}</span>{/if}
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
        class:compact
        class:on={menu === "sub"}
        onclick={() => (menu = menu === "sub" ? null : "sub")}
        aria-label={t("player.subtitles")}
        title={t("player.subtitles")}
      >
        <Icon name="subtitles" size={16} />
        {#if !compact}<span>{subLabel}</span>{/if}
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
        class:compact
        class:on={menu === "audio"}
        onclick={() => (menu = menu === "audio" ? null : "audio")}
        aria-label={t("player.audioTracks")}
        title={t("player.audioTracks")}
      >
        <Icon name="audio" size={16} />
        {#if !compact}<span>{audioLabel}</span>{/if}
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
{/snippet}

{#snippet seekBar()}
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
    </div>{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" class:hidden={!visible} style:--controls="{controlsHeight}px" class:remote={!player.embedded} class:web={player.web && !player.native} class:loading={!player.web && player.embedded && !player.videoReady} onpointermove={poke} onpointerleave={onPointerLeave}>
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
    {#if playerBar.style !== "minimal"}
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
    {/if}
  </div>
  {/if}

  <!-- Vidéo figée (Android) : prévenir au lieu de rester bloqué sans rien dire, avec de quoi repartir. -->
  {#if player.stalled && !player.endScreen}
    <div class="stalled" role="dialog" aria-label={t("player.stalled", { time: fmt(player.stalled.at) })} bind:this={stalledEl}>
      <strong>{t("player.stalled", { time: fmt(player.stalled.at) })}</strong>
      <p>{t("player.stalledHint")}</p>
      <div class="stalled-actions">
        <button class="btn primary" onclick={() => unstall("retry")}><Icon name="restart" size={18} /> {t("player.stalledRetry")}</button>
        <button class="btn" onclick={() => mpv("seek", 10, "relative")}><Icon name="forward10" size={18} /> {t("player.stalledSkip")}</button>
        {#if player.stalled.passthrough}
          <button class="btn" onclick={() => unstall("decodeAudio")}><Icon name="audio" size={18} /> {t("player.stalledDecode")}</button>
        {/if}
        <!-- Le fichier d'origine : le serveur peut le convertir (pas pour un téléchargement, ni déjà converti). -->
        {#if !player.transcoded && !player.offline}
          <button class="btn" onclick={() => unstall("transcode")}><Icon name="server" size={18} /> {t("player.stalledTranscode")}</button>
        {/if}
      </div>
    </div>
  {/if}

  <!-- Repère en cours (intro, générique) : un bouton pour le passer, comme dans Plex et Netflix. -->
  {#if skip && !player.endScreen}
    <button class="btn skip" bind:this={skipEl} onclick={() => (skip.kind === "credits" && hasEndScreen() ? openEndScreen() : mpv("seek", skip.endMs / 1000, "absolute"))}>
      {skip.kind === "credits" ? t("player.skipCredits") : t("player.skipIntro")}
      <Icon name="skipNext" size={18} filled />
    </button>
  {/if}

  {#if playerBar.style === "minimal"}
  <!-- Barre minimaliste (Réglages → Interface) : sans cadre, sur un dégradé, le titre en petit puis une seule
       rangée — lecture, ±10 s, épisode suivant, arrêt, progression, temps restant, pistes, volume et plein écran. -->
  <div class="bottom minimal" bind:clientHeight={controlsHeight}>
    <div class="mini">
      <div class="mini-title">{player.title}{#if player.subtitle}<span>{" · "}{player.subtitle}</span>{/if}</div>
      <div class="mini-row">
        <button class="icon-btn main" class:paused={player.paused} onclick={() => mpv("cycle", "pause")} aria-label={player.paused ? t("player.play") : t("player.pause")}>
          <Icon name={player.paused ? "play" : "pause"} size={24} filled />
        </button>
        <button class="icon-btn" onclick={() => mpv("seek", -10, "relative")} aria-label={t("player.back10")} title={t("player.back10")}>
          <Icon name="replay10" size={22} />
        </button>
        <button class="icon-btn" onclick={() => mpv("seek", 10, "relative")} aria-label={t("player.forward10")} title={t("player.forward10")}>
          <Icon name="forward10" size={22} />
        </button>
        {#if player.nextKey}
          <button class="icon-btn" onclick={() => goTo(player.nextKey)} aria-label={t("player.nextEpisode")} title={t("player.nextEpisode")}>
            <Icon name="skipNext" size={20} filled />
          </button>
        {/if}
        <!-- Arrêter : la lecture s'arrête et le lecteur se ferme (la reprise reste enregistrée). -->
        <button class="icon-btn" onclick={close} aria-label={t("common.c1.stop")} title={t("common.c1.stop")}>
          <Icon name="stop" size={22} filled />
        </button>
        <div class="mini-bar">{@render seekBar()}</div>
        <span class="time end">−{fmt(player.duration - player.position)}</span>
        <div class="pills">{@render pickers(true)}</div>
        <!-- Téléphone : ses touches font le volume, et la vidéo est déjà plein écran. -->
        {#if device.phone}
          <!-- rien de plus -->
        {:else if !device.tv}
          <div class="volume">
            <input type="range" min="0" max="130" value={player.muted ? 0 : player.volume} oninput={setVolume} aria-label={t("player.volume")} />
            <button class="icon-btn" onclick={() => mpv("cycle", "mute")} aria-label={t("player.mute")}>
              <Icon name={player.muted || player.volume === 0 ? "mute" : "volume"} size={20} />
            </button>
          </div>
          <button class="icon-btn" onclick={toggleFullscreen} aria-label={t("player.fullscreen")} title={t("player.fullscreen")}>
            <Icon name={fullscreen ? "exitFullscreen" : "fullscreen"} size={20} />
          </button>
        {/if}
      </div>
    </div>
  </div>
  {:else}
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
            {@render pickers(false)}
          </div>
      </div>

      {@render seekBar()}

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
  {/if}
  {#if player.endScreen && player.upNext}<EndScreen />{/if}
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

  .stalled {
    position: absolute;
    top: 50%;
    left: 50%;
    z-index: 2;
    display: grid;
    gap: 10px;
    width: min(560px, calc(100% - 48px));
    padding: 22px 24px;
    border: 1px solid #ffffff33;
    border-radius: 18px;
    background: #000000cc;
    backdrop-filter: blur(10px);
    text-align: center;
    transform: translate(-50%, -50%);
  }

  .stalled strong {
    font-size: 19px;
  }

  .stalled p {
    margin: 0;
    color: #ffffffbb;
  }

  .stalled-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
    margin-top: 6px;
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

  /* « Passer » : aligné sur le bord droit de la capsule (1100 px au plus, centrée), juste au-dessus des
     commandes quand elles sont affichées, dans le coin en bas à droite sinon. */
  .overlay {
    --corner: 28px;
    --edge: max(var(--corner), calc((100% - 1100px) / 2));
    --above: calc(var(--controls) + 12px);
  }

  .overlay.hidden {
    --above: var(--corner);
    --edge: var(--corner);
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

  /* ---------- Barre minimaliste ---------- */

  .bottom.minimal {
    padding: 0;
  }

  .mini {
    padding: 70px 28px 16px;
    background: linear-gradient(#0000, #000000b8 55%, #000000e0);
  }

  .mini-title {
    margin: 0 0 4px 10px;
    overflow: hidden;
    color: #fff;
    font-size: 14px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mini-title span {
    color: #fffb;
    font-weight: 500;
  }

  .mini-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .mini-bar {
    flex: 1;
    min-width: 80px;
    margin: 0 10px;
  }

  .mini-row .time {
    min-width: 56px;
  }

  .mini-row .pills {
    gap: 2px;
    margin-left: 6px;
  }

  /* Téléphone en portrait : la progression passe sur sa propre ligne, au-dessus des boutons. */
  :global(html.phone) .mini-row {
    flex-wrap: wrap;
  }

  :global(html.phone) .mini-bar {
    order: -1;
    flex-basis: 100%;
    margin: 0 4px 2px;
  }

  :global(html.phone) .mini-row .time {
    margin-left: auto;
  }

  /* Pistes en icônes seules : rondes comme les autres boutons de la rangée. */
  .pill.compact {
    justify-content: center;
    width: 40px;
    height: 40px;
    padding: 0;
    background: none;
  }

  .pill.compact:hover,
  .pill.compact.on {
    background: #ffffff1f;
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
      --corner: 12px;
      --edge: 10px;
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
