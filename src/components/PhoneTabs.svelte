<script lang="ts">
  import { go, nav, type View } from "../lib/nav.svelte";
  import { t } from "../lib/i18n";
  import Icon, { type IconName } from "./Icon.svelte";

  /** Barre d'onglets du téléphone, comme dans l'application Plex : les grandes rubriques en bas. */
  const TABS: { view: View; names: string[]; icon: IconName; label: string }[] = [
    { view: { name: "home" }, names: ["home"], icon: "home", label: t("phone.tabs.home") },
    { view: { name: "libraries" }, names: ["libraries", "section"], icon: "library", label: t("phone.tabs.libraries") },
    { view: { name: "discover" }, names: ["discover", "seerr"], icon: "compass", label: t("phone.tabs.discover") },
    { view: { name: "stats" }, names: ["stats"], icon: "stats", label: t("phone.tabs.stats") },
    { view: { name: "settings" }, names: ["settings"], icon: "settings", label: t("phone.tabs.settings") },
  ];

  const active = (names: string[]) => names.includes(nav.view.name);

  function open(view: View) {
    // Un onglet déjà ouvert ramène à son début plutôt que d'empiler la même page.
    if (nav.view.name !== view.name) go(view);
  }
</script>

<nav class="tabs">
  {#each TABS as tab (tab.icon)}
    <button class:active={active(tab.names)} aria-current={active(tab.names) ? "page" : undefined} onclick={() => open(tab.view)}>
      <Icon name={tab.icon} size={22} />
      <span>{tab.label}</span>
    </button>
  {/each}
</nav>

<style>
  /* Au-dessus des fonds fixés des pages (Statistiques, fiches), qui la recouvraient. */
  .tabs {
    position: relative;
    z-index: 30;
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    padding: 6px 4px calc(6px + var(--safe-bottom));
    border-top: 1px solid var(--border);
    background: #0e0f11f2;
  }

  button {
    display: grid;
    justify-items: center;
    gap: 3px;
    min-width: 0;
    padding: 6px 2px;
    border: none;
    background: none;
    color: var(--text-dim);
    font-size: 11px;
    font-weight: 600;
  }

  span {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  button.active {
    color: var(--accent);
  }
</style>
