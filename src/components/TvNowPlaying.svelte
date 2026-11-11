<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { formatClock } from "../lib/api";
  import { current, mpv, next, player, previous, stop, cycleRepeat, toggleShuffle, repeatLabel } from "../lib/player.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n";
  import Lyrics from "./Lyrics.svelte";
  import { loadLyrics, lyricsPrefs } from "../lib/lyrics.svelte";

  /**
   * « En cours de lecture » des téléviseurs, comme dans Plex : la pochette en grand, le titre, puis les
   * commandes ; au repos, seules la pochette et le titre restent. Bas descend sur la file d'attente, qui
   * remonte d'un bloc. Le fond prend les couleurs de la pochette à partir d'une image minuscule agrandie :
   * l'effet d'un flou, sans le calcul d'un flou. Les flèches sont gérées ici (`data-nav-grid`), tout
   * bouge par translation.
   */
  const track = $derived(current());
  const queue = $derived(player.queue);

  // Pochette réduite à 24 px par le serveur (Plex ou Jellyfin) : agrandie, elle donne un dégradé.
  const tiny = (url: string | null | undefined) =>
    url ? url.replace(/width=\d+&height=\d+/, "width=24&height=24").replace(/maxWidth=\d+/, "maxWidth=24") : null;
  const backdrop = $derived(tiny(track?.cover));

  let root = $state<HTMLElement>();
  let zone = $state<"controls" | "queue">("controls");
  let button = $state(3);
  let qi = $state(0);
  let idle = $state(false);
  let idleTimer: ReturnType<typeof setTimeout> | undefined;

  // Paroles (lrclib.net, si la recherche est activée) : un sixième bouton les ouvre à droite de la pochette.
  let showLyrics = $state(false);
  $effect(() => {
    if (lyricsPrefs.enabled) loadLyrics(track);
  });
  const BUTTONS = $derived(lyricsPrefs.enabled ? 7 : 6);
  const progress = $derived(player.duration ? Math.min(100, (player.position / player.duration) * 100) : 0);

  function wake() {
    idle = false;
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => {
      if (zone === "controls") idle = true;
    }, 6000);
  }

  function focusButton(i: number) {
    button = i;
    root?.querySelector<HTMLElement>(`[data-ctl="${i}"]`)?.focus({ preventScroll: true });
  }

  function focusQueue(i: number) {
    qi = Math.max(0, Math.min(queue.length - 1, i));
    root?.querySelector<HTMLElement>(`[data-q="${qi}"]`)?.focus({ preventScroll: true });
  }

  function onKey(e: KeyboardEvent) {
    const arrow = e.key.startsWith("Arrow");
    // Au repos, la première touche ne fait que réafficher les commandes.
    if (idle && (arrow || e.key === "Enter")) {
      e.preventDefault();
      e.stopPropagation();
      wake();
      return;
    }
    wake();
    if (!arrow) return;
    e.preventDefault();
    if (zone === "controls") {
      if (e.key === "ArrowLeft") focusButton(Math.max(0, button - 1));
      else if (e.key === "ArrowRight") focusButton(Math.min(BUTTONS - 1, button + 1));
      else if (e.key === "ArrowDown" && queue.length > 1) {
        zone = "queue";
        focusQueue(player.index + 1 < queue.length ? player.index + 1 : player.index);
      }
    } else {
      if (e.key === "ArrowLeft") focusQueue(qi - 1);
      else if (e.key === "ArrowRight") focusQueue(qi + 1);
      else if (e.key === "ArrowUp") {
        zone = "controls";
        focusButton(button);
      }
    }
  }

  onMount(() => {
    focusButton(3);
    wake();
  });

  onDestroy(() => clearTimeout(idleTimer));
</script>

<div class="tv-np" class:idle class:queue-open={zone === "queue"} class:lyrics-open={showLyrics && lyricsPrefs.enabled} bind:this={root} role="dialog" tabindex="-1" aria-label={t("common.c1.nowPlaying")} data-nav-grid onkeydown={onKey}>
  {#key backdrop}
    {#if backdrop}<div class="bg" style:background-image={`url("${backdrop}")`}></div>{/if}
  {/key}
  <div class="shade"></div>

  <div class="sheet">
    <div class="stage">
      <div class="cover">
        {#if track?.cover}<img src={track.cover} alt="" />{:else}<Icon name="music" size={96} />{/if}
      </div>
      <h1>{track?.title ?? ""}</h1>
      <div class="sub">{[track?.artist, track?.album].filter(Boolean).join("  •  ")}</div>
    </div>
    {#if showLyrics && lyricsPrefs.enabled}<div class="lyr"><Lyrics tv /></div>{/if}

    <div class="controls">
      <div class="bar">
        <span class="time">{formatClock(player.position)}</span>
        <div class="track"><div class="played" style:width="{progress}%"></div></div>
        <span class="time">-{formatClock(Math.max(0, player.duration - player.position))}</span>
      </div>
      <div class="buttons">
        <button data-ctl="0" class="ctl" class:on={player.shuffled} onclick={toggleShuffle} aria-pressed={player.shuffled} aria-label={t("musicBar.shuffle")}>
          <Icon name="shuffle" size={21} />
        </button>
        <button data-ctl="1" class="ctl" class:on={player.repeat || player.repeatOne} onclick={cycleRepeat} aria-label={repeatLabel()}>
          <Icon name={player.repeatOne ? "repeatOne" : "repeat"} size={22} />
        </button>
        <button data-ctl="2" class="ctl" onclick={previous} aria-label={t("common.c1.previous")}><Icon name="backward" size={26} filled /></button>
        <button data-ctl="3" class="ctl" onclick={() => mpv("cycle", "pause")} aria-label={player.paused ? t("common.c1.playback") : t("common.c1.pause")}>
          <Icon name={player.paused ? "play" : "pause"} size={26} filled />
        </button>
        <button data-ctl="4" class="ctl" onclick={next} aria-label={t("common.c1.next")}><Icon name="forward" size={26} filled /></button>
        <!-- Arrêter : la lecture s'arrête et le lecteur se ferme, la vignette disparaît avec lui. -->
        <button data-ctl="5" class="ctl" onclick={stop} aria-label={t("common.c1.stop")}><Icon name="stop" size={20} filled /></button>
        {#if lyricsPrefs.enabled}
          <button data-ctl="6" class="ctl" class:on={showLyrics} onclick={() => (showLyrics = !showLyrics)} aria-label={t("lyrics.title")} aria-pressed={showLyrics}>
            <Icon name="lyrics" size={22} />
          </button>
        {/if}
      </div>
      {#if queue.length > 1}<div class="hint"><Icon name="chevronDown" size={20} /></div>{/if}
    </div>

    {#if queue.length > 1}
      <section class="queue">
        <h2>{t("common.c1.queue")}</h2>
        <div class="viewport">
          <div class="row" style:transform="translateX(calc({-Math.max(0, qi - 1)} * var(--q-step)))">
            {#each queue as q, i (q.ratingKey + i)}
              <button data-q={i} class="q" class:now={i === player.index} tabindex="-1" onclick={() => mpv("set_property", "playlist-pos", i)}>
                <span class="q-cover">
                  {#if q.cover}<img src={q.cover} alt="" loading="lazy" decoding="async" />{/if}
                  {#if i === player.index}<span class="q-now"><Icon name={player.paused ? "pause" : "play"} size={22} filled /></span>{/if}
                </span>
                <span class="q-title">{q.title}</span>
                <span class="q-sub">{q.artist ?? ""}</span>
              </button>
            {/each}
          </div>
        </div>
      </section>
    {/if}
  </div>
</div>

<style>
  .tv-np {
    position: fixed;
    inset: 0;
    z-index: 40;
    overflow: hidden;
    background: #0d0e10;
    animation: appear 0.3s var(--ease);
  }

  @keyframes appear {
    from {
      opacity: 0;
    }
  }

  .bg {
    position: absolute;
    inset: 0;
    background-size: cover;
    background-position: center;
    animation: appear 0.6s var(--ease);
  }

  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, #00000059 0%, #0000008c 55%, #000000d9 100%);
  }

  /* Tout le contenu monte d'un bloc quand la file d'attente prend la main. */
  .sheet {
    position: relative;
    height: 100%;
    transition: transform 0.35s var(--ease);
  }

  .queue-open .sheet {
    transform: translateY(-250px);
  }

  .stage {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding-top: 56px;
    text-align: center;
  }

  /* Paroles ouvertes : la pochette et le titre à gauche, les paroles à droite. */
  .lyrics-open .stage {
    position: absolute;
    top: 0;
    left: 64px;
    width: 400px;
    align-items: flex-start;
    text-align: left;
  }

  .lyrics-open .cover {
    width: 300px;
    height: 300px;
  }

  .lyrics-open h1 {
    max-width: 400px;
    font-size: 28px;
  }

  .lyr {
    position: absolute;
    top: 36px;
    left: 520px;
    right: 64px;
    height: 480px;
  }

  .cover {
    display: grid;
    place-items: center;
    width: 330px;
    height: 330px;
    overflow: hidden;
    border-radius: 14px;
    background: #ffffff14;
    color: #fff6;
    box-shadow: 0 24px 60px #000a;
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  h1 {
    max-width: 900px;
    margin: 26px 0 6px;
    overflow: hidden;
    font-size: 34px;
    font-weight: 800;
    letter-spacing: -0.01em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    color: #ffffffc0;
    font-size: 18px;
  }

  .controls {
    position: absolute;
    top: 545px;
    left: 64px;
    right: 64px;
    transition: opacity 0.3s;
  }

  .idle .controls {
    opacity: 0;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .time {
    min-width: 54px;
    color: #ffffffc0;
    font-size: 15px;
    font-variant-numeric: tabular-nums;
  }

  .time:last-child {
    text-align: right;
  }

  .track {
    flex: 1;
    height: 5px;
    overflow: hidden;
    border-radius: 3px;
    background: #ffffff33;
  }

  .played {
    height: 100%;
    background: var(--accent);
  }

  .buttons {
    display: flex;
    justify-content: center;
    gap: 18px;
    margin-top: 18px;
  }

  .ctl {
    display: grid;
    place-items: center;
    width: 56px;
    height: 56px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: #ffffff1f;
    color: #fff;
    transition: background 0.15s, transform 0.15s;
  }

  .ctl.on {
    color: var(--accent);
  }

  /* Sélection à la télécommande : le bouton se remplit, sans contour. */
  .ctl:focus {
    outline: none;
    background: #fff;
    color: #111;
    transform: scale(1.08);
  }

  .hint {
    display: grid;
    place-items: center;
    margin-top: 6px;
    color: #ffffff80;
    transition: opacity 0.3s;
  }

  .queue-open .hint {
    opacity: 0;
  }

  .queue {
    position: absolute;
    top: 720px;
    left: 0;
    right: 0;
    --q-w: 150px;
    --q-step: 172px;
    transition: opacity 0.3s;
  }

  .idle .queue {
    opacity: 0;
  }

  .queue h2 {
    margin: 0 0 4px 64px;
    font-size: 20px;
    font-weight: 700;
  }

  /* Assez de marge pour la pochette sélectionnée, agrandie et entourée : la rangée coupe ce qui dépasse. */
  .viewport {
    overflow: hidden;
    padding: 16px 0 12px;
  }

  .row {
    display: flex;
    gap: calc(var(--q-step) - var(--q-w));
    padding-left: 64px;
    transition: transform 0.3s var(--ease);
  }

  .q {
    display: grid;
    flex: none;
    gap: 4px;
    width: var(--q-w);
    padding: 0;
    border: none;
    background: none;
    color: #fff;
    text-align: left;
  }

  .q:focus {
    outline: none;
  }

  .q-cover {
    position: relative;
    display: block;
    width: var(--q-w);
    height: var(--q-w);
    overflow: hidden;
    border-radius: 10px;
    background: #ffffff14;
    transition: transform 0.15s;
  }

  .q:focus .q-cover {
    outline: 3px solid #fff;
    outline-offset: 3px;
    transform: scale(1.05);
  }

  .q-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .q-now {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: #0008;
    color: #fff;
  }

  .q-title,
  .q-sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .q-title {
    margin-top: 6px;
    font-size: 15px;
    font-weight: 600;
  }

  .q.now .q-title {
    color: var(--accent);
  }

  .q-sub {
    color: #ffffffa0;
    font-size: 13px;
  }
</style>
