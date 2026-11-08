<script lang="ts">
  // Utilisateurs du serveur Plex (administrateur), comme « Gérer l'accès à la bibliothèque » de Plex : la liste
  // des comptes à gauche, la fiche du compte choisi à droite (bibliothèques, autorisations, restrictions).
  // Chaque changement est envoyé aussitôt à plex.tv (plex_users.rs).
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "./Icon.svelte";
  import { errorText } from "../lib/api";
  import { t } from "../lib/i18n";
  import { toast } from "../lib/nav.svelte";
  import { device } from "../lib/device";

  type Library = { id: number; key: string; title: string; kind: string; shared: boolean };
  type User = {
    shareId: number;
    userId: number;
    name: string;
    username: string;
    email: string;
    thumb: string | null;
    home: boolean;
    pending: boolean;
    allLibraries: boolean;
    libraries: Library[];
    allowSync: boolean;
    allowCameraUpload: boolean;
    allowChannels: boolean;
    allowSubtitleAdmin: boolean;
    filterMovies: string;
    filterTelevision: string;
    filterMusic: string;
  };

  let users = $state<User[] | null>(null);
  let error = $state<string | null>(null);
  let selected = $state<number | null>(null);
  let tab = $state<"libraries" | "permissions" | "restrictions">("libraries");
  let busy = $state(false);
  let confirmRemove = $state(false);
  // Invitation : un compte, ses bibliothèques, le droit de télécharger.
  let inviting = $state(false);
  let who = $state("");
  let inviteLibs = $state<Library[]>([]);
  let inviteSync = $state(false);

  const user = $derived(users?.find((u) => u.shareId === selected) ?? null);

  async function load() {
    try {
      users = await invoke<User[]>("plex_users");
      error = null;
      if (selected === null && users.length && !device.phone) selected = users[0].shareId;
    } catch (e) {
      error = errorText(e);
    }
  }
  onMount(load);

  async function save(f: () => Promise<unknown>, rollback?: () => void) {
    busy = true;
    try {
      await f();
      toast(t("settings.users.saved"));
    } catch (e) {
      rollback?.();
      toast(errorText(e), true);
    } finally {
      busy = false;
    }
  }

  function toggleLibrary(u: User, lib: Library) {
    const before = lib.shared;
    lib.shared = !lib.shared;
    const ids = u.libraries.filter((l) => l.shared).map((l) => l.id);
    if (!ids.length) {
      lib.shared = before;
      return toast(t("settings.users.needLibrary"), true);
    }
    save(() => invoke("plex_user_libraries", { shareId: u.shareId, libraryIds: ids }), () => (lib.shared = before));
  }

  function permissions(u: User) {
    return {
      allowSync: u.allowSync,
      allowCameraUpload: u.allowCameraUpload,
      allowChannels: u.allowChannels,
      allowSubtitleAdmin: u.allowSubtitleAdmin,
      filterMovies: u.filterMovies,
      filterTelevision: u.filterTelevision,
      filterMusic: u.filterMusic,
    };
  }

  function setPermission(u: User, key: "allowSync" | "allowCameraUpload" | "allowChannels" | "allowSubtitleAdmin" | "filterMovies" | "filterTelevision" | "filterMusic", value: boolean | string) {
    const before = permissions(u);
    (u as Record<string, unknown>)[key] = value;
    save(() => invoke("plex_user_permissions", { userId: u.userId, permissions: permissions(u) }), () => Object.assign(u, before));
  }

  async function remove(u: User) {
    if (!confirmRemove) {
      confirmRemove = true;
      setTimeout(() => (confirmRemove = false), 5000);
      return;
    }
    confirmRemove = false;
    await save(async () => {
      await invoke("plex_user_remove", { shareId: u.shareId });
      users = users!.filter((x) => x.shareId !== u.shareId);
      selected = device.phone ? null : (users[0]?.shareId ?? null);
    });
  }

  async function startInvite() {
    inviting = true;
    selected = null;
    who = "";
    inviteSync = false;
    try {
      inviteLibs = (await invoke<Library[]>("plex_server_libraries")).map((l) => ({ ...l, shared: true }));
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  async function sendInvite() {
    const ids = inviteLibs.filter((l) => l.shared).map((l) => l.id);
    await save(async () => {
      await invoke("plex_user_invite", { who, libraryIds: ids, allowSync: inviteSync });
      inviting = false;
      await load();
    });
  }

  function choose(u: User) {
    inviting = false;
    confirmRemove = false;
    selected = u.shareId;
  }

  // « contentRating=PG,PG-13|label=Enfants » → « Classifications : PG, PG-13 · Étiquettes : Enfants ».
  function describe(filter: string): string {
    if (!filter) return t("settings.users.noRestriction");
    return filter
      .split("|")
      .map((part) => {
        const [k, v = ""] = part.split("=");
        const values = decodeURIComponent(v).replace(/,/g, ", ");
        const neg = k.endsWith("!");
        const name = k.replace("!", "") === "contentRating" ? t("settings.users.ratings") : t("settings.users.labels");
        return `${name}${neg ? ` (${t("settings.users.excluded")})` : ""} : ${values}`;
      })
      .join(" · ");
  }

  const kindIcon = (k: string) => (k === "movie" ? "film" : k === "artist" ? "music" : "tv");
</script>

{#snippet avatar(u: User)}
  {#if u.thumb}<img class="avatar" src={u.thumb} alt="" />{:else}<span class="avatar ph">{u.name.charAt(0).toUpperCase()}</span>{/if}
{/snippet}

{#if error}
  <p class="error">{error}</p>
{:else if !users}
  <div class="skeleton" style:height="320px"></div>
{:else}
  <div class="users" class:detail-open={device.phone && (user || inviting)}>
    <div class="list">
      <div class="list-head">
        <span class="muted small">{t("settings.users.count", { count: users.length })}</span>
        <button class="btn" onclick={startInvite}><Icon name="plus" size={16} /> {t("settings.users.invite")}</button>
      </div>
      {#each users as u (u.shareId)}
        <button class="user" class:on={u.shareId === selected} aria-current={u.shareId === selected ? "true" : undefined} onclick={() => choose(u)}>
          {@render avatar(u)}
          <span class="who">
            <strong>{u.name}</strong>
            <span class="muted small">
              {u.allLibraries ? t("settings.users.allLibraries") : t("settings.users.libraries", { count: u.libraries.filter((l) => l.shared).length })}
              {#if u.pending} · {t("settings.users.pending")}{/if}
              {#if u.home} · Plex Home{/if}
            </span>
          </span>
        </button>
      {/each}
    </div>

    <div class="detail">
      {#if inviting}
        <div class="head"><strong class="big">{t("settings.users.inviteTitle")}</strong></div>
        <label>
          {t("settings.users.who")}
          <input bind:value={who} placeholder="ami@exemple.fr" autocomplete="off" spellcheck="false" />
        </label>
        <div class="lines">
          {#each inviteLibs as lib (lib.id)}
            <label class="line">
              <span class="line-text"><Icon name={kindIcon(lib.kind)} size={16} /> {lib.title}</span>
              <input type="checkbox" class="switch" bind:checked={lib.shared} />
            </label>
          {/each}
          <label class="line">
            <span class="line-text">{t("settings.users.allowSync")}</span>
            <input type="checkbox" class="switch" bind:checked={inviteSync} />
          </label>
        </div>
        <div class="row">
          <button class="btn primary" disabled={busy || !who.trim()} onclick={sendInvite}>{t("settings.users.send")}</button>
          <button class="btn" onclick={() => (inviting = false)}>{t("settings.users.cancel")}</button>
        </div>
      {:else if user}
        <div class="head">
          {#if device.phone}<button class="btn round" onclick={() => (selected = null)} aria-label={t("common.c1.back")}><Icon name="back" size={18} /></button>{/if}
          {@render avatar(user)}
          <span class="who">
            <strong class="big">{user.name}</strong>
            <span class="muted small">{user.username}{#if user.pending} · {t("settings.users.pending")}{/if}</span>
          </span>
        </div>
        <div class="tabs" role="tablist">
          {#each [{ id: "libraries", label: t("settings.users.tabLibraries") }, { id: "permissions", label: t("settings.users.tabPermissions") }, { id: "restrictions", label: t("settings.users.tabRestrictions") }] as x (x.id)}
            <button role="tab" class:on={tab === x.id} aria-selected={tab === x.id} onclick={() => (tab = x.id as typeof tab)}>{x.label}</button>
          {/each}
        </div>

        <div class="lines">
          {#if tab === "libraries"}
            {#each user.libraries as lib (lib.id)}
              <label class="line">
                <span class="line-text"><Icon name={kindIcon(lib.kind)} size={16} /> {lib.title}</span>
                <input type="checkbox" class="switch" checked={lib.shared} disabled={busy} onchange={() => toggleLibrary(user!, lib)} />
              </label>
            {/each}
          {:else if tab === "permissions"}
            {#each [{ key: "allowSync", label: t("settings.users.allowSync"), hint: t("settings.users.allowSyncHint") }, { key: "allowSubtitleAdmin", label: t("settings.users.allowSubtitles"), hint: t("settings.users.allowSubtitlesHint") }, { key: "allowChannels", label: t("settings.users.allowChannels"), hint: "" }, { key: "allowCameraUpload", label: t("settings.users.allowCamera"), hint: "" }] as p (p.key)}
              <label class="line">
                <span class="line-text">
                  <strong>{p.label}</strong>
                  {#if p.hint}<span class="muted small">{p.hint}</span>{/if}
                </span>
                <input
                  type="checkbox"
                  class="switch"
                  checked={user[p.key as "allowSync"]}
                  disabled={busy}
                  onchange={(e) => setPermission(user!, p.key as "allowSync", e.currentTarget.checked)}
                />
              </label>
            {/each}
          {:else}
            {#each [{ key: "filterMovies", label: t("settings.users.movies") }, { key: "filterTelevision", label: t("settings.users.shows") }, { key: "filterMusic", label: t("settings.users.music") }] as f (f.key)}
              {@const value = user[f.key as "filterMovies"]}
              <div class="line">
                <span class="line-text">
                  <strong>{f.label}</strong>
                  <span class="muted small">{describe(value)}</span>
                </span>
                {#if value}
                  <button class="btn" disabled={busy} onclick={() => setPermission(user!, f.key as "filterMovies", "")}>{t("settings.users.clear")}</button>
                {/if}
              </div>
            {/each}
            <p class="muted small note">{t("settings.users.restrictionsHint")}</p>
          {/if}
        </div>

        <div class="row remove">
          <button class="btn" class:danger={confirmRemove} disabled={busy} onclick={() => remove(user!)}>
            <Icon name="x" size={16} />
            {confirmRemove ? t("settings.users.confirmRemove", { name: user.name }) : t("settings.users.remove")}
          </button>
        </div>
      {:else}
        <p class="muted">{t("settings.users.choose")}</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  .users {
    display: grid;
    grid-template-columns: minmax(220px, 320px) minmax(0, 1fr);
    gap: 18px;
    align-items: start;
  }

  .list {
    display: grid;
    gap: 4px;
  }

  .list-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 6px;
  }

  .user {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border: none;
    border-radius: 12px;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .user:hover {
    background: #ffffff0a;
  }

  .user.on {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .who {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .who strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .avatar {
    display: grid;
    flex: none;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    object-fit: cover;
  }

  .avatar.ph {
    background: var(--bg-hover);
    color: var(--text-dim);
    font-weight: 700;
  }

  .detail {
    display: grid;
    gap: 14px;
    padding: 18px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: #ffffff05;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .head .avatar {
    width: 52px;
    height: 52px;
  }

  .big {
    font-size: 18px;
  }

  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .tabs button {
    padding: 6px 14px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: none;
    color: var(--text-dim);
    font: inherit;
    font-weight: 600;
  }

  .tabs button.on {
    border-color: transparent;
    background: var(--accent);
    color: var(--accent-text);
  }

  .lines {
    display: grid;
  }

  .line {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 0;
    border-top: 1px solid var(--border);
    color: var(--text);
    font-size: 14px;
  }

  .line:first-child {
    border-top: none;
  }

  .line-text {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    min-width: 0;
  }

  .line-text strong {
    width: 100%;
  }

  label {
    display: grid;
    gap: 4px;
    color: var(--text-dim);
    font-size: 13px;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .remove {
    padding-top: 6px;
    border-top: 1px solid var(--border);
  }

  .btn.danger {
    border-color: transparent;
    background: var(--danger);
    color: #fff;
  }

  .note {
    margin: 8px 0 0;
  }

  /* Téléphone : la liste, puis la fiche du compte choisi à sa place. */
  :global(html.phone) .users {
    grid-template-columns: minmax(0, 1fr);
  }

  :global(html.phone) .users .detail {
    display: none;
  }

  :global(html.phone) .users.detail-open .list {
    display: none;
  }

  :global(html.phone) .users.detail-open .detail {
    display: grid;
  }
</style>
