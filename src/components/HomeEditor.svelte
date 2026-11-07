<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Section } from "../lib/api";
  import { arrange, availableRows, resetHome, saveHome, type HomeRow } from "../lib/homeRows.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n";

  /**
   * Réglages → Interface : les rangées de l'accueil, à afficher ou masquer (une bibliothèque
   * s'épingle ainsi à l'accueil) et à monter ou descendre. Le choix est gardé sur cet appareil.
   */
  let sections = $state<Section[]>([]);
  const rows = $derived(arrange(availableRows(sections)));

  onMount(() => {
    api.sections().then((s) => (sections = s ?? []), () => {});
  });

  function label(id: string) {
    if (id === "continue") return t("home.continueWatching");
    if (id === "recent") return t("home.recentlyAdded");
    if (id === "trending") return t("home.trendingToRequest");
    const s = sections.find((x) => `lib:${x.key}` === id);
    return t("home.recentIn", { library: s?.title ?? "" });
  }

  function toggle(row: HomeRow) {
    saveHome(rows.map((r) => (r.id === row.id ? { ...r, on: !r.on } : r)));
  }

  function move(i: number, by: -1 | 1) {
    const next = [...rows];
    const [r] = next.splice(i, 1);
    next.splice(i + by, 0, r);
    saveHome(next);
  }

</script>

<div class="rows">
  {#each rows as row, i (row.id)}
    <div class="row-line" class:off={!row.on} data-nav-row>
      <label class="check">
        <input type="checkbox" class="switch" checked={row.on} onchange={() => toggle(row)} />
        <span>{label(row.id)}</span>
        {#if row.id.startsWith("lib:")}<span class="muted small">{t("settings.interface.homePinned")}</span>{/if}
      </label>
      <button class="tool" disabled={i === 0} onclick={() => move(i, -1)} aria-label={t("settings.interface.rowUp")} title={t("settings.interface.rowUp")}>
        <Icon name="chevronUp" size={18} />
      </button>
      <button class="tool" disabled={i === rows.length - 1} onclick={() => move(i, 1)} aria-label={t("settings.interface.rowDown")} title={t("settings.interface.rowDown")}>
        <Icon name="chevronDown" size={18} />
      </button>
    </div>
  {/each}
</div>
<div><button class="btn" onclick={resetHome}>{t("settings.interface.resetHome")}</button></div>

<style>
  .rows {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-bottom: 12px;
  }

  .row-line {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 6px;
    border-radius: 10px;
  }

  .row-line:hover,
  .row-line:focus-within {
    background: #ffffff0a;
  }

  .check {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 12px;
    min-width: 0;
    padding: 6px 0;
    cursor: pointer;
  }

  .check span:not(.muted) {
    overflow: hidden;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .off .check span:not(.muted) {
    color: var(--text-dim);
  }

  .small {
    font-size: 12px;
  }

  .tool {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--text-dim);
  }

  .tool:hover:not(:disabled),
  .tool:focus-visible {
    background: #ffffff1a;
    color: var(--text);
  }

  .tool:disabled {
    opacity: 0.3;
  }
</style>
