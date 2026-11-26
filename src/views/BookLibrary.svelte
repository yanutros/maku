<script lang="ts" module>
  import type { BookArt, BookSeries, BookSeriesDetail, BookSort } from "../lib/api";

  /** Ce qu'une bibliothèque avait chargé, gardé pour la session : au retour d'une série, la page revient telle
   *  quelle, à la même hauteur, et Maku y retrouve l'endroit quitté (puis la rafraîchit sans bouger). */
  interface Kept {
    reading: BookSeries[];
    featured: { detail: BookSeriesDetail; art: BookArt } | null;
    series: BookSeries[];
    total: number;
    page: number;
    sort: BookSort;
    keys: { letter: string; size: number }[];
  }
  const memory = new Map<number, Kept>();
</script>

<script lang="ts">
  // Bibliothèque Kavita (livres, BD, mangas) :
  // - « À la une » : la série en cours de lecture, en grand avec son fond et son logo, et « Continuer » ;
  // - les séries en cours, puis toutes les séries, chargées par pages au fil du défilement, par nom, par derniers
  //   ajouts, ou seulement celles en cours ;
  // - une recherche dans la bibliothèque (titre, auteur, genre), comme celle de Kavita ;
  // - sur chaque couverture, la progression et le menu « ⋮ » (lu, non lu, fiche) ; sur ordinateur, par nom, le
  //   bandeau des lettres mène à la première série d'une lettre, comme dans les autres bibliothèques.
  import { onMount, tick, untrack } from "svelte";
  import { api, errorText } from "../lib/api";
  import { books, seriesArt } from "../lib/books.svelte";
  import { back, go, nav, toast } from "../lib/nav.svelte";
  import { device } from "../lib/device";
  import { glideTo } from "../lib/spatial";
  import { t } from "../lib/i18n";
  import ErrorCard from "../components/ErrorCard.svelte";
  import BookCard from "../components/BookCard.svelte";
  import BookMenu, { type BookMenuEntry } from "../components/BookMenu.svelte";
  import Icon from "../components/Icon.svelte";
  import LetterBar, { letterOf } from "../components/LetterBar.svelte";
  import TitleLogo from "../components/TitleLogo.svelte";

  let { libraryId, title }: { libraryId: number; title: string } = $props();

  const PAGE = 60;
  // La page est recréée pour chaque bibliothèque : ce qu'elle avait gardé, lu une fois.
  const kept = untrack(() => memory.get(libraryId));
  let reading = $state<BookSeries[]>(kept?.reading ?? []);
  let series = $state<BookSeries[]>(kept?.series ?? []);
  let total = $state(kept?.total ?? 0);
  let page = $state(kept?.page ?? 0);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let sentinel = $state<HTMLElement>();
  let grid = $state<HTMLElement>();

  // --- Ordre de la liste, retenu pour chaque bibliothèque ---
  const SORT_KEY = "maku.bookSort";
  function savedSort(): BookSort {
    try {
      const s = JSON.parse(localStorage.getItem(SORT_KEY) ?? "{}")[`lib:${libraryId}`];
      return s === "added" || s === "reading" ? s : "name";
    } catch {
      return "name";
    }
  }
  let sort = $state<BookSort>(kept?.sort ?? savedSort());
  /** Chaque changement d'ordre recommence la liste ; une réponse d'un ordre précédent est ignorée. */
  let generation = 0;

  function setSort(next: BookSort) {
    if (next === sort) return;
    sort = next;
    try {
      const all = JSON.parse(localStorage.getItem(SORT_KEY) ?? "{}");
      all[`lib:${libraryId}`] = next;
      localStorage.setItem(SORT_KEY, JSON.stringify(all));
    } catch {
      // Stockage indisponible : l'ordre vaut pour la visite.
    }
    generation++;
    series = [];
    total = 0;
    page = 0;
    loading = false;
    more();
  }

  /** Pages suivantes de la liste, jusqu'à la page `upTo` (la suivante seulement, par défaut). */
  async function more(upTo = page + 1) {
    if (loading || query.trim() || (page > 0 && series.length >= total)) return;
    loading = true;
    const forGeneration = generation;
    try {
      const last = page > 0 ? Math.min(upTo, Math.ceil(total / PAGE)) : Math.max(upTo, 1);
      const numbers = Array.from({ length: Math.max(1, last - page) }, (_, i) => page + 1 + i);
      const pages = await Promise.all(numbers.map((n) => api.kavitaSeries(libraryId, n, PAGE, sort)));
      if (forGeneration !== generation) return;
      for (const next of pages) {
        series = [...series, ...next.items];
        total = next.total;
      }
      page = numbers[numbers.length - 1];
    } catch (e) {
      if (forGeneration === generation) error = errorText(e);
    } finally {
      if (forGeneration === generation) loading = false;
    }
  }

  const loadDeck = () => api.kavitaOnDeck(libraryId).then((r) => (reading = r), () => {});

  /** Retour sur la bibliothèque : les pages déjà là sont redemandées (progression à jour), sans changer leur place. */
  async function refreshPages() {
    const forGeneration = generation;
    try {
      const pages = await Promise.all(Array.from({ length: page }, (_, i) => api.kavitaSeries(libraryId, i + 1, PAGE, sort)));
      const fresh = pages.flatMap((p) => p.items);
      if (forGeneration === generation && fresh.length === series.length) series = fresh;
    } catch {
      // Hors ligne : la liste gardée reste affichée.
    }
  }

  onMount(() => {
    loadDeck();
    if (kept && page > 0) refreshPages();
    else more();
    if (!kept) loadLetters();
    // Le bas de la grille approche : la page suivante est demandée.
    const io = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && more(), { rootMargin: "600px" });
    if (sentinel) io.observe(sentinel);
    return () => {
      io.disconnect();
      clearTimeout(searchTimer);
      memory.set(libraryId, $state.snapshot({ reading, featured, series, total, page, sort, keys }) as Kept);
    };
  });

  const open = (s: BookSeries) => go({ name: "book", id: s.id });

  // --- « À la une » : la série en cours de lecture, la plus récente ---
  // Gardé : affiché tout de suite au retour, puis remplacé par sa version à jour (même place).
  let featured = $state<{ detail: BookSeriesDetail; art: BookArt } | null>(kept?.featured ?? null);
  let featuredFor = 0;
  $effect(() => {
    const s = reading[0];
    if (!s || s.id === featuredFor) return;
    featuredFor = s.id;
    api.kavitaSeriesDetail(s.id).then(
      async (detail) => {
        const art = await seriesArt(detail.series.name, detail.libraryKind, detail.anilistId);
        if (featuredFor === s.id) featured = { detail, art };
      },
      () => {},
    );
  });

  function continueFeatured() {
    const d = featured?.detail;
    const chapterId = d?.continueChapter ?? d?.volumes[0]?.chapters[0]?.id ?? d?.chapters[0]?.id;
    if (d && chapterId && d.series.format !== "unknown") books.reading = { chapterId, seriesId: d.series.id, format: d.series.format };
  }

  // Retour du lecteur : la progression a changé.
  let wasReading = false;
  $effect(() => {
    const now = books.reading !== null;
    if (wasReading && !now) loadDeck();
    wasReading = now;
  });

  // --- Recherche dans la bibliothèque ---
  let query = $state("");
  let results = $state<BookSeries[] | null>(null);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;

  function onSearch() {
    clearTimeout(searchTimer);
    const q = query.trim();
    if (!q) {
      results = null;
      return;
    }
    searchTimer = setTimeout(async () => {
      try {
        const found = await api.kavitaSearch(libraryId, q);
        if (query.trim() === q) results = found;
      } catch (e) {
        if (query.trim() === q) toast(errorText(e), true);
      }
    }, 300);
  }

  // --- Menu « ⋮ » d'une série ---
  let menu = $state<{ x: number; y: number; series: BookSeries } | null>(null);

  async function markRead(s: BookSeries, read: boolean) {
    await api.kavitaMarkRead({ seriesId: s.id, read });
    // Les listes montrent tout de suite la série lue (ou non lue) ; « Continuer la lecture » suit Kavita.
    const pagesRead = read ? s.pages : 0;
    for (const list of [series, reading, results ?? []]) for (const x of list) if (x.id === s.id) x.pagesRead = pagesRead;
    loadDeck();
    toast(read ? t("books.markedRead") : t("books.markedUnread"));
  }

  const entries = (s: BookSeries): BookMenuEntry[] => [
    ...(s.pagesRead < s.pages ? [{ icon: "checkCircle" as const, label: t("books.markRead"), run: () => markRead(s, true) }] : []),
    ...(s.pagesRead > 0 ? [{ icon: "restart" as const, label: t("books.markUnread"), run: () => markRead(s, false) }] : []),
    { icon: "chevronRight", label: t("cardMenu.openDetails"), run: () => open(s) },
  ];
  const onmenu = (s: BookSeries) => (e: MouseEvent) => (menu = { x: e.clientX, y: e.clientY, series: s });

  // --- Bandeau des lettres (ordinateur seulement, par nom, comme dans les autres bibliothèques) ---
  // Kavita dit combien de séries commencent par chaque initiale, dans l'ordre de la liste : d'où la place de chaque
  // lettre, même pas encore chargée.
  const lettersHere = !device.tv && !device.mobile && !device.phone;
  let keys = $state<{ letter: string; size: number }[]>(kept?.keys ?? []);
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
  const showLetters = $derived(offsets.size > 0 && sort === "name" && results === null);

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

  const sorts: [BookSort, string][] = [
    ["name", "books.sortName"],
    ["added", "books.sortAdded"],
    ["reading", "books.sortReading"],
  ];
</script>

<div class="page books">
  <!-- Comme les bibliothèques de films et de séries : le titre, puis à droite la recherche et le tri ; le retour à côté
       du titre (pas de barre du haut ici : sa recherche ne trouve que films, séries et artistes). -->
  <div class="head" class:lettered={showLetters}>
    {#if !device.phone}
      <button class="btn round back" disabled={!nav.history.length} onclick={back} aria-label={t("common.c1.back")} title={t("common.c1.back")}>
        <Icon name="back" />
      </button>
    {/if}
    <h1>{title}</h1>
    <div class="tools">
      <label class="search">
        <Icon name="search" size={16} />
        <input type="search" bind:value={query} oninput={onSearch} placeholder={t("books.searchHint")} aria-label={t("books.search", { library: title })} />
      </label>
      <div class="modes" role="radiogroup" aria-label={t("books.sort")}>
        {#each sorts as [id, label] (id)}
          <button role="radio" aria-checked={sort === id} class:on={sort === id} onclick={() => setSort(id)}>{t(label)}</button>
        {/each}
      </div>
    </div>
  </div>
  {#if featured && results === null}
    {@const d = featured.detail}
    <section class="hero" class:lettered={showLetters} aria-label={t("books.featured")} style:--tint={d.color ?? "transparent"}>
      <img class="hero-art" class:cover-art={!featured.art.backdrop} src={featured.art.backdrop ?? d.series.cover} alt="" />
      <div class="hero-body">
        <div class="eyebrow">{t("books.featured")}</div>
        <h2 class="hero-title" style:--logo-h="110px" style:--logo-w="min(420px, 100%)"><TitleLogo logo={featured.art.logo} title={d.series.name} genres={d.genres} /></h2>
        <div class="hero-facts">
          {#if d.writers.length}<span>{d.writers.slice(0, 2).join(", ")}</span>{/if}
          {#if d.series.pages > 0}<span>{t("books.readPercent", { percent: Math.round((Math.min(d.series.pagesRead, d.series.pages) / d.series.pages) * 100) })}</span>{/if}
        </div>
        <div class="hero-actions">
          <button class="btn primary" onclick={continueFeatured}><Icon name="play" size={16} filled /> {t("books.continue")}</button>
          <button class="btn" onclick={() => go({ name: "book", id: d.series.id })}>{t("books.openSeries")}</button>
        </div>
      </div>
      <img class="hero-cover" src={d.series.cover} alt="" />
    </section>
  {/if}

  {#if error}<ErrorCard message={error} />{/if}

  {#if results !== null}
    <h2>{t("books.results")}{" "}<span class="count">{results.length}</span></h2>
    <div class="grid">
      {#each results as s (s.id)}<BookCard series={s} onclick={() => open(s)} onmenu={onmenu(s)} />{/each}
    </div>
    {#if !results.length}<p class="muted">{t("books.noResults")}</p>{/if}
  {:else}
    {#if reading.length && sort === "name"}
      <h2>{t("books.continueReading")}</h2>
      <div class="grid" class:lettered={showLetters}>
        {#each reading as s (s.id)}<BookCard series={s} onclick={() => open(s)} onmenu={onmenu(s)} />{/each}
      </div>
    {/if}

    <h2>{sort === "reading" ? t("books.sortReading") : t("books.allSeries")}{#if total}{" "}<span class="count">{total}</span>{/if}</h2>
    <div class="grid" class:lettered={showLetters} bind:this={grid}>
      {#each series as s (s.id)}<BookCard series={s} onclick={() => open(s)} onmenu={onmenu(s)} />{/each}
      {#if loading}{#each [0, 1, 2, 3, 4, 5] as i (i)}<div class="skeleton tile"></div>{/each}{/if}
    </div>
    {#if !loading && !error && page > 0 && !series.length}<p class="muted">{t("books.empty")}</p>{/if}
  {/if}
  <div class="sentinel" bind:this={sentinel}></div>
  {#if showLetters}<LetterBar has={(l) => offsets.has(l)} {jump} />{/if}
</div>

{#if menu}
  <BookMenu x={menu.x} y={menu.y} title={menu.series.name} entries={entries(menu.series)} onclose={() => (menu = null)} />
{/if}

<style>
  /* « À la une » : un grand bandeau, le fond de la série à droite qui se fond vers la gauche, sous le titre. */
  .hero {
    position: relative;
    display: flex;
    align-items: flex-end;
    gap: 24px;
    min-height: 340px;
    margin: 4px 0 28px;
    padding: 32px;
    overflow: hidden;
    /* Pas de bordure : un liseré clair se voyait sur les bords sombres (à gauche et en bas). Le fond découpé à
       l'arrondi sur son propre calque, sans bavure aux bords. */
    border-radius: 22px;
    isolation: isolate;
    background: radial-gradient(120% 120% at 80% 0%, color-mix(in srgb, var(--tint) 40%, transparent), transparent 70%), var(--bg-elev);
    box-shadow: 0 24px 60px #0008;
    animation: rise 0.6s var(--ease) backwards;
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(14px);
    }
  }

  .hero-art {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 25%;
    mask-image: linear-gradient(90deg, transparent 0%, #000 55%);
  }

  /* Sans illustration en ligne : la couverture floutée. */
  .hero-art.cover-art {
    filter: blur(22px) saturate(1.3) brightness(0.7);
    transform: scale(1.15);
    opacity: 0.8;
  }

  .hero::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, #000000c0 0%, #00000060 45%, transparent 75%), linear-gradient(0deg, #000000a0 0%, transparent 50%);
    pointer-events: none;
  }

  .hero-body {
    position: relative;
    z-index: 1;
    flex: 1;
    min-width: 0;
  }

  .eyebrow {
    color: var(--accent);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  .hero-title {
    margin: 8px 0;
    font-size: clamp(26px, 3.4vw, 40px);
    line-height: 1.1;
    text-shadow: 0 2px 18px #000a;
  }

  .hero-facts {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    color: #fffc;
    font-size: 14px;
  }

  .hero-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 16px;
  }

  .hero-cover {
    position: relative;
    z-index: 1;
    flex: none;
    width: 150px;
    aspect-ratio: 2 / 3;
    border-radius: var(--radius);
    object-fit: cover;
    box-shadow: 0 18px 40px #000b;
  }

  :global(html.phone) .hero {
    min-height: 260px;
    padding: 20px;
  }

  :global(html.phone) .hero-cover {
    display: none;
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px 16px;
    margin-bottom: 22px;
  }

  .books h1 {
    margin: 0;
    font-size: 30px;
    font-weight: 700;
  }

  .back {
    flex: none;
    background: #ffffff0d;
  }

  .back:hover:not(:disabled) {
    background: #ffffff1a;
  }

  /* À droite du titre : la recherche, puis le tri. */
  .tools {
    display: flex;
    flex: 1 1 auto;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
  }

  /* Recherche et tri : la même pilule que les onglets des bibliothèques de films et de séries. */
  .search,
  .modes {
    height: 44px;
    border-radius: 22px;
    background: #ffffff0d;
  }

  .search {
    display: flex;
    flex: 0 1 320px;
    align-items: center;
    gap: 8px;
    min-width: 180px;
    padding: 0 16px;
    border: 1px solid transparent;
    color: var(--text-dim);
    transition: border-color 0.15s;
  }

  .search:focus-within {
    border-color: var(--accent);
  }

  .search input {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: none;
    background: none;
    color: var(--text);
    outline: none;
  }

  :global(html.phone) .search {
    flex: 1 1 100%;
  }

  h2 {
    margin: 28px 0 14px;
    font-size: 20px;
  }

  .modes {
    display: flex;
    flex: none;
    align-items: center;
    gap: 4px;
    padding: 4px;
  }

  .modes button {
    height: 36px;
    padding: 0 16px;
    border: none;
    border-radius: 18px;
    background: none;
    color: var(--text-dim);
    font-weight: 600;
    white-space: nowrap;
    transition:
      background 0.15s,
      color 0.15s;
  }

  .modes button:hover {
    color: var(--text);
  }

  .modes button.on {
    background: var(--accent);
    color: var(--accent-text);
  }

  :global(html.phone) .modes button {
    padding: 0 10px;
    font-size: 12px;
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

  /* Le bandeau des lettres a sa place, à droite. */
  .hero.lettered,
  .head.lettered,
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
