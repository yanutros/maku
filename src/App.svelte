<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Section, type Session } from "./lib/api";
  import { back, go, nav, onBack, reload, restoreSpot, takeSpot } from "./lib/nav.svelte";
  import { player, stop } from "./lib/player.svelte";
  import Login from "./views/Login.svelte";
  import ServerPicker from "./views/ServerPicker.svelte";
  import ProfilePicker from "./views/ProfilePicker.svelte";
  import Home from "./views/Home.svelte";
  import Library from "./views/Library.svelte";
  import MusicLibrary from "./views/MusicLibrary.svelte";
  import ItemDetail from "./views/ItemDetail.svelte";
  import MixDetail from "./views/MixDetail.svelte";
  import Search from "./views/Search.svelte";
  import Libraries from "./views/Libraries.svelte";
  import Person from "./views/Person.svelte";
  import PhoneTabs from "./components/PhoneTabs.svelte";
  import Discover from "./views/Discover.svelte";
  import SeerrDetail from "./views/SeerrDetail.svelte";
  import Settings from "./views/Settings.svelte";
  import Stats from "./views/Stats.svelte";
  import Downloads from "./views/Downloads.svelte";
  import { checkOfflineAllowed, loadOffline, offlineAvailable, syncOffline } from "./lib/offline.svelte";
  import { ambient } from "./lib/ambient.svelte";
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
  import AddToCollection from "./components/AddToCollection.svelte";
  import AddToPlaylist from "./components/AddToPlaylist.svelte";
  import TrackInfo from "./components/TrackInfo.svelte";
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
    nav.serverKind = session.serverKind;
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
      // Serveur joignable : la progression jouée hors ligne lui est renvoyée, les téléchargements reprennent.
      if (sections.length) {
        checkOfflineAllowed();
        syncOffline();
      }
      // Édition des métadonnées et réglages du serveur : réservés à son administrateur.
      nav.admin = await api.isAdmin().catch(() => false);
      // Seerr ou Tracearr sans adresse : le relais Maku Server du serveur, s'il y en a un, les fournit.
      if (!session.seerrUrl || !session.tracearrUrl) {
        api.relayDiscover().then(async (r) => {
          if (r?.applied) session = await api.session();
        }, () => {});
      }
    }
  }

  // Retour dans l'application (téléphone surtout) ou réseau revenu : l'adresse du serveur a pu changer
  // (Wi-Fi ↔ 4G). Sans cette vérification, la page attendait une adresse injoignable et restait vide.
  // La page elle-même peut aussi avoir été rechargée par Android pendant l'absence : on vérifie d'abord.
  let hiddenAt = 0;
  let checking = false;
  async function checkServer(force = false) {
    if (checking || !session?.serverName || session.serverKind !== "plex") return;
    if (!force && Date.now() - hiddenAt < 15000) return;
    checking = true;
    try {
      if (await api.reconnect()) {
        sections = await api.sections().catch(() => sections);
        syncOffline();
        reload();
      }
    } catch {
      // Toujours injoignable : la page garde son message d'erreur, une nouvelle tentative viendra.
    } finally {
      checking = false;
    }
  }

  onMount(() => {
    // Téléchargements de l'appareil : lisibles même sans réseau, avant toute réponse du serveur.
    if (offlineAvailable()) loadOffline().catch(() => {});
    api.reconnect().catch(() => false).finally(refresh);
    const onVisibility = () => {
      if (document.visibilityState === "hidden") hiddenAt = Date.now();
      else checkServer();
    };
    const onOnline = () => checkServer(true);
    document.addEventListener("visibilitychange", onVisibility);
    window.addEventListener("online", onOnline);
    // Android (MainActivity.kt) : retour dans l'application et changement de réseau. La vérification est
    // rapide quand l'adresse répond toujours, on la fait donc à chaque retour.
    const w = window as { __makuResume?: () => void; __makuNetwork?: () => void };
    w.__makuResume = () => checkServer(true);
    w.__makuNetwork = () => checkServer(true);
    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("online", onOnline);
    };
  });

  // Bouton Retour (Android, souris) : il ferme d'abord ce qui est ouvert par-dessus la page.
  onBack(() => {
    if (nav.closePlayerMenu) nav.closePlayerMenu();
    else if (nav.profiles) nav.profiles = false;
    else if (nav.collectionFor || nav.collectionBatch) nav.collectionFor = nav.collectionBatch = null;
    else if (nav.playlistFor) nav.playlistFor = null;
    else if (nav.infoFor) nav.infoFor = null;
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
  <!-- Téléviseur : fond flouté de la page (musique, statistiques), derrière le menu translucide. -->
  {#if device.tv}
    {#key ambient.image}
      {#if ambient.image}<div class="tv-ambient" aria-hidden="true"><div style:background-image={`url("${ambient.image}")`}></div></div>{/if}
    {/key}
  {/if}
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
      <header data-nav-low class:clear={nav.view.name === "stats" || nav.view.name === "item" || nav.view.name === "mix" || nav.view.name === "seerr"}>
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
              smart={nav.view.smart}
            />
          {:else if nav.view.name === "item"}
            <ItemDetail ratingKey={nav.view.ratingKey} />
          {:else if nav.view.name === "mix"}
            <MixDetail id={nav.view.id} />
          {:else if nav.view.name === "person"}
            <Person id={nav.view.id} name={nav.view.personName} photo={nav.view.photo} seerr={nav.view.seerr ?? false} />
          {:else if nav.view.name === "search"}
            <Search query={nav.view.query} />
          {:else if nav.view.name === "discover"}
            <Discover />
          {:else if nav.view.name === "seerr"}
            <SeerrDetail mediaType={nav.view.mediaType} tmdbId={nav.view.tmdbId} />
          {:else if nav.view.name === "stats"}
            <Stats {session} onChange={refresh} />
          {:else if nav.view.name === "downloads"}
            <Downloads />
          {:else if nav.view.name === "settings"}
            {#key nav.view.page}<Settings {session} onChange={refresh} initialPage={nav.view.page} />{/key}
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

{#if nav.collectionFor || nav.collectionBatch}
  <AddToCollection />
{/if}

{#if nav.playlistFor}
  <AddToPlaylist />
{/if}

{#if nav.infoFor}
  <TrackInfo />
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

  /* Le flou se fait sur un calque dix fois plus petit, agrandi ensuite : même rendu qu'un grand flou,
     pour une fraction du travail du processeur graphique de la TV. */
  .tv-ambient {
    position: fixed;
    inset: 0;
    z-index: 0;
    overflow: hidden;
    pointer-events: none;
    animation: ambient-in 0.5s ease-out;
  }

  .tv-ambient div {
    position: absolute;
    top: -1%;
    left: -1%;
    width: 10.2%;
    height: 10.2%;
    background-position: center;
    background-size: cover;
    filter: blur(5px) saturate(1.5) brightness(0.42);
    transform: scale(10);
    transform-origin: 0 0;
  }

  @keyframes ambient-in {
    from {
      opacity: 0;
    }
  }

  :global(html.tv) .shell {
    position: relative;
    z-index: 1;
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

  /* translateX(0) plutôt que rien : la page reste le repère de ses fonds fixés (fiches, chargement de l'accueil),
     menu plié, déplié ou en train de se replier. Sans cela, ils changeaient de repère à la fin du repli et l'on
     voyait le fond noir une demi-seconde. */
  :global(html.tv) main {
    transform: translateX(0);
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

  /* Statistiques et fiches : le fond de la page couvre toute la fenêtre, menu compris ; le dégradé du haut, qui ne
     couvre que la page, y faisait une marque sombre en haut à gauche, contre le menu. Les fiches ont leur propre
     ombre en haut de l'image. */
  header.clear {
    background: none;
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
