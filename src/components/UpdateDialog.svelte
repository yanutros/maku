<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { api, errorText, type UpdateInfo } from "../lib/api";
  import { markdown } from "../lib/markdown";
  import { onBack } from "../lib/nav.svelte";
  import Icon from "./Icon.svelte";
  import { locale, t } from "../lib/i18n";
  import { checkForUpdate, releaseHighlights, update as pending } from "../lib/update.svelte";

  /**
   * Au lancement, si une nouvelle version est publiée, une fenêtre la présente par-dessus Maku
   * flouté : nouveautés, puis « Mettre à jour » télécharge et installe le bon paquet pour l'appareil.
   */
  // La version proposée vit dans lib/update.svelte.ts (la recherche peut venir des Réglages).
  const info = $derived(pending.info);
  let progress = $state<number | null>(null);
  let message = $state<string | null>(null);
  let error = $state<string | null>(null);
  let primary = $state<HTMLButtonElement>();

  const RELEASES = "https://github.com/yanutros/maku/releases";

  const notes = $derived(info ? releaseHighlights(info.notes, locale) : "");

  const date = $derived(
    info?.date ? new Date(info.date).toLocaleDateString(locale, { day: "numeric", month: "long", year: "numeric" }) : null,
  );

  onMount(() => {
    // Pas de vérification dans le mode démo (captures du README).
    if (new URLSearchParams(location.search).has("demo")) return;
    checkForUpdate().catch(() => {});
    const unlisten = listen<number>("update-progress", (e) => (progress = e.payload));
    onBack(() => {
      if (!info || progress !== null) return false;
      pending.info = null;
      return true;
    });
    return () => unlisten.then((f) => f());
  });

  $effect(() => {
    if (info) primary?.focus();
  });

  async function update() {
    if (!info) return;
    error = null;
    message = null;
    progress = 0;
    try {
      const apk = await api.updateInstall(info);
      if (apk) {
        // Android : l'installateur du système prend le relais.
        const device = (window as { PlexrDevice?: { installApk(path: string): string } }).PlexrDevice;
        const result = device?.installApk(apk);
        if (result === "permission") {
          message = t("update.allowInstall");
        } else {
          message = t("update.confirmInstall");
        }
      } else {
        // Ordinateur : soit Maku s'est fermé pour laisser faire l'installeur, soit celui-ci s'est ouvert.
        message = t("update.installStarted");
      }
    } catch (e) {
      error = errorText(e);
    } finally {
      progress = null;
    }
  }
</script>

{#if info}
  <div class="update-backdrop">
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="update-title">
      <header>
        <span class="logo"><Icon name="play" size={18} filled /></span>
        <div class="titles">
          <h2 id="update-title">{pending.notesOnly ? t("update.notesTitle") : t("update.title")}</h2>
          <p>Maku {info.version}{#if date}{" · "}{date}{/if}{#if !pending.notesOnly} — {t("update.current", { version: info.current })}{/if}</p>
        </div>
        <button class="close" onclick={() => (pending.info = null)} disabled={progress !== null} aria-label={t("update.close")}>
          <Icon name="x" size={18} />
        </button>
      </header>

      <div class="notes">
        {#if notes}
          <!-- Texte échappé par markdown() avant mise en forme. -->
          {@html markdown(notes)}
        {:else}
          <p>{t("update.defaultNotes")}</p>
        {/if}
      </div>

      {#if progress !== null}
        <div class="bar"><div style:width="{Math.round(progress * 100)}%"></div></div>
      {/if}
      {#if message}<p class="message">{message}</p>{/if}
      {#if error}<p class="message error">{error}</p>{/if}

      <footer>
        <button class="link" onclick={() => invoke("open_url", { url: RELEASES })}>
          {t("update.allReleases")} <Icon name="external" size={14} />
        </button>
        <span class="spacer"></span>
        {#if pending.notesOnly}
          <button class="btn primary" bind:this={primary} onclick={() => (pending.info = null)}>{t("update.close")}</button>
        {:else}
          <button class="btn" onclick={() => (pending.info = null)} disabled={progress !== null}>{t("update.later")}</button>
          <button class="btn primary" bind:this={primary} onclick={update} disabled={progress !== null}>
            {progress !== null ? t("update.downloading", { percent: Math.round(progress * 100) }) : t("update.update")}
          </button>
        {/if}
      </footer>
    </div>
  </div>
{/if}

<style>
  /* Le reste de Maku, flouté derrière la fenêtre (y compris sur téléviseur : voir app.css). */
  .update-backdrop {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: grid;
    place-items: center;
    padding: 24px;
    background: #0a0b0d8c;
    backdrop-filter: blur(18px) saturate(1.2);
    animation: fade 0.25s ease-out;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  .dialog {
    display: flex;
    flex-direction: column;
    width: min(620px, 100%);
    max-height: min(680px, 100%);
    border: 1px solid #ffffff1f;
    border-radius: 18px;
    background: color-mix(in srgb, var(--bg-mid) 95%, transparent);
    box-shadow: 0 30px 80px #000c;
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 22px 22px 18px;
    border-bottom: 1px solid var(--border);
  }

  .logo {
    display: grid;
    flex: none;
    place-items: center;
    width: 44px;
    height: 44px;
    padding-left: 3px;
    border-radius: 13px;
    background: linear-gradient(145deg, var(--accent-light), var(--accent) 55%, var(--accent-dark));
    color: var(--accent-text);
    box-shadow: 0 0 28px color-mix(in srgb, var(--accent) 33%, transparent);
  }

  .titles {
    flex: 1;
    min-width: 0;
  }

  h2 {
    margin: 0;
    font-size: 20px;
  }

  .titles p {
    margin: 2px 0 0;
    color: var(--text-dim);
    font-size: 13px;
  }

  .close {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--text-dim);
  }

  .close:hover:not(:disabled) {
    background: #ffffff14;
    color: var(--text);
  }

  .notes {
    flex: 1;
    min-height: 0;
    padding: 6px 24px 14px;
    overflow-y: auto;
    color: #ffffffd8;
    font-size: 14px;
    line-height: 1.55;
  }

  .notes :global(h4) {
    margin: 18px 0 8px;
    color: var(--accent);
    font-size: 12px;
    font-weight: 800;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  .notes :global(ul) {
    margin: 0;
    padding-left: 18px;
  }

  .notes :global(li) {
    margin: 6px 0;
  }

  .notes :global(code) {
    padding: 1px 5px;
    border-radius: 4px;
    background: #ffffff14;
    font-size: 12.5px;
  }

  .notes :global(a) {
    color: var(--accent);
  }

  .bar {
    height: 4px;
    margin: 0 24px;
    border-radius: 2px;
    background: #ffffff1f;
    overflow: hidden;
  }

  .bar div {
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }

  .message {
    margin: 12px 24px 0;
    color: var(--text-dim);
    font-size: 13px;
  }

  .message.error {
    color: var(--danger);
  }

  footer {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 16px 22px 20px;
  }

  .link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0;
    border: none;
    background: none;
    color: var(--text-dim);
    font-size: 13px;
    font-weight: 600;
  }

  .link:hover {
    color: var(--text);
  }

  .spacer {
    flex: 1;
  }
</style>
