<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type Item, type SeerrMedia } from "../lib/api";
  import { go } from "../lib/nav.svelte";
  import PosterCard from "../components/PosterCard.svelte";
  import SeerrCard from "../components/SeerrCard.svelte";
  import Row from "../components/Row.svelte";
  import { t } from "../lib/i18n";

  let { query }: { query: string } = $props();

  let plex = $state<Item[] | null>(null);
  let seerr = $state<SeerrMedia[] | null>(null);
  let plexError = $state<string | null>(null);
  let seerrError = $state<string | null>(null);

  onMount(() => {
    // Page ouverte depuis le menu (téléviseur) : rien à chercher tant qu'aucun titre n'est saisi.
    if (!query) return;
    api.search(query).then((r) => (plex = r), (e) => (plexError = errorText(e)));
    api.seerrSearch(query).then((r) => (seerr = r), (e) => (seerrError = errorText(e)));
  });

  /*
   * Résultats rangés par type, une rangée chacun : films, séries (saisons et épisodes compris), musique (artistes,
   * albums, titres, playlists), puis le reste (collections…). Une rangée sans résultat ne s'affiche pas.
   */
  const KINDS: { key: string; title: string; kinds: string[] }[] = [
    { key: "movies", title: t("search.movies"), kinds: ["movie"] },
    { key: "shows", title: t("search.shows"), kinds: ["show", "season", "episode"] },
    { key: "music", title: t("search.music"), kinds: ["artist", "album", "track", "playlist"] },
  ];
  const known = KINDS.flatMap((k) => k.kinds);
  const rows = $derived.by(() => {
    if (!plex) return [];
    const list = plex;
    return [
      ...KINDS.map((k) => ({ key: k.key, title: k.title, items: list.filter((i) => k.kinds.includes(i.kind)) })),
      { key: "other", title: t("search.other"), items: list.filter((i) => !known.includes(i.kind)) },
    ].filter((r) => r.items.length);
  });

  // Les titres déjà disponibles apparaissent dans la bibliothèque : on ne garde que le reste, films puis séries.
  const requestable = $derived(seerr?.filter((m) => m.status !== 5) ?? null);
  const seerrRows = $derived(
    requestable
      ? [
          { key: "movies", title: t("search.movies"), items: requestable.filter((m) => m.mediaType === "movie") },
          { key: "shows", title: t("search.shows"), items: requestable.filter((m) => m.mediaType === "tv") },
        ].filter((r) => r.items.length)
      : [],
  );
</script>

<div class="page">
  {#if !query}
    <p class="muted hint">{t("search.hint")}</p>
  {:else}
  <div class="row-title section">{t("search.inLibrary")}</div>
  {#if plexError}
    <p class="error">{plexError}</p>
  {:else if !plex}
    <p class="muted">{t("search.searching")}</p>
  {:else if !plex.length}
    <p class="muted">{t("search.noResults", { query })}</p>
  {:else}
    {#each rows as r (r.key)}
      <Row title={r.title}>
        {#each r.items as item (item.ratingKey)}<PosterCard {item} />{/each}
      </Row>
    {/each}
  {/if}

  <div class="row-title section">{t("search.notInLibrary")}</div>
  {#if seerrError}
    <p class="muted">
      {t("search.seerrUnavailable", { error: seerrError })}
      <button class="btn" onclick={() => go({ name: "settings" })}>{t("search.settings")}</button>
    </p>
  {:else if !requestable}
    <p class="muted">{t("search.searchingSeerr")}</p>
  {:else if !requestable.length}
    <p class="muted">{t("search.nothingElse")}</p>
  {:else}
    {#each seerrRows as r (r.key)}
      <Row title={r.title}>
        {#each r.items as media (media.mediaType + media.tmdbId)}<SeerrCard {media} />{/each}
      </Row>
    {/each}
  {/if}
  {/if}
</div>

<style>
  /* « Dans ta bibliothèque », « Pas dans la bibliothèque » : au-dessus de leurs rangées par type. */
  .section {
    margin-top: 36px;
    font-size: 22px;
    font-weight: 700;
  }

  .section:first-child {
    margin-top: 8px;
  }
</style>
