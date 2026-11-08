<script lang="ts">
  // Choix de l'appareil qui joue la musique : cet appareil, ou une enceinte / un téléviseur Chromecast du réseau.
  // Sur iPad et iPhone, le bouton ouvre le sélecteur AirPlay du système.
  import Icon from "./Icon.svelte";
  import { errorText } from "../lib/api";
  import { t } from "../lib/i18n";
  import { toast } from "../lib/nav.svelte";
  import { airPlayAvailable, cast, castAvailable, castTo, findCastDevices, showAirPlay, type CastDevice } from "../lib/player.svelte";

  let { size = 19, up = true, big = false }: { size?: number; up?: boolean; big?: boolean } = $props();

  let open = $state(false);
  let root = $state<HTMLElement>();
  let menu = $state<HTMLElement>();
  // Position du menu à l'écran : la barre coupe ce qui la dépasse, le menu est donc rattaché à la page.
  let pos = $state({ right: 0, top: 0, bottom: 0 });

  function place() {
    const r = root?.getBoundingClientRect();
    if (!r) return;
    pos = { right: window.innerWidth - r.right, top: r.bottom + 8, bottom: window.innerHeight - r.top + 10 };
  }

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  function toggle() {
    if (airPlayAvailable()) return showAirPlay();
    open = !open;
    if (open) place();
    if (open) findCastDevices().catch((e) => toast(errorText(e), true));
  }

  async function choose(d: CastDevice | null) {
    open = false;
    try {
      await castTo(d);
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  function outside(e: PointerEvent) {
    const t = e.target as Node;
    if (open && root && !root.contains(t) && !menu?.contains(t)) open = false;
  }
</script>

<svelte:window onpointerdown={outside} onresize={() => (open = false)} />

{#if castAvailable() || airPlayAvailable()}
  <div class="cast" bind:this={root}>
    <button
      class="ctl"
      class:big
      class:on={!!cast.device}
      class:busy={cast.connecting}
      onclick={toggle}
      aria-label={airPlayAvailable() ? t("cast.airplay") : t("cast.title")}
      title={cast.device ? t("cast.on", { name: cast.device.name }) : airPlayAvailable() ? t("cast.airplay") : t("cast.title")}
      aria-expanded={open}
    >
      <Icon name="cast" {size} />
    </button>
    {#if open}
      <div
        class="menu"
        class:up
        role="menu"
        bind:this={menu}
        use:portal
        style:right="{pos.right}px"
        style:top={up ? null : `${pos.top}px`}
        style:bottom={up ? `${pos.bottom}px` : null}
      >
        <div class="head">{t("cast.title")}</div>
        <button role="menuitemradio" aria-checked={!cast.device} class:on={!cast.device} onclick={() => choose(null)}>
          <Icon name="music" size={16} />
          <span>{t("cast.thisDevice")}</span>
          {#if !cast.device}<Icon name="check" size={16} />{/if}
        </button>
        {#each cast.devices as d (d.id)}
          <button role="menuitemradio" aria-checked={cast.device?.id === d.id} class:on={cast.device?.id === d.id} onclick={() => choose(d)}>
            <Icon name="cast" size={16} />
            <span>{d.name}</span>
            {#if cast.device?.id === d.id}<Icon name="check" size={16} />{/if}
          </button>
        {/each}
        {#if cast.searching}
          <div class="note">{t("cast.searching")}</div>
        {:else if !cast.devices.length}
          <div class="note">{t("cast.none")}</div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .cast {
    position: relative;
  }

  .ctl {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: #ffffffb0;
    transition: background 0.15s, color 0.15s;
  }

  .ctl.big {
    width: 48px;
    height: 48px;
    color: #fff;
  }

  .ctl:hover {
    background: #ffffff1a;
    color: #fff;
  }

  .ctl.on {
    color: var(--accent);
  }

  .busy {
    animation: pulse 1s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.4;
    }
  }

  .menu {
    position: fixed;
    z-index: 200;
    display: grid;
    min-width: 250px;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--bg-elev, #16181d);
    box-shadow: 0 18px 40px #0009;
  }

  .head {
    padding: 6px 10px 8px;
    color: var(--text-dim);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .menu button {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) 18px;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    border: none;
    border-radius: 9px;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 14px;
    text-align: left;
  }

  .menu button span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .menu button:hover {
    background: #ffffff10;
  }

  .menu button.on {
    color: var(--accent);
  }

  .note {
    padding: 8px 10px;
    color: var(--text-dim);
    font-size: 12px;
  }
</style>
