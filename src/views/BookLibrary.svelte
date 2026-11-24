<script lang="ts">
  // Bibliothèque Kavita (livres, BD, mangas) : les séries en cours de lecture, puis toutes les séries par nom,
  // chargées par pages au fil du défilement.
  import { onMount } from "svelte";
  import { api, errorText, type BookSeries } from "../lib/api";
  import { go } from "../lib/nav.svelte";
  import { t } from "../lib/i18n";
  import ErrorCard from "../components/ErrorCard.svelte";
  import BookCard from "../components/BookCard.svelte";

  let { libraryId, title }: { libraryId: number; title: string } = $props();

  const PAGE = 60;
  let reading = $state<BookSeries[]>([]);
  let series = $state<BookSeries[]>([]);
  let total = $state(0);
  let page = $state(0);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let sentinel = $state<HTMLElement>();

  async function more() {
    if (loading || (page > 0 && series.length >= total)) return;
    loading = true;
    try {
      const next = await api.kavitaSeries(libraryId, page + 1, PAGE);
      page += 1;
      series = [...series, ...next.items];
      total = next.total;
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    api.kavitaOnDeck(libraryId).then((r) => (reading = r), () => {});
    more();
    // Le bas de la grille approche : la page suivante est demandée.
    const io = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && more(), { rootMargin: "600px" });
    if (sentinel) io.observe(sentinel);
    return () => io.disconnect();
  });

  const open = (s: BookSeries) => go({ name: "book", id: s.id });
</script>

<div class="page books">
  <h1>{title}</h1>
  {#if error}<ErrorCard message={error} />{/if}

  {#if reading.length}
    <h2>{t("books.continueReading")}</h2>
    <div class="grid">
      {#each reading as s (s.id)}<BookCard series={s} onclick={() => open(s)} />{/each}
    </div>
  {/if}

  <h2>{t("books.allSeries")}{#if total}{" "}<span class="count">{total}</span>{/if}</h2>
  <div class="grid">
    {#each series as s (s.id)}<BookCard series={s} onclick={() => open(s)} />{/each}
    {#if loading}{#each [0, 1, 2, 3, 4, 5] as i (i)}<div class="skeleton tile"></div>{/each}{/if}
  </div>
  {#if !loading && !error && page > 0 && !series.length}<p class="muted">{t("books.empty")}</p>{/if}
  <div class="sentinel" bind:this={sentinel}></div>
</div>

<style>
  .books h1 {
    margin: 8px 0 20px;
    font-size: 32px;
  }

  h2 {
    margin: 28px 0 14px;
    font-size: 20px;
  }

  .count {
    color: var(--text-faint);
    font-size: 15px;
    font-weight: 600;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 22px 18px;
  }

  :global(html.phone) .grid {
    grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
    gap: 16px 12px;
  }

  .tile {
    aspect-ratio: 2 / 3;
    border-radius: var(--radius);
  }

  .sentinel {
    height: 1px;
  }
</style>
