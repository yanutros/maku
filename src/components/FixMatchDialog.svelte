<script lang="ts">
  // « Corriger l'association » d'un film, d'une série, d'un artiste ou d'un album (administrateur du serveur Plex),
  // comme dans Plex Web : les titres que l'agent de Plex propose, une recherche par titre et année (et artiste, pour un
  // album) si le bon n'y est pas, puis « Associer ». Plex télécharge alors les informations et les images du titre
  // choisi. Pas pour une saison, un épisode ou un morceau : ils suivent l'association de leur série ou de leur album.
  import ErrorCard from "./ErrorCard.svelte";
  import Icon from "./Icon.svelte";
  import { onMount } from "svelte";
  import { api, errorText, type MatchResult } from "../lib/api";
  import { nav, reload, toast } from "../lib/nav.svelte";
  import { t } from "../lib/i18n";

  const item = nav.matching!;

  let title = $state(item.title);
  let year = $state(item.year ? String(item.year) : "");
  // Album : son artiste, pour une recherche plus juste.
  const album = item.kind === "album";
  let artist = $state(album ? (item.showTitle ?? "") : "");
  const icon = item.kind === "show" ? "tv" : item.kind === "artist" ? "user" : album ? "music" : "film";
  let results = $state<MatchResult[] | null>(null);
  let searching = $state(false);
  let error = $state<string | null>(null);
  let chosen = $state<MatchResult | null>(null);
  let saving = $state(false);

  /** `manual` : la recherche tapée ; sinon les propositions de Plex d'après le titre actuel. */
  async function search(manual: boolean) {
    searching = true;
    error = null;
    chosen = null;
    try {
      const y = Number.parseInt(year, 10);
      results = await api.plexMatches(item.ratingKey, manual ? title : null, manual && !Number.isNaN(y) ? y : null, manual && album ? artist : null);
    } catch (e) {
      error = errorText(e);
      results = [];
    } finally {
      searching = false;
    }
  }
  onMount(() => search(false));

  function close() {
    nav.matching = null;
  }

  async function apply() {
    if (!chosen) return;
    saving = true;
    error = null;
    try {
      await api.plexFixMatch(item.ratingKey, chosen.guid, chosen.name, chosen.year);
      toast(t("fixMatch.done", { title: chosen.name }));
      close();
      reload();
      // Plex télécharge les nouvelles informations en arrière-plan : la page se recharge une fois de plus ensuite.
      setTimeout(reload, 4000);
    } catch (e) {
      error = errorText(e);
    } finally {
      saving = false;
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="overlay" onclick={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("fixMatch.title", { title: item.title })}>
    <header>
      <Icon name="search" size={18} />
      <h2>{t("fixMatch.title", { title: item.title })}</h2>
      <button class="x" onclick={close} aria-label={t("common.c1.close")}><Icon name="x" size={18} /></button>
    </header>

    <form class="search" onsubmit={(e) => (e.preventDefault(), search(true))}>
      <label class="field grow">
        <span>{t("fixMatch.searchTitle")}</span>
        <input bind:value={title} />
      </label>
      {#if album}
        <label class="field grow">
          <span>{t("fixMatch.artist")}</span>
          <input bind:value={artist} />
        </label>
      {/if}
      <label class="field year">
        <span>{t("fixMatch.year")}</span>
        <input bind:value={year} inputmode="numeric" maxlength="4" />
      </label>
      <button class="btn" type="submit" disabled={searching || !title.trim()}><Icon name="search" size={16} /> {t("fixMatch.search")}</button>
    </form>

    <div class="results">
      {#if error}<ErrorCard message={error} />{/if}
      {#if searching || !results}
        {#each [0, 1, 2, 3] as i (i)}<div class="skeleton row-skeleton"></div>{/each}
      {:else if !results.length}
        {#if !error}<p class="muted empty">{t("fixMatch.none")}</p>{/if}
      {:else}
        <p class="muted hint">{t("fixMatch.pick")}</p>
        <div class="list" role="listbox" aria-label={t("fixMatch.results")}>
          {#each results as r (r.guid)}
            <button class="result" class:chosen={chosen?.guid === r.guid} role="option" aria-selected={chosen?.guid === r.guid} onclick={() => (chosen = r)}>
              <span class="thumb">{#if r.thumb}<img src={r.thumb} alt="" loading="lazy" />{:else}<Icon name={icon} size={20} />{/if}</span>
              <span class="text">
                <span class="name">{r.name}{#if r.year}{" "}<span class="muted">({r.year})</span>{/if}</span>
                {#if r.artist}<span class="summary">{r.artist}</span>{/if}
                {#if r.summary}<span class="summary">{r.summary}</span>{/if}
              </span>
              {#if r.score !== null}<span class="score" title={t("fixMatch.score")}>{r.score} %</span>{/if}
              {#if chosen?.guid === r.guid}<span class="check"><Icon name="check" size={16} /></span>{/if}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <footer>
      <button class="btn" onclick={close}>{t("common.c1.cancel")}</button>
      <button class="btn primary" disabled={!chosen || saving} onclick={apply}>{saving ? t("fixMatch.applying") : t("fixMatch.apply")}</button>
    </footer>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 70;
    display: grid;
    place-items: center;
    background: #000a;
    backdrop-filter: blur(4px);
    animation: fade 0.15s;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  .dialog {
    display: flex;
    flex-direction: column;
    width: min(680px, 94vw);
    height: min(720px, 88vh);
    overflow: hidden;
    border: 1px solid #ffffff1a;
    border-radius: 18px;
    background: var(--bg-elev);
    box-shadow: 0 30px 80px #000c;
  }

  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
    color: var(--accent);
  }

  h2 {
    flex: 1;
    margin: 0;
    overflow: hidden;
    color: var(--text);
    font-size: 18px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .x {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--text-dim);
  }

  .x:hover {
    background: #ffffff14;
  }

  .search {
    display: flex;
    align-items: flex-end;
    gap: 10px;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
  }

  .field {
    display: grid;
    gap: 6px;
    min-width: 0;
  }

  .field span {
    color: var(--text-dim);
    font-size: 13px;
    font-weight: 600;
  }

  .field.grow {
    flex: 1;
  }

  .field.year {
    width: 90px;
  }

  .field input {
    width: 100%;
  }

  .results {
    flex: 1;
    min-height: 0;
    padding: 14px 20px;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .hint {
    margin: 0 0 10px;
    font-size: 13px;
  }

  .empty {
    margin: 30px 0;
    text-align: center;
  }

  .row-skeleton {
    height: 96px;
    margin-bottom: 8px;
    border-radius: 12px;
  }

  .list {
    display: grid;
    gap: 8px;
  }

  .result {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    padding: 8px 12px 8px 8px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: #ffffff06;
    color: var(--text);
    text-align: left;
    transition: background 0.15s, border-color 0.15s;
  }

  .result:hover {
    background: #ffffff10;
  }

  .result.chosen {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .thumb {
    display: grid;
    flex: none;
    place-items: center;
    width: 54px;
    height: 80px;
    overflow: hidden;
    border-radius: 8px;
    background: var(--bg-hover);
    color: var(--text-faint);
  }

  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .text {
    display: grid;
    flex: 1;
    gap: 4px;
    min-width: 0;
  }

  .name {
    font-weight: 700;
  }

  .summary {
    display: -webkit-box;
    overflow: hidden;
    color: var(--text-dim);
    font-size: 12px;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .score {
    flex: none;
    padding: 4px 8px;
    border-radius: 8px;
    background: #ffffff12;
    color: var(--text-dim);
    font-size: 12px;
    font-weight: 700;
  }

  .check {
    display: grid;
    flex: none;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-text);
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
    padding: 14px 20px;
    border-top: 1px solid var(--border);
  }

  /* Téléphone : la fenêtre prend tout l'écran, l'année passe sous le titre. */
  :global(html.phone) .dialog {
    width: 100vw;
    height: 100dvh;
    border-radius: 0;
  }

  :global(html.phone) .search {
    flex-wrap: wrap;
  }

  :global(html.phone) .search .field.grow {
    flex-basis: 100%;
  }

  :global(html.phone) .search .btn {
    flex: 1;
  }
</style>
