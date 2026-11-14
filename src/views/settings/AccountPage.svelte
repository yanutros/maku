<!-- Réglages → Compte : profil, serveur, connexion automatique (Plex Home), version et soutien. -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { onMount } from "svelte";
  import QrCode from "../../components/QrCode.svelte";
  import KofiLogo from "../../components/KofiLogo.svelte";
  import Icon from "../../components/Icon.svelte";
  import QrButton from "./QrButton.svelte";
  import { revealQr } from "./qr";
  import { api, errorText, type Account, type HomeUser, type ServerKinds, type Session } from "../../lib/api";
  import { nav, toast } from "../../lib/nav.svelte";
  import { device } from "../../lib/device";
  import { checkForUpdate, showReleaseNotes, update } from "../../lib/update.svelte";
  import { t } from "../../lib/i18n";

  let { session, onChange }: { session: Session; onChange: () => void } = $props();

  /** QR code déplié sous sa ligne : un seul à la fois. */
  let qr = $state<string | null>(null);
  function toggleQr(id: string) {
    qr = qr === id ? null : id;
    if (qr) revealQr();
  }

  const KOFI = "https://ko-fi.com/yanutros";
  let account = $state<Account | null>(null);

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
  });
</script>

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
        <QrButton open={qr === "kofi"} onclick={() => toggleQr("kofi")} />
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
