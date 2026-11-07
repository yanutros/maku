<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type SeerrMedia, type SeerrSlider } from "../lib/api";
  import { go } from "../lib/nav.svelte";
  import SeerrCard from "../components/SeerrCard.svelte";
  import Row from "../components/Row.svelte";
  import TvRows from "../components/TvRows.svelte";
  import { device } from "../lib/device";
  import { fromSeerr } from "../lib/tvcards";
  import { t } from "../lib/i18n";

  // Rangées de l'accueil de Seerr, dans le même ordre.
  type Source = { id: string; title: string; load: () => Promise<SeerrMedia[]> };
  const slider = (s: SeerrSlider) => () => api.seerrSlider(s);
  const SOURCES: Source[] = [
    { id: "requests", title: t("discover.rows.requests"), load: api.seerrRecentRequests },
    { id: "watchlist", title: t("discover.rows.watchlist"), load: api.seerrWatchlist },
    { id: "trending", title: t("discover.rows.trending"), load: slider("trending") },
    { id: "popularMovies", title: t("discover.rows.popularMovies"), load: slider("popularMovies") },
    { id: "upcomingMovies", title: t("discover.rows.upcomingMovies"), load: slider("upcomingMovies") },
    { id: "popularTv", title: t("discover.rows.popularTv"), load: slider("popularTv") },
    { id: "upcomingTv", title: t("discover.rows.upcomingTv"), load: slider("upcomingTv") },
  ];

  let rows = $state<Record<string, SeerrMedia[] | null>>({});
  let error = $state<string | null>(null);

  // Téléviseur : les mêmes rangées que l'accueil, une fois toutes chargées (une rangée arrivée en retard
  // décalerait la sélection). Les cartes d'ordinateur ne s'y prêtent pas : leurs boutons n'apparaissent
  // qu'au survol de la souris, la télécommande ne pouvait pas les atteindre.
  const tvRows = $derived(
    SOURCES.some((s) => rows[s.id] === null)
      ? []
      : SOURCES.map((s) => ({ title: s.title, cards: (rows[s.id] ?? []).map((m) => ({ ...fromSeerr(m), eyebrow: s.title })) })).filter(
          (r) => r.cards.length,
        ),
  );

  onMount(() => {
    for (const s of SOURCES) {
      rows[s.id] = null;
      s.load().then(
        (list) => (rows[s.id] = list),
        (e) => {
          rows[s.id] = [];
          // Seerr injoignable ou non configuré : un seul message suffit.
          if (s.id === "trending") error = errorText(e);
        },
      );
    }
  });
</script>

{#if device.tv && !error}
  {#if tvRows.length}
    <TvRows rows={tvRows} />
  {:else}
    <div class="page"><h1 class="row-title page-title">{t("discover.title")}</h1></div>
  {/if}
{:else}
<div class="page">
  <h1 class="row-title page-title">{t("discover.title")}</h1>
  {#if error}
    <p class="error">{error}</p>
    <button class="btn" onclick={() => go({ name: "settings", page: "services" })}>{t("discover.configureSeerr")}</button>
  {:else}
    {#each SOURCES as s (s.id)}
      {#if rows[s.id] === null}
        <div class="loading">
          <div class="row-title">{s.title}</div>
          <div class="skeleton" style:height="240px"></div>
        </div>
      {:else if rows[s.id]?.length}
        <Row title={s.title}>
          {#each rows[s.id]! as media (media.mediaType + media.tmdbId)}<SeerrCard {media} />{/each}
        </Row>
      {/if}
    {/each}
  {/if}
</div>
{/if}

<style>
  .loading .row-title {
    margin: 28px 0 12px;
  }
</style>
