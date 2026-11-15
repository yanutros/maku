<script lang="ts">
  // Demandes de tous les utilisateurs (Admin › Demandes, administrateur de Seerr) : les valider ou les refuser,
  // corriger leur serveur, profil ou dossier avant validation, les relancer après un échec, les supprimer.
  import ErrorCard from "./ErrorCard.svelte";
  import RequestEditDialog from "./RequestEditDialog.svelte";
  import Icon from "./Icon.svelte";
  import { onMount } from "svelte";
  import { api, errorText, type RequestFilter, type SeerrRequest } from "../lib/api";
  import { go, toast } from "../lib/nav.svelte";
  import { ago, refreshPending, requestState } from "../lib/seerrAdmin.svelte";
  import { t, locale } from "../lib/i18n";

  let { filter }: { filter: RequestFilter } = $props();

  const PAGE = 20;
  let list = $state<SeerrRequest[] | null>(null);
  let total = $state(0);
  let error = $state<string | null>(null);
  let more = $state(false);
  /** Demande dont un geste est en cours (boutons désactivés). */
  let busy = $state<number | null>(null);
  /** Demande dont la suppression attend sa confirmation. */
  let confirming = $state<number | null>(null);
  let editing = $state<SeerrRequest | null>(null);

  /** Recharge la liste ; `count` garde autant de demandes qu'il y en avait d'affichées. */
  async function load(count = PAGE) {
    try {
      const page = await api.seerrRequests(filter, 0, Math.max(count, PAGE));
      list = page.results;
      total = page.total;
      error = null;
    } catch (e) {
      error = errorText(e);
    }
  }
  onMount(() => load());

  async function loadMore() {
    if (!list) return;
    more = true;
    try {
      const page = await api.seerrRequests(filter, list.length, PAGE);
      list = [...list, ...page.results];
      total = page.total;
    } catch (e) {
      toast(errorText(e), true);
    } finally {
      more = false;
    }
  }

  /** Un geste sur une demande, puis la liste et la pastille du menu à jour. */
  async function act(r: SeerrRequest, run: () => Promise<unknown>, done: string) {
    busy = r.id;
    try {
      await run();
      toast(done);
      confirming = null;
      await Promise.all([load(list?.length), refreshPending()]);
    } catch (e) {
      toast(errorText(e), true);
    } finally {
      busy = null;
    }
  }

  const approve = (r: SeerrRequest) => act(r, () => api.seerrRequestStatus(r.id, true), t("requests.done.approved", { title: r.media.title }));
  const decline = (r: SeerrRequest) => act(r, () => api.seerrRequestStatus(r.id, false), t("requests.done.declined", { title: r.media.title }));
  const retry = (r: SeerrRequest) => act(r, () => api.seerrRequestRetry(r.id), t("requests.done.retried", { title: r.media.title }));
  const remove = (r: SeerrRequest) => act(r, () => api.seerrRequestDelete(r.id), t("requests.done.deleted", { title: r.media.title }));

  async function edited(approved: boolean) {
    const r = editing!;
    editing = null;
    toast(t(approved ? "requests.done.approved" : "requests.done.saved", { title: r.media.title }));
    await Promise.all([load(list?.length), refreshPending()]);
  }

  const open = (r: SeerrRequest) => go({ name: "seerr", mediaType: r.media.mediaType, tmdbId: r.media.tmdbId });

  function seasons(r: SeerrRequest) {
    if (r.media.mediaType !== "tv" || !r.seasons.length) return null;
    return t("requests.seasons", { count: r.seasons.length, list: r.seasons.join(", ") });
  }
</script>

{#if error}
  <ErrorCard message={error} onretry={() => load(list?.length)} />
{:else if !list}
  <div class="list">
    {#each [0, 1, 2] as i (i)}<div class="skeleton" style:height="150px" style:border-radius="16px"></div>{/each}
  </div>
{:else if !list.length}
  <div class="empty">
    <Icon name="checkCircle" size={34} />
    <p>{t(filter === "pending" ? "requests.emptyPending" : "requests.empty")}</p>
  </div>
{:else}
  <div class="list">
    {#each list as r (r.id)}
      {@const st = requestState(r)}
      <article class="req" class:busy={busy === r.id}>
        {#if r.media.backdrop}<div class="bg" style:background-image="url({r.media.backdrop})"></div>{/if}
        <button class="poster" onclick={() => open(r)} aria-label={r.media.title}>
          {#if r.media.poster}<img src={r.media.poster} alt="" loading="lazy" />{:else}<Icon name={r.media.mediaType === "tv" ? "tv" : "film"} size={26} />{/if}
        </button>
        <div class="info">
          <div class="kicker">
            <span>{t(r.media.mediaType === "tv" ? "requests.kind.tv" : "requests.kind.movie")}</span>
            {#if r.media.year}<span>· {r.media.year}</span>{/if}
            {#if r.is4k}<span class="badge outline">4K</span>{/if}
          </div>
          <button class="title" onclick={() => open(r)}>{r.media.title || t("requests.unknownTitle")}</button>
          {#if seasons(r)}<div class="seasons">{seasons(r)}</div>{/if}
          <div class="who">
            {#if r.requestedBy.avatar}<img class="avatar" src={r.requestedBy.avatar} alt="" />{:else}<span class="avatar letter">{r.requestedBy.name.slice(0, 1).toUpperCase()}</span>{/if}
            <span>{t("requests.requestedBy", { name: r.requestedBy.name })}</span>
            <span class="muted">· {ago(r.createdAt, locale)}</span>
          </div>
          {#if r.modifiedBy && r.status !== 1}
            <div class="who small muted">{t(r.status === 3 ? "requests.declinedBy" : "requests.approvedBy", { name: r.modifiedBy.name, when: ago(r.updatedAt, locale) })}</div>
          {/if}
          {#if r.profileName || r.rootFolder}
            <div class="chips">
              {#if r.profileName}<span class="chip"><Icon name="sliders" size={13} /> {r.profileName}</span>{/if}
              {#if r.rootFolder}<span class="chip mono"><Icon name="library" size={13} /> {r.rootFolder}</span>{/if}
            </div>
          {/if}
        </div>
        <div class="side">
          <span class="state {st.tone}">{st.label}</span>
          <div class="actions">
            {#if confirming === r.id}
              <span class="confirm">{t("requests.confirmDelete")}</span>
              <button class="btn" disabled={busy === r.id} onclick={() => (confirming = null)}>{t("request.cancel")}</button>
              <button class="btn danger" disabled={busy === r.id} onclick={() => remove(r)}>{t("requests.delete")}</button>
            {:else}
              {#if r.status === 1}
                <button class="btn primary" disabled={busy === r.id} onclick={() => approve(r)}><Icon name="check" size={16} /> {t("requests.approve")}</button>
                <button class="btn" disabled={busy === r.id} onclick={() => decline(r)}><Icon name="x" size={16} /> {t("requests.decline")}</button>
                <button class="btn" disabled={busy === r.id} onclick={() => (editing = r)}><Icon name="edit" size={16} /> {t("requests.edit.open")}</button>
              {:else if r.status === 4}
                <button class="btn primary" disabled={busy === r.id} onclick={() => retry(r)}><Icon name="restart" size={16} /> {t("requests.retry")}</button>
              {/if}
              <button class="btn ghost-danger" disabled={busy === r.id} onclick={() => (confirming = r.id)} aria-label={t("requests.delete")} title={t("requests.delete")}>
                <Icon name="trash" size={16} /> <span class="del-label">{t("requests.delete")}</span>
              </button>
            {/if}
          </div>
        </div>
      </article>
    {/each}
  </div>
  {#if list.length < total}
    <div class="more">
      <button class="btn" disabled={more} onclick={loadMore}>{t("requests.more", { count: total - list.length })}</button>
    </div>
  {/if}
{/if}

{#if editing}
  <RequestEditDialog request={editing} onclose={() => (editing = null)} ondone={edited} />
{/if}

<style>
  .list {
    display: grid;
    gap: 14px;
  }

  .req {
    position: relative;
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 18px;
    padding: 16px;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: #ffffff05;
    transition: opacity 0.15s;
  }

  .req.busy {
    opacity: 0.6;
  }

  /* L'image du titre, fondue vers la droite, derrière la carte (comme dans Seerr). */
  .bg {
    position: absolute;
    inset: 0;
    z-index: 0;
    background-position: center 25%;
    background-size: cover;
    opacity: 0.22;
    -webkit-mask-image: linear-gradient(90deg, #000 0%, transparent 75%);
    mask-image: linear-gradient(90deg, #000 0%, transparent 75%);
    pointer-events: none;
  }

  .poster,
  .info,
  .side {
    position: relative;
    z-index: 1;
  }

  .poster {
    display: grid;
    place-items: center;
    width: 84px;
    aspect-ratio: 2 / 3;
    padding: 0;
    overflow: hidden;
    border: none;
    border-radius: 10px;
    background: var(--bg-hover);
    color: var(--text-faint);
  }

  .poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .info {
    display: grid;
    gap: 6px;
    min-width: 0;
  }

  .kicker {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    color: var(--text-dim);
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .title {
    justify-self: start;
    max-width: 100%;
    padding: 0;
    overflow: hidden;
    border: none;
    background: none;
    color: var(--text);
    font-size: 19px;
    font-weight: 800;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title:hover {
    color: var(--accent);
  }

  .seasons {
    color: var(--text-dim);
    font-size: 13px;
  }

  .who {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }

  .who.small {
    font-size: 12px;
  }

  .avatar {
    width: 22px;
    height: 22px;
    flex: none;
    border-radius: 50%;
    object-fit: cover;
  }

  .avatar.letter {
    display: grid;
    place-items: center;
    background: var(--accent);
    color: var(--accent-text);
    font-size: 11px;
    font-weight: 800;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 2px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
    padding: 4px 9px;
    overflow: hidden;
    border-radius: 8px;
    background: #ffffff10;
    color: var(--text-dim);
    font-size: 12px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .chip.mono {
    font-family: ui-monospace, Consolas, monospace;
  }

  /* L'état et les boutons en une colonne de même largeur d'une carte à l'autre : l'état a la taille et la forme
     d'un bouton, et un bouton seul (Supprimer) prend toute la largeur, juste sous l'état. */
  .side {
    display: grid;
    grid-template-columns: minmax(210px, auto);
    justify-items: stretch;
    gap: 8px;
  }

  .state {
    --tone: var(--text-dim);
    display: flex;
    align-items: center;
    justify-content: center;
    height: 36px;
    padding: 0 14px;
    border: 1px solid color-mix(in srgb, var(--tone) 35%, transparent);
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--tone) 16%, transparent);
    color: var(--tone);
    font-size: 13px;
    font-weight: 700;
    white-space: nowrap;
  }

  .state.pending {
    --tone: var(--accent);
  }

  .state.processing {
    --tone: var(--info);
  }

  .state.available,
  .state.partial {
    --tone: var(--ok);
  }

  .state.declined,
  .state.failed {
    --tone: var(--danger);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .actions .btn {
    flex: 1 1 auto;
    height: 36px;
    padding: 0 14px;
  }

  .btn.danger {
    border-color: transparent;
    background: var(--danger);
    color: #fff;
  }

  .btn.danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--danger) 85%, #fff);
  }

  .btn.ghost-danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--danger) 22%, transparent);
    color: var(--danger);
  }

  .confirm {
    color: var(--danger);
    font-size: 13px;
    font-weight: 600;
  }

  .empty {
    display: grid;
    justify-items: center;
    gap: 10px;
    padding: 60px 20px;
    border: 1px dashed var(--border);
    border-radius: 16px;
    color: var(--text-faint);
    text-align: center;
  }

  .empty p {
    margin: 0;
    color: var(--text-dim);
  }

  .more {
    display: flex;
    justify-content: center;
    margin: 18px 0 8px;
  }

  /* Téléphone (et fenêtre étroite) : l'affiche et le texte, puis l'état et les gestes dessous, sur toute la largeur. */
  @media (max-width: 760px) {
    .req {
      grid-template-columns: auto 1fr;
      gap: 14px;
      padding: 14px;
    }

    .poster {
      width: 64px;
    }

    .side {
      grid-column: 1 / -1;
      grid-template-columns: 1fr;
    }

    .actions {
      justify-content: stretch;
    }

    .actions .btn {
      flex: 1 1 auto;
    }

    .title {
      font-size: 17px;
      white-space: normal;
    }
  }
</style>
