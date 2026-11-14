<!-- Réglages → Services : configuration rapide, Seerr, Tracearr, clé TMDB, invitations de l'administrateur. -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { untrack } from "svelte";
  import QrCode from "../../components/QrCode.svelte";
  import PasswordInput from "../../components/PasswordInput.svelte";
  import Icon from "../../components/Icon.svelte";
  import Banner from "./Banner.svelte";
  import QrButton from "./QrButton.svelte";
  import { revealQr } from "./qr";
  import { applyInvite, decodeInvite, encodeInvite, readQrImage, type InviteStatus } from "../../lib/invite";
  import { api, errorText, tracearrLogin, type RelayFound, type Session } from "../../lib/api";
  import { go, nav, toast } from "../../lib/nav.svelte";
  import { device } from "../../lib/device";
  import { forgetRemoteLogos } from "../../lib/logos";
  import { t } from "../../lib/i18n";

  /** `relay` : le Maku Server trouvé sur le réseau à l'ouverture des réglages (voir Settings.svelte). */
  let { session, onChange, desk, relay }: { session: Session; onChange: () => void; desk: boolean; relay: RelayFound | null } = $props();

  /** QR code déplié sous sa ligne : un seul à la fois. */
  let qr = $state<string | null>(null);
  function toggleQr(id: string) {
    qr = qr === id ? null : id;
    if (qr) revealQr();
  }

  // Champs éditables, initialisés une fois depuis la session.
  let seerrUrl = $state("");
  let tracearrUrl = $state("");
  $effect.pre(() => {
    seerrUrl = session.seerrUrl ?? "";
    tracearrUrl = session.tracearrUrl ?? "";
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

  // Android : le presse-papiers de la page n'y garde pas d'image (« copiée », mais rien à coller) ; l'image passe
  // par le partage d'Android, qui propose aussi de la copier.
  const shareBridge = (window as { PlexrDevice?: { shareImage?(base64: string, name: string): void } }).PlexrDevice;
  const canShareImage = !!shareBridge?.shareImage;

  // Le QR code en image PNG, à coller dans un message (ou, sur Android, à partager).
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
      if (canShareImage) {
        shareBridge!.shareImage!(canvas.toDataURL("image/png").split(",")[1], "maku-invitation");
        return;
      }
      const blob = await new Promise<Blob>((ok, ko) => canvas.toBlob((b) => (b ? ok(b) : ko(new Error("image"))), "image/png"));
      await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
      toast(t("settings.invite.imageCopied"));
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  // Tablette et téléviseur : le téléphone envoie le code par le relais, tant que la page est ouverte.
  $effect(() => {
    if (!device.mobile || device.phone) return;
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

  // Relais Maku Server : ses adresses remplissent les champs vides (et Seerr se connecte avec Plex).
  $effect(() => {
    const r = relay;
    if (!r) return;
    untrack(() => {
      relayFound = r.url;
      if (r.seerr && !seerrUrl) seerrUrl = r.url + "/seerr";
      if (r.tracearr && !tracearrUrl) tracearrUrl = r.url + "/tracearr";
      if (!inviteSeerr && r.seerr) inviteSeerr = r.url + "/seerr";
      if (!inviteTracearr && r.tracearr) inviteTracearr = r.url + "/tracearr";
    });
  });
</script>

<h1 class="title">{t("settings.menu.services")}</h1>
{#if desk}<Banner icon="compass" title={t("settings.menu.services")} text={t("settings.banner.services")} />{/if}
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
      <QrButton open={qr === "invite"} onclick={() => toggleQr("invite")} />
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

<div class="tile">
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
</div>

<div class="tile">
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
</div>

<div class="tile">
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
    <div class="row"><QrButton open={qr === "tmdb"} onclick={() => toggleQr("tmdb")} /></div>
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
</div>

{#if nav.admin}
  <div class="tile">
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
              <button class="btn" onclick={copyInviteImage}><Icon name="image" size={16} /> {canShareImage ? t("settings.invite.shareImage") : t("settings.invite.copyImage")}</button>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </section>
  </div>
{/if}
