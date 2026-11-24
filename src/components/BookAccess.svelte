<script lang="ts">
  // Administration › Livres (Kavita par Maku Server) : les bibliothèques Kavita ouvertes à chaque utilisateur du
  // serveur. La première case cochée lui crée son compte Kavita (le relais s'en charge) ; tout décocher lui retire
  // l'accès, en gardant son compte et sa progression s'il revient. Les utilisateurs : ceux de Plex (que Maku connaît
  // déjà), ou ceux de Jellyfin et d'Emby (lus par le relais) ; un compte dont l'utilisateur a quitté le serveur reste
  // listé, pour pouvoir lui retirer l'accès.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { api, errorText, type KavitaAdmin } from "../lib/api";
  import { toast } from "../lib/nav.svelte";
  import { t } from "../lib/i18n";
  import ErrorCard from "./ErrorCard.svelte";

  type Row = { id: string; name: string; libraries: number[]; gone: boolean };

  let libraries = $state<KavitaAdmin["libraries"]>([]);
  let rows = $state<Row[] | null>(null);
  let error = $state<string | null>(null);
  let busy = $state<string | null>(null);

  async function load() {
    error = null;
    try {
      const admin = await api.kavitaAdmin();
      // Plex : les comptes avec qui le serveur est partagé (identifiants plex.tv, comme le relais).
      const members =
        admin.members ??
        (await invoke<{ userId: number; name: string }[]>("plex_users").catch(() => [])).map((u) => ({ id: `plex:${u.userId}`, name: u.name }));
      const accounts = new Map(admin.users.map((u) => [u.id, u]));
      rows = [
        ...members.map((m) => ({ id: m.id, name: m.name, libraries: accounts.get(m.id)?.libraries ?? [], gone: false })),
        ...admin.users.filter((u) => !members.some((m) => m.id === u.id)).map((u) => ({ id: u.id, name: u.name, libraries: u.libraries, gone: true })),
      ];
      libraries = admin.libraries;
    } catch (e) {
      error = errorText(e);
    }
  }
  onMount(load);

  async function toggle(row: Row, id: number) {
    const next = row.libraries.includes(id) ? row.libraries.filter((x) => x !== id) : [...row.libraries, id].sort((a, b) => a - b);
    busy = row.id;
    try {
      const saved = await api.kavitaAdminSet(row.id, row.name, next);
      row.libraries = saved.libraries;
      toast(next.length ? t("books.access.saved", { name: row.name }) : t("books.access.removed", { name: row.name }));
    } catch (e) {
      toast(errorText(e), true);
    } finally {
      busy = null;
    }
  }
</script>

{#if error}
  <ErrorCard message={error} onretry={load} />
{:else if !rows}
  <div class="skeleton" style:height="200px"></div>
{:else if !rows.length}
  <p class="muted">{t("books.access.noUsers")}</p>
{:else}
  <div class="list">
    {#each rows as row (row.id)}
      <section class="user" aria-label={row.name}>
        <div class="who">
          <strong>{row.name}</strong>
          {#if row.gone}<span class="muted small">{t("books.access.gone")}</span>{/if}
          <span class="muted small state">{row.libraries.length ? t("books.access.count", { count: row.libraries.length }) : t("books.access.none")}</span>
        </div>
        <div class="libs">
          {#each libraries as lib (lib.id)}
            <label class="chip" class:on={row.libraries.includes(lib.id)}>
              <input type="checkbox" checked={row.libraries.includes(lib.id)} disabled={busy === row.id} onchange={() => toggle(row, lib.id)} />
              {lib.name}
            </label>
          {/each}
        </div>
      </section>
    {/each}
  </div>
{/if}

<style>
  .list {
    display: grid;
    gap: 12px;
  }

  .user {
    display: grid;
    gap: 12px;
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-elev);
  }

  .who {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 6px 12px;
  }

  .state {
    margin-left: auto;
  }

  .small {
    font-size: 13px;
  }

  .libs {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 7px 14px;
    border: 1px solid var(--border);
    border-radius: 999px;
    cursor: pointer;
    font-size: 14px;
  }

  .chip.on {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }

  .chip input {
    accent-color: var(--accent);
  }
</style>
