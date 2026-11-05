<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type ServerSummary } from "../lib/api";
  import { t } from "../lib/i18n";

  /**
   * Choix du serveur, façon sélection de profil (Netflix, Apple TV) : la photo du compte Plex du
   * propriétaire dans un cercle, le nom du serveur et à qui il appartient.
   */
  let { onDone }: { onDone: () => void } = $props();

  let servers = $state<ServerSummary[] | null>(null);
  let connecting = $state<string | null>(null);
  let error = $state<string | null>(null);
  let broken = $state<string[]>([]);

  onMount(async () => {
    try {
      servers = await api.servers();
      // Le sien d'abord, les serveurs partagés ensuite.
      servers.sort((a, b) => Number(b.owned) - Number(a.owned));
    } catch (e) {
      error = errorText(e);
    }
  });

  async function select(s: ServerSummary) {
    connecting = s.machineId;
    error = null;
    try {
      await api.selectServer(s.machineId);
      onDone();
    } catch (e) {
      error = errorText(e);
    } finally {
      connecting = null;
    }
  }

  // Sans photo : l'initiale sur une couleur tirée du nom, toujours la même pour un serveur donné.
  function hue(name: string) {
    let h = 0;
    for (const c of name) h = (h * 31 + c.charCodeAt(0)) % 360;
    return h;
  }

  // Autre service (Jellyfin, Emby) : l'écran de choix des services, connexions gardées. Avec « Plex »
  // seul dans les Réglages, Maku passe à « les deux » : on utilise désormais plusieurs services.
  let plexOnly = $state(false);
  api.session().then((s) => (plexOnly = s.serverKinds === "plex"), () => {});

  async function otherService() {
    if (plexOnly) await api.setServerKinds("both");
    await api.forgetServer();
    onDone();
  }
</script>

<div class="picker">
  <h1>{t("servers.title")}</h1>
  {#if !servers && !error}
    <p class="muted">{t("servers.searching")}</p>
  {:else if servers && !servers.length}
    <p class="muted">{t("servers.none")}</p>
  {/if}

  <div class="profiles">
    {#each servers ?? [] as s (s.machineId)}
      <button
        class="profile"
        class:busy={connecting === s.machineId}
        disabled={connecting !== null && connecting !== s.machineId}
        onclick={() => select(s)}
      >
        <span class="avatar" style:--hue={hue(s.name)}>
          {#if s.ownerThumb && !broken.includes(s.machineId)}
            <img src={s.ownerThumb} alt="" onerror={() => (broken = [...broken, s.machineId])} />
          {:else}
            {s.name.charAt(0).toUpperCase()}
          {/if}
        </span>
        <span class="name">{s.name}</span>
        <span class="owner">
          {connecting === s.machineId
            ? t("servers.connecting")
            : s.owned
              ? t("servers.mine")
              : s.owner
                ? t("servers.sharedBy", { owner: s.owner })
                : t("servers.shared")}
        </span>
      </button>
    {/each}
  </div>

  {#if error}<p class="error">{error}</p>{/if}
  <div class="row logout">
    <button class="btn" onclick={otherService}>{t("servers.chooseKind")}</button>
    <button class="btn" onclick={() => api.switchPlexAccount().then(onDone)}>{t("servers.switchAccount")}</button>
  </div>
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    align-items: center;
    /* `safe` : une liste plus haute que l'écran (téléphone) défile au lieu d'être coupée en haut. */
    justify-content: safe center;
    gap: 8px;
    height: 100%;
    overflow-y: auto;
    padding: 48px 24px;
    text-align: center;
  }

  h1 {
    margin: 0 0 36px;
    font-size: 34px;
    font-weight: 700;
  }

  .profiles {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 36px 40px;
    max-width: 920px;
  }

  .profile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    width: 160px;
    padding: 0;
    border: none;
    background: none;
    color: var(--text-dim);
  }

  .profile:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .avatar {
    display: grid;
    place-items: center;
    width: 136px;
    height: 136px;
    margin-bottom: 10px;
    overflow: hidden;
    border-radius: 50%;
    background: linear-gradient(145deg, hsl(var(--hue) 55% 48%), hsl(var(--hue) 60% 30%));
    color: #fff;
    font-size: 54px;
    font-weight: 800;
    box-shadow:
      0 0 0 3px transparent,
      0 14px 34px #0009;
    transition:
      transform 0.18s var(--ease),
      box-shadow 0.18s;
  }

  .avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .profile:hover:not(:disabled) .avatar,
  .profile:focus-visible .avatar,
  .profile.busy .avatar {
    transform: scale(1.06);
    box-shadow:
      0 0 0 4px var(--accent),
      0 18px 40px #000b;
  }

  /* Le cercle doré suffit : pas de liseré autour du bouton (télécommande comprise). */
  .profile:focus {
    outline: none;
  }

  .profile.busy .avatar {
    animation: pulse 1s ease-in-out infinite alternate;
  }

  @keyframes pulse {
    to {
      opacity: 0.6;
    }
  }

  .name {
    max-width: 100%;
    overflow: hidden;
    color: var(--text);
    font-size: 17px;
    font-weight: 700;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .profile:hover:not(:disabled) .name,
  .profile:focus-visible .name {
    color: #fff;
  }

  .owner {
    max-width: 100%;
    overflow: hidden;
    font-size: 13px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .logout {
    margin-top: 44px;
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    justify-content: center;
  }
</style>
