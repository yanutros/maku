<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, episodeTitle, errorText, formatDuration, isMusic, type Item, type ItemDetail } from "../lib/api";
  import MusicDetail from "./MusicDetail.svelte";
  import { go, nav, toast } from "../lib/nav.svelte";
  import { play, type PlayOptions } from "../lib/player.svelte";
  import PosterCard from "../components/PosterCard.svelte";
  import Row from "../components/Row.svelte";
  import Icon from "../components/Icon.svelte";
  import TitleLogo from "../components/TitleLogo.svelte";
  import { t, locale } from "../lib/i18n";
  import { device } from "../lib/device";

  let { ratingKey }: { ratingKey: string } = $props();

  let item = $state<ItemDetail | null>(null);
  let children = $state<Item[]>([]);
  let error = $state<string | null>(null);
  let audioStream = $state<number | null>(null);
  let subtitleStream = $state<number>(0);
  // Version choisie (4K, 1080p…) : ses propres pistes audio et sous-titres.
  let version = $state(0);
  const media = $derived(item ? (item.versions?.[version] ?? item) : null);

  /** Pistes par défaut d'une version : celles retenues par le serveur. */
  function pickTracks(v: { audioTracks: { id: number; selected: boolean }[]; subtitleTracks: { id: number; selected: boolean }[] }) {
    audioStream = v.audioTracks.find((tr) => tr.selected)?.id ?? v.audioTracks[0]?.id ?? null;
    subtitleStream = v.subtitleTracks.find((tr) => tr.selected)?.id ?? 0;
  }

  function chooseVersion(e: Event) {
    version = Number((e.target as HTMLSelectElement).value);
    if (media) pickTracks(media);
  }
  let related = $state<Item[]>([]);
  let summaryOpen = $state(false);
  let root = $state<HTMLElement>();
  let fade = $state(0);

  async function load() {
    try {
      const detail = await api.metadata(ratingKey);
      item = detail;
      if (version >= (detail.versions?.length ?? 1)) version = 0;
      pickTracks(detail.versions?.[version] ?? detail);
      if (detail.kind === "movie" || detail.kind === "show") api.related(ratingKey).then((r) => (related = r), () => {});
      children =
        detail.kind === "show" || detail.kind === "season"
          ? await api.children(ratingKey)
          : detail.kind === "collection"
            ? await api.collectionItems(ratingKey)
            : [];
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
    start(item!.ratingKey, { audioStream, subtitleStream, fromStart, mediaIndex: version });
  }

  async function toggleWatched() {
    if (!item) return;
    try {
      await api.setWatched(item.ratingKey, !item.watched);
      await load();
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  // Pour une saison : premier épisode non vu.
  // Pour une saison : premier épisode non vu ; pour une collection : premier film non vu (ordre de sortie).
  const nextUp = $derived(
    item?.kind === "season" || item?.kind === "collection" ? (children.find((c) => !c.watched) ?? children[0]) : undefined,
  );
  const collection = $derived(item?.kind === "collection");
  const progress = $derived(
    item?.viewOffsetMs && item.durationMs ? Math.min(100, (item.viewOffsetMs / item.durationMs) * 100) : 0,
  );
  const remaining = $derived(item?.viewOffsetMs && item.durationMs ? item.durationMs - item.viewOffsetMs : null);
  const releaseDate = $derived(
    item?.releaseDate
      ? new Date(item.releaseDate).toLocaleDateString(locale, { day: "numeric", month: "long", year: "numeric" })
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
    if (!item?.playable || !item.durationMs) return null;
    const end = new Date(Date.now() + (item.durationMs - (item.viewOffsetMs ?? 0)));
    return end.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
  });
  const hasCredits = $derived(!!item && (item.directors.length + item.writers.length + item.genres.length > 0 || !!item.studio));
  const hasMedia = $derived(!!item && (!!item.video || item.audioTracks.length > 0 || item.playable));

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
        {#if item.showKey && (item.kind === "episode" || item.kind === "season")}
          <!-- Épisode ou saison : le logo de la série, qui y ramène, puis l'épisode ou la saison. -->
          <h1>
            <button class="show-logo" tabindex={device.tv ? -1 : 0} onclick={() => go({ name: "item", ratingKey: item!.showKey! })} title={item.showTitle ?? ""}>
              <TitleLogo logo={null} guid={item.logoGuid} genres={item.genres} title={item.showTitle ?? item.title} />
            </button>
          </h1>
          <div class="line">
            {#if item.kind === "season"}
              {item.title}
            {:else if episodeTitle(item.title)}
              {t("hero.episodeLine", { season: item.parentIndex ?? "", episode: item.index ?? "", title: episodeTitle(item.title) ?? "" })}
            {:else}
              {t("item.seasonEpisode", { season: item.parentIndex ?? "", episode: item.index ?? "" })}
            {/if}
          </div>
        {:else}
          <h1><TitleLogo logo={item.logo} guid={item.logoGuid} genres={item.genres} title={item.title} /></h1>
          {#if item.originalTitle}<div class="line original">{item.originalTitle}</div>{/if}
        {/if}

        <div class="meta">
          {#if releaseDate}<span>{releaseDate}</span>{:else if item.year}<span>{item.year}</span>{/if}
          {#if item.durationMs && item.kind !== "show" && item.kind !== "season"}
            <span>{formatDuration(item.durationMs)}</span>
          {/if}
          {#if item.kind === "show" || item.kind === "season"}
            <span>{t(item.kind === "show" ? "item.seasonCount" : "item.episodeCount", { count: children.length })}</span>
          {:else if collection}
            <span>{t("item.movieCount", { count: children.length })}</span>
          {/if}
          {#if item.contentRating}<span class="badge outline">{item.contentRating}</span>{/if}
          {#if item.rating}<span class="rating"><Icon name="star" size={14} filled /> {item.rating.toFixed(1)}</span>{/if}
          {#if endsAt}<span>{t("item.endsAt", { time: endsAt })}</span>{/if}
          {#if item.watched}<span class="badge ok">{t("item.watched")}</span>{/if}
        </div>

        <div class="actions" data-nav-row>
          {#if item.playable}
            <button class="btn primary big" onclick={() => playThis()}>
              <Icon name="play" size={18} filled />
              {item.viewOffsetMs ? t("item.resume") : t("item.play")}
            </button>
            {#if item.viewOffsetMs}
              <button class="btn round big" onclick={() => playThis(true)} title={t("item.playFromStart")}>
                <Icon name="restart" size={18} />
              </button>
            {/if}
          {:else if nextUp}
            <button class="btn primary big" onclick={() => start(nextUp!.ratingKey)}>
              <Icon name="play" size={18} filled />
              {collection ? t("item.playTitle", { title: nextUp.title }) : t("item.episodeNumber", { number: nextUp.index ?? "" })}
            </button>
          {/if}
          {#if !collection && (item.kind === "movie" || item.kind === "show")}
            <button class="btn big" disabled={trailerBusy} onclick={openTrailer} title={t("item.trailer")}>
              <Icon name="film" size={18} />
              {t("item.trailer")}
            </button>
          {/if}
          {#if !collection}
          <button
            class="btn round big"
            class:on={item.watched}
            onclick={toggleWatched}
            title={item.watched ? t("item.markUnwatched") : t("item.markWatched")}
          >
            <Icon name="checkCircle" size={20} />
          </button>
          {#if nav.admin}
            <button class="btn round big" onclick={() => (nav.editing = item)} title={t("item.editInfo")}>
              <Icon name="edit" size={18} />
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
    {#if item.tagline || item.summary}
      <section class="about">
        {#if item.tagline && item.kind !== "episode"}<p class="tagline">{item.tagline}</p>{/if}
        {#if item.summary}
          <!-- Sur téléviseur, le résumé n'est pas une étape de la navigation. -->
          <button class="summary" class:open={summaryOpen} tabindex={device.tv ? -1 : 0} onclick={() => (summaryOpen = !summaryOpen)}>
            {item.summary}
          </button>
        {/if}
      </section>
    {/if}

    <!-- Deux panneaux, comme Jellyfin : l'équipe à gauche, les pistes à lire à droite. -->
    {#if hasCredits || hasMedia}
      <div class="panels">
        {#if hasCredits}
          <dl class="panel credits">
            {#if item.directors.length}<div><dt>{t("item.facts.director")}</dt><dd>{item.directors.join(", ")}</dd></div>{/if}
            {#if item.writers.length}<div><dt>{t("item.facts.writer")}</dt><dd>{item.writers.slice(0, 3).join(", ")}</dd></div>{/if}
            {#if item.studio}<div><dt>{t("item.facts.studio")}</dt><dd>{item.studio}</dd></div>{/if}
            {#if item.genres.length}
              <div><dt>{t("item.facts.genres")}</dt><dd class="genres">{#each item.genres as g}<span>{g}</span>{/each}</dd></div>
            {/if}
          </dl>
        {/if}
        {#if hasMedia}
          <div class="panel media">
            <!-- Plusieurs versions (4K, 1080p…) : la ligne vidéo devient un choix, comme dans Plex. -->
            {#if item.versions?.length > 1}
              <label class="track" data-nav-row>
                <span class="ico" title={t("item.facts.version")}><Icon name="film" size={18} /></span>
                <select value={version} onchange={chooseVersion} aria-label={t("item.facts.version")}>
                  {#each item.versions as v, i (i)}<option value={i}>{v.label}</option>{/each}
                </select>
              </label>
            {:else if media?.video}
              <div class="track"><span class="ico" title={t("item.facts.video")}><Icon name="film" size={18} /></span><span class="video">{media.video}</span></div>
            {/if}
            {#if media?.audioTracks.length}
              <label class="track" data-nav-row>
                <span class="ico" title={t("item.facts.audio")}><Icon name="audio" size={18} /></span>
                <select bind:value={audioStream} aria-label={t("item.facts.audio")}>
                  {#each media.audioTracks as tr (tr.id)}<option value={tr.id}>{tr.label}</option>{/each}
                </select>
              </label>
            {/if}
            {#if item.playable}
              <label class="track" data-nav-row>
                <span class="ico" title={t("item.facts.subtitles")}><Icon name="subtitles" size={18} /></span>
                <select bind:value={subtitleStream} aria-label={t("item.facts.subtitles")}>
                  <option value={0}>{t("item.facts.none")}</option>
                  {#each media?.subtitleTracks ?? [] as tr (tr.id)}<option value={tr.id}>{tr.label}</option>{/each}
                </select>
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
    {:else if item.kind === "season"}
      <h2 class="row-title">{t("item.episodes")}</h2>
      <div class="episodes">
        {#each children as ep (ep.ratingKey)}
          <div class="episode" data-nav-row>
            <button class="thumb" onclick={() => start(ep.ratingKey)} aria-label={t("item.playTitle", { title: ep.title })}>
              {#if ep.thumb}<img src={ep.thumb} alt="" loading="lazy" />{/if}
              <span class="play"><Icon name="play" size={20} filled /></span>
              {#if ep.viewOffsetMs && ep.durationMs}
                <div class="progress"><div style:width="{(ep.viewOffsetMs / ep.durationMs) * 100}%"></div></div>
              {/if}
            </button>
            <div class="ep-body">
              <button class="ep-title" onclick={() => go({ name: "item", ratingKey: ep.ratingKey })}>
                <span class="ep-num">{ep.index}</span>
                {ep.title}
                {#if ep.watched}<span class="badge ok">{t("item.watched")}</span>{/if}
              </button>
              <div class="ep-meta">{formatDuration(ep.durationMs)}</div>
              {#if ep.summary}<p class="ep-summary">{ep.summary}</p>{/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}

    {#if item.cast.length}
      <h2 class="row-title">{t("item.cast")}</h2>
      <div class="cast" data-nav-row>
        {#each item.cast as person (person.name + person.role)}
          <!-- Comme dans Plex : un acteur ouvre la liste de ses films et séries du serveur. -->
          <button
            class="person"
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
    inset: 0 0 0 var(--sidebar-w, 237px);
    z-index: 0;
    overflow: hidden;
    background: var(--bg);
    pointer-events: none;
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

  .track select {
    flex: 1;
    min-width: 0;
    padding: 8px 10px;
    color: var(--text);
    font-weight: 500;
  }

  .episodes {
    display: grid;
    gap: 6px;
  }

  .episode {
    display: grid;
    grid-template-columns: 260px 1fr;
    gap: 20px;
    padding: 10px;
    border-radius: var(--radius-lg);
    transition: background 0.15s;
  }

  .episode:hover {
    background: #ffffff0a;
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

  .thumb:hover .play {
    opacity: 1;
  }

  .ep-body {
    min-width: 0;
    padding-top: 4px;
  }

  .ep-title {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0;
    border: none;
    background: none;
    font-size: 16px;
    font-weight: 600;
    text-align: left;
  }

  .ep-title:hover {
    color: var(--accent);
  }

  .ep-num {
    color: var(--text-faint);
  }

  .ep-meta {
    margin-top: 2px;
    color: var(--text-dim);
    font-size: 13px;
  }

  .ep-summary {
    display: -webkit-box;
    margin: 8px 0 0;
    overflow: hidden;
    color: var(--text-dim);
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
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

  .person:not(:disabled):hover .avatar {
    transform: scale(1.05);
    box-shadow: 0 0 0 3px var(--accent);
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
    padding: 6px 0;
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
