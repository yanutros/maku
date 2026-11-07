<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { api, errorText, seerrStatusLabel, type SeerrDetail, type SeerrMedia } from "../lib/api";
  import { go, nav, toast } from "../lib/nav.svelte";
  import SeerrCard from "../components/SeerrCard.svelte";
  import Row from "../components/Row.svelte";
  import Icon from "../components/Icon.svelte";
  import { locale, t } from "../lib/i18n";
  import { device } from "../lib/device";

  let { mediaType, tmdbId }: { mediaType: "movie" | "tv"; tmdbId: number } = $props();

  let item = $state<SeerrDetail | null>(null);
  let recommendations = $state<SeerrMedia[]>([]);
  let similar = $state<SeerrMedia[]>([]);
  let error = $state<string | null>(null);

  const STATUS: Record<string, string> = {
    Released: t("seerr.productionStatus.released"),
    "Post Production": t("seerr.productionStatus.postProduction"),
    "In Production": t("seerr.productionStatus.inProduction"),
    Planned: t("seerr.productionStatus.planned"),
    Rumored: t("seerr.productionStatus.rumored"),
    Canceled: t("seerr.productionStatus.canceled"),
    Ended: t("seerr.productionStatus.ended"),
    "Returning Series": t("seerr.productionStatus.returningSeries"),
    Pilot: t("seerr.productionStatus.pilot"),
  };
  const languages = new Intl.DisplayNames([locale], { type: "language" });
  const money = new Intl.NumberFormat(locale, { style: "currency", currency: "USD", maximumFractionDigits: 0 });

  const label = $derived(item ? seerrStatusLabel(item.status) : null);
  const requestable = $derived(
    !!item && (item.status === 0 || item.status === 1 || (item.mediaType === "tv" && item.status === 4)),
  );
  const releaseDate = $derived(
    item?.releaseDate
      ? new Date(item.releaseDate).toLocaleDateString(locale, { day: "numeric", month: "long", year: "numeric" })
      : null,
  );

  /** Acteur : ses autres films et séries (TMDB, par Seerr). */
  function openPerson(p: { id: number | null; name: string; photo: string | null }) {
    if (p.id !== null) go({ name: "person", id: String(p.id), personName: p.name, photo: p.photo, seerr: true });
  }

  function language(code: string | null): string | null {
    if (!code) return null;
    try {
      const name = languages.of(code);
      return name ? name[0].toUpperCase() + name.slice(1) : code;
    } catch {
      return code;
    }
  }

  async function load() {
    try {
      item = await api.seerrDetails(mediaType, tmdbId);
    } catch (e) {
      error = errorText(e);
    }
  }

  function openLink(url: string) {
    invoke("open_url", { url }).catch((e) => toast(errorText(e), true));
  }

  // Après une demande, la fiche se recharge pour afficher le nouveau statut.
  let requested = false;
  $effect(() => {
    if (nav.requesting) requested = true;
    else if (requested) {
      requested = false;
      load();
    }
  });

  onMount(() => {
    load();
    api.seerrRelated(mediaType, tmdbId, "recommendations").then((r) => (recommendations = r), () => {});
    api.seerrRelated(mediaType, tmdbId, "similar").then((r) => (similar = r), () => {});
  });
</script>

{#if error}
  <div class="page"><p class="error">{error}</p></div>
{:else if !item}
  <div class="page"><div class="skeleton" style:height="360px"></div></div>
{:else}
  <div class="backdrop" style:background-image={item.backdrop ? `url("${item.backdrop}")` : undefined}></div>

  <!-- Télécommande : la fiche se parcourt rangée par rangée, comme celles de la bibliothèque (boutons, distribution,
       recommandations, titres similaires) : la page descend jusqu'en bas. -->
  <div class="page detail" data-nav-rows>
    <div class="head">
      <div class="poster">{#if item.poster}<img src={item.poster} alt="" />{/if}</div>
      <div class="info">
        {#if label}
          <span class="badge" class:ok={item.status === 5} class:info={item.status !== 5}>{label}</span>
        {/if}
        <h1>{item.title} {#if item.year}<span class="year">({item.year})</span>{/if}</h1>
        <div class="meta">
          {#if item.certification}<span class="badge outline">{item.certification}</span>{/if}
          {#if item.runtime}<span>{t("seerr.detail.runtime", { minutes: item.runtime })}</span>{/if}
          {#if item.mediaType === "tv" && item.seasons}
            <span>{t("seerr.detail.seasons", { count: item.seasons })}</span>
          {/if}
          {#if item.genres.length}<span>{item.genres.join(", ")}</span>{/if}
        </div>
        <div class="actions" data-nav-row>
          {#if item.plexRatingKey && item.status >= 4}
            <button class="btn primary big" onclick={() => go({ name: "item", ratingKey: item!.plexRatingKey! })}>
              <Icon name="play" size={18} filled /> {t("seerr.detail.watchInPlexr")}
            </button>
          {/if}
          {#if requestable}
            <button class="btn big" class:primary={!item.plexRatingKey} onclick={() => (nav.requesting = item)}>
              <Icon name="plus" size={18} /> {item.status === 4 ? t("seerr.detail.requestMoreSeasons") : t("seerr.detail.request")}
            </button>
          {/if}
          {#if item.trailer}
            <button class="btn big" onclick={() => openLink(item!.trailer!)}>
              <Icon name="film" size={18} /> {t("seerr.detail.trailer")}
            </button>
          {/if}
        </div>
      </div>
    </div>

    <div class="body">
      <div class="main">
        {#if item.tagline}<p class="tagline">{item.tagline}</p>{/if}
        <h2 class="section-title">{t("seerr.detail.overview")}</h2>
        <p class="overview">{item.overview ?? t("seerr.detail.noOverview")}</p>

        {#if item.crew.length}
          <div class="crew">
            {#each item.crew as p, i (i)}
              <div><div class="job">{p.role}</div><div>{p.name}</div></div>
            {/each}
          </div>
        {/if}

        {#if item.keywords.length}
          <div class="keywords">{#each item.keywords as k (k)}<span>{k}</span>{/each}</div>
        {/if}
      </div>

      <aside class="facts">
        {#if item.collection}<div class="collection">{item.collection}</div>{/if}
        {#if item.ratings.rtCritics !== null || item.ratings.imdb !== null || item.ratings.tmdb}
          <div class="ratings">
            {#if item.ratings.rtCritics !== null}
              <button class="rating" tabindex={device.tv ? -1 : undefined} title={t("seerr.detail.rtCritics")} onclick={() => item!.ratings.rtUrl && openLink(item!.ratings.rtUrl)}>
                <span class="logo rt">RT</span>{t("seerr.detail.percent", { value: item.ratings.rtCritics })}
              </button>
            {/if}
            {#if item.ratings.rtAudience !== null}
              <button class="rating" tabindex={device.tv ? -1 : undefined} title={t("seerr.detail.rtAudience")} onclick={() => item!.ratings.rtUrl && openLink(item!.ratings.rtUrl)}>
                <span class="logo pop">🍿</span>{t("seerr.detail.percent", { value: item.ratings.rtAudience })}
              </button>
            {/if}
            {#if item.ratings.imdb !== null}
              <button class="rating" tabindex={device.tv ? -1 : undefined} title="IMDb" onclick={() => item!.ratings.imdbUrl && openLink(item!.ratings.imdbUrl)}>
                <span class="logo imdb">IMDb</span>{item.ratings.imdb.toFixed(1)}
              </button>
            {/if}
            {#if item.ratings.tmdb}
              <button class="rating" tabindex={device.tv ? -1 : undefined} title="TMDB" onclick={() => openLink(item!.tmdbUrl)}>
                <span class="logo tmdb">TMDB</span>{t("seerr.detail.percent", { value: Math.round(item.ratings.tmdb * 10) })}
              </button>
            {/if}
          </div>
        {/if}
        <dl>
          {#if item.originalTitle && item.originalTitle !== item.title}<dt>{t("seerr.facts.originalTitle")}</dt><dd>{item.originalTitle}</dd>{/if}
          {#if item.productionStatus}<dt>{t("seerr.facts.status")}</dt><dd>{STATUS[item.productionStatus] ?? item.productionStatus}</dd>{/if}
          {#if releaseDate}<dt>{item.mediaType === "tv" ? t("seerr.facts.firstAired") : t("seerr.facts.releaseDate")}</dt><dd>{releaseDate}</dd>{/if}
          {#if item.mediaType === "tv" && item.episodes}<dt>{t("seerr.facts.episodes")}</dt><dd>{item.episodes}</dd>{/if}
          {#if item.revenue}<dt>{t("seerr.facts.revenue")}</dt><dd>{money.format(item.revenue)}</dd>{/if}
          {#if item.budget}<dt>{t("seerr.facts.budget")}</dt><dd>{money.format(item.budget)}</dd>{/if}
          {#if item.originalLanguage}<dt>{t("seerr.facts.originalLanguage")}</dt><dd>{language(item.originalLanguage)}</dd>{/if}
          {#if item.countries.length}<dt>{t("seerr.facts.countries")}</dt><dd>{item.countries.join(", ")}</dd>{/if}
          {#if item.studios.length}
            <dt>{item.mediaType === "tv" ? t("seerr.facts.networks") : t("seerr.facts.studios")}</dt>
            <dd>{#each item.studios.slice(0, 5) as s (s)}<div>{s}</div>{/each}</dd>
          {/if}
        </dl>
        {#if item.providers.length}
          <div class="providers">
            <div class="providers-title">{t("seerr.facts.streamingOn")}</div>
            <div class="logos">
              {#each item.providers as p (p.name)}<img src={p.logo} alt={p.name} title={p.name} />{/each}
            </div>
          </div>
        {/if}
      </aside>
    </div>

    {#if item.cast.length}
      <Row title={t("seerr.detail.cast")}>
        {#each item.cast as p, i (i)}
          <!-- Sur téléviseur, chaque acteur est un arrêt de la télécommande : la rangée se parcourt. -->
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div
            class="person"
            class:link={p.id !== null}
            role={p.id !== null ? "button" : undefined}
            tabindex={device.tv || p.id !== null ? 0 : undefined}
            data-key="person-{p.id ?? p.name}"
            onclick={() => openPerson(p)}
            onkeydown={(e) => e.key === "Enter" && openPerson(p)}
          >
            <div class="avatar">{#if p.photo}<img src={p.photo} alt="" loading="lazy" />{:else}{p.name[0]}{/if}</div>
            <div class="p-name">{p.name}</div>
            {#if p.role}<div class="p-role">{p.role}</div>{/if}
          </div>
        {/each}
      </Row>
    {/if}

    {#if recommendations.length}
      <Row title={t("seerr.detail.recommendations")}>
        {#each recommendations as m (m.mediaType + m.tmdbId)}<SeerrCard media={m} />{/each}
      </Row>
    {/if}

    {#if similar.length}
      <Row title={t("seerr.detail.similar")}>
        {#each similar as m (m.mediaType + m.tmdbId)}<SeerrCard media={m} />{/each}
      </Row>
    {/if}
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    /* Jusqu'au bord gauche de la fenêtre, derrière le menu (translucide sur ordinateur, replié ou en pilule sur TV). */
    inset: 0;
    z-index: 0;
    background-size: cover;
    background-position: center top;
    opacity: 0.35;
    mask-image: linear-gradient(#000 0%, #0008 40%, transparent 75%);
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

  .detail {
    position: relative;
    z-index: 1;
    padding-top: 24px;
  }

  .head {
    display: flex;
    align-items: flex-end;
    gap: 32px;
  }

  .poster {
    flex: none;
    width: 220px;
    aspect-ratio: 2 / 3;
    overflow: hidden;
    border-radius: var(--radius-lg);
    background: var(--bg-elev);
    box-shadow: 0 20px 50px #000c;
  }

  .poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .info {
    min-width: 0;
  }

  h1 {
    margin: 8px 0 6px;
    font-size: 38px;
    line-height: 1.1;
  }

  .year {
    color: var(--text-dim);
    font-weight: 500;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    color: var(--text-dim);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 20px;
  }

  .big {
    height: 44px;
    padding: 0 20px;
  }

  .body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    gap: 40px;
    margin-top: 32px;
  }

  .tagline {
    margin: 0 0 18px;
    color: var(--text-dim);
    font-size: 18px;
    font-style: italic;
  }

  .section-title {
    margin: 0 0 8px;
    font-size: 19px;
  }

  .overview {
    margin: 0;
    font-size: 15px;
    line-height: 1.65;
    color: #e4e5e8;
  }

  .crew {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 16px;
    margin-top: 24px;
  }

  .job {
    color: var(--text-faint);
    font-size: 13px;
    font-weight: 600;
  }

  .keywords {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 24px;
  }

  .keywords span {
    padding: 3px 10px;
    border: 1px solid var(--border);
    border-radius: 12px;
    color: var(--text-dim);
    font-size: 12px;
  }

  .facts {
    align-self: start;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: #1d1f23cc;
    backdrop-filter: blur(10px);
  }

  .collection {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    font-weight: 600;
  }

  .ratings {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-around;
    gap: 8px;
    padding: 12px;
    border-bottom: 1px solid var(--border);
  }

  .rating {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 4px;
    border: none;
    background: none;
    font-weight: 600;
  }

  .logo {
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 10px;
    font-weight: 800;
  }

  .rt {
    background: #fa320a;
    color: #fff;
  }

  .pop {
    padding: 0;
    font-size: 14px;
  }

  .imdb {
    background: #f5c518;
    color: #000;
  }

  .tmdb {
    background: #01b4e4;
    color: #04203a;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    margin: 0;
  }

  dt,
  dd {
    margin: 0;
    padding: 9px 16px;
    border-bottom: 1px solid var(--border);
    font-size: 13px;
  }

  dt {
    color: var(--text-dim);
    font-weight: 600;
  }

  dd {
    text-align: right;
  }

  .providers {
    padding: 12px 16px;
  }

  .providers-title {
    margin-bottom: 8px;
    font-size: 13px;
    font-weight: 600;
  }

  .logos {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .logos img {
    width: 34px;
    height: 34px;
    border-radius: 8px;
  }

  .person {
    text-align: center;
  }

  /* Télécommande : cercle doré autour de la photo et nom en couleur, comme sur les fiches de la bibliothèque. */
  :global(html.tv) .person:focus {
    outline: none;
  }

  :global(html.tv) .person:focus .avatar {
    transform: scale(1.06);
    box-shadow: 0 0 0 4px var(--accent);
  }

  :global(html.tv) .person:focus .p-name {
    color: var(--accent);
  }

  .avatar {
    display: grid;
    place-items: center;
    width: 120px;
    height: 120px;
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

  .p-name {
    font-size: 13px;
    font-weight: 600;
  }

  .p-role {
    color: var(--text-dim);
    font-size: 12px;
  }

  @media (max-width: 1100px) {
    .body {
      grid-template-columns: 1fr;
    }
  }

  /* ---------- Téléphone : tout s'empile, en pleine largeur ---------- */

  :global(html.phone) .head {
    flex-direction: column;
    align-items: stretch;
    gap: 18px;
  }

  :global(html.phone) .poster {
    align-self: center;
    width: 46%;
  }

  :global(html.phone) h1 {
    font-size: 26px;
  }

  :global(html.phone) .actions .btn {
    flex: 1 1 auto;
  }

  :global(html.phone) .body {
    grid-template-columns: minmax(0, 1fr);
    gap: 24px;
    margin-top: 24px;
  }

  :global(html.phone) .crew {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
</style>
