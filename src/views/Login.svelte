<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, errorText, type LoginPin, type Session } from "../lib/api";
  import { device } from "../lib/device";
  import { t } from "../lib/i18n";
  import Logo from "../components/Logo.svelte";
  import Wordmark from "../components/Wordmark.svelte";

  let { onDone }: { onDone: () => void } = $props();

  // D'abord le choix du serveur : Plex ou Jellyfin.
  let server = $state<"plex" | "jellyfin" | null>(null);
  let pin = $state<LoginPin | null>(null);
  let mode = $state<"browser" | "link">("browser");
  let error = $state<string | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;

  // Deux façons de se connecter : valider dans le navigateur de l'appareil, ou saisir un code
  // sur plex.tv/link depuis un téléphone (indispensable sur téléviseur).
  async function login(next: "browser" | "link") {
    server = "plex";
    mode = next;
    error = null;
    clearInterval(timer);
    try {
      const p = await api.startLogin(next === "link");
      pin = p;
      timer = setInterval(async () => {
        try {
          if (await api.pollLogin(p.pinId)) {
            clearInterval(timer);
            onDone();
          }
        } catch (e) {
          clearInterval(timer);
          error = errorText(e);
        }
      }, 2000);
    } catch (e) {
      error = errorText(e);
    }
  }

  // Connexions gardées : on revient à Plex ou au serveur Jellyfin sans se reconnecter.
  let remembered = $state<Pick<Session, "plexRemembered" | "jellyfinRemembered" | "serverKinds"> | null>(null);
  let resuming = $state(false);
  onMount(() => {
    api.session().then((s) => {
      remembered = s;
      // Un seul type de serveur utilisé (Réglages → Compte) : on va droit à sa connexion.
      if (s.serverKinds === "plex") choosePlex();
      else if (s.serverKinds === "jellyfin") otherJellyfin();
    }, () => {});
  });

  // Plex : sur téléviseur, pas de navigateur, le code s'affiche tout de suite.
  async function choosePlex() {
    if (remembered?.plexRemembered) {
      await api.resumePlex();
      onDone();
    } else if (device.tv) login("link");
    else server = "plex";
  }

  async function chooseJellyfin() {
    const saved = remembered?.jellyfinRemembered;
    if (!saved) return (server = "jellyfin");
    resuming = true;
    error = null;
    try {
      await api.resumeJellyfin();
      onDone();
    } catch (e) {
      // Session fermée ou serveur injoignable : le formulaire, avec l'adresse déjà remplie.
      jfUrl = saved.url;
      server = "jellyfin";
      error = errorText(e);
    } finally {
      resuming = false;
    }
  }

  /** Jellyfin, avec un autre compte ou un autre serveur que celui gardé. */
  function otherJellyfin() {
    jfUrl = remembered?.jellyfinRemembered?.url ?? "";
    server = "jellyfin";
  }

  // --- Jellyfin : adresse du serveur, identifiant, mot de passe ---
  let jfUrl = $state("");
  let jfUser = $state("");
  let jfPassword = $state("");
  let jfBusy = $state(false);

  async function loginJellyfin(e: SubmitEvent) {
    e.preventDefault();
    jfBusy = true;
    error = null;
    try {
      await api.jellyfinLogin(jfUrl, jfUser, jfPassword);
      onDone();
    } catch (err) {
      error = errorText(err);
    } finally {
      jfBusy = false;
    }
  }

  // Quick Connect : un code à valider dans Jellyfin (Profil → Quick Connect), sans mot de passe.
  let quickCode = $state<string | null>(null);

  async function quickConnect() {
    error = null;
    clearInterval(timer);
    try {
      const qc = await api.jellyfinQuickStart(jfUrl);
      quickCode = qc.code;
      timer = setInterval(async () => {
        try {
          if (await api.jellyfinQuickPoll(jfUrl, qc.secret)) {
            clearInterval(timer);
            onDone();
          }
        } catch (e) {
          clearInterval(timer);
          error = errorText(e);
        }
      }, 2000);
    } catch (e) {
      error = errorText(e);
    }
  }

  function back() {
    clearInterval(timer);
    quickCode = null;
    server = null;
    pin = null;
    error = null;
  }


  onDestroy(() => clearInterval(timer));
</script>

<div class="center">
  <div class="card">
    <div class="logo"><Logo size={80} glow /></div>
    <h1><Wordmark size={72} /></h1>
    <p class="muted">{t("login.tagline")}</p>

    {#if server === null}
      <!-- Le serveur de médias : Plex ou Jellyfin. -->
      <p>{t("login.chooseServer")}</p>
      <div class="servers">
        <button class="server-btn" onclick={choosePlex}>
          <!-- Pictogramme neutre (le logo de Plex est une marque déposée) : un écran et sa lecture. -->
          <svg class="server-icon" viewBox="0 0 512 512" aria-hidden="true">
            <rect width="512" height="512" rx="96" fill="#282a2d" />
            <rect x="96" y="128" width="320" height="216" rx="28" fill="none" stroke="#ebaf00" stroke-width="28" />
            <path d="M222 190v92l78-46z" fill="#ebaf00" />
            <path d="M192 400h128" stroke="#ebaf00" stroke-width="28" stroke-linecap="round" />
          </svg>
          <span class="server-name">Plex</span>
          <span class="muted small">{remembered?.plexRemembered ? t("login.connected") : t("login.plexHint")}</span>
        </button>
        <button class="server-btn" onclick={chooseJellyfin} disabled={resuming}>
          <!-- Logo Jellyfin (CC BY-SA 4.0, projet Jellyfin). -->
          <svg class="server-icon" viewBox="0 0 512 512" aria-hidden="true">
            <defs>
              <linearGradient id="jf-gradient" x1="110.25" y1="213.3" x2="496.14" y2="436.09" gradientUnits="userSpaceOnUse">
                <stop offset="0" stop-color="#aa5cc3" />
                <stop offset="1" stop-color="#00a4dc" />
              </linearGradient>
            </defs>
            <rect width="512" height="512" rx="96" fill="#282a2d" />
            <g transform="translate(51 51) scale(0.8)">
              <path d="M256 201.62c-20.44 0-86.23 119.29-76.2 139.43s142.48 19.92 152.4 0-55.73-139.42-76.2-139.43z" fill="url(#jf-gradient)" />
              <path d="M256 23.3c-61.56 0-259.82 359.48-229.63 420.1s429.63 60 459.26 0S317.56 23.3 256 23.3zm150.51 367.46c-19.4 38.9-281.15 39.37-300.79 0S215.71 115.48 256.12 115.48s169.79 236.38 150.39 275.28z" fill="url(#jf-gradient)" />
            </g>
          </svg>
          <span class="server-name">Jellyfin</span>
          <span class="muted small">
            {resuming ? t("login.connecting") : (remembered?.jellyfinRemembered?.name ?? t("login.jellyfinHint"))}
          </span>
        </button>
      </div>
      {#if remembered?.jellyfinRemembered}
        <button class="btn link-btn" onclick={otherJellyfin}>{t("login.otherAccount")} · Jellyfin</button>
      {/if}
    {:else if server === "jellyfin" && quickCode}
      <p>{t("login.quickConnectHint")}</p>
      <div class="code">{#each quickCode.split("") as c, i (i)}<span>{c}</span>{/each}</div>
      <p class="muted small">{t("login.autoConnect")}</p>
      <div class="row"><button class="btn" onclick={back}>{t("login.back")}</button></div>
    {:else if server === "jellyfin"}
      <form class="jf" onsubmit={loginJellyfin}>
        <label>{t("login.jellyfinUrl")} <input bind:value={jfUrl} placeholder="http://192.168.1.10:8096" autocomplete="url" spellcheck="false" required /></label>
        <label>{t("login.username")} <input bind:value={jfUser} autocomplete="username" spellcheck="false" /></label>
        <label>{t("login.password")} <input type="password" bind:value={jfPassword} autocomplete="current-password" /></label>
        <div class="row">
          <button type="button" class="btn" onclick={back}>{t("login.back")}</button>
          <button type="submit" class="btn primary" disabled={jfBusy}>{jfBusy ? t("login.connecting") : t("login.connect")}</button>
          <button type="button" class="btn" disabled={!jfUrl.trim()} onclick={quickConnect}>{t("login.quickConnect")}</button>
        </div>
      </form>
    {:else if pin && mode === "link"}
      <p>{t("login.goTo")}</p>
      <p class="site">plex.tv/link</p>
      <p>{t("login.enterCode")}</p>
      <div class="code">{#each pin.code.split("") as c, i (i)}<span>{c}</span>{/each}</div>
      <p class="muted small">{t("login.autoConnect")}</p>
      <div class="row">
        <button class="btn" onclick={() => login("link")}>{t("login.newCode")}</button>
        {#if !device.tv}<button class="btn" onclick={() => login("browser")}>{t("login.useBrowser")}</button>{/if}
      </div>
    {:else if pin}
      <p>{t("login.validateInBrowser")}</p>
      <div class="row">
        <button class="btn" onclick={() => login("browser")}>{t("login.restart")}</button>
        <button class="btn" onclick={() => login("link")}>{t("login.useCode")}</button>
      </div>
    {:else}
      <div class="row">
        <button class="btn primary" onclick={() => login("browser")}>{t("login.signIn")}</button>
        <button class="btn" onclick={() => login("link")}>{t("login.withCode", { site: "plex.tv/link" })}</button>
        <button class="btn" onclick={back}>{t("login.back")}</button>
      </div>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </div>
</div>

<style>
  .card {
    display: grid;
    justify-items: center;
    gap: 6px;
    width: min(560px, 92vw);
    text-align: center;
  }

  .card p {
    margin: 4px 0;
  }

  .logo {
    display: grid;
    margin-bottom: 8px;
  }

  h1 {
    margin: 0 0 4px;
    font-size: 40px;
  }

  .site {
    color: var(--accent);
    font-size: 26px;
    font-weight: 800;
  }

  .code {
    display: flex;
    gap: 10px;
    margin: 10px 0;
  }

  .code span {
    display: grid;
    place-items: center;
    width: 64px;
    height: 78px;
    border: 1px solid #ffffff26;
    border-radius: 14px;
    background: var(--bg-elev);
    font-family: "Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif;
    font-size: 40px;
    font-weight: 800;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 10px;
    margin-top: 12px;
  }

  .small {
    font-size: 13px;
  }

  .servers {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
    width: 100%;
    margin-top: 14px;
  }

  .server-btn {
    display: grid;
    gap: 6px;
    padding: 22px 18px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--bg-elev);
    text-align: center;
    transition: border-color 0.15s, transform 0.15s;
  }

  .server-btn:hover,
  .server-btn:focus-visible {
    border-color: var(--accent);
    transform: translateY(-2px);
  }

  .server-name {
    font-size: 22px;
    font-weight: 800;
  }

  .server-icon {
    width: 64px;
    height: 64px;
    justify-self: center;
    margin-bottom: 4px;
  }

  .link-btn {
    margin-top: 10px;
    font-size: 13px;
  }

  .jf {
    display: grid;
    gap: 12px;
    width: 100%;
    margin-top: 14px;
    text-align: left;
  }

  .jf label {
    display: grid;
    gap: 6px;
    color: var(--text-dim);
    font-size: 13px;
  }
</style>
