<script lang="ts">
  import { api, type ItemDetail, type SiteRating } from "../lib/api";
  import { ratingPrefs } from "../lib/ratings.svelte";
  import RatingLogo from "./RatingLogo.svelte";
  import { locale, t } from "../lib/i18n";

  /**
   * Notes des sites d'évaluation d'un film ou d'une série, comme dans Plex : IMDb, Rotten Tomatoes (tomate de la
   * presse, pop-corn du public), TMDB, et Allociné (presse et spectateurs) cherché à part. Chaque note se masque
   * dans Réglages → Interface.
   */
  let { item }: { item: ItemDetail } = $props();

  let allocine = $state<{ key: string; list: SiteRating[] }>({ key: "", list: [] });
  const wantsAllocine = $derived((ratingPrefs.allocinePress || ratingPrefs.allocineUsers) && !!item.imdbId && (item.kind === "movie" || item.kind === "show"));

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
    [...item.siteRatings, ...(wantsAllocine && allocine.key === item.ratingKey ? allocine.list : [])].filter((r) => ratingPrefs[r.source]),
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
        <RatingLogo source={r.source} good={r.good} />
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
</style>
