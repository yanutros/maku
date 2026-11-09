<script lang="ts">
  import { lyricsPrefs, setLyricsEnabled } from "../lib/lyrics.svelte";
  import { ratingPrefs, setRatingSite, type RatingSite } from "../lib/ratings.svelte";
  import { NEXT_DELAYS, autoNext, setNextDelay, diagAvailable, diagShare, diagText, passthroughAvailable, passthroughFormats, passthroughMode, setPassthroughMode } from "../lib/player.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import QrCode from "../components/QrCode.svelte";
  import PasswordInput from "../components/PasswordInput.svelte";
  import TrackSelect from "../components/TrackSelect.svelte";
  import KofiLogo from "../components/KofiLogo.svelte";
  import RatingLogo from "../components/RatingLogo.svelte";
  import { applyInvite, decodeInvite, encodeInvite, readQrImage, type InviteStatus } from "../lib/invite";
  import { api, errorText, tracearrLogin, type Account, type HomeUser, type Pref, type Section, type ServerKinds, type Session } from "../lib/api";
  import { go, nav, toast } from "../lib/nav.svelte";
  import { ordered, saveSidebar, sidebar } from "../lib/sidebar.svelte";
  import { device } from "../lib/device";
  import { reveal } from "../lib/spatial";
  import { forgetRemoteLogos } from "../lib/logos";
  import { getVersion } from "@tauri-apps/api/app";
  import { checkForUpdate, showReleaseNotes, update } from "../lib/update.svelte";
  import { applyTheme, savedTheme, THEME_COLORS, THEMES, type Theme } from "../lib/theme";
  import Icon, { type IconName } from "../components/Icon.svelte";
  import HomeEditor from "../components/HomeEditor.svelte";
  import ServerPrefs from "../components/ServerPrefs.svelte";
  import ServerUsers from "../components/ServerUsers.svelte";
  import ServerDashboard from "../components/ServerDashboard.svelte";
  import { LANGUAGES, savedLanguage, setLanguage, t } from "../lib/i18n";

  let { session, onChange, initialPage }: { session: Session; onChange: () => void; initialPage?: string } = $props();

  // Rubriques de Maku, puis celles du serveur Plex (administrateur seulement), comme dans Plex Web.
  const GROUPS: Record<string, { label: string; icon: IconName }> = {
    general: { label: t("settings.groups.general"), icon: "sliders" },
    library: { label: t("settings.groups.library"), icon: "film" },
    network: { label: t("settings.groups.network"), icon: "compass" },
    transcoder: { label: t("settings.groups.transcoder"), icon: "play" },
    dlna: { label: t("settings.groups.dlna"), icon: "tv" },
    extras: { label: t("settings.groups.extras"), icon: "star" },
    butler: { label: t("settings.groups.butler"), icon: "clock" },
  };

  // Anciennes adresses des rubriques du serveur (une par rubrique) : elles ouvrent l'onglet correspondant.
  // svelte-ignore state_referenced_locally
  const legacy = initialPage && (initialPage === "dashboard" || initialPage in GROUPS) ? initialPage : null;
  // svelte-ignore state_referenced_locally
  let page = $state(legacy ? "server" : (initialPage ?? "account"));
  let serverTab = $state(legacy ?? "dashboard");

  /** QR code déplié sous sa ligne (journal, clé TMDB, configuration rapide, Ko-fi) : un seul à la fois. */
  let qr = $state<string | null>(null);

  async function toggleQr(id: string) {
    qr = qr === id ? null : id;
    if (!qr) return;
    await tick();
    // La sélection descend sur le QR code, qui vient à l'écran.
    const el = document.querySelector<HTMLElement>(".settings .qr-open");
    el?.focus({ preventScroll: true });
    if (el) reveal(el);
  }

  function choosePage(id: string) {
    clearTimeout(previewTimer);
    if (id === page) return;
    page = id;
    qr = null;
    document.querySelector(".content")?.scrollTo({ top: 0 });
  }

  // Téléviseur : la rubrique s'ouvre au passage des flèches sur le rail (comme les onglets des bibliothèques).
  let previewTimer: ReturnType<typeof setTimeout> | undefined;
  function previewPage(id: string) {
    clearTimeout(previewTimer);
    if (id !== page) previewTimer = setTimeout(() => choosePage(id), 250);
  }
  const KOFI = "https://ko-fi.com/yanutros";
  let account = $state<Account | null>(null);
  let prefs = $state<Pref[] | null>(null);
  let prefsError = $state<string | null>(null);
  let advanced = $state(false);

  const groups = $derived(
    Object.keys(GROUPS).filter((g) => prefs?.some((p) => p.group === g)),
  );

  // Champs éditables, initialisés une fois depuis la session.
  let seerrUrl = $state("");
  let tracearrUrl = $state("");
  let mpvPath = $state("");
  $effect.pre(() => {
    seerrUrl = session.seerrUrl ?? "";
    tracearrUrl = session.tracearrUrl ?? "";
    mpvPath = session.mpvPath ?? "";
  });

  // Avec Jellyfin, Seerr demande l'identifiant et le mot de passe Jellyfin (gardés le temps de la connexion).
  let seerrUser = $state("");
  let seerrPassword = $state("");
  let seerrBusy = $state(false);
  let seerrError = $state<string | null>(null);
  let tracearrBusy = $state(false);
  /** Relais Maku Server trouvé sur la machine du serveur. */
  let relayFound = $state<string | null>(null);
  let tracearrError = $state<string | null>(null);

  // --- TMDB : logos des titres que Plex n'a pas ---
  // Android : son vers l'ampli (automatique) ou décodé sur l'appareil.
  let passthrough = $state(passthroughMode());
  const audioOut = passthroughAvailable ? passthroughFormats() : null;
  function choosePassthrough(mode: "auto" | "off") {
    passthrough = mode;
    setPassthroughMode(mode);
  }

  let tmdbKey = $state("");
  let tmdbBusy = $state(false);
  let tmdbError = $state<string | null>(null);

  async function saveTmdb(remove = false) {
    tmdbBusy = true;
    tmdbError = null;
    try {
      await api.tmdbConfigure(remove ? "" : tmdbKey);
      forgetRemoteLogos();
      tmdbKey = "";
      toast(remove ? t("settings.tmdb.removed") : t("settings.tmdb.saved"));
      onChange();
      return true;
    } catch (e) {
      tmdbError = errorText(e);
      return false;
    } finally {
      tmdbBusy = false;
    }
  }

  // Clé collée sur le téléphone (page …/tmdb du relais) : vérifiée ici, résultat renvoyé au téléphone.
  async function tmdbFromPhone(key: string) {
    tmdbKey = key;
    const ok = await saveTmdb();
    api.relayStatus(ok ? { step: "done" } : { step: "error", error: tmdbError }, true).catch(() => {});
  }

  async function loadPrefs() {
    try {
      prefs = await api.prefs();
      prefsError = null;
    } catch (e) {
      prefsError = errorText(e);
    }
  }

  async function saveSeerr() {
    seerrBusy = true;
    seerrError = null;
    try {
      if (session.serverKind === "jellyfin" && seerrUrl) {
        await api.seerrConfigureJellyfin(seerrUrl, seerrUser, seerrPassword);
        seerrPassword = "";
      } else {
        await api.seerrConfigure(seerrUrl);
      }
      toast(seerrUrl ? t("settings.services.seerrConnected") : t("settings.services.seerrDisabled"));
      onChange();
    } catch (e) {
      seerrError = errorText(e);
    } finally {
      seerrBusy = false;
    }
  }

  async function connectTracearr() {
    tracearrBusy = true;
    tracearrError = null;
    try {
      if (tracearrUrl.trim() !== (session.tracearrUrl ?? "")) await api.tracearrConfigure(tracearrUrl);
      if (!tracearrUrl.trim()) {
        toast(t("settings.services.tracearrDisabled"));
      } else {
        await tracearrLogin();
        toast(t("settings.services.tracearrConnected"));
        go({ name: "stats" });
      }
      onChange();
    } catch (e) {
      tracearrError = errorText(e);
    } finally {
      tracearrBusy = false;
    }
  }

  // --- Invitations ---
  // L'administrateur partage ses adresses Seerr et Tracearr ; les autres les appliquent en un geste.
  // Adresses à partager : celles de l'administrateur par défaut, modifiables (il n'a pas besoin
  // de s'être connecté lui-même pour inviter).
  let inviteSeerr = $state("");
  let inviteTracearr = $state("");
  $effect.pre(() => {
    inviteSeerr = session.seerrUrl ?? "";
    inviteTracearr = session.tracearrUrl ?? "";
  });
  const isUrl = (v: string) => /^https?:\/\/[^\s]+$/.test(v.trim());
  const inviteCode = $derived(
    isUrl(inviteSeerr) || isUrl(inviteTracearr)
      ? encodeInvite({
          seerr: isUrl(inviteSeerr) ? inviteSeerr.trim().replace(/\/+$/, "") : undefined,
          tracearr: isUrl(inviteTracearr) ? inviteTracearr.trim().replace(/\/+$/, "") : undefined,
        })
      : "",
  );
  let inviteQr = $state<HTMLElement>();
  let relayUrl = $state<string | null>(null);
  let relayError = $state<string | null>(null);
  let applying = $state<InviteStatus | null>(null);
  let pasted = $state("");
  let inviteError = $state<string | null>(null);

  async function runInvite(text: string, phone: boolean) {
    const invite = decodeInvite(text);
    if (!invite) {
      inviteError = t("settings.invite.invalid");
      if (phone) api.relayStatus({ step: "error", error: inviteError }).catch(() => {});
      return;
    }
    inviteError = null;
    const result = await applyInvite(
      invite,
      (status) => {
        applying = status;
        if (phone) api.relayStatus(status).catch(() => {});
      },
      phone,
    );
    onChange();
    if (result.step === "done") toast(t("settings.invite.configured"));
  }

  async function onInviteImage(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    const text = await readQrImage(file).catch(() => null);
    if (text) runInvite(text, false);
    else inviteError = t("settings.invite.noQr");
  }

  async function copyInviteCode() {
    await navigator.clipboard.writeText(inviteCode);
    toast(t("settings.invite.codeCopied"));
  }

  // Le QR code en image PNG, à coller dans un message.
  async function copyInviteImage() {
    const svg = inviteQr?.querySelector("svg");
    if (!svg) return;
    try {
      const img = new Image();
      img.src = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(new XMLSerializer().serializeToString(svg));
      await img.decode();
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = 600;
      canvas.getContext("2d")!.drawImage(img, 0, 0, 600, 600);
      const blob = await new Promise<Blob>((ok, ko) => canvas.toBlob((b) => (b ? ok(b) : ko(new Error("image"))), "image/png"));
      await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
      toast(t("settings.invite.imageCopied"));
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  // Tablette et téléviseur : le téléphone envoie le code par le relais, tant que la page est ouverte.
  $effect(() => {
    if (page !== "services" || !device.mobile || device.phone) return;
    let closed = false;
    let unlisten: (() => void) | undefined;
    api.relayStart().then(
      (url) => {
        if (!closed) relayUrl = url;
      },
      (e) => (relayError = errorText(e)),
    );
    listen<string>("relay-code", (e) => runInvite(e.payload, true)).then((f) => (closed ? f() : (unlisten = f)));
    let unlistenTmdb: (() => void) | undefined;
    listen<string>("relay-tmdb", (e) => tmdbFromPhone(e.payload)).then((f) => (closed ? f() : (unlistenTmdb = f)));
    return () => {
      closed = true;
      unlisten?.();
      unlistenTmdb?.();
      api.relayStop().catch(() => {});
      relayUrl = null;
    };
  });

  // Journal de diagnostic, sur tablette et téléviseur : le téléphone le télécharge par le relais (QR code).
  let diagUrl = $state<string | null>(null);
  let diagError = $state<string | null>(null);
  $effect(() => {
    if (page !== "player" || !diagAvailable || device.phone) return;
    let closed = false;
    api.relayStart().then(
      async (url) => {
        if (closed) return;
        const header = `Serveur : ${session.serverBrand} · interface ${device.tv ? "TV" : "tablette"}\n`;
        await invoke("relay_journal", { text: header + diagText() });
        if (!closed) diagUrl = url + "/journal";
      },
      (e) => (diagError = errorText(e)),
    );
    return () => {
      closed = true;
      api.relayStop().catch(() => {});
      diagUrl = null;
    };
  });

  const STEP: Record<string, string> = {
    run: t("settings.invite.steps.run"),
    ok: t("settings.invite.steps.ok"),
    err: t("settings.invite.steps.err"),
  };

  async function disconnectTracearr() {
    await api.tracearrLogout();
    toast(t("settings.services.tracearrDisconnected"));
    onChange();
  }

  async function saveMpv() {
    try {
      await api.setMpvPath(mpvPath || null);
      toast(t("settings.player.saved"));
      onChange();
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  function resetSidebar() {
    sidebar.order = [];
    sidebar.hidden = [];
    // Sur téléviseur, le menu reste replié hors sélection.
    sidebar.collapsed = device.tv;
    saveSidebar();
    toast(t("settings.interface.menuReset"));
  }

  // Serveurs utilisés : choisir Plex ou Jellyfin seul y bascule aussitôt (connexion gardée si possible).
  let serverKinds = $state<ServerKinds>("both");
  $effect.pre(() => {
    serverKinds = session.serverKinds;
  });
  async function chooseKinds(kinds: ServerKinds) {
    serverKinds = kinds;
    await api.setServerKinds(kinds);
    if (kinds === "both" || kinds === session.serverKind) return onChange();
    if (kinds === "jellyfin") {
      try {
        await api.resumeJellyfin();
        return onChange();
      } catch {
        // Pas de connexion Jellyfin gardée : l'écran de connexion Jellyfin.
      }
    }
    await api.forgetServer();
    onChange();
  }

  async function changeServer() {
    // Oublier le serveur ramène à l'écran de choix.
    await api.forgetServer();
    onChange();
  }

  // Téléviseur : ordre et visibilité des bibliothèques du menu, aux flèches de la télécommande.
  let sections = $state<Section[]>([]);
  const libraries = $derived(ordered(sections));

  async function moveLibrary(key: string, delta: number) {
    const keys = libraries.map((s) => s.key);
    const i = keys.indexOf(key);
    keys.splice(i, 1);
    keys.splice(i + delta, 0, key);
    sidebar.order = keys;
    saveSidebar();
    // La ligne a bougé : la sélection la suit (ou passe à l'autre flèche en bout de liste).
    await tick();
    const own = document.querySelector<HTMLButtonElement>(`[data-lib="${key}"][data-dir="${delta}"]`);
    const other = document.querySelector<HTMLButtonElement>(`[data-lib="${key}"][data-dir="${-delta}"]`);
    (own && !own.disabled ? own : other)?.focus();
  }

  function toggleLibrary(key: string) {
    sidebar.hidden = sidebar.hidden.includes(key) ? sidebar.hidden.filter((k) => k !== key) : [...sidebar.hidden, key];
    saveSidebar();
  }

  let theme = $state<Theme>(savedTheme());
  const THEME_LABELS: Record<Theme, string> = {
    jaune: t("settings.interface.themes.jaune"),
    rouge: t("settings.interface.themes.rouge"),
    vert: t("settings.interface.themes.vert"),
    violet: t("settings.interface.themes.violet"),
  };

  // Langue choisie (« auto » : celle du système) ; en changer recharge l'interface.
  const language = savedLanguage();

  function chooseTheme(th: Theme) {
    theme = th;
    applyTheme(th, true);
  }

  // Version installée, et recherche manuelle d'une nouvelle version.
  let version = $state("");
  getVersion().then((v) => (version = v), () => {});

  async function openNotes() {
    try {
      await showReleaseNotes();
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  async function checkUpdates() {
    try {
      if (!(await checkForUpdate())) toast(t("settings.account.upToDate", { version }));
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  async function logout() {
    await api.logout();
    onChange();
  }

  // Plex Home : la connexion automatique se règle depuis un compte complet (pas un profil géré).
  let autoSignIn = $state(false);
  let homeUsers = $state<HomeUser[]>([]);
  let homeCurrent = $state<string | null>(null);
  const restricted = $derived(homeUsers.find((u) => u.uuid === homeCurrent)?.restricted ?? false);

  async function toggleAutoSignIn() {
    try {
      await api.setAutoSignIn(autoSignIn);
    } catch (e) {
      autoSignIn = !autoSignIn;
      toast(errorText(e), true);
    }
  }

  onMount(() => {
    api.account().then((a) => (account = a), () => {});
    // Valeur à jour : la session reçue par la page date de son ouverture.
    api.session().then((s) => (autoSignIn = s.autoSignIn), () => {});
    if (session.serverKind === "plex" && session.plexRemembered) {
      api.homeUsers().then((h) => ((homeUsers = h.users), (homeCurrent = h.current)), () => {});
    }
    if (device.tv) api.sections().then((s) => (sections = s), () => {});
    if (nav.admin) loadPrefs();
    // Relais Maku Server : ses adresses remplissent les champs vides (et Seerr se connecte avec Plex).
    api.relayDiscover().then((r) => {
      if (!r) return;
      relayFound = r.url;
      if (r.seerr && !seerrUrl) seerrUrl = r.url + "/seerr";
      if (r.tracearr && !tracearrUrl) tracearrUrl = r.url + "/tracearr";
      if (!inviteSeerr && r.seerr) inviteSeerr = r.url + "/seerr";
      if (!inviteTracearr && r.tracearr) inviteTracearr = r.url + "/tracearr";
      if (r.applied) onChange();
    }, () => {});
  });
</script>

{#snippet link(id: string, label: string, icon: IconName)}
  <button class:active={page === id} aria-current={page === id ? "page" : undefined} onclick={() => choosePage(id)} onfocus={() => device.tv && previewPage(id)}>
    <Icon name={icon} size={device.tv ? 20 : 17} />
    <span>{label}</span>
  </button>
{/snippet}

{#snippet qrToggle(id: string)}
  <button class="btn" onclick={() => toggleQr(id)}>
    <Icon name="image" size={16} />
    {qr === id ? t("settings.hideQr") : t("settings.showQr")}
  </button>
{/snippet}

<div class="page settings">
  <!-- Rail des rubriques (Netflix, Wholphin) : sur téléviseur, la rubrique s'affiche au passage des flèches ;
       droite entre dans le panneau, gauche revient au rail, sur la rubrique quittée. -->
  <nav class="menu" data-nav-zone="settings-rail">
    <div class="menu-group">Maku</div>
    {@render link("account", t("settings.menu.account"), "user")}
    {@render link("player", t("settings.menu.player"), "play")}
    {@render link("interface", t("settings.menu.interface"), "menu")}
    {@render link("services", t("settings.menu.services"), "compass")}

    <div class="menu-group">{t("settings.menu.server", { name: session.serverName ?? "" })}</div>
    {#if nav.admin}
      {@render link("server", t("settings.menu.serverSettings"), "server")}
    {:else}
      <p class="locked"><Icon name="lock" size={14} /> {t("settings.menu.locked")}</p>
    {/if}
  </nav>

  <!-- `data-nav-lines` : haut / bas passent par chaque ligne, dans l'ordre (télécommande). -->
  <div class="panel" data-nav-zone="settings-panel" data-nav-lines>
    {#if page === "account"}
      <h1 class="title">{t("settings.menu.account")}</h1>
      <!-- En-tête du profil, aux couleurs du thème : photo, nom, badges, et le serveur en cours. -->
      <section class="card hero">
        <div class="hero-row">
          {#if account?.thumb}<img class="avatar" src={account.thumb} alt="" />{:else}<span class="avatar ph"><Icon name="user" size={34} /></span>{/if}
          <div class="hero-text">
            <div class="big">{account?.username ?? "…"}</div>
            {#if account?.email}<div class="muted">{account.email}</div>{/if}
            <div class="badges">
              {#if account?.plexPass}<span class="badge pass">Plex Pass</span>{/if}
              <span class="badge outline">{(account?.admin ?? nav.admin) ? t("settings.account.admin") : t("settings.account.shared")}</span>
            </div>
          </div>
        </div>
        <div class="hero-actions">
          <span class="chip"><Icon name="server" size={16} /> {t("settings.account.server")} <strong>{session.serverName}</strong></span>
          {#if device.phone && homeUsers.length > 1}
            <button class="btn" onclick={() => (nav.profiles = true)}><Icon name="user" size={16} /> {t("sidebar.switchUser")}</button>
          {/if}
          <button class="btn" onclick={changeServer}>{t("settings.account.changeServer")}</button>
          <button class="btn" onclick={logout}><Icon name="logout" size={16} /> {t("settings.account.logout")}</button>
        </div>
      </section>

      <section class="card list">
        {#if homeUsers.length > 1 && !restricted}
          <label class="line">
            <div class="line-text">
              <strong>{t("settings.account.autoSignIn")}</strong>
              <span class="muted small">{t("settings.account.autoSignInHint")}</span>
            </div>
            <input type="checkbox" class="switch" bind:checked={autoSignIn} onchange={toggleAutoSignIn} />
          </label>
        {/if}
        <div class="line">
          <div class="line-text">
            <strong>{t("settings.account.serverKinds")}</strong>
            <span class="muted small">{t("settings.account.serverKindsHint")}</span>
          </div>
          <div class="seg" role="radiogroup" aria-label={t("settings.account.serverKinds")}>
            {#each [{ id: "plex", label: "Plex" }, { id: "jellyfin", label: "Jellyfin / Emby" }, { id: "both", label: t("settings.account.bothKinds") }] as k (k.id)}
              <button
                class:on={serverKinds === k.id}
                role="radio"
                aria-checked={serverKinds === k.id}
                onclick={() => serverKinds !== k.id && chooseKinds(k.id as ServerKinds)}
              >
                {k.label}
              </button>
            {/each}
          </div>
        </div>
        <div class="line">
          <div class="line-text"><strong>{t("settings.account.version", { version })}</strong></div>
          <div class="row">
            {#if !session.store}
              <button class="btn" disabled={update.checking} onclick={checkUpdates}>
                <Icon name="restart" size={16} />
                {update.checking ? t("settings.account.checking") : t("settings.account.checkUpdates")}
              </button>
            {/if}
            <button class="btn" disabled={update.checking} onclick={openNotes}>
              <Icon name="library" size={16} />
              {t("settings.account.releaseNotes")}
            </button>
          </div>
        </div>
        <!-- Ko-fi : pas dans les versions des magasins, dont les règles interdisent les dons par un lien externe.
             Sur téléviseur, pas de navigateur : un QR code, affiché à la demande. -->
        {#if !session.store}
          <div class="line">
            <div class="line-text">
              <strong class="kofi-title"><KofiLogo size={20} /> {t("settings.account.support")}</strong>
              <span class="muted small">{t("settings.account.supportHint")}</span>
            </div>
            {#if device.tv}
              {@render qrToggle("kofi")}
            {:else}
              <button class="btn kofi-btn" onclick={() => invoke("open_url", { url: KOFI }).catch((e) => toast(errorText(e), true))}>{t("settings.account.supportButton")}</button>
            {/if}
          </div>
          {#if qr === "kofi"}
            <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
            <div class="qr-open" tabindex="0"><QrCode text={KOFI} size={200} /></div>
          {/if}
        {/if}
      </section>
    {:else if page === "player"}
      <h1 class="title">{t("settings.menu.player")}</h1>
      <section class="card list">
        {#if passthroughAvailable}
          <!-- Android : son envoyé tel quel à l'ampli, ou décodé sur l'appareil. -->
          <div class="line">
            <div class="line-text">
              <strong>{t("settings.player.passthrough")}</strong>
              <span class="muted small">{t("settings.player.passthroughHint")}</span>
              <span class="muted small">
                {audioOut?.formats.length
                  ? t("settings.player.passthroughDetected", { formats: audioOut.formats.join(", "), channels: audioOut.channels })
                  : t("settings.player.passthroughNone", { channels: audioOut?.channels ?? 2 })}
              </span>
            </div>
            <div class="seg" role="radiogroup" aria-label={t("settings.player.passthrough")}>
              {#each [{ id: "auto", label: t("settings.player.passthroughAuto") }, { id: "off", label: t("settings.player.passthroughOff") }] as m (m.id)}
                <button role="radio" class:on={passthrough === m.id} aria-checked={passthrough === m.id} onclick={() => choosePassthrough(m.id as "auto" | "off")}>
                  {m.label}
                </button>
              {/each}
            </div>
          </div>
        {:else if !device.tv}
          <div class="line">
            <div class="line-text">
              <strong>{t("settings.player.mpvPath")}</strong>
              <span class="muted small">{t("settings.player.intro")}</span>
            </div>
            <div class="row grow-row">
              <input class="grow" bind:value={mpvPath} placeholder="C:\Program Files\mpv\mpv.exe" />
              <button class="btn" onclick={saveMpv}>{t("settings.player.save")}</button>
            </div>
          </div>
        {/if}
        <!-- Épisode suivant au générique, après un compte à rebours (PlayerOverlay.svelte). -->
        <div class="line">
          <div class="line-text">
            <strong>{t("settings.player.autoNext")}</strong>
            <span class="muted small">{t("settings.player.autoNextHint")}</span>
          </div>
          <div class="seg" role="radiogroup" aria-label={t("settings.player.autoNext")}>
            {#each NEXT_DELAYS as d (d)}
              <button role="radio" class:on={autoNext.delay === d} aria-checked={autoNext.delay === d} onclick={() => setNextDelay(d)}>
                {d ? `${d} s` : t("settings.player.autoNextOff")}
              </button>
            {/each}
          </div>
        </div>
        <label class="line">
          <span class="line-text">
            <strong>{t("settings.player.lyrics")}</strong>
            <span class="muted small">{t("settings.player.lyricsHint")}</span>
          </span>
          <input type="checkbox" class="switch" checked={lyricsPrefs.enabled} onchange={(e) => setLyricsEnabled(e.currentTarget.checked)} />
        </label>
        {#if diagAvailable}
          <!-- Android : journal à transmettre quand l'appareil se bloque ou qu'une lecture échoue (DiagLog.kt). -->
          <div class="line">
            <div class="line-text">
              <strong>{t("settings.player.diag")}</strong>
              <span class="muted small">{t("settings.player.diagHint")}</span>
            </div>
            <div class="row">
              {#if !device.tv}<button class="btn" onclick={diagShare}>{t("settings.player.diagShare")}</button>{/if}
              {#if !device.phone}{@render qrToggle("diag")}{/if}
            </div>
          </div>
          {#if qr === "diag"}
            <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
            <div class="qr-open" tabindex="0">
              {#if diagUrl}<QrCode text={diagUrl} size={device.tv ? 240 : 160} />{:else}<div class="qr-wait skeleton"></div>{/if}
              <p class="muted">{t("settings.player.diagPhoneHint")}</p>
            </div>
            {#if diagError}<p class="error">{diagError}</p>{/if}
          {/if}
        {/if}
      </section>
    {:else if page === "interface"}
      <h1 class="title">{t("settings.menu.interface")}</h1>
      <section class="card list">
        <div class="line">
          <div class="line-text"><strong>{t("settings.interface.color")}</strong></div>
          <div class="themes" role="radiogroup" aria-label={t("settings.interface.color")}>
            {#each THEMES as th (th)}
              <button
                class="theme"
                class:on={theme === th}
                role="radio"
                aria-checked={theme === th}
                aria-label={THEME_LABELS[th]}
                title={THEME_LABELS[th]}
                style:--swatch={THEME_COLORS[th]}
                onclick={() => chooseTheme(th)}
              >
                <span class="swatch">{#if theme === th}<Icon name="check" size={18} />{/if}</span>
              </button>
            {/each}
          </div>
        </div>
        <div class="line">
          <div class="line-text"><strong>{t("settings.interface.language")}</strong></div>
          <div class="field-slot">
            <TrackSelect
              value={language}
              options={[{ value: "auto", label: t("settings.interface.languageAuto") }, ...LANGUAGES.map((l) => ({ value: l.id, label: l.label }))]}
              label={t("settings.interface.language")}
              onchange={(id) => id !== language && setLanguage(id as Parameters<typeof setLanguage>[0])}
            />
          </div>
        </div>
        {#if device.tv}
          <label class="line">
            <div class="line-text">
              <strong>{t("settings.interface.simpleBar")}</strong>
              <span class="muted small">{t("settings.interface.simpleBarHint")}</span>
            </div>
            <input type="checkbox" class="switch" bind:checked={sidebar.simple} onchange={saveSidebar} />
          </label>
        {:else}
          <label class="line">
            <div class="line-text">
              <strong>{t("settings.interface.collapsed")}</strong>
              <span class="muted small">{t("settings.interface.collapsedHint")}</span>
            </div>
            <input type="checkbox" class="switch" bind:checked={sidebar.collapsed} onchange={saveSidebar} />
          </label>
          <div class="line">
            <div class="line-text">
              <strong>{t("settings.interface.resetMenu")}</strong>
              <span class="muted small">{t("settings.interface.reorderHint")}</span>
            </div>
            <button class="btn" onclick={resetSidebar}>{t("settings.interface.resetMenu")}</button>
          </div>
        {/if}
      </section>

      <!-- Notes des sites sur les fiches des films et des séries : celles que le serveur connaît, et Allociné. -->
      <h2 class="group-title">{t("settings.ratings.title")}</h2>
      <section class="card list">
        {#each (nav.serverKind === "plex" ? ["imdb", "rtCritic", "rtAudience", "tmdb", "allocinePress", "allocineUsers"] : ["community", "rtCritic", "allocinePress", "allocineUsers"]) as RatingSite[] as site (site)}
          <label class="line">
            <span class="rating-logo"><RatingLogo source={site} /></span>
            <span class="line-text">
              <strong>{t(`settings.ratings.${site}`)}</strong>
              {#if site === "allocinePress" || site === "allocineUsers"}<span class="muted small">{t("settings.ratings.allocineHint")}</span>{/if}
            </span>
            <input type="checkbox" class="switch" checked={ratingPrefs[site]} onchange={(e) => setRatingSite(site, e.currentTarget.checked)} />
          </label>
        {/each}
      </section>

      <h2 class="group-title">{t("settings.interface.home")}</h2>
      <section class="card">
        <p class="muted">{t("settings.interface.homeHint")}</p>
        <HomeEditor />
      </section>

      {#if device.tv}
        <h2 class="group-title">{t("settings.interface.libraries")}</h2>
        <section class="card">
          <div class="libs">
            {#each libraries as s, i (s.key)}
              {@const off = sidebar.hidden.includes(s.key)}
              <div class="lib" class:off>
                <Icon name={s.kind === "movie" ? "film" : s.kind === "artist" ? "music" : "tv"} />
                <span class="lib-name">{s.title}</span>
                <button class="btn round" data-lib={s.key} data-dir="-1" disabled={i === 0} onclick={() => moveLibrary(s.key, -1)} aria-label={t("settings.interface.moveUp", { title: s.title })}>
                  <Icon name="chevronUp" size={18} />
                </button>
                <button class="btn round" data-lib={s.key} data-dir="1" disabled={i === libraries.length - 1} onclick={() => moveLibrary(s.key, 1)} aria-label={t("settings.interface.moveDown", { title: s.title })}>
                  <Icon name="chevronDown" size={18} />
                </button>
                <button class="btn round" onclick={() => toggleLibrary(s.key)} aria-label={off ? t("settings.interface.show", { title: s.title }) : t("settings.interface.hide", { title: s.title })}>
                  <Icon name={off ? "eyeOff" : "eye"} size={18} />
                </button>
              </div>
            {/each}
          </div>
          <div><button class="btn" onclick={resetSidebar}>{t("settings.interface.resetMenu")}</button></div>
        </section>
      {/if}
    {:else if page === "services"}
      <h1 class="title">{t("settings.menu.services")}</h1>
      <section class="card list">
        <div class="line">
          <div class="line-text">
            <strong>{t("settings.invite.quickSetup")}</strong>
            <span class="muted small">
              {device.phone ? t("settings.invite.photoHint") : device.mobile ? t("settings.invite.phoneHint") : t("settings.invite.pasteHint")}
            </span>
          </div>
          {#if device.phone}
            <label class="btn primary" class:disabled={applying?.step === "running"}>
              <Icon name="image" size={18} /> {t("settings.invite.pickPhoto")}
              <input type="file" accept="image/*" hidden onchange={onInviteImage} />
            </label>
          {:else if device.mobile}
            {@render qrToggle("invite")}
          {:else}
            <label class="btn">
              <Icon name="image" size={16} /> {t("settings.invite.importImage")}
              <input type="file" accept="image/*" hidden onchange={onInviteImage} />
            </label>
          {/if}
        </div>
        {#if device.mobile && !device.phone}
          {#if qr === "invite"}
            <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
            <div class="qr-open" tabindex="0">
              {#if relayUrl}<QrCode text={relayUrl} size={device.tv ? 260 : 190} />{:else}<div class="qr-wait skeleton"></div>{/if}
              <p class="muted">{t("settings.invite.phoneHint")}</p>
            </div>
            {#if relayError}<p class="error">{relayError}</p>{/if}
          {/if}
        {:else}
          <div class="line-body row">
            <input class="grow" bind:value={pasted} placeholder="MAKU-INVITE:…" spellcheck="false" />
            <button class="btn primary" disabled={!pasted.trim() || applying?.step === "running"} onclick={() => runInvite(pasted, false)}>
              {t("settings.invite.apply")}
            </button>
          </div>
        {/if}
        {#if applying}
          <ul class="steps line-body">
            {#if applying.seerr}<li class={applying.seerr}>{t("settings.invite.stepSeerr", { status: applying.seerrError ?? STEP[applying.seerr] })}</li>{/if}
            {#if applying.tracearr}
              <li class={applying.tracearr}>
                {t("settings.invite.stepTracearr", {
                  status: applying.tracearrError ?? (applying.authUrl ? t("settings.invite.authorizeOnPhone") : STEP[applying.tracearr]),
                })}
              </li>
            {/if}
          </ul>
        {/if}
        {#if inviteError}<p class="error line-body">{inviteError}</p>{/if}
      </section>

      {#if relayFound}
        <p class="relay-note"><Icon name="lock" size={16} /> {t("settings.services.relayFound", { url: relayFound })}</p>
      {/if}

      <h2 class="group-title">Seerr {#if session.seerrUrl}<span class="ok">● {t("settings.services.connected")}</span>{/if}</h2>
      <section class="card">
        <p class="muted">
          {session.serverKind === "jellyfin" ? t("settings.services.seerrIntroJellyfin") : t("settings.services.seerrIntro")}
        </p>
        <label>{t("settings.services.seerrAddress")} <input bind:value={seerrUrl} placeholder="https://seerr.mondomaine.fr" /></label>
        {#if session.serverKind === "jellyfin"}
          <label>{t("login.username")} <input bind:value={seerrUser} autocomplete="username" spellcheck="false" /></label>
          <label>{t("login.password")} <PasswordInput bind:value={seerrPassword} /></label>
        {/if}
        <div>
          <button class="btn primary" disabled={seerrBusy} onclick={saveSeerr}>
            {seerrBusy
              ? t("settings.services.connecting")
              : session.serverKind === "jellyfin"
                ? t("settings.services.connectWithJellyfin")
                : t("settings.services.connectWithPlex")}
          </button>
        </div>
        {#if seerrError}<p class="error">{seerrError}</p>{/if}
      </section>

      <h2 class="group-title">Tracearr {#if session.tracearrConnected}<span class="ok">● {t("settings.services.connected")}</span>{/if}</h2>
      <section class="card">
        <p class="muted">{t("settings.services.tracearrIntro")}</p>
        <label>{t("settings.services.tracearrAddress")} <input bind:value={tracearrUrl} placeholder="https://tracearr.mondomaine.fr" /></label>
        <div class="row">
          <button class="btn primary" disabled={tracearrBusy} onclick={connectTracearr}>
            {tracearrBusy ? t("settings.services.waitingBrowser") : t("settings.services.connectWithPlex")}
          </button>
          {#if session.tracearrConnected}
            <button class="btn" onclick={disconnectTracearr}>{t("settings.services.logout")}</button>
          {/if}
        </div>
        {#if tracearrError}<p class="error">{tracearrError}</p>{/if}
      </section>

      <h2 class="group-title">{t("settings.tmdb.title")} {#if session.tmdb}<span class="ok">● {t("settings.tmdb.active")}</span>{/if}</h2>
      <section class="card">
        <p class="muted">
          {t("settings.tmdb.intro")}
          <button class="link" onclick={() => invoke("open_url", { url: "https://www.themoviedb.org/settings/api" }).catch(() => {})}>
            themoviedb.org
          </button>
        </p>
        <!-- Tablette et téléviseur : la clé se colle sur le téléphone, plutôt qu'au clavier de la télécommande. -->
        {#if device.mobile && !device.phone && relayUrl}
          <div class="row">{@render qrToggle("tmdb")}</div>
          {#if qr === "tmdb"}
            <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
            <div class="qr-open" tabindex="0">
              <QrCode text={relayUrl + "/tmdb"} size={device.tv ? 240 : 160} />
              <p class="muted">{t("settings.tmdb.phoneHint")}</p>
            </div>
          {/if}
        {/if}
        <label>
          {t("settings.tmdb.key")}
          <input bind:value={tmdbKey} placeholder={session.tmdb ? "••••••••••••" : ""} spellcheck="false" autocomplete="off" />
        </label>
        <div class="row">
          <button class="btn primary" disabled={tmdbBusy || !tmdbKey.trim()} onclick={() => saveTmdb()}>
            {tmdbBusy ? t("settings.services.connecting") : t("settings.tmdb.save")}
          </button>
          {#if session.tmdb}
            <button class="btn" disabled={tmdbBusy} onclick={() => saveTmdb(true)}>{t("settings.tmdb.remove")}</button>
          {/if}
        </div>
        {#if tmdbError}<p class="error">{tmdbError}</p>{/if}
      </section>

      {#if nav.admin}
        <h2 class="group-title">{t("settings.invite.title")}</h2>
        <section class="card">
          <p class="muted">{t("settings.invite.intro")}</p>
          <p class="muted small">{t("settings.invite.relayHint")}</p>
          <div class="invite-fields">
            <label>{t("settings.services.seerrAddress")} <input bind:value={inviteSeerr} placeholder="https://seerr.mondomaine.fr" /></label>
            <label>{t("settings.services.tracearrAddress")} <input bind:value={inviteTracearr} placeholder="https://tracearr.mondomaine.fr" /></label>
          </div>
          {#if inviteCode}
            <div class="invite">
              <!-- Téléviseur : un arrêt de la télécommande, pour descendre jusqu'au QR code. -->
              <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
              <div class="invite-qr" bind:this={inviteQr} tabindex={device.tv ? 0 : undefined}><QrCode text={inviteCode} size={170} /></div>
              <div class="invite-side">
                <code class="invite-code">{inviteCode}</code>
                {#if !device.tv}
                  <div class="row">
                    <button class="btn" onclick={copyInviteCode}><Icon name="copy" size={16} /> {t("settings.invite.copyCode")}</button>
                    <button class="btn" onclick={copyInviteImage}><Icon name="image" size={16} /> {t("settings.invite.copyImage")}</button>
                  </div>
                {/if}
              </div>
            </div>
          {/if}
        </section>
      {/if}
    {:else if page === "server" && nav.admin}
      <h1 class="title">{t("settings.menu.serverSettings")}</h1>
      <!-- Réglages du serveur Plex, comme dans Plex Web : un onglet par rubrique, au lieu de neuf entrées dans le rail. -->
      <div class="server-tabs">
        {#each [{ id: "dashboard", label: t("settings.menu.dashboard") }, ...(session.serverKind === "plex" ? [{ id: "users", label: t("settings.users.title") }] : []), ...groups.map((g) => ({ id: g, label: GROUPS[g].label }))] as tab (tab.id)}
          <button class="btn" class:primary={serverTab === tab.id} aria-pressed={serverTab === tab.id} onclick={() => (serverTab = tab.id)}>{tab.label}</button>
        {/each}
      </div>
      {#if serverTab === "dashboard"}
        <ServerDashboard />
      {:else if serverTab === "users"}
        <ServerUsers />
      {:else if prefsError}
        <p class="error">{prefsError}</p>
      {:else if !prefs}
        <div class="skeleton" style:height="300px"></div>
      {:else}
        <label class="switch-line adv-line">
          <span><strong>{t("settings.menu.advanced")}</strong></span>
          <input type="checkbox" class="switch" bind:checked={advanced} />
        </label>
        <ServerPrefs prefs={prefs.filter((p) => p.group === serverTab)} {advanced} onSaved={loadPrefs} />
      {/if}
    {/if}
  </div>
</div>

<style>
  .qr-wait {
    flex: none;
    width: 190px;
    height: 190px;
  }

  .grow {
    flex: 1;
    min-width: 220px;
  }

  .steps {
    margin: 4px 0 0;
    padding: 0;
    list-style: none;
  }

  .steps li {
    position: relative;
    padding: 3px 0 3px 18px;
  }

  .steps li::before {
    content: "";
    position: absolute;
    top: 50%;
    left: 0;
    width: 9px;
    height: 9px;
    margin-top: -4px;
    border-radius: 50%;
    background: var(--accent);
  }

  .steps li.ok::before {
    background: var(--ok);
  }

  .steps li.err::before {
    background: var(--danger);
  }

  .invite-fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .invite {
    display: flex;
    align-items: center;
    gap: 24px;
  }

  .invite-qr {
    flex: none;
    border-radius: 12px;
  }

  .invite-qr:focus {
    outline: 3px solid var(--accent);
    outline-offset: 6px;
  }

  .invite-side {
    display: grid;
    gap: 12px;
    min-width: 0;
  }

  .invite-code {
    display: block;
    padding: 10px 12px;
    overflow: hidden;
    border-radius: var(--radius);
    background: #00000040;
    color: var(--text-dim);
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .themes {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
  }

  .theme {
    display: grid;
    justify-items: center;
    gap: 8px;
    padding: 4px;
    border: none;
    border-radius: 12px;
    background: none;
    color: var(--text-dim);
    font-weight: 600;
  }

  .theme.on {
    color: var(--text);
  }

  .swatch {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: var(--swatch);
    color: #fff;
    box-shadow:
      0 0 0 3px var(--bg-elev),
      0 0 0 5px transparent;
    transition: box-shadow 0.15s, transform 0.15s;
  }

  .theme:hover .swatch {
    transform: scale(1.06);
  }

  .theme.on .swatch {
    box-shadow:
      0 0 0 3px var(--bg-elev),
      0 0 0 5px var(--swatch);
  }

  .libs {
    display: grid;
    gap: 6px;
  }

  .lib {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 6px 6px 14px;
    border-radius: var(--radius);
    background: #ffffff08;
  }

  .lib-name {
    flex: 1;
    font-weight: 600;
  }

  .lib.off .lib-name,
  .lib.off > :global(svg) {
    opacity: 0.4;
  }

  .settings {
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr);
    gap: 32px;
    align-items: start;
  }

  .menu {
    position: sticky;
    top: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .menu-group {
    padding: 18px 12px 6px;
    overflow: hidden;
    color: var(--text-faint);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .menu-group:first-child {
    padding-top: 4px;
  }

  .menu button {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border: none;
    border-radius: 10px;
    background: none;
    color: var(--text-dim);
    font-weight: 500;
    text-align: left;
  }

  .menu button:hover {
    background: #ffffff0d;
    color: var(--text);
  }

  .menu button.active {
    background: #ffffff14;
    color: var(--text);
  }

  .menu button.active :global(svg) {
    color: var(--accent);
  }

  .locked {
    display: flex;
    gap: 8px;
    margin: 4px 12px;
    color: var(--text-faint);
    font-size: 13px;
  }

  .title {
    margin: 4px 0 18px;
    font-size: 30px;
  }

  .card {
    display: grid;
    gap: 12px;
    max-width: 720px;
    margin-bottom: 16px;
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: #ffffff05;
  }


  .card p {
    margin: 0;
  }

  label {
    display: grid;
    gap: 4px;
    color: var(--text-dim);
    font-size: 13px;
  }

  /* Un label qui sert de bouton (choix de fichier) garde l'aspect du bouton : icône et texte sur une ligne. */
  label.btn {
    display: inline-flex;
    gap: 8px;
    color: var(--text);
    font-size: inherit;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .relay-note {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    color: var(--ok);
    font-size: 14px;
  }

  .ok {
    margin-left: 4px;
    color: var(--ok);
    font-size: 13px;
    font-weight: 600;
  }

  /* ---------- Lignes et groupes, les mêmes sur toutes les rubriques ---------- */

  .group-title {
    display: flex;
    align-items: center;
    gap: 12px;
    max-width: 720px;
    margin: 26px 0 10px;
    font-size: 17px;
  }

  /* Contenu sous une ligne (champ, étapes, erreur) : aligné sur les lignes de la carte. */
  .line-body {
    padding-bottom: 16px;
  }

  .grow-row {
    flex: 1;
    min-width: 260px;
  }

  .field-slot {
    display: flex;
    width: 280px;
    max-width: 100%;
  }

  /* QR code déplié sous sa ligne. */
  .qr-open {
    display: flex;
    align-items: center;
    gap: 24px;
    margin: 0 0 16px;
    border-radius: 14px;
  }

  .qr-open p {
    max-width: 320px;
    margin: 0;
  }

  .qr-open:focus {
    outline: 3px solid var(--accent);
    outline-offset: 8px;
  }

  .kofi-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .server-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    max-width: 900px;
    margin-bottom: 20px;
  }

  .adv-line {
    max-width: 720px;
    margin-bottom: 16px;
  }

  /* ---------- Téléviseur : rail plus grand, la rubrique ouverte bien marquée ---------- */

  :global(html.tv) .settings {
    grid-template-columns: 280px minmax(0, 1fr);
    gap: 40px;
  }

  :global(html.tv) .menu {
    padding: 8px;
    border-radius: 18px;
    background: #ffffff08;
  }

  :global(html.tv) .menu button {
    gap: 14px;
    padding: 13px 16px;
    border-radius: 12px;
    font-size: 16px;
  }

  :global(html.tv) .menu button.active {
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    color: var(--text);
  }

  :global(html.tv) .menu button:focus {
    outline: 3px solid var(--accent);
    outline-offset: 0;
  }

  :global(html.tv) .card {
    max-width: none;
  }

  /* ---------- Page Compte ---------- */

  .hero {
    position: relative;
    gap: 18px;
    overflow: hidden;
  }

  .hero::before {
    content: "";
    position: absolute;
    inset: 0;
    background:
      radial-gradient(70% 140% at 8% 10%, color-mix(in srgb, var(--accent) 30%, transparent), transparent 65%),
      linear-gradient(135deg, color-mix(in srgb, var(--accent) 12%, transparent), transparent 55%);
    pointer-events: none;
  }

  .hero > * {
    position: relative;
  }

  .hero-row {
    display: flex;
    align-items: center;
    gap: 18px;
  }

  .hero-text {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .hero-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 40px;
    padding: 0 14px;
    border-radius: var(--radius);
    background: #00000033;
    color: var(--text-dim);
  }

  .chip strong {
    color: var(--text);
  }

  .avatar {
    width: 80px;
    height: 80px;
    flex: none;
    border-radius: 50%;
    object-fit: cover;
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 60%, transparent), 0 10px 30px #0008;
  }

  .list {
    gap: 0;
    padding: 4px 20px;
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px 20px;
    padding: 16px 0;
    border-top: 1px solid var(--border);
  }

  .line:first-child {
    border-top: none;
  }

  /* Pastille du site devant son nom (Notes des films et séries), sur une même largeur pour aligner les noms. */
  .rating-logo {
    display: flex;
    flex: none;
    justify-content: center;
    width: 40px;
    margin-right: -8px;
  }

  .line-text {
    display: grid;
    flex: 1;
    gap: 3px;
    min-width: 200px;
  }

  .line-stack {
    display: grid;
  }

  /* Choix à plusieurs positions (Plex / Jellyfin / Les deux). */
  .seg {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: #00000040;
  }

  .seg button {
    height: 34px;
    padding: 0 14px;
    border: none;
    border-radius: 9px;
    background: none;
    color: var(--text-dim);
    font-weight: 600;
    transition: background 0.15s, color 0.15s;
  }

  .seg button:hover {
    color: var(--text);
  }

  .seg button.on {
    background: var(--accent);
    color: var(--accent-text);
  }

  .kofi-btn {
    border-color: transparent;
    background: #ff5e5b;
    color: #fff;
    font-weight: 600;
  }

  .kofi-btn:hover {
    background: #ff7572;
  }

  .avatar.ph {
    display: grid;
    place-items: center;
    background: var(--bg-hover);
    color: var(--text-faint);
  }

  .big {
    font-size: 22px;
    font-weight: 700;
  }

  .badges {
    display: flex;
    gap: 6px;
    margin-top: 8px;
  }

  .badge.pass {
    background: var(--accent);
    color: var(--accent-text);
  }

  .switch-line {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    color: var(--text);
    font-size: 14px;
    cursor: pointer;
  }

  .switch-line span {
    display: grid;
    gap: 2px;
  }


  /* ---------- Téléphone : rubriques en bande défilante, contenu en pleine largeur ---------- */

  :global(html.phone) .settings {
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
  }

  :global(html.phone) .menu {
    position: static;
    flex-direction: row;
    gap: 6px;
    margin: 0 -16px;
    padding: 4px 16px;
    overflow-x: auto;
    scrollbar-width: none;
  }

  :global(html.phone) .menu button {
    flex: none;
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: 18px;
    white-space: nowrap;
  }

  :global(html.phone) .menu-group,
  :global(html.phone) .locked {
    display: none;
  }


  :global(html.phone) .invite,
  .link {
    padding: 0;
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    text-decoration: underline;
  }

  label.btn.disabled {
    opacity: 0.5;
    pointer-events: none;
  }

  :global(html.phone) .qr-open {
    flex-direction: column;
    align-items: flex-start;
  }

  :global(html.phone) .invite-fields {
    grid-template-columns: minmax(0, 1fr);
  }
</style>
