<script lang="ts">
  import type { TvCard } from "../lib/tvcards";
  import TvRows from "./TvRows.svelte";

  /**
   * Accueil des téléviseurs : les rangées choisies dans Réglages → Interface (reprise, derniers
   * ajouts, bibliothèques épinglées, tendances Seerr), en rangées TV.
   */
  let { rows }: { rows: { title: string; cards: TvCard[] }[] } = $props();
</script>

<TvRows {rows} />
