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
</script>

<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, errorText, formatDuration, isMusic, type Item, type ItemDetail } from "../lib/api";
  import MusicDetail from "./MusicDetail.svelte";
  import { go, nav, replace, toast } from "../lib/nav.svelte";
  import { onPlaybackEnded, play, player, type PlayOptions } from "../lib/player.svelte";
  import PosterCard from "../components/PosterCard.svelte";
  import Row from "../components/Row.svelte";
  import Icon from "../components/Icon.svelte";
  import TrackSelect from "../components/TrackSelect.svelte";
  import DownloadButton from "../components/DownloadButton.svelte";
  import { canDownload } from "../lib/offline.svelte";
  import TitleLogo from "../components/TitleLogo.svelte";
  import { t, locale } from "../lib/i18n";
  import { device } from "../lib/device";
  import { reveal } from "../lib/spatial";
  import SiteRatings from "../components/SiteRatings.svelte";
  import Stars from "../components/Stars.svelte";
  import ScrollArrows from "../components/ScrollArrows.svelte";

  let { ratingKey }: { ratingKey: string } = $props();

  /*
   * Comme Plex : un épisode n'a pas de fiche à lui. L'ouvrir (Continuer à regarder, recherche, ajouts
   * récents…) affiche sa saison, avec la liste de tous ses épisodes de haut en bas ; l'épisode ouvert y est
   * mis en avant (et sélectionné à la télécommande), et le bouton principal le reprend.
   */
  // svelte-ignore state_referenced_locally
  const seasonOfKey = episodeSeason.get(ratingKey);
  // svelte-ignore state_referenced_locally
  const cached = cache.get(ratingKey) ?? (seasonOfKey ? cache.get(seasonOfKey) : undefined);
  let item = $state<ItemDetail | null>(cached?.item ?? null);
  let children = $state<Item[]>(cached?.children ?? []);
  /** Saison : l'épisode ouvert, sinon celui en cours, sinon le premier à voir. */
  // svelte-ignore state_referenced_locally
  let target = $state<string | null>(cached && seasonOfKey ? ratingKey : (cached?.sel ?? null));
  /** Saisons de la série, en onglets au-dessus d'une saison. */
  let seasons = $state<Item[]>(cached?.seasons ?? []);
  /** Fiche d'une série : l'épisode à reprendre (ou le prochain à voir). */
  let showNext = $state<Item | null>(cached?.showNext ?? null);
  let error = $state<string | null>(null);
  let listEl = $state<HTMLElement>();

  // Vide seulement tant que la page charge : le gabarit ne s'en sert qu'une fois `item` arrivé.
  const view = $derived(item as ItemDetail);
  const targetItem = $derived(item?.kind === "season" ? (children.find((c) => c.ratingKey === target) ?? null) : null);
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
  if (cached) pickTracks(cached.item.versions?.[0] ?? cached.item);

  function chooseVersion(v: number) {
    version = v;
    if (media) pickTracks(media);
  }
  let related = $state<Item[]>(cached?.related ?? []);
  let summaryOpen = $state(false);
  let root = $state<HTMLElement>();
  let fade = $state(0);

  function remember() {
    if (!item) return;
    const key = item.ratingKey;
    cache.delete(key);
    cache.set(key, { item, children, related, sel: target, seasons, showNext });
    if (cache.size > CACHE_SIZE) cache.delete(cache.keys().next().value!);
    if (item.kind === "season") for (const c of children) episodeSeason.set(c.ratingKey, key);
  }

  /** Épisode mis en avant en ouvrant une saison : celui en cours, sinon le premier à voir, sinon le premier. */
  const firstToWatch = (eps: Item[]) => eps.find((e) => e.viewOffsetMs) ?? eps.find((e) => !e.watched) ?? eps[0] ?? null;

  // Télécommande : à l'ouverture de la saison, la sélection va sur l'épisode visé (une fois ; au retour
  // d'une autre page, elle revient là où l'on était).
  // svelte-ignore state_referenced_locally
  let focusPending = device.tv && !cached;

  /** Télécommande : sélectionne l'épisode visé et l'amène au milieu de l'écran. */
  function focusTarget() {
    if (!focusPending) return;
    focusPending = false;
    tick().then(() => {
      const el = listEl?.querySelector<HTMLElement>(`[data-ep="${target}"] .thumb`);
      if (!el) return;
      el.focus({ preventScroll: true });
      reveal(el, { center: true });
    });
  }

  /** Une saison, ses épisodes et l'épisode visé ; les autres saisons de la série en onglets. */
  function showSeason(detail: ItemDetail, kids: Item[], want: string | null) {
    item = detail;
    children = kids;
    target = want && kids.some((k) => k.ratingKey === want) ? want : (firstToWatch(kids)?.ratingKey ?? null);
    focusTarget();
    if (device.tv && detail.showKey && seasons[0]?.showKey !== detail.showKey)
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

  /**
   * Charge la fiche. `again` : rechargement après une lecture (état « vu », reprise) ; dans une saison ouverte
   * depuis un épisode, la sélection reste alors sur l'épisode visé (le dernier lu), au lieu de revenir à celui
   * par lequel la page a été ouverte.
   */
  async function load(again = false) {
    try {
      let detail = await api.metadata(ratingKey);
      // Un épisode s'ouvre dans sa saison ; sans saison joignable, il garde sa propre fiche.
      if (detail.kind === "episode" && detail.seasonKey) {
        const episode = detail;
        try {
          const [season, kids] = await Promise.all([api.metadata(episode.seasonKey!), api.children(episode.seasonKey!)]);
          showSeason(season, kids, again && target ? target : episode.ratingKey);
          remember();
          return;
        } catch {
          detail = episode;
        }
      }
      if (detail.kind === "season") {
        showSeason(detail, await api.children(detail.ratingKey), target);
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

  // Télécommande : un épisode lancé depuis sa jaquette. Au retour du lecteur, la sélection revient sur sa
  // jaquette (celle du dernier épisode lu, si la lecture a enchaîné les suivants), au lieu de se perdre et
  // de repartir au bouton « Lire » tout en haut.
  let fromEpisode = false;
  let lastPlayed: string | null = null;

  function startEpisode(key: string) {
    fromEpisode = device.tv;
    start(key);
  }

  function focusEpisode(key: string, tries = 20) {
    const el = listEl?.querySelector<HTMLElement>(`[data-ep="${key}"] .thumb`);
    // La page réapparaît un peu après la fermeture du lecteur : on attend qu'elle soit affichée.
    if (!el || !el.getClientRects().length) {
      if (tries > 0) setTimeout(() => focusEpisode(key, tries - 1), 75);
      return;
    }
    target = key;
    el.focus({ preventScroll: true });
    reveal(el, { center: true });
  }

  $effect(() => {
    if (player.active) {
      if (player.ratingKey && children.some((c) => c.ratingKey === player.ratingKey)) lastPlayed = player.ratingKey;
      return;
    }
    if (!fromEpisode || !lastPlayed) return;
    fromEpisode = false;
    focusEpisode(lastPlayed);
  });

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
    start(view.ratingKey, { audioStream, subtitleStream, fromStart, mediaIndex: version });
  }

  /** Menu « ⋯ » de la fiche (ou d'un épisode) : informations, vu / non vu, retrait de « Continuer à regarder », gestion. */
  function openMenu(e: MouseEvent, of: Item = view, here = true) {
    e.preventDefault();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    nav.menu = e.type === "contextmenu" ? { item: of, x: e.clientX, y: e.clientY, here } : { item: of, x: r.left, y: r.bottom + 6, here };
  }

  async function toggleWatched() {
    if (!view) return;
    try {
      await api.setWatched(view.ratingKey, !view.watched);
      await load();
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  /** Ouvre les filtres d'une collection intelligente dans sa bibliothèque, pour les modifier. */
  async function editSmart() {
    try {
      const [smart, sections] = await Promise.all([api.smartCollection(item!.ratingKey), api.sections()]);
      const section = sections.find((s) => s.key === smart.sectionKey);
      if (!section || !smart.content) throw new Error(t("item.smartUnavailable"));
      go({
        name: "section",
        key: section.key,
        title: section.title,
        kind: section.kind,
        tab: "library",
        smart: { ratingKey: item!.ratingKey, title: smart.title, content: smart.content },
      });
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  // --- Collection, pour l'administrateur sur ordinateur : son ordre (date de sortie, alphabétique ou personnalisé,
  // les jaquettes se glissant alors pour les ranger), ou la conversion d'une collection intelligente ---
  const manages = $derived(nav.admin && nav.serverKind !== "jellyfin" && !device.tv && !device.mobile && !device.phone);
  let order = $state<number | null>(null);
  /** Collection intelligente (elle se convertit, mais ne se range pas à la main). */
  let colSmart = $state(false);
  /** Roue dentée : les réglages de la collection (ordre, conversion). */
  let settingsOpen = $state(false);
  let converting = $state(false);
  let confirmConvert = $state(false);
  let confirmTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const key = item?.kind === "collection" && manages ? item.ratingKey : null;
    order = null;
    colSmart = false;
    settingsOpen = false;
    if (!key) return;
    api.smartCollection(key).then(
      (c) => ((colSmart = c.smart), (order = c.smart ? null : c.sort)),
      () => {},
    );
  });

  async function setOrder(sort: number) {
    const key = item!.ratingKey;
    try {
      await api.setCollectionSort(key, sort);
      order = sort;
      children = await api.collectionItems(key);
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  // Glisser une jaquette (ordre personnalisé), à la souris : le glisser-déposer du navigateur ne fonctionne pas
  // dans la fenêtre de l'application (elle réserve le dépôt aux fichiers). Maintenir le bouton et bouger de quelques
  // pixels saisit la jaquette ; la grille se réarrange sous la souris, et Plex n'est prévenu qu'au lâcher, avec la
  // nouvelle place (juste après la jaquette qui la précède). Un simple clic ouvre toujours le film.
  let dragKey = $state<string | null>(null);
  let dragFrom = -1;
  let press: { key: string; x: number; y: number } | null = null;

  function pressDown(e: PointerEvent, key: string) {
    if (e.button !== 0) return;
    press = { key, x: e.clientX, y: e.clientY };
  }

  function pointerMove(e: PointerEvent) {
    if (!press) return;
    if (!dragKey) {
      if (Math.hypot(e.clientX - press.x, e.clientY - press.y) < 8) return;
      dragKey = press.key;
      dragFrom = children.findIndex((c) => c.ratingKey === dragKey);
    }
    e.preventDefault();
    const slot = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>("[data-slot]");
    const i = slot ? Number(slot.dataset.slot) : -1;
    const from = children.findIndex((c) => c.ratingKey === dragKey);
    if (i < 0 || from < 0 || from === i) return;
    const next = [...children];
    const [moved] = next.splice(from, 1);
    next.splice(i, 0, moved);
    children = next;
  }

  async function pointerUp() {
    press = null;
    const key = dragKey;
    if (!key) return;
    // Le clic qui suit aussitôt le lâcher n'ouvre pas le film (et seulement lui : un lâcher sans clic ne doit
    // pas avaler le clic d'après).
    const swallow = (e: Event) => (e.stopPropagation(), e.preventDefault());
    window.addEventListener("click", swallow, { capture: true, once: true });
    setTimeout(() => window.removeEventListener("click", swallow, { capture: true }));
    setTimeout(() => (dragKey = null));
    const to = children.findIndex((c) => c.ratingKey === key);
    if (to < 0 || to === dragFrom) return;
    try {
      await api.moveInCollection(item!.ratingKey, key, to > 0 ? children[to - 1].ratingKey : null);
    } catch (e) {
      toast(errorText(e), true);
      children = await api.collectionItems(item!.ratingKey).catch(() => children);
    }
  }

  /** Collection intelligente → ordinaire : un premier clic demande confirmation, le second convertit. */
  async function convert() {
    if (!confirmConvert) {
      confirmConvert = true;
      clearTimeout(confirmTimer);
      confirmTimer = setTimeout(() => (confirmConvert = false), 5000);
      return;
    }
    confirmConvert = false;
    converting = true;
    try {
      const id = await api.convertCollection(item!.ratingKey);
      toast(t("item.converted"));
      // L'ancienne collection n'existe plus : Retour ne doit pas y ramener.
      replace({ name: "item", ratingKey: id });
    } catch (e) {
      toast(errorText(e), true);
    } finally {
      converting = false;
    }
  }

  /** Rangée de la distribution, que ses flèches font défiler (ordinateur). */
  let castRow = $state<HTMLElement>();

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


  /*
   * Saisons d'une série : sur une ligne pleine, la grille habituelle ; moins nombreuses que les places d'une
   * ligne, elles gardent la même taille et se centrent. La largeur est lue avant l'affichage (rien ne saute).
   */
  let seasonsWidth = $state(0);
  const few = $derived.by(() => {
    if (!seasonsWidth || item?.kind !== "show") return null;
    const gap = device.phone ? 10 : 20;
    const min = device.phone ? 0 : 160;
    const cols = device.phone ? 3 : Math.max(1, Math.floor((seasonsWidth + gap) / (min + gap)));
    if (children.length >= cols) return null;
    const card = (seasonsWidth - (cols - 1) * gap) / cols;
    return { card, width: children.length * card + (children.length - 1) * gap };
  });

  /**
   * Onglets des saisons, sur une seule ligne qui défile (une série de 29 saisons tenait sur quatre lignes) :
   * la saison ouverte vient au milieu avant l'affichage, et la molette fait défiler la bande de côté.
   */
  function tabStrip(el: HTMLElement, _key: string) {
    const center = () => {
      const tab = el.querySelector<HTMLElement>('[aria-selected="true"]');
      if (tab && el.scrollWidth > el.clientWidth) el.scrollLeft = tab.offsetLeft - (el.clientWidth - tab.offsetWidth) / 2;
    };
    const wheel = (e: WheelEvent) => {
      if (el.scrollWidth <= el.clientWidth || Math.abs(e.deltaX) > Math.abs(e.deltaY)) return;
      e.preventDefault();
      el.scrollLeft += e.deltaY;
    };
    center();
    el.addEventListener("wheel", wheel, { passive: false });
    return {
      update: () => tick().then(center),
      destroy: () => el.removeEventListener("wheel", wheel),
    };
  }

  /** Temps restant d'un épisode commencé. */
  const left = (ep: Item) => (ep.viewOffsetMs && ep.durationMs ? ep.durationMs - ep.viewOffsetMs : null);

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
    // Fin d'une lecture : mpv sur ordinateur, lecteur du système sur Android / TV.
    const reload = () => load(true);
    const unlisten = listen("player-stopped", reload);
    const off = onPlaybackEnded(reload);
    return () => {
      off();
      unlisten.then((f) => f());
    };
  });
</script>

<svelte:window onpointermove={pointerMove} onpointerup={pointerUp} />

{#if error}
  <div class="page"><p class="error">{error}</p></div>
{:else if !item}
  <div class="page"><div class="skeleton" style:height="360px"></div></div>
{:else if isMusic(item.kind)}
  <MusicDetail {item} />
{:else}
  <!-- Le fond reste en place pendant que la page défile par-dessus, et s'assombrit peu à peu. Le défilement
       ne change que l'agrandissement de l'image et l'opacité du voile, chacun sur son calque : rien n'est
       redessiné à chaque image (une variable de style forçait à repeindre tout le fond, saccadé sur TV). -->
  <div class="backdrop">
    {#if heroArt}<img src={heroArt} alt="" style:transform="scale({1 + fade * 0.05})" />{/if}
    <div class="veil" style:opacity={fade * 0.72}></div>
  </div>

  <div class="page detail" bind:this={root} data-nav-rows>
    <section class="hero">
      <div class="hero-main">
        {#if device.tv && item.kind === "season" && item.showKey}
          <!-- Télécommande, comme Plex : la série, puis chacune de ses saisons, en onglets. Ailleurs, le logo ramène à la série. -->
          <div class="season-tabs" role="tablist" aria-label={item.showTitle ?? ""} data-nav-row use:tabStrip={item.ratingKey + ":" + seasons.length}>
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
          <!-- Épisode ou saison : le logo de la série, qui y ramène, puis la saison (ou l'épisode). -->
          <h1>
            <button class="show-logo" tabindex={device.tv ? -1 : 0} onclick={() => go({ name: "item", ratingKey: item!.showKey! })} title={item.showTitle ?? ""}>
              <TitleLogo logo={null} guid={item.logoGuid} genres={item.genres} title={item.showTitle ?? item.title} />
            </button>
          </h1>
          <div class="line">
            {#if item.kind === "season"}
              {item.title}
            {:else}
              {t("item.seasonEpisode", { season: item.parentIndex ?? "", episode: item.index ?? "" })} · {item.title}
            {/if}
          </div>
        {:else}
          <h1><TitleLogo logo={item.logo} guid={item.logoGuid} genres={item.genres} title={item.title} /></h1>
          {#if item.originalTitle}<div class="line original">{item.originalTitle}</div>{/if}
        {/if}

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
            {#if view.siteRatings}<SiteRatings item={view} />{/if}
            {#if nav.serverKind === "plex" && !device.tv}<Stars item={view} size={17} />{/if}
          </div>
        {/if}

        <div class="actions" data-nav-row>
          {#if view.playable}
            <button class="btn primary big" onclick={() => playThis()} data-nav-default>
              <Icon name="play" size={18} filled />
              {view.viewOffsetMs ? t("item.resume") : t("item.play")}
            </button>
          {:else if targetItem}
            <!-- Saison : l'épisode ouvert (ou celui en cours, le prochain à voir), en un clic. -->
            <button class="btn primary big" onclick={() => start(targetItem!.ratingKey)}>
              <Icon name="play" size={18} filled />
              {t(targetItem.viewOffsetMs ? "item.resumeEpisode" : "item.playEpisode", { season: targetItem.parentIndex ?? "", episode: targetItem.index ?? "" })}
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
          <!-- Android : film ou épisode, ou toute la saison, à regarder hors ligne. -->
          {#if canDownload() && view.playable && (view.kind === "movie" || view.kind === "episode")}
            <DownloadButton item={view} />
          {:else if canDownload() && item.kind === "season" && children.length}
            <DownloadButton {item} episodes={children} />
          {/if}
          {#if !collection && (view.kind === "movie" || view.kind === "show")}
            <button class="btn big" disabled={trailerBusy} onclick={openTrailer} title={t("item.trailer")}>
              <Icon name="film" size={18} />
              {t("item.trailer")}
            </button>
          {/if}
          <!-- Reprise en cours : recommencer depuis le début, avec les autres boutons ronds (après la bande-annonce). -->
          {#if view.playable && view.viewOffsetMs}
            <button class="btn round big" onclick={() => playThis(true)} title={t("item.playFromStart")}>
              <Icon name="restart" size={18} />
            </button>
          {/if}
          <!-- Collection intelligente (administrateur, sur ordinateur) : ses règles s'ouvrent dans la bibliothèque, comme dans Plex. -->
          {#if collection && view.smart && nav.admin && !device.tv && !device.mobile && !device.phone}
            <button class="btn big" onclick={editSmart}>
              <Icon name="sliders" size={18} />
              {t("item.editSmart")}
            </button>
          {/if}
          <!-- Roue dentée (administrateur, sur ordinateur) : l'ordre de la collection, ou la conversion d'une intelligente. -->
          {#if collection && manages && (order !== null || colSmart)}
            <div class="coll-settings">
              <button class="btn round big" class:on={settingsOpen} onclick={() => (settingsOpen = !settingsOpen)} title={t("item.collectionSettings")} aria-label={t("item.collectionSettings")} aria-expanded={settingsOpen}>
                <Icon name="settings" size={19} />
              </button>
              {#if settingsOpen}
                <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
                <div class="settings-backdrop" onclick={() => (settingsOpen = false)}></div>
                <div class="settings-panel" role="dialog" aria-label={t("item.collectionSettings")}>
                  {#if order !== null}
                    <label class="order">
                      <span>{t("item.order")}</span>
                      <TrackSelect
                        value={order}
                        label={t("item.order")}
                        options={[
                          { value: 0, label: t("item.orderRelease") },
                          { value: 1, label: t("item.orderAlpha") },
                          { value: 2, label: t("item.orderCustom") },
                        ]}
                        onchange={(v) => setOrder(Number(v))}
                      />
                    </label>
                    {#if order === 2}<p class="muted small">{t("item.orderHint")}</p>{/if}
                  {:else}
                    <!-- Une collection intelligente ne se range pas à la main (Plex la trie selon ses filtres) : la changer
                         en collection ordinaire le permet, mais elle ne se remplira plus seule. -->
                    <p class="muted small">{t("item.convertHint")}</p>
                    <button class="btn" class:danger={confirmConvert} disabled={converting} onclick={convert}>
                      <Icon name="library" size={16} />
                      {converting ? t("item.converting") : confirmConvert ? t("item.convertConfirm") : t("item.convert")}
                    </button>
                  {/if}
                </div>
              {/if}
            </div>
          {/if}
          {#if !collection}
            <button class="btn round big" class:on={view.watched} onclick={toggleWatched} title={view.watched ? t("item.markUnwatched") : t("item.markWatched")}>
              <Icon name="checkCircle" size={20} />
            </button>
            {#if nav.admin}
              <button class="btn round big" onclick={() => (nav.editing = view)} title={t("item.editInfo")}>
                <Icon name="edit" size={18} />
              </button>
            {/if}
            {#if !device.tv}
              <button class="btn round big" onclick={(e) => openMenu(e)} title={t("trackGrid.moreActions")} aria-label={t("trackGrid.moreActions")}>
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
      </div>
    </section>

    <!-- Sous le bandeau, en pleine largeur : l'accroche et le résumé, dépliable. -->
    {#if view.tagline || view.summary}
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
      <div class="panels" class:single={!hasMedia || !hasCredits}>
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
      <div class="grid" data-nav-row="grid" class:sortable={manages && order === 2} class:dragging-on={dragKey !== null}>
        {#each children as movie, i (movie.ratingKey)}
          {#if manages && order === 2}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="slot" data-slot={i} class:dragging={dragKey === movie.ratingKey} onpointerdown={(e) => pressDown(e, movie.ratingKey)}>
              <PosterCard item={movie} />
            </div>
          {:else}
            <PosterCard item={movie} />
          {/if}
        {/each}
      </div>
    {:else if item.kind === "show" && children.length}
      <div class="seasons" bind:clientWidth={seasonsWidth}>
        <div class="seasons-in" style:width={few ? `${few.width}px` : undefined}>
          <h2 class="row-title">{t("item.seasons")}</h2>
          <div class="grid" data-nav-row="grid" style:grid-template-columns={few ? `repeat(${children.length}, ${few.card}px)` : undefined}>
            {#each children as season (season.ratingKey)}<PosterCard item={season} />{/each}
          </div>
        </div>
      </div>
    {:else if item.kind === "season"}
      <!-- Les épisodes de haut en bas, chacun avec ses informations à droite. La vignette (ou OK) le lance ;
           l'épisode ouvert est mis en avant. -->
      <h2 class="row-title">{t("item.episodes")}</h2>
      <div class="episodes" bind:this={listEl}>
        {#each children as ep (ep.ratingKey)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="episode" class:target={ep.ratingKey === target} data-ep={ep.ratingKey} data-nav-row oncontextmenu={(e) => openMenu(e, ep, false)}>
            <button class="thumb" data-key="ep-{ep.ratingKey}" onclick={() => startEpisode(ep.ratingKey)} aria-label={t("item.playTitle", { title: ep.title })}>
              {#if ep.thumb}<img src={ep.thumb} alt="" loading="lazy" />{/if}
              <span class="ep-badge">
                {#if ep.watched}<Icon name="check" size={13} />{/if}
                {t("item.episodeShort", { number: ep.index ?? "" })}
              </span>
              <span class="play"><Icon name="play" size={22} filled /></span>
              {#if ep.viewOffsetMs && ep.durationMs}
                <div class="progress"><div style:width="{(ep.viewOffsetMs / ep.durationMs) * 100}%"></div></div>
              {/if}
            </button>
            <div class="ep-body">
              <div class="ep-title">
                <span class="ep-num">{ep.index}</span>
                {ep.title}
                {#if ep.watched}<span class="badge ok">{t("item.watched")}</span>{/if}
              </div>
              <div class="ep-meta">
                {#if ep.durationMs}<span>{formatDuration(ep.durationMs)}</span>{/if}
                {#if left(ep)}<span class="left">{t("item.remaining", { time: formatDuration(left(ep)) })}</span>{/if}
                {#if ep.contentRating}<span class="badge outline">{ep.contentRating}</span>{/if}
                {#if ep.rating}<span class="rating"><Icon name="star" size={13} filled /> {ep.rating.toFixed(1)}</span>{/if}
                {#if nav.serverKind === "plex" && !device.tv}<Stars item={ep} size={14} />{/if}
              </div>
              {#if ep.summary}<p class="ep-summary">{ep.summary}</p>{/if}
            </div>
            {#if !device.tv}
              <button class="btn round ep-more" onclick={(e) => openMenu(e, ep, false)} title={t("trackGrid.moreActions")} aria-label={t("trackGrid.moreActions")}>
                <Icon name="more" size={18} />
              </button>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    {#if item.cast.length}
      <h2 class="row-title">{t("item.cast")}<ScrollArrows target={castRow} /></h2>
      <div class="cast" data-nav-row bind:this={castRow}>
        {#each item.cast as person (person.name + person.role)}
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

  /* L'accroche et le résumé au milieu, sous le titre centré, le résumé justifié (films, séries, saisons). */
  .about {
    max-width: 1100px;
    margin: 0 auto 28px;
    text-align: center;
    animation: rise 0.7s 0.3s var(--ease) backwards;
  }

  .about .summary {
    width: 100%;
    text-align: justify;
    /* La dernière ligne (ou un résumé d'une seule ligne) au milieu, comme le reste de la fiche. */
    text-align-last: center;
  }

  /* Un seul panneau (série, saison : pas de pistes à choisir) : à la même largeur, centré. */
  .panels.single {
    grid-template-columns: minmax(0, calc((100% - 20px) / 2));
    justify-content: center;
  }

  .seasons-in {
    margin: 0 auto;
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

  /* ---------- Collection : roue dentée (ordre, conversion) et jaquettes à glisser (ordre personnalisé) ---------- */

  .coll-settings {
    position: relative;
  }

  .settings-backdrop {
    position: fixed;
    inset: 0;
    z-index: 20;
  }

  .settings-panel {
    position: absolute;
    top: calc(100% + 10px);
    left: 50%;
    z-index: 21;
    display: grid;
    gap: 10px;
    width: 320px;
    padding: 14px;
    border: 1px solid #ffffff1a;
    border-radius: 14px;
    background: #1c1e23f2;
    backdrop-filter: blur(18px);
    box-shadow: 0 18px 44px #000b;
    text-align: left;
    transform: translateX(-50%);
  }

  .settings-panel .small {
    margin: 0;
    font-size: 12px;
  }

  .order {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-dim);
    font-size: 13px;
    font-weight: 600;
  }

  .sortable {
    user-select: none;
  }

  .sortable .slot {
    cursor: grab;
    transition: opacity 0.15s;
  }

  .dragging-on,
  .dragging-on .slot {
    cursor: grabbing;
  }

  .sortable .slot.dragging {
    opacity: 0.35;
  }

  /* L'image ne part pas seule (glisser du navigateur) : c'est la jaquette entière qui se déplace. */
  .sortable :global(img) {
    -webkit-user-drag: none;
  }

  .btn.danger {
    background: #c0392b;
    color: #fff;
  }

  /* ---------- Saison : onglets, puis les épisodes de haut en bas ---------- */

  .season-tabs {
    position: relative;
    display: flex;
    gap: 4px;
    width: fit-content;
    max-width: min(100%, 760px);
    margin-bottom: 18px;
    padding: 4px;
    overflow-x: auto;
    border-radius: 999px;
    background: #00000066;
    backdrop-filter: blur(12px);
    scrollbar-width: none;
  }

  .season-tabs::-webkit-scrollbar {
    display: none;
  }

  .season-tabs button {
    flex: none;
    white-space: nowrap;
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

  /* Téléviseur : la saison sélectionnée se remplit de la couleur du thème, dans la bulle (le contour habituel,
     qui déborde de 6 px, était coupé par la bande qui défile). */
  :global(html.tv) .season-tabs button:focus {
    outline: none;
    border-color: transparent;
    background: var(--accent);
    color: var(--accent-text);
  }

  .episodes {
    display: grid;
    gap: 6px;
  }

  .episode {
    position: relative;
    display: grid;
    grid-template-columns: 280px minmax(0, 1fr) auto;
    align-items: start;
    gap: 20px;
    padding: 10px;
    border: 1px solid transparent;
    border-radius: var(--radius-lg);
    transition: background 0.15s, border-color 0.15s;
  }

  .episode:hover {
    background: #ffffff0a;
  }

  /* L'épisode ouvert (Continuer à regarder…) ou celui à reprendre : mis en avant dans la liste. */
  .episode.target {
    border-color: #ffffff1f;
    background: #ffffff0d;
  }

  /* Télécommande : toute la ligne de l'épisode sélectionné s'éclaire, cadre doré autour de la vignette. */
  :global(html.tv) .episode:focus-within {
    background: #ffffff17;
  }

  .thumb {
    position: relative;
    aspect-ratio: 16 / 9;
    padding: 0;
    overflow: hidden;
    border: none;
    border-radius: var(--radius);
    background: var(--bg-elev);
  }

  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
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

  .thumb .progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
  }

  /* Souris : ▶ au survol de la vignette. */
  .play {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: #0007;
    color: var(--accent);
    opacity: 0;
    transition: opacity 0.15s;
  }

  :global(html:not(.tv)) .thumb:hover .play {
    opacity: 1;
  }

  .ep-body {
    min-width: 0;
    padding-top: 4px;
  }

  .ep-title {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    font-size: 17px;
    font-weight: 600;
  }

  .ep-num {
    color: var(--text-faint);
  }

  .ep-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    margin-top: 4px;
    color: var(--text-dim);
    font-size: 13px;
  }

  .ep-meta .left {
    color: var(--accent);
    font-weight: 600;
  }

  .ep-summary {
    display: -webkit-box;
    margin: 10px 0 0;
    overflow: hidden;
    color: var(--text-dim);
    line-height: 1.55;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
  }

  /* ⋯ d'un épisode : visible au survol (et sur l'épisode mis en avant). */
  .ep-more {
    width: 36px;
    height: 36px;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .episode:hover .ep-more,
  .episode.target .ep-more,
  .ep-more:focus-visible {
    opacity: 1;
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

  :global(html.phone) .episode {
    grid-template-columns: 132px minmax(0, 1fr);
    gap: 12px;
    padding: 6px;
  }

  :global(html.phone) .ep-more {
    display: none;
  }

  :global(html.phone) .ep-title {
    font-size: 15px;
  }

  :global(html.phone) .ep-summary {
    -webkit-line-clamp: 2;
    line-clamp: 2;
    font-size: 13px;
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
</style>
