<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Section, type Session } from "./lib/api";
  import { back, go, nav, onBack, restoreSpot, takeSpot } from "./lib/nav.svelte";
  import { player, stop } from "./lib/player.svelte";
  import Login from "./views/Login.svelte";
  import ServerPicker from "./views/ServerPicker.svelte";
  import ProfilePicker from "./views/ProfilePicker.svelte";
  import Home from "./views/Home.svelte";
  import Library from "./views/Library.svelte";
  import MusicLibrary from "./views/MusicLibrary.svelte";
  import ItemDetail from "./views/ItemDetail.svelte";
  import Search from "./views/Search.svelte";
  import Libraries from "./views/Libraries.svelte";
  import Person from "./views/Person.svelte";
  import PhoneTabs from "./components/PhoneTabs.svelte";
  import Discover from "./views/Discover.svelte";
  import SeerrDetail from "./views/SeerrDetail.svelte";
  import Settings from "./views/Settings.svelte";
  import Stats from "./views/Stats.svelte";
  import RequestDialog from "./components/RequestDialog.svelte";
  import PlayerOverlay from "./components/PlayerOverlay.svelte";
  import MusicBar from "./components/MusicBar.svelte";
  import NowPlaying from "./components/NowPlaying.svelte";
  import TvNowPlaying from "./components/TvNowPlaying.svelte";
  import TvMiniPlayer from "./components/TvMiniPlayer.svelte";
  import Icon from "./components/Icon.svelte";
  import Logo from "./components/Logo.svelte";
  import Wordmark from "./components/Wordmark.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import CardMenu from "./components/CardMenu.svelte";
  import TvMenu from "./components/TvMenu.svelte";
  import EditDialog from "./components/EditDialog.svelte";
  import UpdateDialog from "./components/UpdateDialog.svelte";
  import TrailerOverlay from "./components/TrailerOverlay.svelte";
  import QrCode from "./components/QrCode.svelte";
  import { sidebar } from "./lib/sidebar.svelte";
  import { device } from "./lib/device";
  import { t } from "./lib/i18n";

  let session = $state<Session | null>(null);
  let sections = $state<Section[]>([]);
  let query = $state("");
  let content = $state<HTMLElement>();
  let searchTimer: ReturnType<typeof setTimeout> | undefined;

  // Plex Home : « Qui regarde ? » une fois au lancement, sauf avec la connexion automatique.
  let choosingProfile = $state(false);
  let profileAsked = false;

  async function refresh() {
    session = await api.session();
    if (session.loggedIn && session.plexRemembered && session.serverKind === "plex") {
      const home = await api.homeUsers().catch(() => null);
      nav.homeProfiles = home?.users.length ?? 0;
      if (!profileAsked && !session.autoSignIn && nav.homeProfiles > 1) {
        profileAsked = true;
        choosingProfile = true;
        return;
      }
      profileAsked = true;
    }
    if (session.serverName) {
      sections = await api.sections().catch(() => []);
      // Édition des métadonnées et réglages du serveur : réservés à son administrateur.
      nav.admin = await api.isAdmin().catch(() => false);
    }
  }

  onMount(refresh);

  // Bouton Retour (Android, souris) : il ferme d'abord ce qui est ouvert par-dessus la page.
  onBack(() => {
    if (nav.profiles) nav.profiles = false;
    else if (nav.trailer) nav.trailer = null;
    else if (nav.tvMenu) nav.tvMenu = null;
    else if (nav.menu) nav.menu = null;
    else if (nav.editing) nav.editing = null;
    else if (nav.requesting) nav.requesting = null;
    else if (player.active && player.video) stop();
    else if (player.expanded) player.expanded = false;
    else return false;
    return true;
  });

  const watching = $derived(player.active && player.video);
  const listening = $derived(player.active && !player.video);

  // La vidéo est dessinée sous la page : on rend le fond transparent pendant la lecture. Sur ordinateur,
  // seulement quand mpv affiche l'image : avant, la fenêtre transparente laisserait voir le bureau.
  $effect(() => {
    document.documentElement.classList.toggle("playing", watching && player.embedded && (player.web || player.videoReady));
  });

  // Largeur de la barre latérale, reprise par les fonds plein écran des fiches.
  $effect(() => {
    document.documentElement.style.setProperty("--sidebar-w", device.phone ? "0px" : sidebar.collapsed || device.tv ? "73px" : "237px");
  });

  // Chaque nouvelle vue repart du haut ; une page rechargée ou retrouvée (Retour) reprend où on l'avait quittée.
  $effect(() => {
    void nav.view;
    if (!content) return;
    const spot = takeSpot();
    if (spot) restoreSpot(content, spot);
    else content.scrollTo({ top: 0 });
  });

  // Sur téléviseur et téléphone, la page de recherche s'ouvre sur le champ de saisie.
  function tvAutofocus(input: HTMLInputElement) {
    if ((device.tv || device.phone) && !query) input.focus();
  }

  // Téléphone : les rubriques des onglets n'ont pas de bouton Retour (le bouton d'Android suffit).
  const TAB_VIEWS = ["home", "libraries", "discover", "stats", "settings"];

  function onSearchInput() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      const q = query.trim();
      if (q.length < 2) return;
      if (nav.view.name === "search") nav.view = { name: "search", query: q };
      else go({ name: "search", query: q });
    }, 350);
  }
</script>

{#if !session}
  <div class="center muted">{t("common.c1.loading")}</div>
{:else if !session.loggedIn}
  <Login onDone={refresh} />
{:else if choosingProfile || nav.profiles}
  <!-- Nouveau profil : on repart de l'accueil, avec ses bibliothèques et ses droits. -->
  <ProfilePicker
    cancel={choosingProfile ? undefined : () => (nav.profiles = false)}
    onDone={(switched) => {
      choosingProfile = false;
      nav.profiles = false;
      if (switched) {
        nav.history = [];
        nav.view = { name: "home" };
      }
      refresh();
    }}
  />
{:else if !session.serverName}
  <ServerPicker onDone={refresh} />
{:else}
  <div class="app">
  <div class="shell" class:behind={watching} class:collapsed={sidebar.collapsed} class:phone={device.phone}>
    {#if !device.phone}<Sidebar {sections} serverName={session.serverName} />{/if}

    <main data-nav-zone>
      {#if device.phone}
        <!-- Téléphone : en-tête compact (logo et recherche), sous la barre d'état. -->
        <header class="phone-header">
          {#if !TAB_VIEWS.includes(nav.view.name) && nav.history.length}
            <button class="icon-btn" onclick={back} aria-label={t("common.c1.back")}><Icon name="back" /></button>
          {/if}
          {#if nav.view.name === "search"}
            <label class="search">
              <Icon name="search" size={18} />
              <input type="search" placeholder={t("app.searchPlaceholder")} bind:value={query} oninput={onSearchInput} use:tvAutofocus />
            </label>
          {:else}
            <span class="brand"><Logo size={30} bare /><Wordmark size={30} /></span>
            <button class="icon-btn" onclick={() => go({ name: "search", query: "" })} aria-label={t("phone.search")}>
              <Icon name="search" />
            </button>
          {/if}
        </header>
      {/if}
      <!-- Sur téléviseur, la recherche passe par le menu : la barre n'apparaît que sur sa page. -->
      {#if !device.phone && (!device.tv || nav.view.name === "search")}
      <header data-nav-low>
        <button class="btn round ghost" disabled={!nav.history.length} onclick={back} aria-label={t("common.c1.back")}>
          <Icon name="back" />
        </button>
        <label class="search">
          <Icon name="search" size={18} />
          <input type="search" placeholder={t("app.searchPlaceholder")} bind:value={query} oninput={onSearchInput} use:tvAutofocus />
        </label>
      </header>
      {/if}
      <div class="content" bind:this={content}>
        {#key nav.view}
          {#if nav.view.name === "home"}
            <Home />
          {:else if nav.view.name === "libraries"}
            <Libraries {sections} />
          {:else if nav.view.name === "section" && nav.view.kind === "artist"}
            <MusicLibrary sectionKey={nav.view.key} title={nav.view.title} />
          {:else if nav.view.name === "section"}
            <Library
              sectionKey={nav.view.key}
              title={nav.view.title}
              kind={nav.view.kind}
              initialTab={nav.view.tab}
              initialGenre={nav.view.genre}
              initialFocus={nav.view.focusKey}
              initialScroll={nav.view.scroll}
              initialLoaded={nav.view.loaded}
            />
          {:else if nav.view.name === "item"}
            <ItemDetail ratingKey={nav.view.ratingKey} />
          {:else if nav.view.name === "person"}
            <Person id={nav.view.id} name={nav.view.personName} photo={nav.view.photo} />
          {:else if nav.view.name === "search"}
            <Search query={nav.view.query} />
          {:else if nav.view.name === "discover"}
            <Discover />
          {:else if nav.view.name === "seerr"}
            <SeerrDetail mediaType={nav.view.mediaType} tmdbId={nav.view.tmdbId} />
          {:else if nav.view.name === "stats"}
            <Stats {session} onChange={refresh} />
          {:else if nav.view.name === "settings"}
            <Settings {session} onChange={refresh} />
          {/if}
        {/key}
      </div>
      <!-- Ordinateur : la barre musique sous le contenu seulement, le menu de gauche garde toute la hauteur. -->
      {#if listening && !device.tv && !device.phone}<MusicBar />{/if}
    </main>
  </div>
  {#if listening && device.tv}<TvMiniPlayer />{/if}
  {#if listening && device.phone}<MusicBar />{/if}
  {#if device.phone && !watching}<PhoneTabs />{/if}
  </div>
{/if}

{#if listening && player.expanded}
  {#if device.tv}<TvNowPlaying />{:else}<NowPlaying />{/if}
{/if}

{#if watching}
  <PlayerOverlay />
{/if}

<!-- Bande-annonce YouTube : s'ouvre par-dessus la fiche, sans lecture en cours. -->
<TrailerOverlay />

{#if nav.tvMenu}
  <TvMenu />
{/if}
{#if nav.menu}
  <CardMenu />
{/if}

{#if nav.editing}
  <EditDialog />
{/if}

{#if nav.requesting}
  <RequestDialog media={nav.requesting} />
{/if}

<!-- Téléviseur : la validation Tracearr se fait sur le téléphone, depuis ce QR code. -->
{#if nav.authUrl}
  <div class="auth-qr" role="dialog" aria-label={t("app.authTracearr")}>
    <div class="auth-card">
      <QrCode text={nav.authUrl} size={260} />
      <div>
        <h2>{t("app.authTracearr")}</h2>
        <p class="muted">{t("app.authScan")}</p>
        <button class="btn" onclick={() => (nav.authUrl = null)}>{t("common.c1.cancel")}</button>
      </div>
    </div>
  </div>
{/if}

<UpdateDialog />

{#if nav.toast}
  <div class="toast" class:error={nav.toast.error} class:raised={listening}>{nav.toast.text}</div>
{/if}

<style>
  /* La barre musique, quand elle est affichée, prend sa hauteur sous l'application. */
  .app {
    display: grid;
    grid-template-rows: minmax(0, 1fr) auto auto;
    height: 100%;
  }

  /* minmax(0, …) : sans ça, la grille grandit avec le contenu et la page ne défile plus. */
  .shell {
    display: grid;
    grid-template-columns: 236px minmax(0, 1fr);
    transition: grid-template-columns 0.25s var(--ease);
    grid-template-rows: minmax(0, 1fr);
    min-height: 0;
  }

  .shell.collapsed {
    grid-template-columns: 72px minmax(0, 1fr);
  }

  /* Téléviseur : la colonne du menu ne change jamais. Menu déplié, la page glisse vers la droite (une
     translation, que la TV anime sans recalculer la mise en page) au lieu de rétrécir. */
  :global(html.tv) .shell {
    grid-template-columns: 72px minmax(0, 1fr);
    transition: none;
    overflow: hidden;
  }

  :global(html.tv) main {
    transition: transform 0.25s var(--ease);
  }

  :global(html.tv) .shell:not(.collapsed) main {
    transform: translateX(164px);
  }

  /* Téléphone : pas de menu latéral (les onglets sont en bas). */
  .shell.phone {
    grid-template-columns: minmax(0, 1fr);
  }

  /* En flux normal (l'en-tête de l'ordinateur, lui, flotte au-dessus du contenu). */
  .phone-header {
    position: relative;
    z-index: 10;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 56px;
    padding: calc(var(--safe-top) + 6px) 12px 6px 16px;
    background: var(--bg);
  }

  .phone-header .search {
    flex: 1;
    width: auto;
  }

  .brand {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 9px;
    font-size: 20px;
    font-weight: 800;
    letter-spacing: -0.02em;
  }

  .icon-btn {
    display: grid;
    flex: none;
    place-items: center;
    width: 42px;
    height: 42px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--text);
  }

  .shell.phone .content {
    padding-top: 0;
  }

  /* Masqué mais conservé pendant la lecture, pour retrouver la page au retour. */
  .shell.behind {
    visibility: hidden;
  }

  main {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  header {
    position: absolute;
    z-index: 10;
    top: 0;
    left: 0;
    right: 0;
    display: flex;
    gap: 12px;
    padding: 14px 40px;
    background: linear-gradient(var(--bg) 0%, #14151800 100%);
    pointer-events: none;
  }

  header > :global(*) {
    pointer-events: auto;
  }

  .ghost {
    background: #00000059;
    backdrop-filter: blur(10px);
  }

  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    width: min(460px, 100%);
    height: 40px;
    padding: 0 16px;
    border: 1px solid var(--border);
    border-radius: 20px;
    background: #0000006b;
    backdrop-filter: blur(10px);
    color: var(--text-dim);
    transition: border-color 0.15s;
  }

  .search:focus-within {
    border-color: var(--accent);
  }

  /* Télécommande : la pilule s'entoure, pas le champ qu'elle contient. */
  :global(html.tv) .search:focus-within {
    box-shadow: 0 0 0 2px var(--accent);
  }

  :global(html.tv) .search input:focus-visible {
    outline: none;
  }

  .search input {
    flex: 1;
    padding: 0;
    border: none;
    background: none;
  }

  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-top: 68px;
  }

  .auth-qr {
    position: fixed;
    inset: 0;
    z-index: 900;
    display: grid;
    place-items: center;
    background: #000000b8;
  }

  .auth-card {
    display: flex;
    align-items: center;
    gap: 32px;
    padding: 28px;
    border-radius: 18px;
    background: var(--bg-elev);
  }

  .auth-card h2 {
    margin: 0 0 6px;
  }

  .auth-card p {
    max-width: 280px;
    margin: 0 0 18px;
  }

  .toast {
    position: fixed;
    bottom: 24px;
    right: 24px;
    z-index: 100;
    max-width: 480px;
    padding: 12px 18px;
    border-radius: var(--radius);
    border-left: 3px solid var(--ok);
    background: var(--bg-elev);
    box-shadow: 0 12px 32px #0009;
    animation: rise 0.25s var(--ease);
  }

  /* Au-dessus de la barre musique. */
  .toast.raised {
    bottom: 124px;
  }

  .toast.error {
    border-left-color: var(--danger);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
  }
</style>
