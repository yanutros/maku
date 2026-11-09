<script lang="ts">
  import { api, type ItemDetail, type SiteRating } from "../lib/api";
  import { ratingPrefs, siteOf } from "../lib/ratings.svelte";
  import { locale, t } from "../lib/i18n";

  /**
   * Notes des sites d'évaluation d'un film ou d'une série, comme dans Plex : IMDb, Rotten Tomatoes (tomate de la
   * presse, pop-corn du public), TMDB, et Allociné (presse et spectateurs) cherché à part. Chaque site se masque
   * dans Réglages → Interface. Les pastilles sont dessinées ici (pas les logos des sites).
   */
  let { item }: { item: ItemDetail } = $props();

  let allocine = $state<{ key: string; list: SiteRating[] }>({ key: "", list: [] });
  const wantsAllocine = $derived(ratingPrefs.allocine && !!item.imdbId && (item.kind === "movie" || item.kind === "show"));

  $effect(() => {
    const key = item.ratingKey;
    if (!wantsAllocine || allocine.key === key) return;
    allocine = { key, list: [] };
    api.allocineRatings(item.imdbId!, item.kind === "show").then(
      (list) => {
        if (allocine.key === key) allocine = { key, list };
      },
      () => {},
    );
  });

  const shown = $derived(
    [...item.siteRatings, ...(wantsAllocine && allocine.key === item.ratingKey ? allocine.list : [])].filter((r) => ratingPrefs[siteOf(r.source)]),
  );

  const num = (v: number, digits = 1) => v.toLocaleString(locale, { minimumFractionDigits: digits, maximumFractionDigits: digits });

  function value(r: SiteRating): string {
    switch (r.source) {
      case "imdb":
      case "community":
        return num(r.value);
      case "allocinePress":
      case "allocineUsers":
        return num(r.value / 2);
      default:
        return `${Math.round(r.value * 10)} %`;
    }
  }

  const names: Record<SiteRating["source"], string> = {
    imdb: "IMDb",
    rtCritic: "Rotten Tomatoes",
    rtAudience: "Rotten Tomatoes",
    tmdb: "TMDB",
    community: t("ratings.community"),
    allocinePress: "Allociné",
    allocineUsers: "Allociné",
  };

  function label(r: SiteRating): string {
    const who =
      r.source === "rtCritic" || r.source === "allocinePress"
        ? t("ratings.press")
        : r.source === "rtAudience" || r.source === "allocineUsers"
          ? t("ratings.audience")
          : null;
    return who ? `${names[r.source]} · ${who} : ${value(r)}` : `${names[r.source]} : ${value(r)}`;
  }
</script>

{#if shown.length}
  <span class="ratings">
    {#each shown as r (r.source)}
      <span class="r" title={label(r)} aria-label={label(r)}>
        {#if r.source === "imdb"}
          <span class="imdb">IMDb</span>
        {:else if r.source === "tmdb"}
          <span class="tmdb">TMDB</span>
        {:else if r.source === "rtCritic"}
          <svg viewBox="0 0 24 24" aria-hidden="true">
            {#if r.good}
              <circle cx="12" cy="13.5" r="8" fill="#fa320a" /><path d="M9 4.5c1.5.3 2.5 1.2 3 2.5.6-1.4 1.8-2.3 3.4-2.4-.8 1.3-2 2.3-3.4 2.6-1.4-.2-2.4-1.3-3-2.7z" fill="#00912d" />
            {:else}
              <path d="M12 3.5l2 4 4.4-1.6-1.6 4.4 4 2-4 2 1.6 4.4L14 17.1l-2 4-2-4-4.4 1.6 1.6-4.4-4-2 4-2-1.6-4.4L10 7.5z" fill="#0ac855" />
            {/if}
          </svg>
        {:else if r.source === "rtAudience"}
          <svg viewBox="0 0 24 24" aria-hidden="true" class:spilled={!r.good}>
            <path d="M6 9h12l-1.6 12H7.6z" fill={r.good ? "#fa320a" : "#0ac855"} /><path d="M9.5 9l.5 12M14.5 9l-.5 12" stroke="#fff" stroke-width="1.4" />
            <circle cx="8" cy="7.5" r="2.4" fill="#fff6dc" /><circle cx="12" cy="6" r="2.8" fill="#fff6dc" /><circle cx="16" cy="7.5" r="2.4" fill="#fff6dc" />
          </svg>
        {:else if r.source === "allocinePress" || r.source === "allocineUsers"}
          <span class="ac">Allo</span>
        {:else}
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 2.8l2.8 5.8 6.3.9-4.6 4.4 1.1 6.3L12 17.2l-5.6 3 1.1-6.3L2.9 9.5l6.3-.9z" fill="var(--accent)" /></svg>
        {/if}
        <span class="v">{value(r)}</span>
      </span>
    {/each}
  </span>
{/if}

<style>
  .ratings {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
  }

  .r {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  svg {
    width: 18px;
    height: 18px;
  }

  svg.spilled {
    transform: rotate(-35deg);
  }

  .imdb,
  .tmdb,
  .ac {
    padding: 1px 4px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 800;
    line-height: 1.3;
  }

  .imdb {
    background: #f5c518;
    color: #000;
  }

  .tmdb {
    background: linear-gradient(90deg, #90cea1, #01b4e4);
    color: #0d253f;
  }

  .ac {
    background: #fecc00;
    color: #1a1a1a;
  }
</style>
