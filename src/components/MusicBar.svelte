<script lang="ts">
  import { formatClock } from "../lib/api";
  import { go } from "../lib/nav.svelte";
  import { current, mpv, next, player, previous, stop } from "../lib/player.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n";
  import { device } from "../lib/device";

  let showQueue = $state(false);
  let bar = $state<HTMLElement>();
  let dragging = $state(false);
  let hoverTime = $state<number | null>(null);
  let hoverX = $state(0);

  const track = $derived(current());
  const progress = $derived(player.duration ? Math.min(100, (player.position / player.duration) * 100) : 0);

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

  function openAlbum() {
    if (!track?.albumKey) return;
    player.expanded = false;
    go({ name: "item", ratingKey: track.albumKey });
  }

  function openArtist() {
    if (!track?.artistKey) return;
    player.expanded = false;
    go({ name: "item", ratingKey: track.artistKey });
  }

  // Raccourcis globaux tant qu'on ne tape pas dans un champ.
  function onKey(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    if (target.closest("input, select, textarea")) return;
    switch (e.key) {
      case " ":
        mpv("cycle", "pause");
        break;
      case "MediaPlayPause":
        mpv("cycle", "pause");
        break;
      case "MediaTrackNext":
        next();
        break;
      case "MediaTrackPrevious":
        previous();
        break;
      case "Escape":
        if (showQueue) showQueue = false;
        else if (player.expanded) player.expanded = false;
        else return;
        break;
      default:
        return;
    }
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="dock">
  <div class="bar" class:playing={!player.paused}>
    <!-- Halo coloré : la pochette en cours, très floutée, derrière le verre dépoli. -->
    {#if track?.cover}<div class="ambient" style:background-image={`url("${track.cover}")`}></div>{/if}

    <div class="now">
      <button class="cover" onclick={() => (player.expanded = !player.expanded)} aria-label={t("common.c1.nowPlaying")} title={t("common.c1.nowPlaying")}>
        {#if track?.cover}<img src={track.cover} alt="" />{:else}<Icon name="music" size={24} />{/if}
        <span class="expand"><Icon name={player.expanded ? "chevronDown" : "expand"} size={18} /></span>
      </button>
      <!-- Téléphone : comme Spotify, toucher le titre ouvre le lecteur plein écran. -->
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="meta" onclick={() => device.phone && (player.expanded = true)}>
        <div class="title">{track?.title ?? t("common.c1.loading")}</div>
        <div class="sub">
          {#if track?.artist}<button class="link" onclick={openArtist}>{track.artist}</button>{/if}
          {#if track?.album}<span class="dot">·</span><button class="link" onclick={openAlbum}>{track.album}</button>{/if}
        </div>
      </div>
    </div>

    <div class="mid">
      <div class="transport">
        <button
          class="ctl small"
          class:on={player.repeat}
          onclick={() => mpv("set_property", "loop-playlist", player.repeat ? "no" : "inf")}
          aria-label={t("musicBar.repeatQueue")}
          title={t("musicBar.repeatQueue")}
        >
          <Icon name="repeat" size={20} />
        </button>
        <button class="ctl" onclick={previous} aria-label={t("common.c1.previous")} title={t("common.c1.previous")}>
          <Icon name="backward" size={24} filled />
        </button>
        <button class="ctl play" class:paused={player.paused} onclick={() => mpv("cycle", "pause")} aria-label={player.paused ? t("common.c1.playback") : t("common.c1.pause")}>
          <Icon name={player.paused ? "play" : "pause"} size={24} filled />
        </button>
        <button
          class="ctl"
          onclick={next}
          disabled={player.index >= player.queue.length - 1 && !player.repeat}
          aria-label={t("common.c1.next")}
          title={t("common.c1.next")}
        >
          <Icon name="forward" size={24} filled />
        </button>
        <button class="ctl small" onclick={stop} aria-label={t("common.c1.stop")} title={t("common.c1.stop")}><Icon name="stop" size={18} filled /></button>
      </div>

      <div class="scrub">
        <span class="t">{formatClock(player.position)}</span>
        <div
          class="seek"
          class:dragging
          bind:this={bar}
          onpointerdown={onBarDown}
          onpointermove={onBarMove}
          onpointerup={onBarUp}
          onpointerleave={() => (hoverTime = null)}
          role="slider"
          tabindex="-1"
          aria-label={t("common.c1.position")}
          aria-valuemin={0}
          aria-valuemax={player.duration}
          aria-valuenow={player.position}
        >
          <div class="line"><div class="played" style:width="{progress}%"></div></div>
          <div class="knob" style:left="{progress}%"></div>
          {#if hoverTime !== null}<div class="hover-time" style:left="{hoverX}px">{formatClock(hoverTime)}</div>{/if}
        </div>
        <span class="t">-{formatClock(Math.max(0, player.duration - player.position))}</span>
      </div>
    </div>

    <div class="extras">
      <button class="ctl small" class:on={showQueue} onclick={() => (showQueue = !showQueue)} aria-label={t("common.c1.queue")} title={t("common.c1.queue")}>
        <Icon name="queue" size={19} />
      </button>
      <div class="volume">
        <button class="ctl small" onclick={() => mpv("cycle", "mute")} aria-label={t("common.c1.mute")} title={t("common.c1.mute")}>
          <Icon name={player.muted || player.volume === 0 ? "mute" : "volume"} size={19} />
        </button>
        <input
          type="range"
          min="0"
          max="130"
          value={player.muted ? 0 : player.volume}
          style:--v="{((player.muted ? 0 : player.volume) / 130) * 100}%"
          oninput={setVolume}
          aria-label={t("common.c1.volume")}
        />
      </div>
      <button class="ctl small" onclick={() => (player.expanded = !player.expanded)} aria-label={t("common.c1.nowPlaying")} title={t("common.c1.nowPlaying")}>
        <Icon name={player.expanded ? "chevronDown" : "expand"} size={17} />
      </button>
    </div>
  </div>

  {#if showQueue}
    <div class="queue">
      <div class="queue-head">
        {t("common.c1.queue")} <span class="muted">{player.index + 1} / {player.queue.length}</span>
      </div>
      <div class="queue-list">
        {#each player.queue as qt, i (qt.ratingKey + i)}
          <button class="q" class:current={i === player.index} class:past={i < player.index} onclick={() => mpv("set_property", "playlist-pos", i)}>
            {#if qt.cover}<img src={qt.cover} alt="" loading="lazy" />{:else}<span class="ph"></span>{/if}
            <span class="q-text">
              <span class="q-title">{qt.title}</span>
              <span class="q-sub">{qt.artist ?? ""}</span>
            </span>
            <span class="q-dur">{formatClock(qt.durationMs / 1000)}</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  /* La barre, une capsule de verre sous le contenu (sans fond autour : la vue « En cours de lecture »
     passe derrière elle). */
  .dock {
    position: relative;
    z-index: 30;
    padding: 8px 14px 12px;
  }

  .bar {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(320px, 1.35fr) minmax(0, 1fr);
    align-items: center;
    gap: 20px;
    height: 84px;
    padding: 0 18px 0 12px;
    overflow: hidden;
    border: 1px solid #ffffff1c;
    border-radius: 22px;
    background: #1c1e23d0;
    backdrop-filter: blur(30px) saturate(1.6);
    box-shadow:
      0 18px 40px #0009,
      inset 0 1px 0 #ffffff14;
  }

  .ambient {
    position: absolute;
    inset: -60px;
    background-size: cover;
    background-position: center;
    filter: blur(50px) saturate(1.8);
    opacity: 0.32;
    pointer-events: none;
    transition: opacity 0.6s;
  }

  .bar:not(.playing) .ambient {
    opacity: 0.18;
  }

  .now,
  .mid,
  .extras {
    position: relative;
  }

  /* ---------- Titre en cours ---------- */

  .now {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }

  .cover {
    position: relative;
    display: grid;
    place-items: center;
    flex: none;
    width: 62px;
    height: 62px;
    padding: 0;
    overflow: hidden;
    border: none;
    border-radius: 12px;
    background: var(--bg-elev);
    color: var(--text-faint);
    box-shadow: 0 8px 20px #0009;
    transition: transform 0.25s var(--ease);
  }

  .bar.playing .cover {
    transform: scale(1.04);
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .expand {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: #0009;
    color: #fff;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .cover:hover .expand {
    opacity: 1;
  }

  .meta {
    min-width: 0;
    line-height: 1.35;
  }

  .title,
  .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    font-size: 15px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }

  .sub {
    color: #ffffffa8;
    font-size: 13px;
  }

  .dot {
    margin: 0 6px;
  }

  .link {
    padding: 0;
    border: none;
    background: none;
    color: inherit;
  }

  .link:hover {
    color: #fff;
    text-decoration: underline;
  }

  /* ---------- Commandes et progression ---------- */

  .mid {
    display: grid;
    gap: 2px;
  }

  .transport {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 14px;
  }

  .ctl {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: #fff;
    transition:
      transform 0.15s var(--ease),
      background 0.15s,
      color 0.15s,
      opacity 0.15s;
  }

  .ctl:hover:not(:disabled) {
    background: #ffffff1a;
    transform: scale(1.08);
  }

  .ctl:active:not(:disabled) {
    transform: scale(0.92);
  }

  .ctl:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .ctl.small {
    width: 34px;
    height: 34px;
    color: #ffffffb0;
  }

  .ctl.small:hover {
    color: #fff;
  }

  /* Les cinq boutons de lecture (répétition, précédent, lecture, suivant, arrêt) : même cercle, même
     couleur, icônes de même taille. */
  .transport .ctl.small {
    width: 40px;
    height: 40px;
    color: #fff;
  }

  .transport .ctl.on {
    color: var(--accent);
  }

  /* Bouton actif (répétition, file d'attente) : couleur d'accent et petit point dessous. */
  .ctl.on {
    position: relative;
    color: var(--accent);
  }

  .ctl.on::after {
    content: "";
    position: absolute;
    bottom: 2px;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--accent);
  }

  /* Sur ordinateur, le bouton lecture est un bouton comme les autres (`ctl`) ; le téléphone garde son
     disque blanc (plus bas). */
  :global(html.phone) .play {
    background: #fff;
    color: #0d0e10;
  }

  .scrub {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) 48px;
    align-items: center;
    gap: 10px;
  }

  .t {
    color: #ffffff99;
    font-size: 11px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .t:first-child {
    text-align: right;
  }

  .seek {
    position: relative;
    height: 16px;
    display: flex;
    align-items: center;
    cursor: pointer;
    touch-action: none;
  }

  .line {
    width: 100%;
    height: 4px;
    overflow: hidden;
    border-radius: 2px;
    background: #ffffff2e;
    transition: height 0.15s;
  }

  .seek:hover .line,
  .seek.dragging .line {
    height: 6px;
  }

  .played {
    height: 100%;
    border-radius: 2px;
    background: linear-gradient(90deg, #ffffffcc, #fff);
  }

  .seek:hover .played,
  .seek.dragging .played {
    background: linear-gradient(90deg, var(--accent), var(--accent-hover));
  }

  .knob {
    position: absolute;
    width: 14px;
    height: 14px;
    margin-left: -7px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 2px 8px #0009;
    transform: scale(0);
    transition: transform 0.15s var(--ease);
  }

  .seek:hover .knob,
  .seek.dragging .knob {
    transform: scale(1);
  }

  .hover-time {
    position: absolute;
    bottom: 20px;
    transform: translateX(-50%);
    padding: 3px 8px;
    border-radius: 6px;
    background: #000d;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    pointer-events: none;
  }

  /* ---------- File d'attente, volume ---------- */

  .extras {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
  }

  .volume {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .volume input {
    appearance: none;
    width: 92px;
    height: 4px;
    padding: 0;
    border: none;
    border-radius: 2px;
    background: linear-gradient(90deg, #fff var(--v), #ffffff2e var(--v));
    cursor: pointer;
  }

  .volume input::-webkit-slider-thumb {
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 2px 6px #0009;
    transform: scale(0.8);
    transition: transform 0.15s;
  }

  .volume:hover input::-webkit-slider-thumb {
    transform: scale(1.1);
  }

  .volume:hover input {
    background: linear-gradient(90deg, var(--accent) var(--v), #ffffff2e var(--v));
  }

  .queue {
    position: absolute;
    right: 24px;
    bottom: calc(100% - 2px);
    width: 380px;
    max-height: min(60vh, 560px);
    display: flex;
    flex-direction: column;
    border: 1px solid #ffffff1a;
    border-radius: 18px;
    background: #16181cf2;
    backdrop-filter: blur(24px);
    box-shadow: 0 20px 48px #000b;
    animation: rise 0.18s var(--ease);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }

  .queue-head {
    padding: 14px 16px 10px;
    font-weight: 600;
  }

  .queue-list {
    overflow-y: auto;
    padding: 0 8px 8px;
  }

  .q {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 6px 8px;
    border: none;
    border-radius: 10px;
    background: none;
    text-align: left;
  }

  .q:hover {
    background: #ffffff12;
  }

  .q.past {
    opacity: 0.5;
  }

  .q.current .q-title {
    color: var(--accent);
  }

  .q img,
  .ph {
    width: 40px;
    height: 40px;
    border-radius: 6px;
    object-fit: cover;
    background: var(--bg-elev);
  }

  .q-text {
    display: grid;
    min-width: 0;
  }

  .q-title,
  .q-sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .q-sub,
  .q-dur {
    color: var(--text-dim);
    font-size: 12px;
  }

  .q-dur {
    font-variant-numeric: tabular-nums;
  }

  @media (max-width: 1100px) {
    .volume input {
      display: none;
    }

    .bar {
      grid-template-columns: minmax(0, 1fr) minmax(280px, 1.4fr) auto;
    }
  }

  /* ---------- Téléphone : mini-lecteur (pochette, titre, précédent / lecture / suivant) ---------- */

  :global(html.phone) .dock {
    padding: 6px 8px;
  }

  :global(html.phone) .bar {
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 6px;
    height: 64px;
    padding: 0 6px 0 8px;
    border-radius: 14px;
  }

  :global(html.phone) .extras,
  :global(html.phone) .ctl.small,
  :global(html.phone) .expand,
  :global(html.phone) .scrub .t,
  :global(html.phone) .knob {
    display: none;
  }

  :global(html.phone) .cover {
    width: 46px;
    height: 46px;
    border-radius: 9px;
  }

  :global(html.phone) .play {
    width: 42px;
    height: 42px;
  }

  :global(html.phone) .transport {
    gap: 2px;
  }

  /* Comme Spotify : la progression est un filet collé au bas de la carte, qui ne se touche pas
     (on avance dans le morceau depuis le lecteur plein écran, en touchant la pochette). */
  :global(html.phone) .scrub {
    position: absolute;
    right: 10px;
    bottom: 0;
    left: 10px;
    display: block;
    pointer-events: none;
  }

  /* Le filet s'aligne sur toute la carte, pas sur le bloc des boutons. */
  :global(html.phone) .mid {
    position: static;
  }

  :global(html.phone) .seek {
    height: 2px;
  }

  :global(html.phone) .line {
    height: 2px;
    border-radius: 1px;
  }

  /* Mini-lecteur : lecture et suivant seulement (précédent est dans le lecteur plein écran). */
  :global(html.phone) .transport > .ctl:nth-child(2) {
    display: none;
  }

  :global(html.phone) .meta {
    min-width: 0;
  }

  :global(html.phone) .meta .link {
    pointer-events: none;
  }
</style>
