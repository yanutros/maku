<script lang="ts">
  import "./settings/settings.css";
  import { onMount } from "svelte";
  import Icon, { type IconName } from "../components/Icon.svelte";
  import AccountPage from "./settings/AccountPage.svelte";
  import PlayerPage from "./settings/PlayerPage.svelte";
  import InterfacePage from "./settings/InterfacePage.svelte";
  import ServicesPage from "./settings/ServicesPage.svelte";
  import { api, type RelayFound, type Session } from "../lib/api";
  import { device } from "../lib/device";
  import { t } from "../lib/i18n";

  let { session, onChange, initialPage }: { session: Session; onChange: () => unknown; initialPage?: string } = $props();

  // Rubriques de Maku. Les réglages du serveur Plex sont dans la catégorie Admin du menu (ordinateur).
  // Les anciennes adresses de leurs rubriques ouvrent le compte.
  const SERVER_PAGES = ["server", "dashboard", "general", "library", "network", "transcoder", "dlna", "extras", "butler"];
  // svelte-ignore state_referenced_locally
  let page = $state(initialPage && !SERVER_PAGES.includes(initialPage) ? initialPage : "account");

  // Ordinateur et téléviseur : les rubriques en onglets sous le titre, le contenu sur toute la largeur. Téléphone : une
  // bande de rubriques qui défile au doigt.
  const desk = !device.tv && !device.phone;
  const SECTIONS: { id: string; label: string; icon: IconName }[] = [
    { id: "account", label: t("settings.menu.account"), icon: "user" },
    { id: "player", label: t("settings.menu.player"), icon: "play" },
    { id: "interface", label: t("settings.menu.interface"), icon: "menu" },
    { id: "services", label: t("settings.menu.services"), icon: "compass" },
  ];

  function choosePage(id: string) {
    clearTimeout(previewTimer);
    if (id === page) return;
    page = id;
    document.querySelector(".content")?.scrollTo({ top: 0 });
  }

  // Téléviseur : la rubrique s'ouvre au passage des flèches sur ses onglets (comme ceux des bibliothèques).
  let previewTimer: ReturnType<typeof setTimeout> | undefined;
  function previewPage(id: string) {
    clearTimeout(previewTimer);
    if (id !== page) previewTimer = setTimeout(() => choosePage(id), 250);
  }

  // Relais Maku Server sur la machine du serveur, cherché dès l'ouverture des réglages (quelle que soit la
  // rubrique) : s'il a pu configurer Seerr ou Tracearr tout seul, la session est rechargée.
  let relay = $state<RelayFound | null>(null);
  onMount(() => {
    api.relayDiscover().then((r) => {
      if (!r) return;
      relay = r;
      if (r.applied) onChange();
    }, () => {});
  });
</script>

{#snippet link(id: string, label: string, icon: IconName)}
  <button class:active={page === id} aria-current={page === id ? "page" : undefined} onclick={() => choosePage(id)}>
    <Icon name={icon} size={17} />
    <span>{label}</span>
  </button>
{/snippet}

<div class="page settings" class:desk>
  <!-- Ordinateur et téléviseur : les rubriques dans la barre de sections des bibliothèques (Recommandé, Bibliothèque…),
       sous le titre. Sur téléviseur, gauche et droite passent d'une rubrique à l'autre, qui s'affiche au passage ;
       bas entre dans la page, haut revient à l'onglet de la rubrique ouverte. -->
  {#if desk || device.tv}
    <div class="desk-head">
      <h1 class="row-title page-title">{t("sidebar.settings")}</h1>
      <div class="tabs" role="tablist" data-nav-zone={device.tv ? "settings-rail" : undefined}>
        {#each SECTIONS as sec (sec.id)}
          <button
            role="tab"
            aria-selected={page === sec.id}
            class:active={page === sec.id}
            data-nav-current={device.tv && page === sec.id ? "" : undefined}
            data-nav-default={device.tv && page === sec.id ? "" : undefined}
            onclick={() => choosePage(sec.id)}
            onfocus={() => device.tv && previewPage(sec.id)}
          >
            {#if device.tv}<Icon name={sec.icon} size={20} />{/if}
            {sec.label}
          </button>
        {/each}
      </div>
    </div>
  {:else}
    <!-- Téléphone : la bande des rubriques. -->
    <nav class="menu">
      {#each SECTIONS as sec (sec.id)}{@render link(sec.id, sec.label, sec.icon)}{/each}
    </nav>
  {/if}

  <!-- `data-nav-lines` : haut / bas passent par chaque ligne, dans l'ordre (télécommande). -->
  <div class="panel" class:solo={page === "account" || page === "player"} data-nav-zone="settings-panel" data-nav-lines>
    {#if page === "account"}
      <AccountPage {session} {onChange} />
    {:else if page === "player"}
      <PlayerPage {session} {onChange} {desk} />
    {:else if page === "interface"}
      <InterfacePage {desk} />
    {:else if page === "services"}
      <ServicesPage {session} {onChange} {desk} {relay} />
    {/if}
  </div>
</div>
