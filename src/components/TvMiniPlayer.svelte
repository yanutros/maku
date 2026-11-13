<script lang="ts">
  import { formatClock } from "../lib/api";
  import { t } from "../lib/i18n";
  import { onBack } from "../lib/nav.svelte";
  import { current, mpv, next, player, stop } from "../lib/player.svelte";
  import Icon from "./Icon.svelte";

  /**
   * Téléviseur : le titre en cours pendant qu'on parcourt les pages (la barre musique de l'ordinateur, faite
   * pour la souris, n'a pas sa place ici). Flèche bas tout en bas d'une page : la sélection y entre (lib/spatial),
   * sur lecture / pause ; gauche / droite parcourent ses commandes (ouvrir le lecteur, lecture / pause, suivant,
   * arrêter) ; haut ou Retour rendent la sélection là où elle était.
   */
  const track = $derived(current());
  const progress = $derived(player.duration ? Math.min(100, (player.position / player.duration) * 100) : 0);

  let root = $state<HTMLElement>();
  /** L'élément de la page d'où l'on est entré, pour y revenir. */
  let from: HTMLElement | null = null;

  function onFocusIn(e: FocusEvent) {
    const before = e.relatedTarget as HTMLElement | null;
    if (before && !root?.contains(before)) from = before;
  }

  /** Rend la sélection à la page. False : elle n'était pas dans le lecteur. */
  function leave(): boolean {
    if (!root?.contains(document.activeElement)) return false;
    if (from?.isConnected) from.focus({ preventScroll: true });
    else (document.activeElement as HTMLElement | null)?.blur();
    return true;
  }

  $effect(() => onBack(leave));

  function onKey(e: KeyboardEvent) {
    const buttons = [...(root?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
    const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      buttons[i + (e.key === "ArrowRight" ? 1 : -1)]?.focus({ preventScroll: true });
    } else if (e.key === "ArrowUp" || e.key === "Escape") {
      e.preventDefault();
      leave();
    } else if (e.key === "ArrowDown") e.preventDefault();
  }

  function stopAll() {
    leave();
    stop();
  }
</script>

{#if track}
  <!-- data-nav-grid : le lecteur gère lui-même ses flèches. -->
  <div class="tv-mini" role="toolbar" tabindex="-1" aria-label={t("common.c1.nowPlaying")} data-nav-grid bind:this={root} onfocusin={onFocusIn} onkeydown={onKey}>
    <button class="info" onclick={() => (player.expanded = true)} aria-label={t("common.c1.nowPlaying")}>
      <span class="cover">
        {#if track.cover}<img src={track.cover} alt="" />{:else}<Icon name="music" size={22} />{/if}
      </span>
      <span class="text">
        <span class="title">{track.title}</span>
        <span class="sub">{track.artist ?? ""}</span>
        <span class="time">{formatClock(player.position)}{player.duration ? ` / ${formatClock(player.duration)}` : ""}</span>
      </span>
    </button>
    <span class="ctls">
      <button class="ctl" data-mini-default onclick={() => mpv("cycle", "pause")} aria-label={player.paused ? t("common.c1.playback") : t("common.c1.pause")}>
        <Icon name={player.paused ? "play" : "pause"} size={20} filled />
      </button>
      <button class="ctl" onclick={next} aria-label={t("common.c1.next")}><Icon name="forward" size={20} filled /></button>
      <button class="ctl" onclick={stopAll} aria-label={t("common.c1.stop")}><Icon name="stop" size={16} filled /></button>
    </span>
    <span class="progress"><span style:width="{progress}%"></span></span>
  </div>
{/if}

<style>
  .tv-mini {
    position: fixed;
    right: 24px;
    bottom: 20px;
    z-index: 20;
    display: flex;
    align-items: center;
    gap: 6px;
    width: 420px;
    padding: 8px 10px 11px 8px;
    overflow: hidden;
    border: 1px solid #ffffff14;
    border-radius: 14px;
    background: #16171ce6;
    box-shadow: 0 10px 30px #000a;
  }

  /* Sélectionné : le lecteur se détache, pour qu'on voie où l'on est. */
  .tv-mini:focus-within {
    border-color: color-mix(in srgb, var(--accent) 60%, transparent);
    background: #1c1d23f5;
  }

  .tv-mini :global(:focus) {
    outline-offset: 1px;
  }

  .info {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 12px;
    min-width: 0;
    padding: 2px;
    border: 0;
    border-radius: 10px;
    background: none;
    color: inherit;
    text-align: left;
  }

  .cover {
    display: grid;
    flex: none;
    place-items: center;
    width: 52px;
    height: 52px;
    overflow: hidden;
    border-radius: 8px;
    background: #ffffff14;
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .text {
    display: grid;
    min-width: 0;
  }

  .title,
  .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    font-size: 15px;
    font-weight: 600;
  }

  .sub,
  .time {
    color: var(--text-dim);
    font-size: 13px;
  }

  .time {
    font-variant-numeric: tabular-nums;
  }

  .ctls {
    display: flex;
    flex: none;
    gap: 4px;
  }

  .ctl {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: #ffffff12;
    color: #fff;
  }

  .ctl:focus {
    background: var(--accent);
    color: var(--accent-text);
  }

  .progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    background: #ffffff1f;
  }

  .progress span {
    display: block;
    height: 100%;
    background: var(--accent);
  }
</style>
