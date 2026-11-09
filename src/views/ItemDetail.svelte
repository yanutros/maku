<script lang="ts" module>
  import type { Item as CachedItem, ItemDetail as CachedDetail } from "../lib/api";

  /**
   * Dernières fiches ouvertes : au retour d'une autre page (fiche d'un acteur…), la fiche s'affiche aussitôt
   * en entier, et la sélection revient directement là où l'on était, sans passer d'abord par le titre le
   * temps du chargement. Elle est ensuite remise à jour depuis le serveur.
   */
  const cache = new Map<
    string,
    { item: CachedDetail; children: CachedItem[]; related: CachedItem[]; sel: string | null; seasons: CachedItem[]; showNext: CachedItem | null }
  >();
  const CACHE_SIZE = 12;

  /** Saison de chaque épisode déjà vu dans une saison : son retour s'affiche aussitôt depuis `cache`. */
  const episodeSeason = new Map<string, string>();
  /** Fiches des épisodes sélectionnés dans la vue d'une saison : y revenir ne recharge rien. */
  const episodeDetails = new Map<string, CachedDetail>();
  const EPISODE_CACHE = 40;

  function rememberEpisode(d: CachedDetail) {
    episodeDetails.delete(d.ratingKey);
    episodeDetails.set(d.ratingKey, d);
    if (episodeDetails.size > EPISODE_CACHE) episodeDetails.delete(episodeDetails.keys().next().value!);
  }
</script>

<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, episodeTitle, errorText, formatDuration, isMusic, type Item, type ItemDetail } from "../lib/api";
  import MusicDetail from "./MusicDetail.svelte";
  import { go, nav, toast } from "../lib/nav.svelte";
  import { play, type PlayOptions } from "../lib/player.svelte";
  import PosterCard from "../components/PosterCard.svelte";
  import Row from "../components/Row.svelte";
  import Icon from "../components/Icon.svelte";
  import TrackSelect from "../components/TrackSelect.svelte";
  import DownloadButton from "../components/DownloadButton.svelte";
  import { canDownload } from "../lib/offline.svelte";
  import TitleLogo from "../components/TitleLogo.svelte";
  import { t, locale } from "../lib/i18n";
  import { device } from "../lib/device";
  import SiteRatings from "../components/SiteRatings.svelte";
  import Stars from "../components/Stars.svelte";

  let { ratingKey }: { ratingKey: string } = $props();

  /*
   * Comme Plex : un épisode n'a pas de fiche à lui. L'ouvrir (Continuer à regarder, recherche, ajouts
   * récents…) affiche la saison, avec cet épisode sélectionné en haut et tous ceux de la saison en rangée.
   * Choisir un autre épisode change le haut de la page sans la quitter ; l'adresse de la page suit
   * l'épisode choisi, pour y revenir au retour d'une autre page et après une lecture.
   */
  // svelte-ignore state_referenced_locally
  const seasonOfKey = episodeSeason.get(ratingKey);
  // svelte-ignore state_referenced_locally
  const cached = cache.get(ratingKey) ?? (seasonOfKey ? cache.get(seasonOfKey) : undefined);
  let item = $state<ItemDetail | null>(cached?.item ?? null);
  let children = $state<Item[]>(cached?.children ?? []);
  // svelte-ignore state_referenced_locally
  let selKey = $state<string | null>(cached && seasonOfKey ? ratingKey : (cached?.sel ?? null));
  // svelte-ignore state_referenced_locally
  let epDetail = $state<ItemDetail | null>(selKey ? (episodeDetails.get(selKey) ?? null) : null);
  /** Saisons de la série, en onglets au-dessus de la vue d'une saison. */
  let seasons = $state<Item[]>(cached?.seasons ?? []);
  /** Fiche d'une série : l'épisode à reprendre (ou le prochain à voir). */
  let showNext = $state<Item | null>(cached?.showNext ?? null);
  let error = $state<string | null>(null);
  let rowEl = $state<HTMLElement>();

  const selItem = $derived(item?.kind === "season" ? (children.find((c) => c.ratingKey === selKey) ?? null) : null);
  const seasonView = $derived(!!selItem);
  /** Téléviseur : le logo seul sur l'image, les informations de l'épisode sous la rangée. */
  const tvSeason = $derived(seasonView && device.tv);

  /** En attendant sa fiche, l'épisode choisi s'affiche avec ce que la saison en sait déjà. */
  const sketch = (i: Item): ItemDetail => ({
    ...i,
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
  });

  /** Ce que montrent le haut de la page, les boutons et les panneaux : l'épisode choisi, sinon le titre de la page. */
  // Vide seulement tant que la page charge (`item` aussi) : le gabarit ne s'en sert qu'une fois `item` arrivé.
  const view = $derived((selItem ? (epDetail?.ratingKey === selItem.ratingKey ? epDetail : sketch(selItem)) : item) as ItemDetail);
  let audioStream = $state<number | null>(null);
  let subtitleStream = $state<number>(0);
  // Version choisie (4K, 1080p…) : ses propres pistes audio et sous-titres.
  let version = $state(0);
  const media = $derived(view ? (view.versions?.[version] ?? view) : null);

  /** Pistes par défaut d'une version : celles retenues par le serveur. */
  function pickTracks(v: { audioTracks: { id: number; selected: boolean }[]; subtitleTracks: { id: number; selected: boolean }[] }) {
    audioStream = v.audioTracks.find((tr) => tr.selected)?.id ?? v.audioTracks[0]?.id ?? null;
    subtitleStream = v.subtitleTracks.find((tr) => tr.selected)?.id ?? 0;
  }

  // Fiche gardée en mémoire : ses pistes par défaut tout de suite, sans attendre la mise à jour.
  // svelte-ignore state_referenced_locally
  if (view) pickTracks(view.versions?.[0] ?? view);

  function chooseVersion(v: number) {
    version = v;
    if (media) pickTracks(media);
  }
  let related = $state<Item[]>(cached?.related ?? []);
  let summaryOpen = $state(false);
  let root = $state<HTMLElement>();
  let fade = $state(0);

  /** Le titre affiché par l'adresse de la page : l'épisode choisi dans une saison, sinon celui ouvert. */
  const routeKey = () => (nav.view.name === "item" ? nav.view.ratingKey : ratingKey);

  function remember() {
    if (!item) return;
    const key = item.ratingKey;
    cache.delete(key);
    cache.set(key, { item, children, related, sel: selKey, seasons, showNext });
    if (cache.size > CACHE_SIZE) cache.delete(cache.keys().next().value!);
    if (item.kind === "season") for (const c of children) episodeSeason.set(c.ratingKey, key);
  }

  /** Épisode sélectionné en ouvrant une saison : celui en cours, sinon le premier à voir, sinon le premier. */
  const firstToWatch = (eps: Item[]) => eps.find((e) => e.viewOffsetMs) ?? eps.find((e) => !e.watched) ?? eps[0] ?? null;

  let selTimer: ReturnType<typeof setTimeout> | undefined;

  /** Choisit l'épisode montré en haut de la saison ; sa fiche arrive ensuite (aussitôt si elle est connue). */
  function select(key: string | null) {
    if (key === selKey) return;
    selKey = key;
    version = 0;
    summaryOpen = false;
    clearTimeout(selTimer);
    if (nav.view.name === "item") nav.view.ratingKey = key ?? item?.ratingKey ?? ratingKey;
    if (!key) return void (epDetail = null);
    const known = episodeDetails.get(key) ?? null;
    epDetail = known;
    if (known) return pickTracks(known.versions?.[0] ?? known);
    // Télécommande : on attend que la sélection se pose avant de charger la fiche de l'épisode.
    selTimer = setTimeout(() => fetchEpisode(key), device.tv ? 180 : 0);
  }

  async function fetchEpisode(key: string) {
    try {
      const d = await api.metadata(key);
      rememberEpisode(d);
      if (selKey !== key) return;
      epDetail = d;
      pickTracks(d.versions?.[version] ?? d);
    } catch {
      // Fiche introuvable : l'épisode reste affiché avec ce que la saison en sait.
    }
  }

  /** Ramène l'épisode choisi au milieu de sa rangée (ouverture de la saison, changement de saison). */
  function centerSelected() {
    tick().then(() => {
      const card = rowEl?.querySelector<HTMLElement>(`[data-ep="${selKey}"]`);
      if (rowEl && card) rowEl.scrollLeft = card.offsetLeft - (rowEl.clientWidth - card.clientWidth) / 2;
    });
  }

  /** Saisons de la série en onglets, et l'épisode choisi dans celle ouverte. */
  function showSeason(detail: ItemDetail, kids: Item[], want: string | null) {
    item = detail;
    children = kids;
    const keep = want && kids.some((k) => k.ratingKey === want) ? want : (firstToWatch(kids)?.ratingKey ?? null);
    selKey = null;
    select(keep);
    centerSelected();
    if (detail.showKey && seasons[0]?.showKey !== detail.showKey)
      api.children(detail.showKey).then(
        (s) => {
          seasons = s;
          remember();
        },
        () => {},
      );
  }

  /** Onglet d'une autre saison : elle remplace celle-ci sur la même page, sans rien ajouter à l'historique. */
  async function openSeason(key: string) {
    if (key === item?.ratingKey) return;
    try {
      const [detail, kids] = await Promise.all([api.metadata(key), api.children(key)]);
      showSeason(detail, kids, null);
      remember();
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  /** Fiche d'une série : l'épisode à reprendre, dans la première saison qui reste à voir (les spéciaux en dernier). */
  async function findShowNext(list: Item[]) {
    const regular = list.filter((s) => s.index !== 0);
    const season = (regular.length ? regular : list).find((s) => (s.unwatchedCount ?? 1) > 0);
    if (!season) return void (showNext = null);
    try {
      const eps = await api.children(season.ratingKey);
      showNext = eps.find((e) => e.viewOffsetMs) ?? eps.find((e) => !e.watched) ?? null;
      remember();
    } catch {
      showNext = null;
    }
  }

  async function load() {
    try {
      let detail = await api.metadata(routeKey());
      // Un épisode s'ouvre dans sa saison ; sans saison joignable, il garde sa propre fiche.
      if (detail.kind === "episode" && detail.seasonKey) {
        const episode = detail;
        rememberEpisode(episode);
        try {
          const [season, kids] = await Promise.all([api.metadata(episode.seasonKey!), api.children(episode.seasonKey!)]);
          showSeason(season, kids, episode.ratingKey);
          remember();
          return;
        } catch {
          detail = episode;
        }
      }
      if (detail.kind === "season") {
        showSeason(detail, await api.children(detail.ratingKey), selKey);
        remember();
        return;
      }
      item = detail;
      if (version >= (detail.versions?.length ?? 1)) version = 0;
      pickTracks(detail.versions?.[version] ?? detail);
      if (detail.kind === "movie" || detail.kind === "show")
        api.related(detail.ratingKey).then(
          (r) => {
            related = r;
            remember();
          },
          () => {},
        );
      children =
        detail.kind === "show"
          ? await api.children(detail.ratingKey)
          : detail.kind === "collection"
            ? await api.collectionItems(detail.ratingKey)
            : [];
      if (detail.kind === "show") findShowNext(children);
      remember();
    } catch (e) {
      error = errorText(e);
    }
  }

  function start(key: string, opts: PlayOptions = {}) {
    play(key, opts).catch((e) => toast(errorText(e), true));
  }

  // Bande-annonce : un titre du serveur à lire, ou une vidéo YouTube par-dessus la page.
  let trailerBusy = $state(false);
  let trailerMissing = $state(false);
  async function openTrailer() {
    trailerBusy = true;
    trailerMissing = false;
    try {
      const found = await api.trailer(ratingKey);
      if (found?.ratingKey) start(found.ratingKey, { fromStart: true });
      else if (found?.youtube) nav.trailer = found.youtube;
      else trailerMissing = true;
    } catch (e) {
      toast(errorText(e), true);
    } finally {
      trailerBusy = false;
    }
  }

  function playThis(fromStart = false) {
    start(view!.ratingKey, { audioStream, subtitleStream, fromStart, mediaIndex: version });
  }

  /** Lecture d'un épisode de la rangée : celui choisi garde ses pistes et sa version. */
  function playEpisode(key: string) {
    if (key === view?.ratingKey) playThis();
    else start(key);
  }

  /**
   * Vignette d'un épisode. À la télécommande, la sélection choisit déjà l'épisode : OK le lance. À la souris
   * et au doigt, un appui le choisit ; le bouton ▶ de la vignette (ou un double-clic) le lance.
   */
  function pressEpisode(key: string) {
    if (device.tv && key === selKey) playEpisode(key);
    else select(key);
  }

  /** Menu « ⋯ » de la fiche : informations, retrait de « Continuer à regarder », et la gestion (administrateur). */
  function openMenu(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    nav.menu = { item: view!, x: r.left, y: r.bottom + 6, here: true };
  }

  async function toggleWatched() {
    if (!view) return;
    try {
      await api.setWatched(view.ratingKey, !view.watched);
      episodeDetails.delete(view.ratingKey);
      await load();
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  // Pour une collection : premier film non vu (ordre de sortie).
  const nextUp = $derived(item?.kind === "collection" ? (children.find((c) => !c.watched) ?? children[0]) : undefined);
  const collection = $derived(item?.kind === "collection");
  const progress = $derived(
    view?.viewOffsetMs && view.durationMs ? Math.min(100, (view.viewOffsetMs / view.durationMs) * 100) : 0,
  );
  const remaining = $derived(view?.viewOffsetMs && view.durationMs ? view.durationMs - view.viewOffsetMs : null);
  const releaseDate = $derived(
    view?.releaseDate
      ? new Date(view.releaseDate).toLocaleDateString(locale, { day: "numeric", month: "long", year: "numeric" })
      : null,
  );
  // Fond du film, ou de la saison pour une série et ses épisodes (Plex reprend celui de la série
  // quand la saison n'a pas le sien), en haute définition : plus soigné que l'image d'un épisode,
  // gardée en dernier recours.
  const heroArt = $derived(
    item?.backdropHd ??
      item?.backdrop ??
      // Une collection sans fond prend celui de son premier film.
      (item?.kind === "collection" ? children.find((c) => c.backdrop)?.backdrop : undefined) ??
      (item?.kind === "episode" ? item.thumb : item?.poster),
  );
  // Heure de fin si la lecture démarre maintenant (reprise comprise), comme dans Jellyfin.
  const endsAt = $derived.by(() => {
    if (!view?.playable || !view.durationMs) return null;
    const end = new Date(Date.now() + (view.durationMs - (view.viewOffsetMs ?? 0)));
    return end.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
  });
  const hasCredits = $derived(!!view && (view.directors.length + view.writers.length + view.genres.length > 0 || !!view.studio));
  const hasMedia = $derived(!!view && (!!view.video || view.audioTracks.length > 0 || view.playable));
  /** Distribution de l'épisode choisi (ses invités), sinon celle de la saison ou de la série. */
  const cast = $derived(view?.cast.length ? view.cast : (item?.cast ?? []));

  // Le fond s'assombrit à mesure que la page défile sur lui.
  $effect(() => {
    const scroller = root?.closest(".content");
    if (!scroller) return;
    const update = () => (fade = Math.min(1, scroller.scrollTop / (window.innerHeight * 0.55)));
    update();
    scroller.addEventListener("scroll", update, { passive: true });
    return () => scroller.removeEventListener("scroll", update);
  });

  onMount(() => {
    load();
    const unlisten = listen("player-stopped", load);
    return () => unlisten.then((f) => f());
  });
</script>

{#if error}
  <div class="page"><p class="error">{error}</p></div>
{:else if !item}
  <div class="page"><div class="skeleton" style:height="360px"></div></div>
{:else if isMusic(item.kind)}
  <MusicDetail {item} />
{:else}
  <!-- Ligne de l'épisode choisi (ou de la saison), sous le logo de la série. -->
  {#snippet episodeLine()}
    <div class="line">
      {#if view.kind === "season"}
        {view.title}
      {:else if episodeTitle(view.title)}
        {t("hero.episodeLine", { season: view.parentIndex ?? "", episode: view.index ?? "", title: episodeTitle(view.title) ?? "" })}
      {:else}
        {t("item.seasonEpisode", { season: view.parentIndex ?? "", episode: view.index ?? "" })}
      {/if}
    </div>
  {/snippet}

  <!-- Date, durée, classement, fin ; puis les notes. -->
  {#snippet facts()}
    <div class="meta">
      {#if releaseDate}<span>{releaseDate}</span>{:else if view.year}<span>{view.year}</span>{/if}
      {#if view.durationMs && view.kind !== "show" && view.kind !== "season"}
        <span>{formatDuration(view.durationMs)}</span>
      {/if}
      {#if view.kind === "show" || view.kind === "season"}
        <span>{t(view.kind === "show" ? "item.seasonCount" : "item.episodeCount", { count: children.length })}</span>
      {:else if collection}
        <span>{t("item.movieCount", { count: children.length })}</span>
      {/if}
      {#if view.contentRating}<span class="badge outline">{view.contentRating}</span>{/if}
      {#if view.rating && !view.siteRatings?.length}<span class="rating"><Icon name="star" size={14} filled /> {view.rating.toFixed(1)}</span>{/if}
      {#if endsAt}<span>{t("item.endsAt", { time: endsAt })}</span>{/if}
      {#if view.watched}<span class="badge ok">{t("item.watched")}</span>{/if}
    </div>

    <!-- Notes des sites (réglables dans Réglages → Interface) et, avec Plex, la note de l'utilisateur. -->
    {#if !collection}
      <div class="scores">
        {#if view.siteRatings}{#key view.ratingKey}<SiteRatings item={view} />{/key}{/if}
        {#if nav.serverKind === "plex" && !device.tv}{#key view.ratingKey}<Stars item={view} size={17} />{/key}{/if}
      </div>
    {/if}
  {/snippet}

  {#snippet episodeSummary()}
    <!-- Saison : le résumé de l'épisode choisi, au-dessus de la rangée (sous elle sur TV). Toujours deux lignes de place, même sans résumé : la rangée ne bouge pas d'un épisode à l'autre. -->
    <button class="summary hero-summary" class:open={summaryOpen} tabindex={device.tv || !view.summary ? -1 : 0} onclick={() => (summaryOpen = !summaryOpen)}>
      {view.summary ?? ""}
    </button>
  {/snippet}

  <!-- Boutons du titre affiché (l'épisode choisi dans une saison) : dans le bandeau, ou sous la rangée des épisodes. -->
  {#snippet actions()}
    {@const v = view}
    <div class="actions" data-nav-row>
      {#if v.playable}
        <button class="btn primary big" onclick={() => playThis()} data-nav-default={seasonView ? undefined : ""}>
          <Icon name="play" size={18} filled />
          {v.viewOffsetMs ? t("item.resume") : t("item.play")}
        </button>
      {:else if showNext}
        <!-- Série : l'épisode à reprendre s'ouvre dans sa saison (rien ne se lance). -->
        <button class="btn primary big" onclick={() => go({ name: "item", ratingKey: showNext!.ratingKey })} data-nav-default>
          <Icon name="play" size={18} filled />
          {t(showNext.viewOffsetMs ? "item.resumeEpisode" : "item.playEpisode", { season: showNext.parentIndex ?? "", episode: showNext.index ?? "" })}
        </button>
      {:else if nextUp}
        <button class="btn primary big" onclick={() => start(nextUp!.ratingKey)} data-nav-default>
          <Icon name="play" size={18} filled />
          {t("item.playTitle", { title: nextUp.title })}
        </button>
      {/if}
      <!-- Reprise en cours : recommencer depuis le début. -->
      {#if v.playable && v.viewOffsetMs}
        <button class="btn round big" onclick={() => playThis(true)} title={t("item.playFromStart")}>
          <Icon name="restart" size={18} />
        </button>
      {/if}
      <!-- Android : film ou épisode, et toute la saison, à regarder hors ligne. -->
      {#if canDownload() && v.playable && (v.kind === "movie" || v.kind === "episode")}
        {#key v.ratingKey}<DownloadButton item={v} />{/key}
      {/if}
      {#if canDownload() && item?.kind === "season" && children.length}
        <DownloadButton {item} episodes={children} />
      {/if}
      {#if !collection && (v.kind === "movie" || v.kind === "show")}
        <button class="btn big" disabled={trailerBusy} onclick={openTrailer} title={t("item.trailer")}>
          <Icon name="film" size={18} />
          {t("item.trailer")}
        </button>
      {/if}
      {#if !collection}
        <button class="btn round big" class:on={v.watched} onclick={toggleWatched} title={v.watched ? t("item.markUnwatched") : t("item.markWatched")}>
          <Icon name="checkCircle" size={20} />
        </button>
        {#if nav.admin}
          <button class="btn round big" onclick={() => (nav.editing = v)} title={t("item.editInfo")}>
            <Icon name="edit" size={18} />
          </button>
        {/if}
        {#if !device.tv}
          <button class="btn round big" onclick={openMenu} title={t("trackGrid.moreActions")} aria-label={t("trackGrid.moreActions")}>
            <Icon name="more" size={20} />
          </button>
        {/if}
      {/if}
    </div>
    {#if trailerMissing}<p class="muted small no-trailer">{t("item.noTrailer")}</p>{/if}
    {#if progress > 0}
      <div class="watch-progress">
        <div class="bar"><div style:width="{progress}%"></div></div>
        {#if remaining}<span>{t("item.remaining", { time: formatDuration(remaining) })}</span>{/if}
      </div>
    {/if}
  {/snippet}

  <!-- Le fond reste en place pendant que la page défile par-dessus, et s'assombrit peu à peu. Le défilement
       ne change que l'agrandissement de l'image et l'opacité du voile, chacun sur son calque : rien n'est
       redessiné à chaque image (une variable de style forçait à repeindre tout le fond, saccadé sur TV). -->
  <div class="backdrop">
    {#if heroArt}<img src={heroArt} alt="" style:transform="scale({1 + fade * 0.05})" />{/if}
    <div class="veil" style:opacity={fade * 0.72}></div>
  </div>

  <div class="page detail" bind:this={root} data-nav-rows>
    <section class="hero" class:season={seasonView}>
      <div class="hero-main">
        {#if item.kind === "season" && item.showKey}
          <!-- Comme Plex : la série, puis chacune de ses saisons, en onglets. -->
          <div class="season-tabs" role="tablist" aria-label={item.showTitle ?? ""} data-nav-row>
            <button role="tab" aria-selected="false" onclick={() => go({ name: "item", ratingKey: item!.showKey! })}>{t("item.showTab")}</button>
            {#each seasons.length ? seasons : [item] as s (s.ratingKey)}
              <button
                role="tab"
                aria-selected={s.ratingKey === item.ratingKey}
                data-nav-current={s.ratingKey === item.ratingKey ? "" : undefined}
                onclick={() => openSeason(s.ratingKey)}>{s.title}</button
              >
            {/each}
          </div>
        {/if}
        {#if item.showKey && (item.kind === "episode" || item.kind === "season")}
          <!-- Épisode ou saison : le logo de la série, qui y ramène, puis l'épisode choisi. -->
          <h1>
            <button class="show-logo" tabindex={device.tv ? -1 : 0} onclick={() => go({ name: "item", ratingKey: item!.showKey! })} title={item.showTitle ?? ""}>
              <TitleLogo logo={null} guid={item.logoGuid} genres={item.genres} title={item.showTitle ?? item.title} />
            </button>
          </h1>
          {#if !tvSeason}{@render episodeLine()}{/if}
        {:else}
          <h1><TitleLogo logo={item.logo} guid={item.logoGuid} genres={item.genres} title={item.title} /></h1>
          {#if item.originalTitle}<div class="line original">{item.originalTitle}</div>{/if}
        {/if}

        {#if !tvSeason}{@render facts()}{/if}

        {#if seasonView}
          {#if !tvSeason}{@render episodeSummary()}{/if}
        {:else}
          {@render actions()}
        {/if}
      </div>
    </section>

    {#if seasonView}
      <!-- Les épisodes de la saison : en choisir un change le haut de la page ; ▶ (ou OK sur TV) le lance. -->
      <div class="ep-row" bind:this={rowEl} data-nav-row>
        {#each children as ep (ep.ratingKey)}
          <div class="ep" class:sel={ep.ratingKey === selKey} data-ep={ep.ratingKey}>
            <button
              class="ep-thumb"
              data-key="ep-{ep.ratingKey}"
              aria-pressed={ep.ratingKey === selKey}
              data-nav-current={ep.ratingKey === selKey ? "" : undefined}
              data-nav-default={ep.ratingKey === selKey ? "" : undefined}
              aria-label={t("item.seasonEpisode", { season: ep.parentIndex ?? "", episode: ep.index ?? "" }) + " · " + ep.title}
              onfocus={() => device.tv && select(ep.ratingKey)}
              onclick={() => pressEpisode(ep.ratingKey)}
              ondblclick={() => !device.tv && playEpisode(ep.ratingKey)}
            >
              {#if ep.thumb}<img src={ep.thumb} alt="" loading="lazy" />{/if}
              <span class="ep-badge">
                {#if ep.watched}<Icon name="check" size={13} />{/if}
                {t("item.episodeShort", { number: ep.index ?? "" })}
              </span>
              {#if ep.viewOffsetMs && ep.durationMs}
                <div class="progress"><div style:width="{(ep.viewOffsetMs / ep.durationMs) * 100}%"></div></div>
              {/if}
            </button>
            {#if !device.tv}
              <button class="ep-play" onclick={() => playEpisode(ep.ratingKey)} aria-label={t("item.playTitle", { title: ep.title })} title={t("item.playTitle", { title: ep.title })}>
                <Icon name="play" size={20} filled />
              </button>
            {/if}
            <div class="ep-name">{ep.title}</div>
          </div>
        {/each}
      </div>
      {#if tvSeason}
        <!-- Sans la ligne de l'épisode : son titre est déjà sous sa vignette, son numéro dessus. -->
        <div class="season-info">
          {@render facts()}
          {@render episodeSummary()}
        </div>
      {/if}
      <div class="season-actions">{@render actions()}</div>
    {/if}

    <!-- Sous le bandeau, en pleine largeur : l'accroche et le résumé, dépliable. -->
    {#if !seasonView && (view.tagline || view.summary)}
      <section class="about">
        {#if view.tagline && view.kind !== "episode"}<p class="tagline">{view.tagline}</p>{/if}
        {#if view.summary}
          <!-- Sur téléviseur, le résumé n'est pas une étape de la navigation. -->
          <button class="summary" class:open={summaryOpen} tabindex={device.tv ? -1 : 0} onclick={() => (summaryOpen = !summaryOpen)}>
            {view.summary}
          </button>
        {/if}
      </section>
    {/if}

    <!-- Deux panneaux, comme Jellyfin : l'équipe à gauche, les pistes à lire à droite. -->
    {#if hasCredits || hasMedia}
      <div class="panels">
        {#if hasCredits}
          <dl class="panel credits">
            {#if view.directors.length}<div><dt>{t("item.facts.director")}</dt><dd>{view.directors.join(", ")}</dd></div>{/if}
            {#if view.writers.length}<div><dt>{t("item.facts.writer")}</dt><dd>{view.writers.slice(0, 3).join(", ")}</dd></div>{/if}
            {#if view.studio}<div><dt>{t("item.facts.studio")}</dt><dd>{view.studio}</dd></div>{/if}
            {#if view.genres.length}
              <div><dt>{t("item.facts.genres")}</dt><dd class="genres">{#each view.genres as g}<span>{g}</span>{/each}</dd></div>
            {/if}
          </dl>
        {/if}
        {#if hasMedia}
          <div class="panel media">
            <!-- Plusieurs versions (4K, 1080p…) : la ligne vidéo devient un choix, comme dans Plex. -->
            {#if view.versions?.length > 1}
              <label class="track" data-nav-row>
                <span class="ico" title={t("item.facts.version")}><Icon name="film" size={18} /></span>
                <TrackSelect
                  value={version}
                  options={view.versions.map((v, i) => ({ value: i, label: v.label }))}
                  label={t("item.facts.version")}
                  onchange={(v) => chooseVersion(Number(v))}
                />
              </label>
            {:else if media?.video}
              <div class="track"><span class="ico" title={t("item.facts.video")}><Icon name="film" size={18} /></span><span class="video">{media.video}</span></div>
            {/if}
            {#if media?.audioTracks.length}
              <label class="track" data-nav-row>
                <span class="ico" title={t("item.facts.audio")}><Icon name="audio" size={18} /></span>
                <TrackSelect
                  bind:value={audioStream}
                  options={media.audioTracks.map((tr) => ({ value: tr.id, label: tr.label }))}
                  label={t("item.facts.audio")}
                />
              </label>
            {/if}
            {#if view.playable}
              <label class="track" data-nav-row>
                <span class="ico" title={t("item.facts.subtitles")}><Icon name="subtitles" size={18} /></span>
                <TrackSelect
                  bind:value={subtitleStream}
                  options={[{ value: 0, label: t("item.facts.none") }, ...(media?.subtitleTracks ?? []).map((tr) => ({ value: tr.id, label: tr.label }))]}
                  label={t("item.facts.subtitles")}
                />
              </label>
            {/if}
          </div>
        {/if}
      </div>
    {/if}

    {#if collection && children.length}
      <h2 class="row-title">{t("item.collectionMovies")}</h2>
      <div class="grid" data-nav-row="grid">
        {#each children as movie (movie.ratingKey)}<PosterCard item={movie} />{/each}
      </div>
    {:else if item.kind === "show" && children.length}
      <h2 class="row-title">{t("item.seasons")}</h2>
      <div class="grid" data-nav-row="grid">
        {#each children as season (season.ratingKey)}<PosterCard item={season} />{/each}
      </div>
    {/if}

    {#if cast.length}
      <h2 class="row-title">{t("item.cast")}</h2>
      <div class="cast" data-nav-row>
        {#each cast as person (person.name + person.role)}
          <!-- Comme dans Plex : un acteur ouvre la liste de ses films et séries du serveur. -->
          <button
            class="person"
            data-key="person-{person.id ?? person.name}"
            disabled={person.id === null}
            onclick={() => person.id !== null && go({ name: "person", id: person.id, personName: person.name, photo: person.photo })}
          >
            <div class="avatar">
              {#if person.photo}<img src={person.photo} alt="" loading="lazy" />{:else}{person.name[0]}{/if}
            </div>
            <div class="name">{person.name}</div>
            {#if person.role}<div class="role">{person.role}</div>{/if}
          </button>
        {/each}
      </div>
    {/if}

    {#if related.length}
      <Row title={t("item.related")}>
        {#each related as r (r.ratingKey)}<PosterCard item={r} />{/each}
      </Row>
    {/if}
  </div>
{/if}

<style>
  /* ---------- Bandeau : le fond du titre plein écran, sous l'en-tête transparent ---------- */

  .backdrop {
    position: fixed;
    /* Jusqu'au bord gauche de la fenêtre, derrière le menu (translucide sur ordinateur, replié ou en pilule sur TV). */
    inset: 0;
    z-index: 0;
    overflow: hidden;
    background: var(--bg);
    pointer-events: none;
  }

  /* Téléviseur : la page est le repère de ce fond fixé (App.svelte) et commence après la colonne de 72 px du menu.
     Le fond est ramené au bord de l'écran, sur toute sa largeur ; menu déplié, la page glisse de 164 px et il
     repart d'autant vers la gauche, au même rythme : il ne bouge pas à l'écran. */
  :global(html.tv) .backdrop {
    right: auto;
    left: -72px;
    width: 100vw;
    transition: transform 0.25s var(--ease);
  }

  :global(html.tv .shell:not(.collapsed)) .backdrop {
    transform: translateX(-164px);
  }

  /* Comme Jellyfin : l'image entière sur toute la largeur, centrée en haut, qui se fond dans le
     noir vers le bas (une fenêtre haute ne la rogne plus sur les côtés). */
  .backdrop img {
    display: block;
    width: 100%;
    height: auto;
    min-height: 62%;
    max-height: 100%;
    object-fit: cover;
    object-position: center top;
    mask-image: linear-gradient(#000 0%, #000 55%, transparent 100%);
    animation: unveil 1.2s var(--ease);
    will-change: transform;
  }

  @keyframes unveil {
    from {
      opacity: 0;
      transform: scale(1.08);
    }
  }

  /* Dégradés : l'image reste intacte en haut, le bas s'assombrit sous le titre centré, puis un voile
     (.veil) s'épaissit au défilement. */
  .backdrop::after {
    content: "";
    position: absolute;
    inset: 0;
    background:
      linear-gradient(0deg, var(--bg) 0%, #000000c8 22%, #00000040 50%, transparent 70%),
      linear-gradient(180deg, #00000080 0%, transparent 14%);
  }

  /* Sous les dégradés, comme l'ancienne couleur de fond qui venait en dernier. */
  .veil {
    position: absolute;
    inset: 0;
    background: #000;
    opacity: 0;
    will-change: opacity;
  }

  .detail {
    position: relative;
    z-index: 1;
  }

  /* Bandeau centré : le logo en très grand, puis les informations et les boutons dessous. */
  .hero {
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    align-items: center;
    min-height: calc(100vh - 68px - 120px);
    padding-bottom: 28px;
    text-align: center;
  }

  .hero-main {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
    max-width: 900px;
    animation: rise 0.7s 0.15s var(--ease) backwards;
  }

  .about {
    max-width: 1100px;
    margin-bottom: 28px;
    animation: rise 0.7s 0.3s var(--ease) backwards;
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(18px);
    }
  }

  .show-logo {
    width: 100%;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    transition: transform 0.2s var(--ease);
  }

  .show-logo:hover {
    transform: scale(1.02);
  }

  h1 {
    width: 100%;
    margin: 4px 0 6px;
    font-size: 72px;
    font-weight: 800;
    letter-spacing: -0.02em;
    line-height: 1.05;
    text-shadow: 0 4px 30px #000c;
  }

  .line {
    color: #ffffffd0;
    font-size: 17px;
    font-weight: 600;
  }

  .original {
    margin-bottom: 2px;
    color: #ffffffb0;
    font-size: 15px;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    align-items: center;
    gap: 8px 16px;
    margin-top: 14px;
    color: #ffffffd8;
    font-size: 15px;
    font-weight: 500;
    text-shadow: 0 1px 8px #000;
  }

  .rating {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--accent);
    font-weight: 700;
  }

  .scores {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    align-items: center;
    gap: 8px 18px;
    margin-top: 12px;
    color: #ffffffe0;
    font-size: 14px;
    text-shadow: 0 1px 8px #000;
  }

  .scores:empty {
    display: none;
  }

  .actions {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 12px;
    margin-top: 24px;
  }

  .big {
    height: 50px;
  }

  .btn.primary.big {
    padding: 0 32px;
    font-size: 16px;
  }

  .btn.round.big {
    width: 50px;
    border-color: #ffffff40;
    background: #00000059;
    backdrop-filter: blur(10px);
  }

  .btn.on {
    color: var(--accent);
  }

  .watch-progress {
    display: flex;
    align-items: center;
    gap: 12px;
    width: min(420px, 100%);
    margin-top: 18px;
    color: #ffffffb8;
    font-size: 13px;
    font-weight: 600;
  }

  .watch-progress .bar {
    flex: 1;
    height: 4px;
    overflow: hidden;
    border-radius: 2px;
    background: #ffffff33;
  }

  .watch-progress .bar div {
    height: 100%;
    background: var(--accent);
  }

  .tagline {
    margin: 0 0 10px;
    color: #fff;
    font-size: 17px;
    font-style: italic;
    font-weight: 600;
    text-shadow: 0 2px 12px #000;
  }

  /* Résumé : quatre lignes, le reste au clic. */
  .summary {
    display: -webkit-box;
    padding: 0;
    overflow: hidden;
    border: none;
    background: none;
    color: #ffffffd8;
    font-size: 15px;
    line-height: 1.6;
    text-align: left;
    text-shadow: 0 1px 10px #000;
    cursor: pointer;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
  }

  .summary.open {
    -webkit-line-clamp: unset;
    line-clamp: unset;
  }

  /* ---------- Panneaux : équipe et pistes ---------- */

  .panels {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 20px;
    margin-bottom: 8px;
  }

  .panel {
    margin: 0;
    padding: 6px 20px;
    border: 1px solid #ffffff14;
    border-radius: var(--radius-lg);
    background: #16171bcc;
    backdrop-filter: blur(16px);
  }

  .credits > div {
    display: grid;
    grid-template-columns: 130px minmax(0, 1fr);
    gap: 16px;
    padding: 12px 0;
  }

  .credits > div + div {
    border-top: 1px solid #ffffff10;
  }

  dt {
    color: var(--text-dim);
    font-size: 13px;
    font-weight: 700;
  }

  dd {
    margin: 0;
    font-size: 14px;
    text-align: right;
  }

  .genres {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 6px;
  }

  .genres span {
    padding: 2px 10px;
    border: 1px solid var(--border);
    border-radius: 12px;
    color: var(--text-dim);
    font-size: 12px;
  }

  .media {
    display: grid;
    align-content: start;
    gap: 10px;
    padding: 16px 20px;
  }

  .track {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }

  .ico {
    flex: none;
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    background: #ffffff0f;
    color: var(--accent);
  }

  .video {
    font-size: 14px;
    font-weight: 600;
  }

  /* ---------- Saison : onglets, rangée des épisodes, boutons de l'épisode choisi ---------- */

  /* Le bandeau d'une saison laisse la place à la rangée des épisodes dans le premier écran. */
  .hero.season {
    min-height: calc(100vh - 68px - 400px);
    padding-bottom: 18px;
  }

  .season-tabs {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 4px;
    margin-bottom: 18px;
    padding: 4px;
    border-radius: 999px;
    background: #00000066;
    backdrop-filter: blur(12px);
  }

  .season-tabs button {
    padding: 6px 16px;
    border: 1px solid transparent;
    border-radius: 999px;
    background: none;
    color: #ffffffb0;
    font: inherit;
    font-size: 14px;
    font-weight: 600;
  }

  .season-tabs button:hover {
    color: #fff;
  }

  .season-tabs button[aria-selected="true"] {
    border-color: #ffffffb0;
    color: #fff;
  }

  .hero-summary {
    max-width: 760px;
    margin-top: 14px;
    text-align: center;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  /* Ce qui change d'un épisode à l'autre garde la même hauteur : une ligne pour le titre et les
     informations, une pour les notes (même vide), deux pour le résumé. */
  .hero-summary:not(.open) {
    height: 3.2em;
  }

  .hero.season .line,
  .season-info .line {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hero.season .meta,
  .season-info .meta {
    flex-wrap: nowrap;
    min-height: 1.6em;
    white-space: nowrap;
  }

  .hero.season .scores,
  .hero.season .scores:empty,
  .season-info .scores,
  .season-info .scores:empty {
    display: flex;
    flex-wrap: nowrap;
    min-height: 24px;
  }

  .ep-row {
    position: relative;
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 280px;
    gap: 16px;
    overflow-x: auto;
    /* La place du cadre de sélection, sans rogner les vignettes. */
    padding: 8px 8px 6px;
    margin: 0 -8px;
    scrollbar-width: thin;
  }

  /* Téléviseur : la télécommande fait défiler la rangée, sans barre de défilement. */
  :global(html.tv) .ep-row {
    scrollbar-width: none;
  }

  .ep {
    position: relative;
    min-width: 0;
  }

  .ep-thumb {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    padding: 0;
    overflow: hidden;
    border: 2px solid transparent;
    border-radius: var(--radius);
    background: var(--bg-elev);
    transition: border-color 0.15s, transform 0.2s var(--ease);
  }

  .ep.sel .ep-thumb {
    border-color: var(--accent);
  }

  .ep-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  /* Les épisodes autres que celui choisi restent un peu en retrait, comme dans Plex. */
  .ep:not(.sel) .ep-thumb img {
    filter: brightness(0.72);
    transition: filter 0.15s;
  }

  .ep:not(.sel):hover .ep-thumb img {
    filter: none;
  }

  .ep-badge {
    position: absolute;
    top: 6px;
    right: 6px;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 1px 7px;
    border-radius: 6px;
    background: #000000b0;
    color: #fff;
    font-size: 12px;
    font-weight: 700;
  }

  .progress {
    height: 4px;
    background: #000a;
  }

  .progress div {
    height: 100%;
    background: var(--accent);
  }

  .ep-thumb .progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
  }

  /* Souris : ▶ au survol de la vignette ; au doigt, sur l'épisode choisi. */
  .ep-play {
    position: absolute;
    top: calc((280px - 16px) * 9 / 32 - 22px);
    left: calc(50% - 22px);
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: var(--accent);
    color: #000;
    opacity: 0;
    transition: opacity 0.15s, transform 0.15s;
  }

  .ep:hover .ep-play,
  .ep-play:focus-visible {
    opacity: 1;
  }

  .ep-play:hover {
    transform: scale(1.08);
  }

  :global(html.phone) .ep.sel .ep-play {
    opacity: 1;
  }

  .ep-name {
    margin-top: 6px;
    overflow: hidden;
    color: var(--text-dim);
    font-size: 14px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ep.sel .ep-name {
    color: var(--text);
  }

  /* Télécommande : la sélection agrandit un peu la vignette, le cadre doré suit l'épisode choisi. */
  :global(html.tv) .ep-thumb:focus {
    outline: none;
    border-color: #fff;
    transform: scale(1.03);
  }

  /* Téléviseur : les informations de l'épisode choisi sous la rangée, l'image dégagée au-dessus. */
  .season-info {
    display: flex;
    flex-direction: column;
    align-items: center;
    margin-top: 14px;
    text-align: center;
  }

  .season-info .meta,
  .season-info .scores {
    margin-top: 6px;
  }

  .season-info .hero-summary {
    margin-top: 8px;
  }

  /* Le bandeau ne garde que les onglets et le logo, sur l'image dégagée : la rangée reste en bas du premier
     écran, et les informations de l'épisode viennent encore dessous. */
  :global(html.tv) .hero.season {
    min-height: 60vh;
  }

  .season-actions {
    display: flex;
    flex-direction: column;
    align-items: center;
    margin-bottom: 28px;
  }

  .season-actions .actions {
    margin-top: 16px;
  }

  :global(html.tv) .season-actions .actions {
    margin-top: 12px;
  }

  .cast {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 164px;
    gap: 16px;
    overflow-x: auto;
    padding: 10px 10px 8px;
    margin: -10px -10px 0;
  }

  .person {
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    text-align: center;
  }

  .person:not(:disabled) .avatar {
    transition: transform 0.2s var(--ease), box-shadow 0.2s;
  }

  /* Souris seulement : sur téléviseur, Android laisse l'acteur ouvert « survolé » au retour, et son
     cercle doré s'ajoutait à la sélection (deux acteurs ciblés). */
  :global(html:not(.tv)) .person:not(:disabled):hover .avatar {
    transform: scale(1.05);
    box-shadow: 0 0 0 3px var(--accent);
  }

  /* Télécommande : la sélection entoure la photo (cercle doré), comme dans Plex, plutôt qu'un carré
     autour de la photo et du nom. */
  :global(html.tv) .person:focus {
    outline: none;
  }

  :global(html.tv) .person:focus .avatar {
    transform: scale(1.06);
    box-shadow: 0 0 0 4px var(--accent);
  }

  :global(html.tv) .person:focus .name {
    color: var(--accent);
  }

  .avatar {
    display: grid;
    place-items: center;
    width: 148px;
    height: 148px;
    margin: 0 auto 10px;
    overflow: hidden;
    border-radius: 50%;
    background: var(--bg-elev);
    color: var(--text-faint);
    font-size: 36px;
    font-weight: 600;
  }

  .avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .name {
    font-weight: 600;
    font-size: 14px;
  }

  .role {
    color: var(--text-dim);
    font-size: 12px;
  }

  /* ---------- Téléphone : tout s'empile, en pleine largeur ---------- */

  :global(html.phone) .backdrop {
    inset: 0 0 auto;
    height: 72vh;
  }

  :global(html.phone) .hero {
    flex-direction: column;
    align-items: stretch;
    gap: 16px;
    min-height: 62vh;
    padding-bottom: 20px;
  }

  :global(html.phone) h1 {
    font-size: 36px;
  }

  :global(html.phone) .meta {
    gap: 6px 12px;
    font-size: 13px;
  }

  :global(html.phone) .actions {
    flex-wrap: wrap;
    margin-top: 18px;
  }

  :global(html.phone) .actions .btn.primary {
    flex: 1 1 100%;
  }

  :global(html.phone) .summary {
    font-size: 14px;
  }

  :global(html.phone) .panels {
    grid-template-columns: minmax(0, 1fr);
    gap: 12px;
  }

  :global(html.phone) .credits > div {
    grid-template-columns: 96px minmax(0, 1fr);
  }

  :global(html.phone) .ep-row {
    grid-auto-columns: 200px;
    gap: 12px;
  }

  :global(html.phone) .ep-play {
    top: calc((200px - 16px) * 9 / 32 - 22px);
  }

  :global(html.phone) .hero.season {
    min-height: 46vh;
  }

  :global(html.phone) .cast {
    grid-auto-columns: 100px;
    gap: 12px;
  }

  :global(html.phone) .avatar {
    width: 88px;
    height: 88px;
    font-size: 26px;
  }

  /* Logo du titre sur la fiche. */
  h1 {
    --logo-h: 230px;
    --logo-w: min(760px, 100%);
    --logo-pos: center bottom;
    --logo-margin: 0 auto;
  }

  :global(html.phone) h1 {
    --logo-h: 120px;
  }

  /* Saison : un logo plus petit, pour voir la rangée des épisodes sans descendre. */
  .hero.season h1 {
    --logo-h: 150px;
  }

  :global(html.phone) .hero.season h1 {
    --logo-h: 90px;
  }
</style>
