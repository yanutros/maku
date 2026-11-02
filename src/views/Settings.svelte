<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import QrCode from "../components/QrCode.svelte";
  import KofiLogo from "../components/KofiLogo.svelte";
  import { applyInvite, decodeInvite, encodeInvite, readQrImage, type InviteStatus } from "../lib/invite";
  import { api, errorText, tracearrLogin, type Account, type Pref, type Section, type ServerKinds, type Session } from "../lib/api";
  import { go, nav, toast } from "../lib/nav.svelte";
  import { ordered, saveSidebar, sidebar } from "../lib/sidebar.svelte";
  import { device } from "../lib/device";
  import { forgetRemoteLogos } from "../lib/logos";
  import { getVersion } from "@tauri-apps/api/app";
  import { checkForUpdate, showReleaseNotes, update } from "../lib/update.svelte";
  import { applyTheme, savedTheme, THEME_COLORS, THEMES, type Theme } from "../lib/theme";
  import Icon, { type IconName } from "../components/Icon.svelte";
  import ServerPrefs from "../components/ServerPrefs.svelte";
  import ServerDashboard from "../components/ServerDashboard.svelte";
  import { LANGUAGES, savedLanguage, setLanguage, t } from "../lib/i18n";

  let { session, onChange }: { session: Session; onChange: () => void } = $props();

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

  let page = $state("account");
  // Téléphone : journal d'Android Auto (AutoLog.kt), pour comprendre un échec en voiture.
  const autoBridge = (window as { PlexrDevice?: { autoLog?: () => string } }).PlexrDevice;
  let autoLog = $state(autoBridge?.autoLog ? autoBridge.autoLog() : null);
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
  let tracearrError = $state<string | null>(null);

  // --- TMDB : logos des titres que Plex n'a pas ---
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
    } catch (e) {
      tmdbError = errorText(e);
    } finally {
      tmdbBusy = false;
    }
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
    return () => {
      closed = true;
      unlisten?.();
      api.relayStop().catch(() => {});
      relayUrl = null;
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

  onMount(() => {
    api.account().then((a) => (account = a), () => {});
    if (device.tv) api.sections().then((s) => (sections = s), () => {});
    if (nav.admin) loadPrefs();
  });
</script>

{#snippet link(id: string, label: string, icon: IconName)}
  <button class:active={page === id} onclick={() => (page = id)}><Icon name={icon} size={17} /> {label}</button>
{/snippet}

<div class="page settings">
  <nav class="menu">
    <div class="menu-group">Maku</div>
    {@render link("account", t("settings.menu.account"), "user")}
    {@render link("services", t("settings.menu.services"), "compass")}
    {#if !device.tv}{@render link("player", t("settings.menu.player"), "play")}{/if}
    {@render link("interface", t("settings.menu.interface"), "menu")}

    <div class="menu-group">{t("settings.menu.server", { name: session.serverName ?? "" })}</div>
    {#if nav.admin}
      {@render link("dashboard", t("settings.menu.dashboard"), "server")}
      {#each groups as g (g)}{@render link(g, GROUPS[g].label, GROUPS[g].icon)}{/each}
      <label class="adv-toggle">
        <input type="checkbox" bind:checked={advanced} />
        {t("settings.menu.advanced")}
      </label>
    {:else}
      <p class="locked"><Icon name="lock" size={14} /> {t("settings.menu.locked")}</p>
    {/if}
  </nav>

  <div class="panel">
    {#if page === "account"}
      <h1 class="title">{t("settings.menu.account")}</h1>
      <section class="card account">
        {#if account?.thumb}<img class="avatar" src={account.thumb} alt="" />{:else}<span class="avatar ph"><Icon name="user" size={30} /></span>{/if}
        <div>
          <div class="big">{account?.username ?? "…"}</div>
          {#if account?.email}<div class="muted">{account.email}</div>{/if}
          <div class="badges">
            {#if account?.plexPass}<span class="badge pass">Plex Pass</span>{/if}
            <span class="badge outline">{(account?.admin ?? nav.admin) ? t("settings.account.admin") : t("settings.account.shared")}</span>
          </div>
        </div>
      </section>
      <section class="card">
        <p>{t("settings.account.server")} <strong>{session.serverName}</strong></p>
        <div class="row">
          <button class="btn" onclick={changeServer}>{t("settings.account.changeServer")}</button>
          <button class="btn" onclick={logout}><Icon name="logout" size={16} /> {t("settings.account.logout")}</button>
        </div>
      </section>
      <section class="card">
        <h2>{t("settings.account.serverKinds")}</h2>
        <div class="languages" role="radiogroup" aria-label={t("settings.account.serverKinds")}>
          {#each [{ id: "plex", label: "Plex" }, { id: "jellyfin", label: "Jellyfin" }, { id: "both", label: t("settings.account.bothKinds") }] as k (k.id)}
            <button
              class="btn"
              class:primary={serverKinds === k.id}
              role="radio"
              aria-checked={serverKinds === k.id}
              onclick={() => serverKinds !== k.id && chooseKinds(k.id as ServerKinds)}
            >
              {k.label}
            </button>
          {/each}
        </div>
        <p class="muted small">{t("settings.account.serverKindsHint")}</p>
      </section>
      <section class="card">
        <p>{t("settings.account.version", { version })}</p>
        <div class="row">
          <button class="btn" disabled={update.checking} onclick={checkUpdates}>
            <Icon name="restart" size={16} />
            {update.checking ? t("settings.account.checking") : t("settings.account.checkUpdates")}
          </button>
          <button class="btn" disabled={update.checking} onclick={openNotes}>
            <Icon name="library" size={16} />
            {t("settings.account.releaseNotes")}
          </button>
        </div>
      </section>
      {#if autoLog !== null}
        <section class="card">
          <h2>Android Auto</h2>
          <p class="muted small">{t("settings.account.autoLogHint")}</p>
          <pre class="auto-log">{autoLog.trim() || t("settings.account.autoLogEmpty")}</pre>
          <div><button class="btn" onclick={() => (autoLog = autoBridge?.autoLog?.() ?? "")}><Icon name="restart" size={16} /> {t("settings.account.autoLogRefresh")}</button></div>
        </section>
      {/if}
      <section class="card support">
        <div>
          <h2 class="support-title"><KofiLogo size={22} /> {t("settings.account.support")}</h2>
          <p class="muted">{t("settings.account.supportHint")}</p>
          {#if device.tv}
            <p class="muted small">{KOFI}</p>
          {:else}
            <button class="btn" onclick={() => invoke("open_url", { url: KOFI }).catch((e) => toast(errorText(e), true))}><KofiLogo size={18} /> {t("settings.account.supportButton")}</button>
          {/if}
        </div>
        <!-- Téléviseur : pas de navigateur, le QR code s'ouvre sur le téléphone. -->
        {#if device.tv}<QrCode text={KOFI} size={120} />{/if}
      </section>
    {:else if page === "services"}
      <h1 class="title">{t("settings.menu.services")}</h1>
      <section class="card">
        <h2>{t("settings.invite.quickSetup")}</h2>
        {#if device.phone}
          <!-- Téléphone : l'invitation est souvent une capture reçue par message, choisie dans les photos. -->
          <p class="muted">{t("settings.invite.photoHint")}</p>
          <label class="btn primary pick-photo" class:disabled={applying?.step === "running"}>
            <Icon name="image" size={18} /> {t("settings.invite.pickPhoto")}
            <input type="file" accept="image/*" hidden onchange={onInviteImage} />
          </label>
          <div class="row">
            <input class="grow" bind:value={pasted} placeholder="MAKU-INVITE:…" spellcheck="false" />
            <button class="btn" disabled={!pasted.trim() || applying?.step === "running"} onclick={() => runInvite(pasted, false)}>
              {t("settings.invite.apply")}
            </button>
          </div>
        {:else if device.mobile}
          <div class="phone-setup">
            {#if relayUrl}<QrCode text={relayUrl} size={device.tv ? 300 : 190} />{:else}<div class="qr-wait skeleton"></div>{/if}
            <p class="muted">
              {t("settings.invite.phoneHint")}
            </p>
          </div>
          {#if relayError}<p class="error">{relayError}</p>{/if}
        {:else}
          <p class="muted">{t("settings.invite.pasteHint")}</p>
          <div class="row">
            <input class="grow" bind:value={pasted} placeholder="MAKU-INVITE:…" spellcheck="false" />
            <button class="btn primary" disabled={!pasted.trim() || applying?.step === "running"} onclick={() => runInvite(pasted, false)}>
              {t("settings.invite.apply")}
            </button>
            <label class="btn">
              <Icon name="image" size={16} /> {t("settings.invite.importImage")}
              <input type="file" accept="image/*" hidden onchange={onInviteImage} />
            </label>
          </div>
        {/if}
        {#if applying}
          <ul class="steps">
            {#if applying.seerr}<li class={applying.seerr}>{t("settings.invite.stepSeerr", { status: applying.seerrError ?? STEP[applying.seerr] })}</li>{/if}
            {#if applying.tracearr}
              <li class={applying.tracearr}>
                {t("settings.invite.stepTracearr", {
                  status:
                    applying.tracearrError ??
                    (applying.authUrl ? t("settings.invite.authorizeOnPhone") : STEP[applying.tracearr]),
                })}
              </li>
            {/if}
          </ul>
        {/if}
        {#if inviteError}<p class="error">{inviteError}</p>{/if}
      </section>
      {#if nav.admin}
        <section class="card">
          <h2>{t("settings.invite.title")}</h2>
          <p class="muted">
            {t("settings.invite.intro")}
          </p>
          <div class="invite-fields">
            <label>{t("settings.services.seerrAddress")} <input bind:value={inviteSeerr} placeholder="https://seerr.mondomaine.fr" /></label>
            <label>{t("settings.services.tracearrAddress")} <input bind:value={inviteTracearr} placeholder="https://tracearr.mondomaine.fr" /></label>
          </div>
          {#if inviteCode}
          <div class="invite">
            <div bind:this={inviteQr}><QrCode text={inviteCode} size={170} /></div>
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
      <section class="card">
        <h2>Seerr</h2>
        <p class="muted">
          {session.serverKind === "jellyfin" ? t("settings.services.seerrIntroJellyfin") : t("settings.services.seerrIntro")}
        </p>
        <label>{t("settings.services.seerrAddress")} <input bind:value={seerrUrl} placeholder="https://seerr.mondomaine.fr" /></label>
        {#if session.serverKind === "jellyfin"}
          <label>{t("login.username")} <input bind:value={seerrUser} autocomplete="username" spellcheck="false" /></label>
          <label>{t("login.password")} <input type="password" bind:value={seerrPassword} autocomplete="current-password" /></label>
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
      <section class="card">
        <h2>Tracearr</h2>
        <p class="muted">
          {t("settings.services.tracearrIntro")}
        </p>
        <label>{t("settings.services.tracearrAddress")} <input bind:value={tracearrUrl} placeholder="https://tracearr.mondomaine.fr" /></label>
        <div class="row">
          <button class="btn primary" disabled={tracearrBusy} onclick={connectTracearr}>
            {tracearrBusy ? t("settings.services.waitingBrowser") : t("settings.services.connectWithPlex")}
          </button>
          {#if session.tracearrConnected}
            <button class="btn" onclick={disconnectTracearr}>{t("settings.services.logout")}</button>
            <span class="ok">● {t("settings.services.connected")}</span>
          {/if}
        </div>
        {#if tracearrError}<p class="error">{tracearrError}</p>{/if}
      </section>
      <section class="card">
        <h2>{t("settings.tmdb.title")}</h2>
        <p class="muted">
          {t("settings.tmdb.intro")}
          <button class="link" onclick={() => invoke("open_url", { url: "https://www.themoviedb.org/settings/api" }).catch(() => {})}>
            themoviedb.org
          </button>
        </p>
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
            <span class="ok">● {t("settings.tmdb.active")}</span>
          {/if}
        </div>
        {#if tmdbError}<p class="error">{tmdbError}</p>{/if}
      </section>
    {:else if page === "player"}
      <h1 class="title">{t("settings.menu.player")}</h1>
      <section class="card">
        <p class="muted">{t("settings.player.intro")}</p>
        <label>{t("settings.player.mpvPath")} <input bind:value={mpvPath} placeholder="C:\Program Files\mpv\mpv.exe" /></label>
        <div><button class="btn" onclick={saveMpv}>{t("settings.player.save")}</button></div>
      </section>
    {:else if page === "interface"}
      <h1 class="title">{t("settings.menu.interface")}</h1>
      <section class="card">
        <h2>{t("settings.interface.color")}</h2>
        <div class="themes" role="radiogroup" aria-label={t("settings.interface.color")}>
          {#each THEMES as th (th)}
            <button
              class="theme"
              class:on={theme === th}
              role="radio"
              aria-checked={theme === th}
              style:--swatch={THEME_COLORS[th]}
              onclick={() => chooseTheme(th)}
            >
              <span class="swatch">{#if theme === th}<Icon name="check" size={20} />{/if}</span>
              {THEME_LABELS[th]}
            </button>
          {/each}
        </div>
      </section>
      <section class="card">
        <h2>{t("settings.interface.language")}</h2>
        <div class="languages" role="radiogroup" aria-label={t("settings.interface.language")}>
          {#each [{ id: "auto", label: t("settings.interface.languageAuto") }, ...LANGUAGES] as lang (lang.id)}
            <button
              class="btn"
              class:primary={language === lang.id}
              role="radio"
              aria-checked={language === lang.id}
              onclick={() => language !== lang.id && setLanguage(lang.id as Parameters<typeof setLanguage>[0])}
            >
              {lang.label}
            </button>
          {/each}
        </div>
      </section>
      {#if device.tv}
      <section class="card">
        <h2>{t("settings.interface.libraries")}</h2>
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
      {:else}
      <section class="card">
        <label class="switch-line">
          <span>
            <strong>{t("settings.interface.collapsed")}</strong>
            <span class="muted">{t("settings.interface.collapsedHint")}</span>
          </span>
          <input type="checkbox" bind:checked={sidebar.collapsed} onchange={saveSidebar} />
        </label>
        <p class="muted">
          {t("settings.interface.reorderHint")}
        </p>
        <div><button class="btn" onclick={resetSidebar}>{t("settings.interface.resetMenu")}</button></div>
      </section>
      {/if}
    {:else if page === "dashboard"}
      <h1 class="title">{t("settings.menu.dashboard")}</h1>
      <ServerDashboard />
    {:else if GROUPS[page]}
      <h1 class="title">{GROUPS[page].label}</h1>
      {#if prefsError}
        <p class="error">{prefsError}</p>
      {:else if !prefs}
        <div class="skeleton" style:height="300px"></div>
      {:else}
        <ServerPrefs prefs={prefs.filter((p) => p.group === page)} {advanced} onSaved={loadPrefs} />
      {/if}
    {/if}
  </div>
</div>

<style>
  .phone-setup {
    display: flex;
    align-items: center;
    gap: 24px;
  }

  .phone-setup p {
    max-width: 320px;
    margin: 0;
  }

  .auto-log {
    max-height: 220px;
    margin: 0;
    padding: 10px 12px;
    overflow: auto;
    border-radius: 10px;
    background: #0000004d;
    font-size: 11px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .support-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .support {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

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

  .languages {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .themes {
    display: flex;
    flex-wrap: wrap;
    gap: 22px;
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
    width: 54px;
    height: 54px;
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

  .adv-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 12px 12px 0;
    color: var(--text-dim);
    font-size: 13px;
    cursor: pointer;
  }

  .adv-toggle input {
    accent-color: var(--accent);
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

  .card h2 {
    margin: 0;
    font-size: 18px;
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

  .ok {
    margin-left: 4px;
    color: var(--ok);
    font-size: 13px;
    font-weight: 600;
  }

  .account {
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 18px;
  }

  .avatar {
    width: 72px;
    height: 72px;
    border-radius: 50%;
    object-fit: cover;
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

  .switch-line input {
    width: 20px;
    height: 20px;
    accent-color: var(--accent);
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

  :global(html.phone) .adv-toggle {
    flex: none;
    margin: 0 4px;
    white-space: nowrap;
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

  .pick-photo {
    justify-content: center;
    width: 100%;
    height: 48px;
    margin-bottom: 12px;
    font-size: 15px;
  }

  .pick-photo.disabled {
    opacity: 0.5;
    pointer-events: none;
  }

  :global(html.phone) .phone-setup {
    flex-direction: column;
    align-items: flex-start;
  }

  :global(html.phone) .invite-fields {
    grid-template-columns: minmax(0, 1fr);
  }
</style>
