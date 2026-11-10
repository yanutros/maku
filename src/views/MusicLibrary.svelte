<script lang="ts">
  import { byLastPlay } from "../lib/playlistPlays";
  import { onMount } from "svelte";
  import { api, errorText, type Hub, type Item } from "../lib/api";
  import PosterCard from "../components/PosterCard.svelte";
  import Row from "../components/Row.svelte";
  import FeaturePanels, { type Panel } from "../components/FeaturePanels.svelte";
  import TrackGrid from "../components/TrackGrid.svelte";
  import { go, toast } from "../lib/nav.svelte";
  import { playMusic } from "../lib/player.svelte";
  import TvRows from "../components/TvRows.svelte";
  import TvGrid from "../components/TvGrid.svelte";
  import { keepTabFocus } from "../lib/spatial";
  import { device } from "../lib/device";
  import { fromItem, type TvCard } from "../lib/tvcards";
  import { createMix, liveMixes, playAllPlaylists, playMix, type Mix } from "../lib/mixes.svelte";
  import MixCard from "../components/MixCard.svelte";
  import Icon from "../components/Icon.svelte";
  import { t } from "../lib/i18n";

  let { sectionKey, title }: { sectionKey: string; title: string } = $props();

  type Tab = "recommended" | "artists" | "albums" | "playlists";
  const TABS: { id: Tab; label: string }[] = [
    { id: "recommended", label: t("music.library.tabs.recommended") },
    { id: "artists", label: t("music.library.tabs.artists") },
    { id: "albums", label: t("music.library.tabs.albums") },
    { id: "playlists", label: t("music.library.tabs.playlists") },
  ];
  // Types Plex : 8 = artiste, 9 = album.
  const KIND: Record<"artists" | "albums", number> = { artists: 8, albums: 9 };
  const PAGE_SIZE = 120;

  let tab = $state<Tab>("recommended");
  let hubs = $state<Hub[] | null>(null);
  let items = $state<Item[]>([]);
  let total = $state(0);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let sentinel = $state<HTMLElement>();

  // Onglet Recommandé, façon Apple Music : grands panneaux des derniers albums, playlists
  // tout de suite après, « Titres du moment », nouveautés en grandes pochettes, puis les rangées Plex.
  let newAlbums = $state<Item[]>([]);
  let hotTracks = $state<Item[]>([]);
  const isPlaylistHub = (h: Hub) => h.items.every((i) => i.kind === "playlist");
  // Playlists : la dernière lue d'abord, comme Plexamp.
  const playlistHub = $derived.by(() => {
    const h = hubs?.find(isPlaylistHub);
    return h ? { ...h, items: byLastPlay(h.items) } : null;
  });
  // « Récemment ajouté » ferait doublon avec les nouveautés.
  const otherHubs = $derived(
    hubs?.filter((h) => !isPlaylistHub(h) && !(newAlbums.length && /recent\.added/.test(h.identifier))) ?? [],
  );
  const panels = $derived<Panel[]>(
    newAlbums.slice(0, 6).map((a, i) => ({
      key: a.ratingKey,
      eyebrow: i === 0 ? t("music.library.latestAddition") : t("music.library.newAlbum"),
      title: a.title,
      subtitle: a.subtitle,
      image: a.backdrop,
      cover: a.poster,
      square: true,
      caption: [a.year, a.genres.slice(0, 2).join(" · ")].filter(Boolean).join(" · ") || null,
      onclick: () => go({ name: "item", ratingKey: a.ratingKey }),
      onplay: () => playMusic(a.ratingKey).catch((e) => toast(errorText(e), true)),
    })),
  );

  /*
   * Onglet Playlists : « Écouter tout » mélange toutes les playlists, comme dans Plex ; « Mix of Three » en
   * fait choisir trois, mélangées le temps d'une écoute (le mix reste ensuite deux jours dans la rangée des
   * playlists de l'onglet Recommandé). La troisième playlist choisie lance le mix.
   */
  let picking = $state(false);
  let picked = $state<Item[]>([]);
  const fail = (e: unknown) => toast(errorText(e), true);

  function playAll() {
    if (items.length) playAllPlaylists(items).catch(fail);
  }

  function startPicking() {
    picked = [];
    picking = true;
  }

  function pick(item: Item) {
    picked = picked.some((p) => p.ratingKey === item.ratingKey) ? picked.filter((p) => p.ratingKey !== item.ratingKey) : [...picked, item];
    if (picked.length < 3) return;
    const mix = createMix(picked);
    picking = false;
    picked = [];
    playMix(mix).catch(fail);
  }

  const mixCard = (mix: Mix): TvCard => ({
    key: `mix-${mix.id}`,
    image: mix.playlists[0]?.poster ?? null,
    mosaic: mix.playlists.map((p) => p.poster),
    badge: t("mix.tag"),
    shape: "square",
    label: t("mix.title"),
    progress: 0,
    eyebrow: t("mix.tag"),
    title: t("mix.title"),
    logo: null,
    logoGuid: null,
    genres: [],
    line: mix.playlists.map((p) => p.title).join(" · "),
    summary: null,
    backdrop: null,
    open: () => go({ name: "mix", id: mix.id }),
  });

  // Téléviseur : mêmes données, en rangées et grilles TV.
  const tvPlaylistRow = $derived([...liveMixes().map(mixCard), ...(playlistHub?.items ?? []).map((i) => fromItem(i, t("music.library.playlist")))]);
  const tvRows = $derived(
    [
      { title: playlistHub?.title ?? t("music.library.tabs.playlists"), cards: tvPlaylistRow },
      { title: t("music.library.hotTracks"), cards: hotTracks.map((i) => fromItem(i, t("music.library.hotTrack"))) },
      { title: t("music.library.newReleases"), cards: newAlbums.map((i) => fromItem(i, t("music.library.newAlbum"))) },
      ...otherHubs.map((h) => ({ title: h.title, cards: h.items.map((i) => fromItem(i, h.title)) })),
    ].filter((r) => r.cards.length),
  );
  // Onglet Playlists sur TV : « Écouter tout » et « Mix of Three » en tête ; pendant le choix d'un mix, OK
  // coche une playlist (la coche de « vu ») au lieu de l'ouvrir.
  const action = (key: string, label: string, icon: TvCard["icon"], mosaic: (string | null)[], open: () => void): TvCard => ({
    ...mixCard({ id: key, playlists: [], at: 0 }),
    key,
    label,
    title: label,
    line: "",
    image: null,
    badge: undefined,
    mosaic: mosaic.length ? mosaic : undefined,
    icon,
    open,
  });
  // Pochettes des playlists pour illustrer les deux actions : quatre pour « Écouter tout », trois autres pour le mix.
  const covers = $derived(items.map((i) => i.poster).filter((p): p is string => !!p));
  const tvItems = $derived(
    tab === "playlists" && items.length
      ? picking
        ? [
            action("cancel-mix", t("mix.cancel"), "x", [], () => (picking = false)),
            ...items.map((i) => ({ ...fromItem(i, title), watched: picked.some((p) => p.ratingKey === i.ratingKey), open: () => pick(i) })),
          ]
        : [
            action("play-all", t("mix.playAll"), "shuffle", covers.length >= 4 ? covers.slice(0, 4) : covers.slice(0, 1), playAll),
            action("new-mix", t("mix.title"), "playlistAdd", covers.length >= 7 ? covers.slice(4, 7) : covers.slice(0, 3).length === 3 ? covers.slice(0, 3) : covers.slice(0, 1), startPicking),
            ...items.map((i) => fromItem(i, title)),
          ]
      : items.map((i) => fromItem(i, title)),
  );

  async function loadMore() {
    if (tab === "recommended" || loading || (total && items.length >= total)) return;
    loading = true;
    const forTab = tab;
    try {
      // Les playlists arrivent d'un bloc ; artistes et albums page par page.
      const page =
        forTab === "playlists"
          ? await api.playlists(sectionKey).then((list) => ({ items: list, total: list.length }))
          : await api.sectionItems(sectionKey, items.length, PAGE_SIZE, KIND[forTab]);
      if (tab !== forTab) return;
      items.push(...page.items);
      total = page.total;
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }

  // Téléviseur : l'onglet s'ouvre dès qu'on y arrive avec les flèches, sans appuyer sur OK. Un court
  // délai évite de charger chaque onglet traversé.
  let tabTimer: ReturnType<typeof setTimeout> | undefined;
  function focusTab(next: Tab) {
    clearTimeout(tabTimer);
    if (next !== tab) tabTimer = setTimeout(() => select(next), 250);
  }

  function select(next: Tab) {
    if (next === tab) return;
    keepTabFocus();
    tab = next;
    items = [];
    total = 0;
    error = null;
    loading = false;
    picking = false;
    loadMore();
  }

  onMount(() => {
    api.sectionHubs(sectionKey).then(
      (h) => (hubs = h),
      (e) => (error = errorText(e)),
    );
    // Types Plex : 9 = album, 10 = titre.
    api.sectionItems(sectionKey, 0, 24, 9, null, "addedAt:desc").then((p) => (newAlbums = p.items), () => {});
    api.sectionItems(sectionKey, 0, 24, 10, null, "lastViewedAt:desc").then(
      async (p) => {
        // Rien d'écouté récemment : les titres les plus écoutés prennent le relais.
        hotTracks = p.items.length ? p.items : (await api.sectionItems(sectionKey, 0, 24, 10, null, "viewCount:desc")).items;
      },
      () => {},
    );
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) loadMore();
      },
      { rootMargin: "600px" },
    );
    if (sentinel) observer.observe(sentinel);
    return () => observer.disconnect();
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
    {#if tab !== "recommended" && total}<span class="muted">{total}</span>{/if}
    {#if picking}<span class="pick-hint">{t("mix.pickHint", { count: picked.length })}</span>{/if}
  </div>
{/snippet}

{#if device.tv}
  {#if tab === "recommended" && hubs && tvRows.length}
    <TvRows rows={tvRows} header={tvHead} blur />
  {:else}
    <div class="page tv-page">
      {@render tvHead()}
      {#if error}<p class="error">{error}</p>{/if}
      {#if tab !== "recommended"}<TvGrid cards={tvItems} onEnd={loadMore} />{/if}
    </div>
  {/if}
{:else}
<div class="page">
  <div class="head">
    <!-- Le titre, et dans l'onglet Playlists ses deux boutons, sur la ligne des onglets. -->
    <div class="title-line">
      <h1 class="row-title page-title">
        {title}
        {#if tab !== "recommended" && total}<span class="muted">{total}</span>{/if}
      </h1>
      {#if tab === "playlists" && items.length}
        <!-- Toutes les playlists d'un coup, ou trois au choix (Mix of Three). -->
        <div class="mix-bar">
          {#if picking}
            <span class="pick-hint">{t("mix.pickHint", { count: picked.length })}</span>
            <button class="btn" onclick={() => (picking = false)}><Icon name="x" size={16} /> {t("mix.cancel")}</button>
          {:else}
            <button class="btn primary" onclick={playAll}><Icon name="shuffle" size={16} /> {t("mix.playAll")}</button>
            <button class="btn" onclick={startPicking}><Icon name="playlistAdd" size={16} /> {t("mix.title")}</button>
          {/if}
        </div>
      {/if}
    </div>
    <div class="tabs" role="tablist">
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
      <div class="skeleton" style:height="260px"></div>
    {:else if !hubs.length}
      <p class="muted">{t("music.library.nothingToRecommend")}</p>
    {:else}
      {#if !device.phone}<FeaturePanels {panels} />{/if}
      {#if playlistHub || liveMixes().length}
        <Row title={playlistHub?.title ?? t("music.library.tabs.playlists")} large>
          {#each liveMixes() as mix (mix.id)}<MixCard {mix} />{/each}
          {#each playlistHub?.items ?? [] as item (item.ratingKey)}<PosterCard {item} />{/each}
        </Row>
      {/if}
      <TrackGrid title={t("music.library.hotTracks")} tracks={hotTracks} />
      {#if newAlbums.length}
        <Row title={t("music.library.newReleases")} large>
          {#each newAlbums as item (item.ratingKey)}<PosterCard {item} />{/each}
        </Row>
      {/if}
      {#each otherHubs as hub, i (i)}
        <Row title={hub.title} large>
          {#each hub.items as item (item.ratingKey)}<PosterCard {item} />{/each}
        </Row>
      {/each}
    {/if}
  {:else}
    {#if picking}
      <div class="grid">
        {#each items as item (item.ratingKey)}
          {@const on = picked.some((p) => p.ratingKey === item.ratingKey)}
          <button class="pick" class:on onclick={() => pick(item)} aria-pressed={on}>
            <span class="pick-img">
              {#if item.poster}<img src={item.poster} alt="" loading="lazy" />{/if}
              <span class="pick-check">{#if on}<Icon name="check" size={18} />{/if}</span>
            </span>
            <span class="pick-title">{item.title}</span>
            {#if item.subtitle}<span class="pick-sub">{item.subtitle}</span>{/if}
          </button>
        {/each}
      </div>
    {:else}
      <div class="grid">
        {#each items as item (item.ratingKey)}<PosterCard {item} />{/each}
      </div>
    {/if}
  {/if}
  <div bind:this={sentinel} class="sentinel">{#if loading}<span class="muted">{t("music.library.loading")}</span>{/if}</div>
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

  .head {
    margin: 8px 0 14px;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px 24px;
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

  .title-line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px 24px;
  }

  .title-line h1 {
    margin: 0;
  }

  .mix-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }

  .mix-bar .btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }

  .pick-hint {
    color: var(--accent);
    font-weight: 700;
  }

  /* Choix des trois playlists d'un mix : la pochette se coche, rien ne s'ouvre. */
  .pick {
    display: grid;
    gap: 2px;
    min-width: 0;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
  }

  .pick-img {
    position: relative;
    display: block;
    aspect-ratio: 1;
    margin-bottom: 8px;
    overflow: hidden;
    border-radius: var(--radius);
    background: var(--bg-elev);
    box-shadow: 0 4px 14px #0006;
    transition: box-shadow 0.15s, transform 0.2s var(--ease);
  }

  .pick-img img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .pick:hover .pick-img {
    transform: translateY(-3px);
  }

  .pick.on .pick-img {
    box-shadow: 0 0 0 3px var(--accent), 0 10px 24px #000a;
  }

  .pick-check {
    position: absolute;
    top: 8px;
    right: 8px;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 2px solid #fff;
    border-radius: 50%;
    background: #0008;
    color: var(--accent-text);
  }

  .pick.on .pick-check {
    border-color: var(--accent);
    background: var(--accent);
  }

  .pick-title {
    overflow: hidden;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pick-sub {
    color: var(--text-dim);
    font-size: 13px;
  }

  .sentinel {
    height: 40px;
    margin-top: 16px;
  }
</style>
