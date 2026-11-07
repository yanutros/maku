<script lang="ts" module>
  import { onBack } from "../lib/nav.svelte";

  /** Liste ouverte (une seule à la fois) : le bouton Retour la referme sans rien changer. */
  let closeOpen: (() => void) | null = null;
  onBack(() => {
    if (!closeOpen) return false;
    closeOpen();
    return true;
  });
</script>

<script lang="ts">
  import { tick } from "svelte";
  import Icon from "./Icon.svelte";
  import { device } from "../lib/device";
  import { reveal } from "../lib/spatial";

  type Option = { value: number; label: string };
  let {
    value = $bindable(),
    options,
    label,
    onchange,
  }: { value: number | null; options: Option[]; label: string; onchange?: (value: number) => void } = $props();

  // Téléviseur : la liste s'ouvre sous le champ, à sa largeur, comme sur Apple TV (au lieu de la grande
  // fenêtre d'Android au milieu de l'écran). Ailleurs, la liste déroulante du système.
  let open = $state(false);
  let index = $state(0);
  let trigger = $state<HTMLButtonElement>();
  let list = $state<HTMLElement>();
  const current = $derived(options.find((o) => o.value === value) ?? options[0]);

  function focusOption(i: number) {
    index = Math.max(0, Math.min(options.length - 1, i));
    const el = list?.querySelectorAll<HTMLElement>("[role=option]")[index];
    if (!el) return;
    el.focus({ preventScroll: true });
    reveal(el);
  }

  async function show() {
    open = true;
    closeOpen = close;
    await tick();
    focusOption(Math.max(0, options.findIndex((o) => o.value === value)));
    // Toute la liste à l'écran, si elle tient.
    if (list) reveal(list);
  }

  function close() {
    open = false;
    if (closeOpen === close) closeOpen = null;
    trigger?.focus({ preventScroll: true });
  }

  function choose(o: Option) {
    value = o.value;
    onchange?.(o.value);
    close();
  }

  function onKey(e: KeyboardEvent) {
    const moves: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };
    if (e.key in moves) focusOption(index + moves[e.key]);
    else if (e.key === "Escape") close();
    else if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    // Gauche / droite ne quittent pas la liste.
    e.preventDefault();
    e.stopPropagation();
  }
</script>

{#if device.tv}
  <div class="tv-select">
    <button bind:this={trigger} class="field" class:open aria-haspopup="listbox" aria-expanded={open} aria-label={label} onclick={() => (open ? close() : show())}>
      <span class="text">{current?.label ?? ""}</span>
      <Icon name={open ? "chevronUp" : "chevronDown"} size={16} />
    </button>
    {#if open}
      <!-- `data-nav-grid` : la liste gère elle-même ses flèches (la navigation ordinaire la quitterait). -->
      <div class="list" bind:this={list} role="listbox" aria-label={label} data-nav-grid tabindex="-1" onkeydown={onKey} onfocusout={(e) => !list?.contains(e.relatedTarget as Node) && e.relatedTarget !== trigger && close()}>
        {#each options as o (o.value)}
          <button role="option" class="option" class:on={o.value === value} aria-selected={o.value === value} onclick={() => choose(o)}>
            <span class="text">{o.label}</span>
            {#if o.value === value}<Icon name="check" size={16} />{/if}
          </button>
        {/each}
      </div>
    {/if}
  </div>
{:else}
  <select
    {value}
    aria-label={label}
    onchange={(e) => {
      value = Number((e.target as HTMLSelectElement).value);
      onchange?.(value);
    }}
  >
    {#each options as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
  </select>
{/if}

<style>
  select {
    flex: 1;
    min-width: 0;
    padding: 8px 10px;
    color: var(--text);
    font-weight: 500;
  }

  .tv-select {
    flex: 1;
    min-width: 0;
  }

  .field,
  .option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
    padding: 9px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
    color: var(--text);
    font: inherit;
    font-weight: 500;
    text-align: left;
  }

  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .list {
    display: grid;
    gap: 2px;
    margin-top: 6px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--bg-elev);
  }

  .option {
    border-color: transparent;
    background: none;
  }

  .option.on {
    color: var(--accent);
  }

  /* La piste sous la sélection se remplit de la couleur du thème, comme les boutons. */
  .option:focus {
    outline: none;
    background: var(--accent);
    color: var(--accent-text);
  }
</style>
