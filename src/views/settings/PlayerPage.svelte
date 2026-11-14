<!-- Réglages → Lecteur : son vers l'ampli (Android), chemin de mpv, épisode suivant, paroles, journal. -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import QrCode from "../../components/QrCode.svelte";
  import Banner from "./Banner.svelte";
  import QrButton from "./QrButton.svelte";
  import { revealQr } from "./qr";
  import { lyricsPrefs, setLyricsEnabled } from "../../lib/lyrics.svelte";
  import { NEXT_DELAYS, autoNext, setNextDelay, diagAvailable, diagShare, diagText, passthroughAvailable, passthroughFormats, passthroughMode, setPassthroughMode } from "../../lib/player.svelte";
  import { api, errorText, type Session } from "../../lib/api";
  import { toast } from "../../lib/nav.svelte";
  import { device } from "../../lib/device";
  import { t } from "../../lib/i18n";

  let { session, onChange, desk }: { session: Session; onChange: () => void; desk: boolean } = $props();

  /** QR code déplié sous sa ligne : un seul à la fois. */
  let qr = $state<string | null>(null);
  function toggleQr(id: string) {
    qr = qr === id ? null : id;
    if (qr) revealQr();
  }

  let mpvPath = $state("");
  $effect.pre(() => {
    mpvPath = session.mpvPath ?? "";
  });

  // Android : son vers l'ampli (automatique) ou décodé sur l'appareil.
  let passthrough = $state(passthroughMode());
  const audioOut = passthroughAvailable ? passthroughFormats() : null;
  function choosePassthrough(mode: "auto" | "off") {
    passthrough = mode;
    setPassthroughMode(mode);
  }

  // Journal de diagnostic, sur tablette et téléviseur : le téléphone le télécharge par le relais (QR code).
  let diagUrl = $state<string | null>(null);
  let diagError = $state<string | null>(null);
  $effect(() => {
    if (!diagAvailable || device.phone) return;
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

  async function saveMpv() {
    try {
      await api.setMpvPath(mpvPath || null);
      toast(t("settings.player.saved"));
      onChange();
    } catch (e) {
      toast(errorText(e), true);
    }
  }
</script>

<h1 class="title">{t("settings.menu.player")}</h1>
{#if desk}<Banner icon="play" title={t("settings.menu.player")} text={t("settings.banner.player")} />{/if}
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
        {#if !device.phone}<QrButton open={qr === "diag"} onclick={() => toggleQr("diag")} />{/if}
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
