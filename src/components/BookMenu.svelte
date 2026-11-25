<script lang="ts" module>
  import type { IconName } from "./Icon.svelte";

  /** Une ligne du menu : l'action s'exécute une fois le menu fermé ; une erreur s'affiche en bas de l'écran. */
  export interface BookMenuEntry {
    icon: IconName;
    label: string;
    run: () => unknown;
  }
</script>

<script lang="ts">
  // Menu « ⋮ » d'une série, d'un volume ou d'un chapitre Kavita, comme celui des jaquettes : marquer comme lu ou non
  // lu, détails… Même allure que le menu des jaquettes (CardMenu).
  import { errorText } from "../lib/api";
  import { toast } from "../lib/nav.svelte";
  import Icon from "./Icon.svelte";

  let { x, y, title, entries, onclose }: { x: number; y: number; title: string; entries: BookMenuEntry[]; onclose: () => void } = $props();

  let el = $state<HTMLElement>();
  // Le menu reste dans la fenêtre, même ouvert près d'un bord.
  const pos = $derived.by(() => {
    const h = el?.offsetHeight ?? 200;
    return { left: Math.max(12, Math.min(x, window.innerWidth - 240 - 12)), top: Math.max(12, Math.min(y, window.innerHeight - h - 12)) };
  });

  async function pick(entry: BookMenuEntry) {
    onclose();
    try {
      await entry.run();
    } catch (e) {
      toast(errorText(e), true);
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} onresize={onclose} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose} oncontextmenu={(e) => (e.preventDefault(), onclose())}></div>
<div class="menu" bind:this={el} style:left="{pos.left}px" style:top="{pos.top}px" role="menu" aria-label={title}>
  <div class="head">{title}</div>
  {#each entries as entry (entry.label)}
    <button role="menuitem" onclick={() => pick(entry)}><Icon name={entry.icon} size={16} /> {entry.label}</button>
  {/each}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
  }

  .menu {
    position: fixed;
    z-index: 61;
    width: 240px;
    padding: 6px;
    border: 1px solid #ffffff1a;
    border-radius: 12px;
    background: color-mix(in srgb, var(--bg-elev) 95%, transparent);
    backdrop-filter: blur(18px);
    box-shadow: 0 18px 44px #000b;
    animation: pop 0.12s var(--ease);
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.96);
    }
  }

  .head {
    padding: 8px 10px 6px;
    overflow: hidden;
    color: var(--text-faint);
    font-size: 12px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border: none;
    border-radius: 8px;
    background: none;
    text-align: left;
    font-size: 13px;
  }

  button:hover,
  button:focus-visible {
    background: #ffffff14;
  }

  button :global(svg) {
    color: var(--text-dim);
  }
</style>
