<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, errorText, type Genre, type Hub, type Item } from "../lib/api";
  import { genreHue, genreLabel } from "../lib/genres";
  import PosterCard from "../components/PosterCard.svelte";
  import Row from "../components/Row.svelte";
  import HeroCarousel from "../components/HeroCarousel.svelte";
  import GenreTile from "../components/GenreTile.svelte";
  import ContinueRow from "../components/ContinueRow.svelte";
  import FeaturePanels, { type Panel } from "../components/FeaturePanels.svelte";
  import { go, nav, toast } from "../lib/nav.svelte";
  import { play } from "../lib/player.svelte";
  import Icon from "../components/Icon.svelte";
  import TvRows from "../components/TvRows.svelte";
  import TvGrid from "../components/TvGrid.svelte";
  import { keepTabFocus } from "../lib/spatial";
  import { device } from "../lib/device";
  import { fromItem, type TvCard } from "../lib/tvcards";
  import { t } from "../lib/i18n";

  let {
    sectionKey,
    title,
    kind = "",
    initialTab = "recommended",
    initialGenre = null,
    initialFocus = null,
    initialScroll = 0,
    initialLoaded = 0,
  }: {
    sectionKey: string;
    title: string;
    kind?: string;
    initialTab?: "recommended" | "library" | "collections" | "categories";
    initialGenre?: Genre | null;
    initialFocus?: string | null;
    initialScroll?: number;
    initialLoaded?: number;
  } = $props();

  // La page telle qu'elle est dans l'historique : l'onglet, la catégorie, la jaquette sélectionnée,
  // le défilement et le nombre de titres chargés y sont notés au fil de la navigation, pour revenir
  // exactement au même endroit depuis une fiche (voir `restore`).
  // svelte-ignore state_referenced_locally
  const memo = nav.view.name === "section" && nav.view.key === sectionKey ? nav.view : null;

  type Tab = "recommended" | "library" | "collections" | "categories";
  // Collections : seulement dans les bibliothèques de films, comme dans Plex.
  const TABS = $derived<{ id: Tab; label: string }[]>([
    { id: "recommended", label: t("library.tabs.recommended") },
    { id: "library", label: t("library.tabs.library") },
    ...(kind === "movie" ? [{ id: "collections" as Tab, label: t("library.tabs.collections") }] : []),
    { id: "categories", label: t("library.tabs.categories") },
  ]);
  const PAGE_SIZE = 120;

  let tab = $state<Tab>("recommended");
  let error = $state<string | null>(null);

  // --- Recommandé ---
  let hubs = $state<Hub[] | null>(null);
  // Bandeau : ce qu'on est en train de regarder d'abord, puis le reste des rangées, avec une image de fond.
  const slides = $derived.by(() => {
    const all = (hubs ?? []).flatMap((h) => h.items).filter((i) => i.backdrop);
    const seen = new Set<string>();
    const unique = [...all.filter((i) => i.viewOffsetMs), ...all].filter((i) => {
      const key = i.showKey ?? i.ratingKey;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
    return unique.slice(0, 5);
  });

  // « Continuer à regarder » en grand, puis des panneaux « À la une » tirés des autres rangées.
  const isContinue = (h: Hub) => /\.inprogress/.test(h.identifier) && h.items.every((it) => it.playable);
  const continueHubs = $derived(hubs?.filter(isContinue) ?? []);
  const restHubs = $derived(hubs?.filter((h) => !isContinue(h)) ?? []);
  const panels = $derived.by<Panel[]>(() => {
    const taken = new Set(slides.map((s) => s.showKey ?? s.ratingKey));
    const out: Panel[] = [];
    // Un titre par rangée, pour varier, puis on complète avec les suivants.
    for (let round = 0; round < 3 && out.length < 6; round++) {
      for (const hub of restHubs) {
        const it = hub.items.filter((i) => i.backdrop && !taken.has(i.showKey ?? i.ratingKey))[0];
        if (!it || out.length >= 6) continue;
        taken.add(it.showKey ?? it.ratingKey);
        out.push({
          key: it.ratingKey,
          eyebrow: hub.title,
          title: it.showTitle ?? it.title,
          subtitle: [it.year, it.genres.slice(0, 2).join(" · ")].filter(Boolean).join(" · ") || null,
          image: it.backdrop,
          cover: it.poster,
          caption: it.summary,
          onclick: () => go({ name: "item", ratingKey: it.ratingKey }),
          onplay: it.playable ? () => play(it.ratingKey).catch((e) => toast(errorText(e), true)) : undefined,
        });
      }
    }
    return out;
  });

  // --- Bibliothèque ---
  let items = $state<Item[]>([]);
  let total = $state(0);
  let loading = $state(false);
  let genre = $state<Genre | null>(null);
  let sentinel = $state<HTMLElement>();

  // --- Collections ---
  let collections = $state<Item[] | null>(null);

  // --- Catégories ---
  let genres = $state<Genre[] | null>(null);

  // --- Téléviseur : mêmes données, en rangées et grilles TV ---
  const tvRows = $derived([
    ...continueHubs.map((h) => ({ title: h.title, cards: h.items.map((i) => fromItem(i, t("library.resume"), true)) })),
    ...restHubs.map((h) => ({ title: h.title, cards: h.items.map((i) => fromItem(i, h.title)) })),
  ].filter((r) => r.cards.length));
  const tvItems = $derived(items.map((i) => fromItem(i, title)));
  const tvCollections = $derived((collections ?? []).map((i) => fromItem(i, t("library.tabs.collections"))));
  const tvGenres = $derived<TvCard[]>(
    (genres ?? []).map((g) => ({
      key: g.id,
      image: g.backdrop ?? null,
      shape: "tile",
      hue: genreHue(g.title),
      posters: g.posters,
      label: genreLabel(g.title),
      progress: 0,
      eyebrow: t("library.category"),
      title: genreLabel(g.title),
      logo: null,
      logoGuid: null,
      genres: [],
      line: t("library.titleCount", { count: g.count }),
      summary: null,
      backdrop: g.backdrop,
      open: () => openGenre(g),
    })),
  );

  async function loadMore() {
    if (tab !== "library" || loading || (total && items.length >= total)) return;
    loading = true;
    const forGenre = genre?.id ?? null;
    try {
      const page = await api.sectionItems(sectionKey, items.length, PAGE_SIZE, null, forGenre);
      if ((genre?.id ?? null) !== forGenre) return;
      items.push(...page.items);
      total = page.total;
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }

  function resetLibrary() {
    items = [];
    total = 0;
    loading = false;
  }

  // Téléviseur : l'onglet s'ouvre dès qu'on y arrive avec les flèches, sans appuyer sur OK. Un court
  // délai évite de charger chaque onglet traversé.
  let tabTimer: ReturnType<typeof setTimeout> | undefined;
  function focusTab(next: Tab) {
    clearTimeout(tabTimer);
    if (next !== tab) tabTimer = setTimeout(() => select(next), 250);
  }

  function select(next: Tab) {
    tab = next;
    keepTabFocus();
    error = null;
    // Un autre onglet repart du haut (sans quoi, sur téléviseur, sa première rangée resterait coupée).
    if (!initialFocus) document.querySelector(".content")?.scrollTo({ top: 0 });
    if (memo) {
      memo.tab = next;
      if (next !== "library") memo.genre = null;
    }
    if (next === "library" && !items.length) loadMore();
    if (next === "collections" && !collections) {
      api.collections(sectionKey).then(
        (c) => (collections = c),
        (e) => (error = errorText(e)),
      );
    }
    if (next === "categories" && !genres) {
      api.genres(sectionKey).then(
        (g) => (genres = g),
        (e) => (error = errorText(e)),
      );
    }
  }

  // Une catégorie ouvre la bibliothèque filtrée sur ce genre, comme une nouvelle page : Retour
  // revient à la liste des catégories, sur la catégorie choisie.
  function openGenre(g: Genre) {
    go({ name: "section", key: sectionKey, title, kind, tab: "library", genre: g });
  }

  /** Affiche sur place la bibliothèque, filtrée sur un genre ou non. */
  function showGenre(g: Genre | null) {
    genre = g;
    resetLibrary();
    select("library");
    if (memo) memo.genre = g;
    loadMore();
  }

  /** Au retour d'une fiche : recharge autant de titres qu'avant, puis remet le défilement et la sélection. */
  async function restore() {
    const scroller = document.querySelector<HTMLElement>(".content");
    const pause = (ms: number) => new Promise((r) => setTimeout(r, ms));
    if (tab === "library") {
      for (let guard = 0; items.length < initialLoaded && (!total || items.length < total) && guard < 20; guard++) {
        if (loading) await pause(50);
        else await loadMore();
      }
    } else if (tab === "collections") {
      for (let guard = 0; !collections && guard < 100; guard++) await pause(50);
    } else if (tab === "categories") {
      for (let guard = 0; !genres && guard < 100; guard++) await pause(50);
    }
    await tick();
    if (scroller && initialScroll) scroller.scrollTop = initialScroll;
    // Les rangées de « Recommandé » retrouvent leur jaquette d'elles-mêmes (restoreSpot, place dans la
    // rangée) : les faire défiler ici les décalerait hors de l'écran.
    if (!initialFocus || !device.tv || tab === "recommended") return;
    // La jaquette est cherchée par comparaison directe des clés (un sélecteur CSS échappé sur une clé
    // numérique s'est avéré peu fiable), et le focus est redonné tant que la page qui s'installe le
    // reprend (grille qui se dessine, sélection par défaut).
    for (let attempt = 0; attempt < 15; attempt++) {
      const card = [...document.querySelectorAll<HTMLElement>("[data-key]")].find((el) => el.dataset.key === initialFocus);
      if (card) {
        card.focus({ preventScroll: true });
        if (document.activeElement === card) {
          card.scrollIntoView({ block: "center" });
          return;
        }
      }
      await pause(100);
    }
  }

  onMount(() => {
    if (initialGenre) showGenre(initialGenre);
    else if (initialTab !== "recommended") select(initialTab);
    if (initialLoaded || initialScroll || initialFocus) restore();
    // Ce qu'on fait sur la page est noté dans l'historique au fur et à mesure.
    const scroller = document.querySelector<HTMLElement>(".content");
    const onScroll = () => {
      if (memo && scroller) memo.scroll = scroller.scrollTop;
    };
    const onPick = (e: Event) => {
      const key = (e.target as HTMLElement).closest?.("[data-key]")?.getAttribute("data-key");
      if (key && memo) {
        memo.focusKey = key;
        memo.loaded = items.length;
      }
    };
    scroller?.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("focusin", onPick);
    window.addEventListener("click", onPick, true);
    const forget = () => {
      scroller?.removeEventListener("scroll", onScroll);
      window.removeEventListener("focusin", onPick);
      window.removeEventListener("click", onPick, true);
    };
    api.sectionHubs(sectionKey).then(
      (h) => (hubs = h),
      (e) => (error = errorText(e)),
    );
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) loadMore();
      },
      { rootMargin: "600px" },
    );
    if (sentinel) observer.observe(sentinel);
    return () => {
      observer.disconnect();
      forget();
    };
  });
</script>

{#snippet tvHead()}
  <div class="tv-head">
    <h1>{title}</h1>
    <div class="tabs" role="tablist" data-nav-low>
      {#each TABS as tb (tb.id)}
        <button role="tab" aria-selected={tab === tb.id} class:active={tab === tb.id} onclick={() => select(tb.id)} onfocus={() => focusTab(tb.id)}>{tb.label}</button>
      {/each}
    </div>
    {#if tab === "library" && genre}
      <button class="filter tv-filter" onclick={() => showGenre(null)} data-nav-low>
        {genreLabel(genre.title)} <Icon name="x" size={14} />
      </button>
    {/if}
    {#if tab === "library" && total}<span class="muted">{t("library.titleCount", { count: total })}</span>{/if}
  </div>
{/snippet}

{#if device.tv}
  {#if tab === "recommended"}
    {#if hubs}
      <TvRows rows={tvRows} header={tvHead} />
    {:else}
      <div class="page tv-page">{@render tvHead()}</div>
    {/if}
  {:else}
    <div class="page tv-page">
      {@render tvHead()}
      {#if error}<p class="error">{error}</p>{/if}
      {#if tab === "library"}
        <TvGrid cards={tvItems} onEnd={loadMore} initialKey={initialFocus} />
      {:else if tab === "collections"}
        <TvGrid cards={tvCollections} initialKey={initialFocus} />
      {:else}
        <TvGrid cards={tvGenres} initialKey={initialFocus} />
      {/if}
    </div>
  {/if}
{:else}
<div class="page">
  <div class="head">
    <h1 class="row-title page-title">
      {title}
      {#if tab === "library" && total}<span class="muted">{total}</span>{/if}
    </h1>
    <div class="tabs" role="tablist" data-nav-low>
      {#each TABS as tb (tb.id)}
        <button role="tab" aria-selected={tab === tb.id} class:active={tab === tb.id} onclick={() => select(tb.id)}>
          {tb.label}
        </button>
      {/each}
    </div>
  </div>

  {#if error}<p class="error">{error}</p>{/if}

  {#if tab === "recommended"}
    {#if !hubs}
      <div class="skeleton" style:height="420px"></div>
    {:else}
      <HeroCarousel items={slides} eyebrow={(i) => (i.viewOffsetMs ? t("library.upNext") : t("library.discover"))} />
      {#each continueHubs as hub, i (i)}
        <ContinueRow title={hub.title} items={hub.items} />
      {/each}
      <!-- Téléphone : pas de panneaux « À la une », trop grands pour l'écran. -->
      {#if panels.length && !device.phone}
        <h2 class="feature-title">{t("library.featured")}</h2>
        <FeaturePanels {panels} />
      {/if}
      {#each restHubs as hub, i (i)}
        <!-- Vignettes larges seulement pour une rangée d'épisodes ; une rangée mêlant saisons et épisodes reste en affiches. -->
        {@const wide = hub.items.every((it) => it.kind === "episode")}
        <Row title={hub.title} {wide}>
          {#each hub.items as item (item.ratingKey)}<PosterCard {item} {wide} />{/each}
        </Row>
      {/each}
    {/if}
  {:else if tab === "library"}
    {#if genre}
      <div class="filter">
        <span>{t("library.genreFilter")} <strong>{genreLabel(genre.title)}</strong></span>
        <button class="chip-x" onclick={() => showGenre(null)} aria-label={t("library.removeFilter")}><Icon name="x" size={14} /></button>
      </div>
    {/if}
    <div class="grid">
      {#each items as item (item.ratingKey)}<PosterCard {item} />{/each}
    </div>
  {:else if tab === "collections"}
    {#if !collections}
      <div class="grid">{#each Array(12) as _, i (i)}<div class="skeleton poster-skeleton"></div>{/each}</div>
    {:else if !collections.length}
      <p class="muted">{t("library.noCollections")}</p>
    {:else}
      <div class="grid">
        {#each collections as item (item.ratingKey)}<PosterCard {item} />{/each}
      </div>
    {/if}
  {:else if !genres}
    <div class="categories">
      {#each Array(6) as _, i (i)}<div class="skeleton tile-skeleton"></div>{/each}
    </div>
  {:else}
    <div class="categories">
      {#each genres as g (g.id)}<GenreTile genre={g} onclick={() => openGenre(g)} />{/each}
    </div>
  {/if}

  <div bind:this={sentinel} class="sentinel">{#if loading}<span class="muted">{t("library.loading")}</span>{/if}</div>
</div>
{/if}

<style>
  .tv-head {
    display: flex;
    align-items: center;
    gap: 24px;
    margin-bottom: 20px;
  }

  .tv-head h1 {
    margin: 0;
    font-size: 26px;
    font-weight: 800;
  }

  .tv-filter {
    margin: 0;
    color: var(--text);
    font: inherit;
  }

  .poster-skeleton {
    aspect-ratio: 2 / 3;
    border-radius: var(--radius);
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px 24px;
    margin-bottom: 20px;
  }

  .head .page-title {
    margin-bottom: 0;
  }

  .tabs {
    display: flex;
    gap: 4px;
    padding: 4px;
    border-radius: 22px;
    background: #ffffff0d;
  }

  .tabs button {
    padding: 7px 16px;
    border: none;
    border-radius: 18px;
    background: none;
    color: var(--text-dim);
    font-weight: 600;
    transition: background 0.15s, color 0.15s;
  }

  .tabs button:hover {
    color: var(--text);
  }

  .tabs button.active {
    background: var(--accent);
    color: var(--accent-text);
  }

  .feature-title {
    margin: 32px 0 14px;
    font-size: 22px;
    font-weight: 700;
  }

  .filter {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 20px;
    padding: 6px 8px 6px 14px;
    border: 1px solid var(--border);
    border-radius: 20px;
    background: var(--bg-elev);
  }

  .chip-x {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: #ffffff14;
  }

  .chip-x:hover {
    background: var(--accent);
    color: var(--accent-text);
  }

  .categories {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: 24px;
  }

  .tile-skeleton {
    aspect-ratio: 16 / 9;
    border-radius: 18px;
  }

  .sentinel {
    height: 40px;
    margin-top: 16px;
  }
</style>
