<script lang="ts">
  // Modifier une demande en attente (Admin › Demandes) : le serveur Radarr / Sonarr, le profil de qualité et le
  // dossier où elle ira, comme les « Options avancées » de Seerr. Seerr n'accepte ces changements que tant que la
  // demande n'est pas validée : d'où « Enregistrer et valider », le geste habituel une fois l'erreur corrigée.
  import ErrorCard from "./ErrorCard.svelte";
  import { onMount } from "svelte";
  import { api, errorText, type ArrOptions, type ArrServer, type SeerrRequest } from "../lib/api";
  import { bytes } from "../lib/seerrAdmin.svelte";
  import { t, locale } from "../lib/i18n";

  let { request, onclose, ondone }: { request: SeerrRequest; onclose: () => void; ondone: (approved: boolean) => void } = $props();

  let servers = $state<ArrServer[]>([]);
  let options = $state<ArrOptions | null>(null);
  let serverId = $state<number | null>(null);
  let profileId = $state<number | null>(null);
  let rootFolder = $state<string | null>(null);
  let loading = $state(true);
  let sending = $state(false);
  let error = $state<string | null>(null);

  /** Profils et dossiers du serveur choisi ; `keep` garde ceux de la demande, sinon ceux du serveur par défaut. */
  async function loadOptions(id: number, keep: boolean) {
    options = null;
    const o = await api.seerrArrOptions(request.media.mediaType, id);
    options = o;
    profileId = (keep ? request.profileId : null) ?? o.activeProfileId ?? o.profiles[0]?.id ?? null;
    rootFolder = (keep ? request.rootFolder : null) ?? o.activeDirectory ?? o.rootFolders[0]?.path ?? null;
  }

  onMount(async () => {
    try {
      // Seuls les serveurs de la même qualité (4K ou non) que la demande lui conviennent.
      servers = (await api.seerrArrServers(request.media.mediaType)).filter((s) => s.is4k === request.is4k);
      serverId = request.serverId ?? servers.find((s) => s.isDefault)?.id ?? servers[0]?.id ?? null;
      if (serverId !== null) await loadOptions(serverId, true);
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  });

  async function changeServer(id: number) {
    serverId = id;
    error = null;
    try {
      await loadOptions(id, false);
    } catch (e) {
      error = errorText(e);
    }
  }

  // Un dossier de la demande que Radarr / Sonarr ne connaît plus reste choisi (et affiché) tel quel.
  const folders = $derived.by(() => {
    const list = options?.rootFolders ?? [];
    return rootFolder && !list.some((f) => f.path === rootFolder) ? [{ path: rootFolder, freeSpace: null }, ...list] : list;
  });

  async function save(approve: boolean) {
    sending = true;
    error = null;
    try {
      await api.seerrRequestUpdate(request.id, {
        mediaType: request.media.mediaType,
        serverId,
        profileId,
        rootFolder,
        languageProfileId: request.languageProfileId,
        seasons: request.seasons,
        tags: request.tags,
      });
      if (approve) await api.seerrRequestStatus(request.id, true);
      ondone(approve);
    } catch (e) {
      error = errorText(e);
    } finally {
      sending = false;
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}>
  <div class="dialog" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-labelledby="edit-request-title" tabindex="-1">
    {#if request.media.backdrop}<img class="hero" src={request.media.backdrop} alt="" />{/if}
    <div class="body">
      <h2 id="edit-request-title">{t("requests.edit.title")}</h2>
      <p class="subtitle">{request.media.title}{request.media.year ? ` (${request.media.year})` : ""}</p>
      <p class="muted who">{t("requests.requestedBy", { name: request.requestedBy.name })}</p>

      {#if loading}
        <div class="skeleton" style:height="150px"></div>
      {:else if !servers.length && !error}
        <p class="muted">{t(request.media.mediaType === "tv" ? "requests.edit.noSonarr" : "requests.edit.noRadarr")}</p>
      {:else if servers.length}
        <div class="fields">
          <label class="field">
            <span>{t("requests.edit.server")}</span>
            <select value={serverId} onchange={(e) => changeServer(Number(e.currentTarget.value))} disabled={servers.length < 2}>
              {#each servers as s (s.id)}
                <option value={s.id}>{s.isDefault ? t("requests.edit.default", { name: s.name }) : s.name}</option>
              {/each}
            </select>
          </label>
          <label class="field">
            <span>{t("requests.edit.profile")}</span>
            <select bind:value={profileId} disabled={!options}>
              {#each options?.profiles ?? [] as p (p.id)}
                <option value={p.id}>{p.name}</option>
              {/each}
            </select>
          </label>
          <label class="field wide">
            <span>{t("requests.edit.folder")}</span>
            <select bind:value={rootFolder} disabled={!options}>
              {#each folders as f (f.path)}
                <option value={f.path}>{f.freeSpace !== null ? t("requests.edit.freeSpace", { path: f.path, size: bytes(f.freeSpace, locale) }) : f.path}</option>
              {/each}
            </select>
          </label>
        </div>
      {/if}

      {#if error}<ErrorCard message={error} />{/if}

      <div class="actions">
        <button class="btn" onclick={onclose}>{t("request.cancel")}</button>
        <button class="btn" disabled={sending || loading || !options} onclick={() => save(false)}>{t("requests.edit.save")}</button>
        <button class="btn primary" disabled={sending || loading || !options} onclick={() => save(true)}>
          {sending ? t("requests.edit.saving") : t("requests.edit.saveApprove")}
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: #000a;
  }

  .dialog {
    width: min(640px, 92vw);
    max-height: 86vh;
    overflow-y: auto;
    scrollbar-width: thin;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--bg-elev);
  }

  .hero {
    display: block;
    width: 100%;
    height: 150px;
    object-fit: cover;
    -webkit-mask-image: linear-gradient(180deg, #000 30%, transparent);
    mask-image: linear-gradient(180deg, #000 30%, transparent);
  }

  .body {
    padding: 20px 24px 24px;
  }

  .hero + .body {
    margin-top: -60px;
    position: relative;
  }

  h2 {
    margin: 0;
    color: var(--accent);
    font-size: 22px;
    font-weight: 800;
  }

  .subtitle {
    margin: 4px 0 0;
    font-size: 17px;
    font-weight: 700;
  }

  .who {
    margin: 4px 0 18px;
    font-size: 13px;
  }

  .fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }

  .field {
    display: grid;
    gap: 6px;
    min-width: 0;
  }

  .field.wide {
    grid-column: 1 / -1;
  }

  .field span {
    color: var(--text-dim);
    font-size: 13px;
    font-weight: 600;
  }

  .field select {
    width: 100%;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 22px;
  }

  :global(html.phone) .fields {
    grid-template-columns: 1fr;
  }

  :global(html.phone) .actions .btn {
    flex: 1 1 auto;
  }
</style>
