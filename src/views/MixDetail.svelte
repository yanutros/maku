<script lang="ts">
  import type { ItemDetail } from "../lib/api";
  import { mixes } from "../lib/mixes.svelte";
  import MusicDetail from "./MusicDetail.svelte";
  import { t } from "../lib/i18n";

  /**
   * Fiche d'un « Mix of Three », comme celle d'une playlist : les titres de ses trois playlists, à lire dans
   * l'ordre, en aléatoire ou à partir d'un titre. Un mix oublié ou passé (deux jours) n'a plus de fiche.
   */
  let { id }: { id: string } = $props();

  const mix = $derived(mixes.list.find((m) => m.id === id) ?? null);

  // La fiche d'une playlist, sans ce qui n'existe pas pour un mix (note, modification, téléchargement).
  const item = $derived<ItemDetail | null>(
    mix && {
      ratingKey: `mix:${mix.id}`,
      kind: "playlist",
      title: t("mix.title"),
      subtitle: null,
      year: null,
      summary: null,
      poster: mix.playlists[0]?.poster ?? null,
      logo: null,
      logoGuid: null,
      thumb: null,
      backdrop: null,
      index: null,
      parentIndex: null,
      showTitle: null,
      showKey: null,
      seasonKey: null,
      albumTitle: null,
      albumKey: null,
      durationMs: null,
      viewOffsetMs: null,
      watched: false,
      unwatchedCount: null,
      contentRating: null,
      rating: null,
      genres: [],
      resolution: null,
      playable: true,
      tagline: null,
      originalTitle: null,
      backdropHd: null,
      studio: null,
      releaseDate: null,
      directors: [],
      writers: [],
      cast: [],
      video: null,
      audioTracks: [],
      subtitleTracks: [],
      versions: [],
      siteRatings: [],
      imdbId: null,
    },
  );
</script>

{#if mix && item}
  {#key mix.id}<MusicDetail {item} {mix} />{/key}
{:else}
  <div class="page"><p class="muted">{t("mix.gone")}</p></div>
{/if}
