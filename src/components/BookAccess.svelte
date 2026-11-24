<script lang="ts">
  // Admin › Livres (Kavita par Maku Server) : les bibliothèques Kavita ouvertes à chaque utilisateur du serveur, mis en
  // page comme Dashboard › Utilisateurs (la liste des comptes à gauche, la fiche du compte choisi à droite, une
  // bibliothèque par interrupteur). Le premier interrupteur allumé lui crée son compte Kavita (le relais s'en charge) ;
  // tout éteindre lui retire l'accès, en gardant son compte et sa progression s'il revient. Les utilisateurs : ceux de
  // Plex (que Maku connaît déjà), ou ceux de Jellyfin et d'Emby (lus par le relais) ; un compte dont l'utilisateur a
  // quitté le serveur reste listé, pour pouvoir lui retirer l'accès.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { api, errorText, type KavitaAdmin } from "../lib/api";
  import { closeOnBack, toast } from "../lib/nav.svelte";
  import { device } from "../lib/device";
  import { t } from "../lib/i18n";
  import ErrorCard from "./ErrorCard.svelte";
  import Icon from "./Icon.svelte";

  type Row = { id: string; name: string; thumb: string | null; username: string | null; libraries: number[]; gone: boolean };

  let libraries = $state<KavitaAdmin["libraries"]>([]);
  let rows = $state<Row[] | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let selected = $state<string | null>(null);
  // Téléphone : la fiche d'un utilisateur remplace la liste ; Retour y ramène.
  $effect(() => (device.phone && selected !== null ? closeOnBack(() => (selected = null)) : undefined));

  const row = $derived(rows?.find((r) => r.id === selected) ?? null);

  async function load() {
    error = null;
    try {
      const admin = await api.kavitaAdmin();
      // Plex : les comptes avec qui le serveur est partagé (identifiants plex.tv, comme le relais).
      const members: { id: string; name: string; thumb: string | null }[] =
        admin.members?.map((m) => ({ ...m, thumb: null })) ??
        (await invoke<{ userId: number; name: string; thumb: string | null }[]>("plex_users").catch(() => [])).map((u) => ({
          id: `plex:${u.userId}`,
          name: u.name,
          thumb: u.thumb,
        }));
      const accounts = new Map(admin.users.map((u) => [u.id, u]));
      rows = [
        ...members.map((m) => ({ ...m, username: accounts.get(m.id)?.username ?? null, libraries: accounts.get(m.id)?.libraries ?? [], gone: false })),
        ...admin.users
          .filter((u) => !members.some((m) => m.id === u.id))
          .map((u) => ({ id: u.id, name: u.name, thumb: null, username: u.username, libraries: u.libraries, gone: true })),
      ];
      libraries = admin.libraries;
      if (selected === null && rows.length && !device.phone) selected = rows[0].id;
    } catch (e) {
      error = errorText(e);
    }
  }
  onMount(load);

  async function save(r: Row, next: number[]) {
    busy = true;
    try {
      const saved = await api.kavitaAdminSet(r.id, r.name, next);
      r.libraries = saved.libraries;
      // Premier accès : le relais vient de créer son compte Kavita.
      if (!r.username && next.length) r.username = (await api.kavitaAdmin()).users.find((u) => u.id === r.id)?.username ?? null;
      toast(next.length ? t("books.access.saved", { name: r.name }) : t("books.access.removed", { name: r.name }));
    } catch (e) {
      toast(errorText(e), true);
    } finally {
      busy = false;
    }
  }

  function toggle(r: Row, id: number) {
    save(r, r.libraries.includes(id) ? r.libraries.filter((x) => x !== id) : [...r.libraries, id].sort((a, b) => a - b));
  }

  const status = (r: Row) => (r.libraries.length ? t("books.access.count", { count: r.libraries.length }) : t("books.access.none"));
  /** Type de bibliothèque Kavita (LibraryType) : son libellé. */
  const kindOf = (type: number) => t(`books.kind.${type === 0 ? "manga" : type === 2 || type === 4 ? "book" : "comic"}`);
</script>

{#snippet avatar(r: Row)}
  {#if r.thumb}<img class="avatar" src={r.thumb} alt="" />{:else}<span class="avatar ph">{r.name.charAt(0).toUpperCase()}</span>{/if}
{/snippet}

{#if error}
  <ErrorCard message={error} onretry={load} />
{:else if !rows}
  <div class="skeleton" style:height="320px"></div>
{:else if !rows.length}
  <p class="muted">{t("books.access.noUsers")}</p>
{:else}
  <div class="users" class:detail-open={device.phone && row}>
    <div class="list">
      <div class="list-head">
        <span class="muted small">{t("settings.users.count", { count: rows.length })}</span>
      </div>
      {#each rows as r (r.id)}
        <button class="user" class:on={r.id === selected} aria-current={r.id === selected ? "true" : undefined} onclick={() => (selected = r.id)}>
          {@render avatar(r)}
          <span class="who">
            <strong>{r.name}</strong>
            <span class="muted small" class:granted={r.libraries.length > 0}>
              {status(r)}{#if r.gone}{" · "}{t("books.access.gone")}{/if}
            </span>
          </span>
        </button>
      {/each}
    </div>

    <div class="detail">
      {#if row}
        <div class="head">
          {#if device.phone}<button class="btn round" onclick={() => (selected = null)} aria-label={t("common.c1.back")}><Icon name="back" size={18} /></button>{/if}
          {@render avatar(row)}
          <span class="who">
            <strong class="big">{row.name}</strong>
            <span class="muted small">
              {row.username ? t("books.access.account", { username: row.username }) : t("books.access.noAccount")}{#if row.gone}{" · "}{t("books.access.gone")}{/if}
            </span>
          </span>
        </div>

        <div class="lines" role="group" aria-label={t("books.access.libraries", { name: row.name })}>
          {#each libraries as lib (lib.id)}
            <label class="line">
              <span class="line-text">
                <Icon name="book" size={16} />
                {lib.name}
                <span class="muted small">{kindOf(lib.type)}</span>
              </span>
              <input type="checkbox" class="switch" checked={row.libraries.includes(lib.id)} disabled={busy} onchange={() => toggle(row!, lib.id)} />
            </label>
          {/each}
        </div>

        {#if row.libraries.length}
          <div class="row remove">
            <button class="btn" disabled={busy} onclick={() => save(row!, [])}>
              <Icon name="x" size={16} />
              {t("settings.users.remove")}
            </button>
            <span class="muted small">{t("books.access.keepsProgress")}</span>
          </div>
        {/if}
      {:else}
        <p class="muted">{t("settings.users.choose")}</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  /* Même mise en page que Dashboard › Utilisateurs (ServerUsers.svelte). */
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
    min-height: 40px;
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

  .granted {
    color: var(--accent);
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

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
  }

  .remove {
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }

  .small {
    font-size: 13px;
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
