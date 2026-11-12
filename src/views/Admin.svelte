<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type Pref } from "../lib/api";
  import { nav } from "../lib/nav.svelte";
  import { t } from "../lib/i18n";
  import Icon, { type IconName } from "../components/Icon.svelte";
  import ServerDashboard from "../components/ServerDashboard.svelte";
  import ServerUsers from "../components/ServerUsers.svelte";
  import ServerPrefs from "../components/ServerPrefs.svelte";

  // Catégorie « Admin » du menu (administrateur, sur ordinateur) : le Dashboard (tableau de bord du serveur et
  // ses utilisateurs) et les Réglages du serveur (une section par rubrique de Plex Web), chacun avec la barre de
  // sections des bibliothèques (Recommandé, Bibliothèque…).
  let { page, initialTab = null }: { page: "dashboard" | "settings"; initialTab?: string | null } = $props();

  const GROUPS: Record<string, string> = {
    general: t("settings.groups.general"),
    library: t("settings.groups.library"),
    network: t("settings.groups.network"),
    transcoder: t("settings.groups.transcoder"),
    dlna: t("settings.groups.dlna"),
    extras: t("settings.groups.extras"),
    butler: t("settings.groups.butler"),
  };

  /** Icône de chaque rubrique, pour son bandeau. */
  const ICONS: Record<string, IconName> = {
    general: "settings",
    library: "library",
    network: "compass",
    transcoder: "play",
    dlna: "tv",
    extras: "star",
    butler: "clock",
  };

  let prefs = $state<Pref[] | null>(null);
  let prefsError = $state<string | null>(null);
  let advanced = $state(false);

  async function loadPrefs() {
    try {
      prefs = await api.prefs();
      prefsError = null;
    } catch (e) {
      prefsError = errorText(e);
    }
  }

  onMount(() => {
    if (page === "settings") loadPrefs();
  });

  const tabs = $derived(
    page === "dashboard"
      ? [{ id: "dashboard", label: t("settings.menu.dashboard") }, ...(nav.serverKind !== "jellyfin" ? [{ id: "users", label: t("settings.users.title") }] : [])]
      : Object.keys(GROUPS)
          .filter((g) => !prefs || prefs.some((p) => p.group === g))
          .map((g) => ({ id: g, label: GROUPS[g] })),
  );

  // svelte-ignore state_referenced_locally
  let tab = $state(initialTab ?? (page === "dashboard" ? "dashboard" : "general"));

  function select(id: string) {
    tab = id;
    document.querySelector(".content")?.scrollTo({ top: 0 });
    // Retour d'une autre page : on retrouve la même section.
    if (nav.view.name === "admin") nav.view.tab = id;
  }
</script>

<div class="page">
  <div class="head">
    <h1 class="row-title page-title">{page === "dashboard" ? t("admin.dashboard") : t("admin.serverSettings")}</h1>
    <div class="tabs" role="tablist">
      {#each tabs as tb (tb.id)}
        <button role="tab" aria-selected={tab === tb.id} class:active={tab === tb.id} onclick={() => select(tb.id)}>{tb.label}</button>
      {/each}
    </div>
  </div>

  {#if !nav.admin}
    <p class="muted">{t("settings.menu.locked")}</p>
  {:else if page === "dashboard"}
    <header class="banner">
      <span class="banner-icon"><Icon name={tab === "users" ? "user" : "stats"} size={30} /></span>
      <div class="banner-text">
        <h2>{tabs.find((x) => x.id === tab)?.label ?? ""}</h2>
        <p>{t(tab === "users" ? "admin.banner.users" : "admin.banner.dashboard")}</p>
      </div>
    </header>
    {#if tab === "users"}<ServerUsers />{:else}<ServerDashboard />{/if}
  {:else if prefsError}
    <p class="error">{prefsError}</p>
  {:else if !prefs}
    <div class="skeleton" style:height="300px"></div>
  {:else}
    <!-- Bandeau de la rubrique, façon Netflix : grande icône, titre, une phrase ; les réglages avancés à droite. -->
    <header class="banner">
      <span class="banner-icon"><Icon name={ICONS[tab] ?? "settings"} size={30} /></span>
      <div class="banner-text">
        <h2>{GROUPS[tab] ?? ""}</h2>
        <p>{t(`admin.banner.${tab}`)}</p>
      </div>
      <label class="adv-toggle">
        <span>{t("settings.menu.advanced")}</span>
        <input type="checkbox" class="switch" bind:checked={advanced} />
      </label>
    </header>
    <ServerPrefs prefs={prefs.filter((p) => p.group === tab)} {advanced} onSaved={loadPrefs} />
  {/if}
</div>

<style>
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px 24px;
    margin-bottom: 24px;
  }

  .head .page-title {
    margin-bottom: 0;
  }

  /* La barre de sections des bibliothèques (Recommandé, Bibliothèque, Collections…). */
  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 4px;
    border-radius: 22px;
    background: #ffffff0d;
  }

  .tabs button {
    padding: 7px 16px;
    border: none;
    border-radius: 18px;
    background: none;
    color: var(--text-dim);
    font-weight: 600;
    transition: background 0.15s, color 0.15s;
  }

  .tabs button:hover {
    color: var(--text);
  }

  .tabs button.active {
    background: var(--accent);
    color: var(--accent-text);
  }

  .banner {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 20px;
    margin-bottom: 24px;
    padding: 28px 30px;
    border: 1px solid var(--border);
    border-radius: 18px;
    background:
      radial-gradient(60% 160% at 0% 0%, color-mix(in srgb, var(--accent) 32%, transparent), transparent 70%),
      linear-gradient(120deg, color-mix(in srgb, var(--accent) 14%, transparent), transparent 60%),
      #ffffff05;
  }

  .banner-icon {
    display: grid;
    flex: none;
    place-items: center;
    width: 64px;
    height: 64px;
    border-radius: 18px;
    background: var(--accent);
    color: var(--accent-text);
    box-shadow: 0 10px 30px color-mix(in srgb, var(--accent) 35%, transparent);
  }

  .banner-text {
    flex: 1;
    min-width: 260px;
  }

  .banner h2 {
    margin: 0 0 4px;
    font-size: 26px;
    font-weight: 800;
  }

  .banner p {
    margin: 0;
    color: var(--text-dim);
  }

  .adv-toggle {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 14px;
    border-radius: 14px;
    background: #0003;
    color: var(--text);
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
  }
</style>
