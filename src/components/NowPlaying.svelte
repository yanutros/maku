<script lang="ts">
  import { formatClock } from "../lib/api";
  import { current, mpv, next, player, previous, stop } from "../lib/player.svelte";
  import { device } from "../lib/device";
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n";
  import Lyrics from "./Lyrics.svelte";
  import { loadLyrics, lyricsPrefs } from "../lib/lyrics.svelte";

  const track = $derived(current());
  const upNext = $derived(player.queue.slice(player.index + 1, player.index + 8));
  // À droite de la pochette : les paroles (lrclib.net, si la recherche est activée) ou la suite de la file.
  let tab = $state<"lyrics" | "queue">("lyrics");
  const showLyrics = $derived(lyricsPrefs.enabled && tab === "lyrics");
  $effect(() => {
    if (lyricsPrefs.enabled) loadLyrics(track);
  });
</script>

<div class="now-playing">
  <div class="bg" style:background-image={track?.cover ? `url("${track.cover}")` : undefined}></div>

  <button class="close" onclick={() => (player.expanded = false)} aria-label={t("nowPlaying.collapse")}>
    <Icon name="chevronDown" size={26} />
  </button>

  <div class="body">
    <div class="main">
      <div class="cover" class:paused={player.paused}>
        {#if track?.cover}<img src={track.cover} alt="" />{:else}<Icon name="music" size={96} />{/if}
      </div>
      <h1>{track?.title ?? ""}</h1>
      <div class="sub">{[track?.artist, track?.album].filter(Boolean).join(" — ")}</div>
      {#if lyricsPrefs.enabled}
        <div class="tabs" role="tablist">
          <button role="tab" class:on={tab === "lyrics"} aria-selected={tab === "lyrics"} onclick={() => (tab = "lyrics")}>{t("lyrics.title")}</button>
          <button role="tab" class:on={tab === "queue"} aria-selected={tab === "queue"} onclick={() => (tab = "queue")}>{t("nowPlaying.upNext")}</button>
        </div>
      {/if}

      <!-- Téléphone : lecteur complet en plein écran (le mini-lecteur et les onglets sont dessous). -->
      {#if device.phone}
        <div class="controls">
          <input
            class="seek"
            type="range"
            min="0"
            max={Math.max(1, player.duration)}
            step="1"
            value={player.position}
            style:--p="{player.duration ? (player.position / player.duration) * 100 : 0}%"
            onchange={(e) => mpv("seek", Number(e.currentTarget.value), "absolute")}
            aria-label={t("common.c1.position")}
          />
          <div class="times">
            <span>{formatClock(player.position)}</span>
            <span>-{formatClock(Math.max(0, player.duration - player.position))}</span>
          </div>
          <div class="buttons">
            <button class="ctl" class:on={player.repeat} onclick={() => mpv("set_property", "loop-playlist", player.repeat ? "no" : "inf")} aria-label={t("musicBar.repeatQueue")}>
              <Icon name="repeat" size={20} />
            </button>
            <button class="ctl" onclick={previous} aria-label={t("common.c1.previous")}><Icon name="backward" size={30} filled /></button>
            <button class="ctl" onclick={() => mpv("cycle", "pause")} aria-label={player.paused ? t("common.c1.playback") : t("common.c1.pause")}>
              <Icon name={player.paused ? "play" : "pause"} size={30} filled />
            </button>
            <button class="ctl" onclick={next} aria-label={t("common.c1.next")}><Icon name="forward" size={30} filled /></button>
            <button class="ctl" onclick={stop} aria-label={t("common.c1.stop")}><Icon name="stop" size={28} filled /></button>
            <button class="ctl" onclick={() => mpv("cycle", "mute")} aria-label={t("common.c1.mute")}>
              <Icon name={player.muted ? "mute" : "volume"} size={20} />
            </button>
          </div>
        </div>
      {/if}
    </div>

    {#if showLyrics}
      <aside class="lyr"><Lyrics /></aside>
    {:else if upNext.length}
      <aside class="next">
        <div class="next-title">{t("nowPlaying.upNext")}</div>
        {#each upNext as qt, i (qt.ratingKey + i)}
          <button class="q" onclick={() => mpv("set_property", "playlist-pos", player.index + 1 + i)}>
            {#if qt.cover}<img src={qt.cover} alt="" loading="lazy" />{/if}
            <span class="q-text">
              <span class="q-title">{qt.title}</span>
              <span class="q-sub">{qt.artist ?? ""}</span>
            </span>
            <span class="q-dur">{formatClock(qt.durationMs / 1000)}</span>
          </button>
        {/each}
      </aside>
    {/if}
  </div>
</div>

<style>
  .now-playing {
    position: fixed;
    /* Le fond couvre toute la fenêtre, menu compris (il flotte par-dessus, voir Sidebar) ; le contenu se
       cale sur la colonne de droite, au-dessus de la barre musique. */
    inset: 0;
    padding: 0 0 106px var(--sidebar-w, 237px);
    z-index: 25;
    overflow: hidden;
    background: #0d0e10;
    animation: rise 0.3s var(--ease);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(24px);
    }
  }

  .bg {
    position: absolute;
    inset: -80px;
    background-size: cover;
    background-position: center;
    filter: blur(70px) saturate(1.5) brightness(0.45);
    transform: scale(1.1);
  }

  .close {
    position: absolute;
    z-index: 1;
    top: 18px;
    left: calc(var(--sidebar-w, 237px) + 18px);
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border: none;
    border-radius: 50%;
    background: #0004;
    color: #fff;
  }

  .close:hover {
    background: #fff2;
  }

  .body {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 64px;
    height: 100%;
    padding: 40px;
  }

  .main {
    display: flex;
    flex-direction: column;
    align-items: center;
    min-width: 0;
    text-align: center;
  }

  .cover {
    display: grid;
    place-items: center;
    width: min(46vh, 460px);
    aspect-ratio: 1;
    overflow: hidden;
    border-radius: 12px;
    background: var(--bg-elev);
    color: var(--text-faint);
    box-shadow: 0 30px 80px #000c;
    transition: transform 0.4s var(--ease);
  }

  .cover.paused {
    transform: scale(0.94);
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  h1 {
    margin: 28px 0 4px;
    font-size: 28px;
  }

  .sub {
    color: #fffb;
    font-size: 16px;
  }

  .next {
    width: 340px;
    max-height: 70vh;
    overflow-y: auto;
  }

  .next-title {
    margin-bottom: 8px;
    color: #fff9;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  .q {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 6px 8px;
    border: none;
    border-radius: 8px;
    background: none;
    text-align: left;
    color: #fff;
  }

  .q:hover {
    background: #ffffff14;
  }

  .q img {
    width: 44px;
    height: 44px;
    border-radius: 4px;
    object-fit: cover;
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
    color: #fff9;
    font-size: 12px;
  }

  .tabs {
    display: flex;
    gap: 8px;
    margin-top: 18px;
  }

  .tabs button {
    padding: 6px 16px;
    border: 1px solid #ffffff40;
    border-radius: 999px;
    background: none;
    color: #ffffffcc;
    font: inherit;
    font-size: 13px;
    font-weight: 600;
  }

  .tabs button.on {
    border-color: transparent;
    background: var(--accent);
    color: var(--accent-text);
  }

  .lyr {
    width: min(460px, 40vw);
    height: min(64vh, 620px);
  }

  @media (max-width: 1100px) {
    .next {
      display: none;
    }
  }

  /* ---------- Téléphone : plein écran, pochette à la largeur, commandes dessous ---------- */

  :global(html.phone) .now-playing {
    inset: 0;
    padding: 0;
    z-index: 60;
    overflow-y: auto;
  }

  :global(html.phone) .close {
    top: calc(var(--safe-top) + 8px);
    left: 12px;
  }

  :global(html.phone) .body {
    flex-direction: column;
    justify-content: flex-start;
    gap: 28px;
    height: auto;
    min-height: 100%;
    padding: calc(var(--safe-top) + 68px) 24px calc(var(--safe-bottom) + 24px);
  }

  :global(html.phone) .main {
    width: 100%;
  }

  :global(html.phone) .cover {
    width: min(100%, 52vh);
  }

  :global(html.phone) h1 {
    margin-top: 22px;
    font-size: 22px;
  }

  :global(html.phone) .sub {
    font-size: 14px;
  }

  :global(html.phone) .next {
    width: 100%;
    max-height: none;
  }

  :global(html.phone) .lyr {
    width: 100%;
    height: 50vh;
  }

  .controls {
    width: 100%;
    margin-top: 22px;
  }

  /* Curseur dessiné à la main (le style d'Android le rend sinon épais et décalé). */
  .seek {
    display: block;
    width: 100%;
    height: 18px;
    margin: 0;
    padding: 0;
    border: none;
    background: none;
    -webkit-appearance: none;
    appearance: none;
  }

  .seek::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 2px;
    background: linear-gradient(90deg, #fff var(--p), #ffffff40 var(--p));
  }

  .seek::-webkit-slider-thumb {
    width: 14px;
    height: 14px;
    margin-top: -5px;
    border: none;
    border-radius: 50%;
    background: #fff;
    -webkit-appearance: none;
    appearance: none;
  }

  .times {
    display: flex;
    justify-content: space-between;
    margin-top: 6px;
    color: #fffa;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .buttons {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 14px;
  }

  .ctl {
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: #fff;
  }

  .ctl.on {
    color: var(--accent);
  }
</style>
