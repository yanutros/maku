<script lang="ts">
  // Bibliothèque Kavita (livres, BD, mangas) : les séries en cours de lecture, puis toutes les séries par nom,
  // chargées par pages au fil du défilement. Chaque série a son menu « ⋮ » (lu, non lu, fiche) ; sur ordinateur, le
  // bandeau des lettres mène à la première série d'une lettre, comme dans les autres bibliothèques.
  import { onMount, tick } from "svelte";
  import { api, errorText, type BookSeries } from "../lib/api";
  import { go, toast } from "../lib/nav.svelte";
  import { device } from "../lib/device";
  import { glideTo } from "../lib/spatial";
  import { t } from "../lib/i18n";
  import ErrorCard from "../components/ErrorCard.svelte";
  import BookCard from "../components/BookCard.svelte";
  import BookMenu, { type BookMenuEntry } from "../components/BookMenu.svelte";
  import LetterBar, { letterOf } from "../components/LetterBar.svelte";

  let { libraryId, title }: { libraryId: number; title: string } = $props();

  const PAGE = 60;
  let reading = $state<BookSeries[]>([]);
  let series = $state<BookSeries[]>([]);
  let total = $state(0);
  let page = $state(0);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let sentinel = $state<HTMLElement>();
  let grid = $state<HTMLElement>();

  /** Pages suivantes de la liste, jusqu'à la page `upTo` (la suivante seulement, par défaut). */
  async function more(upTo = page + 1) {
    if (loading || (page > 0 && series.length >= total)) return;
    loading = true;
    try {
      const last = page > 0 ? Math.min(upTo, Math.ceil(total / PAGE)) : Math.max(upTo, 1);
      const numbers = Array.from({ length: Math.max(1, last - page) }, (_, i) => page + 1 + i);
      const pages = await Promise.all(numbers.map((n) => api.kavitaSeries(libraryId, n, PAGE)));
      for (const next of pages) {
        series = [...series, ...next.items];
        total = next.total;
      }
      page = numbers[numbers.length - 1];
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }

  const loadDeck = () => api.kavitaOnDeck(libraryId).then((r) => (reading = r), () => {});

  onMount(() => {
    loadDeck();
    more();
    loadLetters();
    // Le bas de la grille approche : la page suivante est demandée.
    const io = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && more(), { rootMargin: "600px" });
    if (sentinel) io.observe(sentinel);
    return () => io.disconnect();
  });

  const open = (s: BookSeries) => go({ name: "book", id: s.id });

  // --- Menu « ⋮ » d'une série ---
  let menu = $state<{ x: number; y: number; series: BookSeries } | null>(null);

  async function markRead(s: BookSeries, read: boolean) {
    await api.kavitaMarkRead({ seriesId: s.id, read });
    // Les deux listes montrent tout de suite la série lue (ou non lue) ; « Continuer la lecture » suit Kavita.
    const pagesRead = read ? s.pages : 0;
    for (const list of [series, reading]) for (const x of list) if (x.id === s.id) x.pagesRead = pagesRead;
    loadDeck();
    toast(read ? t("books.markedRead") : t("books.markedUnread"));
  }

  const entries = (s: BookSeries): BookMenuEntry[] => [
    ...(s.pagesRead < s.pages ? [{ icon: "checkCircle" as const, label: t("books.markRead"), run: () => markRead(s, true) }] : []),
    ...(s.pagesRead > 0 ? [{ icon: "restart" as const, label: t("books.markUnread"), run: () => markRead(s, false) }] : []),
    { icon: "chevronRight", label: t("cardMenu.openDetails"), run: () => open(s) },
  ];

  // --- Bandeau des lettres (ordinateur seulement, comme dans les autres bibliothèques) ---
  // Kavita dit combien de séries commencent par chaque initiale, dans l'ordre de la liste : d'où la place de chaque
  // lettre, même pas encore chargée.
  const lettersHere = !device.tv && !device.mobile && !device.phone;
  let keys = $state<{ letter: string; size: number }[]>([]);
  const offsets = $derived.by(() => {
    const map = new Map<string, number>();
    let at = 0;
    for (const { letter, size } of keys) {
      const l = letterOf(letter);
      if (!map.has(l)) map.set(l, at);
      at += size;
    }
    return map;
  });

  function loadLetters() {
    if (lettersHere) api.kavitaJumpBar(libraryId).then((k) => (keys = k), () => {});
  }

  /** Amène la première série de la lettre en haut de la page, en chargeant d'un coup la liste jusque-là. */
  async function jump(letter: string) {
    const offset = offsets.get(letter);
    if (offset === undefined) return;
    for (let guard = 0; loading && guard < 200; guard++) await new Promise((r) => setTimeout(r, 50));
    if (series.length <= offset) await more(Math.floor(offset / PAGE) + 1);
    await tick();
    // L'ordre de Kavita (nom de tri) peut différer un peu de l'initiale du nom : la première série chargée de cette
    // lettre, sinon la place annoncée.
    const target = series.find((s) => letterOf(s.name) === letter) ?? series[offset];
    const card = target ? grid?.querySelector<HTMLElement>(`[data-series="${target.id}"]`) : null;
    const scroller = document.querySelector<HTMLElement>(".content");
    if (!card || !scroller) return;
    glideTo(scroller, { top: scroller.scrollTop + card.getBoundingClientRect().top - scroller.getBoundingClientRect().top - 84 });
  }
</script>

<div class="page books">
  <h1>{title}</h1>
  {#if error}<ErrorCard message={error} />{/if}

  {#if reading.length}
    <h2>{t("books.continueReading")}</h2>
    <div class="grid" class:lettered={offsets.size > 0}>
      {#each reading as s (s.id)}<BookCard series={s} onclick={() => open(s)} onmenu={(e) => (menu = { x: e.clientX, y: e.clientY, series: s })} />{/each}
    </div>
  {/if}

  <h2>{t("books.allSeries")}{#if total}{" "}<span class="count">{total}</span>{/if}</h2>
  <div class="grid" class:lettered={offsets.size > 0} bind:this={grid}>
    {#each series as s (s.id)}<BookCard series={s} onclick={() => open(s)} onmenu={(e) => (menu = { x: e.clientX, y: e.clientY, series: s })} />{/each}
    {#if loading}{#each [0, 1, 2, 3, 4, 5] as i (i)}<div class="skeleton tile"></div>{/each}{/if}
  </div>
  {#if !loading && !error && page > 0 && !series.length}<p class="muted">{t("books.empty")}</p>{/if}
  <div class="sentinel" bind:this={sentinel}></div>
  {#if offsets.size > 0}<LetterBar has={(l) => offsets.has(l)} {jump} />{/if}
</div>

{#if menu}
  <BookMenu x={menu.x} y={menu.y} title={menu.series.name} entries={entries(menu.series)} onclose={() => (menu = null)} />
{/if}

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

  /* La grille laisse la place du bandeau des lettres. */
  .grid.lettered {
    margin-right: 28px;
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
