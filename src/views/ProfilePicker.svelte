<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, errorText, type HomeUser } from "../lib/api";
  import { stop } from "../lib/player.svelte";
  import Icon from "../components/Icon.svelte";
  import { t } from "../lib/i18n";

  /**
   * « Qui regarde ? » : les profils du Plex Home, comme dans les applications Plex. Un profil protégé
   * demande son code PIN (4 chiffres), au clavier, à la souris ou à la télécommande. Au lancement,
   * l'écran s'affiche tant que « Connexion automatique » n'est pas cochée (Réglages → Compte).
   * `cancel` : ouvert depuis le menu, on peut revenir sans changer de profil.
   */
  let { onDone, cancel }: { onDone: (switched: boolean) => void; cancel?: () => void } = $props();

  let users = $state<HomeUser[] | null>(null);
  let current = $state<string | null>(null);
  let error = $state<string | null>(null);
  let chosen = $state<HomeUser | null>(null);
  let pin = $state("");
  let busy = $state(false);
  let shake = $state(false);
  let broken = $state<string[]>([]);
  let root = $state<HTMLElement>();

  onMount(async () => {
    try {
      const home = await api.homeUsers();
      // L'administrateur d'abord, puis les autres dans l'ordre de Plex.
      users = [...home.users].sort((a, b) => Number(b.admin) - Number(a.admin));
      current = home.current;
      await tick();
      const start = root?.querySelector<HTMLElement>(`[data-uuid="${current}"]`) ?? root?.querySelector<HTMLElement>(".profile");
      start?.focus();
    } catch (e) {
      error = errorText(e);
    }
  });

  async function choose(u: HomeUser) {
    error = null;
    // Depuis le menu, le profil déjà connecté n'a rien à changer.
    if (cancel && u.uuid === current) return cancel();
    if (u.protected) {
      chosen = u;
      pin = "";
      await tick();
      root?.querySelector<HTMLElement>(".key")?.focus();
      return;
    }
    await switchTo(u, null);
  }

  async function switchTo(u: HomeUser, code: string | null) {
    busy = true;
    try {
      stop();
      await api.switchUser(u.uuid, code);
      onDone(true);
    } catch (e) {
      error = errorText(e);
      pin = "";
      shake = true;
      setTimeout(() => (shake = false), 450);
    } finally {
      busy = false;
    }
  }

  function press(digit: string) {
    if (busy || !chosen || pin.length >= 4) return;
    error = null;
    pin += digit;
    if (pin.length === 4) switchTo(chosen, pin);
  }

  function erase() {
    pin = pin.slice(0, -1);
  }

  function backToList() {
    chosen = null;
    pin = "";
    error = null;
  }

  // Chiffres du clavier ou de la télécommande ; Retour arrière efface, Échap revient aux profils.
  function onKey(e: KeyboardEvent) {
    if (!chosen) {
      if (e.key === "Escape" && cancel) cancel();
      return;
    }
    if (/^[0-9]$/.test(e.key)) {
      e.preventDefault();
      press(e.key);
    } else if (e.key === "Backspace") {
      e.preventDefault();
      erase();
    } else if (e.key === "Escape") {
      e.preventDefault();
      backToList();
    }
  }

  function hue(name: string) {
    let h = 0;
    for (const c of name) h = (h * 31 + c.charCodeAt(0)) % 360;
    return h;
  }

  const KEYS = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];
</script>

<svelte:window onkeydown={onKey} />

<div class="picker" bind:this={root}>
  {#if !chosen}
    <h1>{t("profiles.title")}</h1>
    {#if !users && !error}<p class="muted">{t("common.c1.loading")}</p>{/if}
    <div class="profiles">
      {#each users ?? [] as u (u.uuid)}
        <button class="profile" data-uuid={u.uuid} class:busy={busy} disabled={busy} onclick={() => choose(u)}>
          <span class="avatar" style:--hue={hue(u.title)}>
            {#if u.thumb && !broken.includes(u.uuid)}
              <img src={u.thumb} alt="" onerror={() => (broken = [...broken, u.uuid])} />
            {:else}
              {u.title.charAt(0).toUpperCase()}
            {/if}
            {#if u.protected}<span class="lock"><Icon name="lock" size={16} /></span>{/if}
          </span>
          <span class="name">{u.title}</span>
          <span class="role">
            {u.admin ? t("profiles.admin") : u.restricted ? t("profiles.managed") : u.uuid === current ? t("profiles.current") : ""}
          </span>
        </button>
      {/each}
    </div>
    {#if error}<p class="error">{error}</p>{/if}
    {#if cancel}<div class="row actions"><button class="btn" onclick={cancel}>{t("profiles.cancel")}</button></div>{/if}
  {:else}
    <!-- Code PIN du profil choisi : quatre points, puis un pavé numérique. -->
    <span class="avatar small" style:--hue={hue(chosen.title)}>
      {#if chosen.thumb && !broken.includes(chosen.uuid)}<img src={chosen.thumb} alt="" />{:else}{chosen.title.charAt(0).toUpperCase()}{/if}
    </span>
    <h1>{t("profiles.enterPin", { name: chosen.title })}</h1>
    <div class="dots" class:shake aria-label={t("profiles.pin")}>
      {#each [0, 1, 2, 3] as i (i)}<span class="dot" class:on={pin.length > i}></span>{/each}
    </div>
    <p class="error" class:hidden={!error}>{error ?? "·"}</p>
    <div class="keypad">
      {#each KEYS as k (k)}<button class="key" disabled={busy} onclick={() => press(k)}>{k}</button>{/each}
      <button class="key ghost" onclick={backToList} aria-label={t("profiles.back")}><Icon name="arrowLeft" size={22} /></button>
      <button class="key" disabled={busy} onclick={() => press("0")}>0</button>
      <button class="key ghost" disabled={!pin} onclick={erase} aria-label={t("profiles.erase")}><Icon name="chevronLeft" size={22} /></button>
    </div>
  {/if}
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    align-items: center;
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
    opacity: 0.6;
    cursor: default;
  }

  .profile:focus {
    outline: none;
  }

  .avatar {
    position: relative;
    display: grid;
    place-items: center;
    width: 136px;
    height: 136px;
    margin-bottom: 10px;
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
    border-radius: 50%;
    object-fit: cover;
  }

  .avatar.small {
    width: 92px;
    height: 92px;
    margin-bottom: 16px;
    font-size: 36px;
  }

  .profile:hover:not(:disabled) .avatar,
  .profile:focus-visible .avatar {
    transform: scale(1.06);
    box-shadow:
      0 0 0 4px var(--accent),
      0 18px 40px #000b;
  }

  /* Cadenas des profils protégés par un code PIN. */
  .lock {
    position: absolute;
    right: 4px;
    bottom: 4px;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: #111c;
    color: #fff;
    box-shadow: 0 2px 8px #0008;
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

  .role {
    min-height: 1.3em;
    font-size: 13px;
  }

  .actions {
    margin-top: 44px;
  }

  .dots {
    display: flex;
    gap: 18px;
    margin: -14px 0 6px;
  }

  .dot {
    width: 16px;
    height: 16px;
    border: 2px solid var(--text-dim);
    border-radius: 50%;
    transition: background 0.12s;
  }

  .dot.on {
    border-color: var(--accent);
    background: var(--accent);
  }

  .shake {
    animation: shake 0.4s;
  }

  @keyframes shake {
    20%,
    60% {
      transform: translateX(-10px);
    }
    40%,
    80% {
      transform: translateX(10px);
    }
  }

  .error.hidden {
    visibility: hidden;
  }

  .keypad {
    display: grid;
    grid-template-columns: repeat(3, 72px);
    gap: 14px;
    margin-top: 10px;
  }

  .key {
    display: grid;
    place-items: center;
    width: 72px;
    height: 72px;
    border: none;
    border-radius: 50%;
    background: #ffffff14;
    color: var(--text);
    font-size: 26px;
    font-weight: 600;
    transition: background 0.12s, transform 0.12s;
  }

  .key.ghost {
    background: none;
    color: var(--text-dim);
  }

  .key:hover:not(:disabled) {
    background: #ffffff26;
  }

  .key:focus-visible {
    outline: none;
    background: #fff;
    color: #111;
    transform: scale(1.06);
  }

  .key:disabled {
    opacity: 0.35;
  }
</style>
