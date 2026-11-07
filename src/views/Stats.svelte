<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, errorText, formatClock, formatLongDuration, tracearrLogin, type Session, type TracearrDashboard } from "../lib/api";
  import { genreLabel } from "../lib/genres";
  import { go } from "../lib/nav.svelte";
  import { device } from "../lib/device";
  import { useAmbient } from "../lib/ambient.svelte";
  import Icon from "../components/Icon.svelte";
  import { t, locale } from "../lib/i18n";

  let { session, onChange }: { session: Session; onChange: () => void } = $props();

  let data = $state<TracearrDashboard | null>(null);
  let error = $state<string | null>(null);
  let needsLogin = $state(false);
  let connecting = $state(false);
  let timer: ReturnType<typeof setInterval> | undefined;

  async function load() {
    try {
      data = await api.tracearrDashboard();
      needsLogin = false;
      error = null;
    } catch (e) {
      const message = errorText(e);
      if (message.includes("SESSION_EXPIRED")) {
        needsLogin = true;
        onChange();
      } else {
        error = message;
      }
    }
  }

  async function connect() {
    connecting = true;
    error = null;
    try {
      await tracearrLogin();
      onChange();
      await load();
    } catch (e) {
      error = errorText(e);
    } finally {
      connecting = false;
    }
  }

  // Variation par rapport aux 30 jours précédents.
  function delta(current: number, previous: number): number | null {
    if (!previous) return null;
    return Math.round(((current - previous) / previous) * 100);
  }

  function longDate(iso: string): string {
    return new Date(iso).toLocaleDateString(locale, { day: "numeric", month: "long", year: "numeric" });
  }

  // « il y a 1 an et 10 mois ».
  function since(iso: string): string {
    const months = Math.max(0, Math.round((Date.now() - new Date(iso).getTime()) / (30.44 * 86_400_000)));
    const y = Math.floor(months / 12);
    const m = months % 12;
    const yPart = y ? t("stats.since.years", { count: y }) : "";
    const mPart = m ? t("stats.since.months", { count: m }) : "";
    const time = yPart && mPart ? t("stats.since.yearsAndMonths", { years: yPart, months: mPart }) : yPart || mPart;
    return time ? t("stats.since.ago", { time }) : t("stats.since.thisMonth");
  }

  const hours = (ms: number) => Math.round(ms / 3_600_000).toLocaleString(locale);

  const kpis = $derived(
    data
      ? [
          {
            icon: "clock" as const,
            label: t("stats.kpi.screenTime"),
            value: formatLongDuration(data.month.watchMs),
            hint: data.month.listenMs ? t("stats.kpi.musicPart", { time: formatLongDuration(data.month.listenMs) }) : null,
            delta: delta(data.month.watchMs, data.previous.watchMs),
          },
          {
            icon: "play" as const,
            label: t("stats.kpi.plays"),
            value: data.month.plays.toLocaleString(locale),
            hint: null,
            delta: delta(data.month.plays, data.previous.plays),
          },
          {
            icon: "film" as const,
            label: t("stats.kpi.titles"),
            value: data.month.titles.toLocaleString(locale),
            hint: null,
            delta: delta(data.month.titles, data.previous.titles),
          },
        ]
      : [],
  );

  // Fond d'ambiance : l'affiche du titre le plus regardé.
  const ambient = $derived(data?.topTitles[0]?.poster ?? data?.bingeArt?.backdrop ?? null);
  // Téléviseur : la page glisse quand le menu se déplie ; le fond, lui, reste derrière toute la fenêtre.
  $effect(() => (device.tv ? useAmbient(ambient) : undefined));
  const genres = $derived(data?.allTime.topGenres?.slice(0, 5) ?? []);
  const maxGenre = $derived(Math.max(1, ...genres.map((g) => g.plays)));

  onMount(() => {
    if (session.tracearrUrl && session.tracearrConnected) {
      load();
      // La lecture en cours se rafraîchit toutes les 15 s.
      timer = setInterval(load, 15000);
    } else {
      needsLogin = !!session.tracearrUrl;
    }
  });

  onDestroy(() => clearInterval(timer));
</script>

{#if !session.tracearrUrl || needsLogin || (error && !data) || !data}
  <div class="page">
    <h1 class="row-title page-title">{t("stats.title")}</h1>
    {#if !session.tracearrUrl}
      <div class="empty">
        <p>{t("stats.noUrl")}</p>
        <button class="btn primary" onclick={() => go({ name: "settings", page: "services" })}>{t("stats.openSettings")}</button>
      </div>
    {:else if needsLogin}
      <div class="empty">
        <p>{t("stats.loginPrompt")}</p>
        <button class="btn primary" disabled={connecting} onclick={connect}>
          {connecting ? t("stats.waitingBrowser") : t("stats.loginWithPlex")}
        </button>
        {#if error}<p class="error">{error}</p>{/if}
      </div>
    {:else if error}
      <p class="error">{error}</p>
    {:else}
      <div class="skeleton" style:height="320px"></div>
    {/if}
  </div>
{:else}
  <!-- Bandeau d'ambiance : affiche du titre le plus regardé, très floutée. Elle est fixée à la fenêtre,
       sur toute sa largeur : elle passe derrière le menu translucide, comme dans « En cours de lecture ». -->
  <div class="backdrop" aria-hidden="true">
    {#if ambient}<div class="ambient" style:background-image={`url("${ambient}")`}></div>{/if}
  </div>
  <section class="hero">
    <div class="hero-body">
      <div class="eyebrow">{t("stats.hero.last30Days")}</div>
      <h1>{t("stats.hero.hello", { name: data.username ?? "" })}</h1>
      <p class="lead">
        {t("stats.hero.lead", { time: formatLongDuration(data.month.watchMs), count: data.month.titles })}
      </p>
      <div class="kpis">
        {#each kpis as k (k.label)}
          <div class="kpi">
            <div class="kpi-top">
              <span class="kpi-icon"><Icon name={k.icon} size={16} /></span>
              {#if k.delta !== null}
                <span class="pill" class:up={k.delta >= 0}>
                  {k.delta >= 0 ? "▲" : "▼"} {t("stats.kpi.percent", { value: Math.abs(k.delta) })}
                </span>
              {/if}
            </div>
            <div class="kpi-value">{k.value}</div>
            <div class="kpi-label">{k.label}{k.hint ? ` · ${k.hint}` : ""}</div>
          </div>
        {/each}
      </div>
    </div>
  </section>

  <div class="page content">
    {#if data.nowPlaying.length}
      <h2 class="section"><span class="live"></span>{t("stats.now.title")}</h2>
      <div class="now">
        {#each data.nowPlaying as s, i (i)}
          <div class="session">
            {#if s.poster}<div class="session-bg" style:background-image={`url("${s.poster}")`}></div>{/if}
            <div class="session-poster">{#if s.poster}<img src={s.poster} alt="" />{:else}<Icon name="music" size={28} />{/if}</div>
            <div class="session-body">
              <div class="s-title">{s.title}</div>
              {#if s.subtitle}<div class="s-sub">{s.subtitle}</div>{/if}
              <div class="bar"><div style:width="{s.durationMs ? (s.progressMs / s.durationMs) * 100 : 0}%"></div></div>
              <div class="s-meta">
                <span>{s.paused ? t("stats.now.paused") : t("stats.now.playing")} · {formatClock(s.progressMs / 1000)} / {formatClock(s.durationMs / 1000)}</span>
                <span>{[s.player, s.local ? t("stats.now.local") : t("stats.now.remote"), s.decision === "transcode" ? t("stats.now.transcode") : t("stats.now.direct")].filter(Boolean).join(" · ")}</span>
              </div>
            </div>
          </div>
        {/each}
      </div>
    {/if}

    {#if data.topTitles.length}
      <h2 class="section">{t("stats.top.title")} <span class="muted">{t("stats.top.period")}</span></h2>
      <div class="top">
        {#each data.topTitles as top (top.rank + top.title)}
          <button
            class="chart"
            disabled={!top.ratingKey}
            onclick={() => top.ratingKey && go({ name: "item", ratingKey: top.ratingKey })}
          >
            <span class="rank">{top.rank}</span>
            <span class="chart-poster">{#if top.poster}<img src={top.poster} alt="" loading="lazy" />{/if}</span>
            <span class="chart-name">{top.title}</span>
            <span class="chart-sub">
              {top.episodes ? t("stats.top.episodes", { count: top.episodes }) : t("stats.top.plays", { count: top.plays })}
              · {formatLongDuration(top.watchMs)}
            </span>
          </button>
        {/each}
      </div>
    {/if}

    <h2 class="section">{t("stats.allTime.title")}</h2>
    <div class="bento">
      {#if data.allTime.totalWatchTimeMs}
        <div class="tile total">
          <div class="tile-label"><Icon name="clock" size={16} /> {t("stats.allTime.watchTime")}</div>
          <div class="huge">{formatLongDuration(data.allTime.totalWatchTimeMs)}</div>
          <div class="tile-foot">
            {t("stats.allTime.hours", { hours: hours(data.allTime.totalWatchTimeMs) })}
            {#if data.allTime.daysWatched}· {t("stats.allTime.spreadOver", { count: data.allTime.daysWatched })}{/if}
          </div>
        </div>
      {/if}

      {#if data.allTime.longestStreak}
        <div class="tile streak">
          <div class="tile-label"><span class="flame">🔥</span> {t("stats.allTime.longestStreak")}</div>
          <div class="big">{data.allTime.longestStreak} <small>{t("stats.allTime.daysInARow")}</small></div>
          <div class="tile-foot">
            {#if data.allTime.currentStreak}
              <span class="pill up">{t("stats.allTime.currentStreak", { count: data.allTime.currentStreak })}</span>
            {:else}
              {t("stats.allTime.noStreak")}
            {/if}
          </div>
        </div>
      {/if}

      {#if genres.length}
        <div class="tile genres">
          <div class="tile-label"><Icon name="stats" size={16} /> {t("stats.allTime.favoriteGenres")}</div>
          <div class="bars">
            {#each genres as g, i (g.genre)}
              <div class="bar-row">
                <div class="bar-head"><span>{genreLabel(g.genre)}</span><span class="dim">{g.plays}</span></div>
                <div class="bar-track"><div class="bar-fill" style:width="{(g.plays / maxGenre) * 100}%" style:--i={i}></div></div>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      {#if data.allTime.biggestBinge}
        <button
          class="tile art binge"
          disabled={!data.bingeArt}
          onclick={() => data?.bingeArt && go({ name: "item", ratingKey: data.bingeArt.ratingKey })}
        >
          {#if data.bingeArt?.backdrop}<span class="art-bg" style:background-image={`url("${data.bingeArt.backdrop}")`}></span>{/if}
          <span class="art-shade"></span>
          <span class="art-body">
            <span class="tile-label">{t("stats.allTime.biggestBinge")}</span>
            <span class="big">{data.allTime.biggestBinge.episodes} <small>{t("stats.allTime.episodesInADay")}</small></span>
            <span class="art-title">{data.allTime.biggestBinge.showTitle}</span>
            <span class="tile-foot">{longDate(data.allTime.biggestBinge.day)}</span>
          </span>
        </button>
      {/if}

      {#if data.allTime.firstPlay}
        <button
          class="tile art first"
          disabled={!data.firstPlayArt}
          onclick={() => data?.firstPlayArt && go({ name: "item", ratingKey: data.firstPlayArt.ratingKey })}
        >
          {#if data.firstPlayArt?.backdrop}<span class="art-bg" style:background-image={`url("${data.firstPlayArt.backdrop}")`}></span>{/if}
          <span class="art-shade"></span>
          <span class="art-body">
            <span class="tile-label">{t("stats.allTime.firstPlay")}</span>
            <span class="art-title">{data.allTime.firstPlay.grandparentTitle ?? data.allTime.firstPlay.mediaTitle}</span>
            <span class="tile-foot">{longDate(data.allTime.firstPlay.startedAt)} · {since(data.allTime.firstPlay.startedAt)}</span>
          </span>
        </button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .empty {
    display: grid;
    justify-items: start;
    gap: 14px;
    max-width: 560px;
    padding: 28px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-elev);
  }

  .empty p {
    margin: 0;
  }

  /* ---------- Bandeau ---------- */

  .hero {
    position: relative;
    margin-top: -68px;
    padding: 120px 40px 36px;
  }

  /* Toute la fenêtre, menu compris, comme dans « En cours de lecture » : le menu translucide flotte par-dessus. */
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 0;
    overflow: hidden;
    pointer-events: none;
  }

  :global(html.tv) .backdrop {
    display: none;
  }

  /* Le contenu de la page passe au-dessus du fond fixé. */
  .backdrop ~ * {
    position: relative;
  }

  /* Même traitement que le fond d'« En cours de lecture » : uniforme sur toute la fenêtre. */
  .ambient {
    position: absolute;
    inset: -80px;
    background-size: cover;
    background-position: center;
    filter: blur(70px) saturate(1.5) brightness(0.45);
    transform: scale(1.1);
  }

  .hero-body {
    position: relative;
  }

  .eyebrow {
    color: var(--accent);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  h1 {
    margin: 6px 0 6px;
    font-size: 46px;
    font-weight: 800;
    letter-spacing: -0.03em;
    line-height: 1.05;
  }

  .lead {
    margin: 0;
    max-width: 640px;
    color: #ffffffc8;
    font-size: 17px;
  }

  .kpis {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 16px;
    margin-top: 28px;
  }

  .kpi {
    padding: 18px 20px;
    border: 1px solid #ffffff1c;
    border-radius: 20px;
    background: #ffffff0f;
    backdrop-filter: blur(24px) saturate(1.4);
    box-shadow: 0 10px 30px #0004;
  }

  .kpi-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .kpi-icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    color: var(--accent);
  }

  .kpi-value {
    margin-top: 14px;
    font-family: "Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif;
    font-size: 40px;
    font-weight: 800;
    letter-spacing: -0.03em;
    line-height: 1;
  }

  .kpi-label {
    margin-top: 6px;
    color: #ffffffa8;
    font-size: 13px;
  }

  .pill {
    padding: 3px 9px;
    border-radius: 20px;
    background: color-mix(in srgb, var(--danger) 18%, transparent);
    color: var(--danger);
    font-size: 12px;
    font-weight: 700;
  }

  .pill.up {
    background: color-mix(in srgb, var(--ok) 18%, transparent);
    color: var(--ok);
  }

  /* ---------- Contenu ---------- */

  .content {
    padding-top: 0;
  }

  .section {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin: 36px 0 16px;
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }

  .section .muted {
    font-size: 14px;
    font-weight: 500;
  }

  .live {
    align-self: center;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #ff453a;
    box-shadow: 0 0 0 0 #ff453a88;
    animation: pulse 1.6s infinite;
  }

  @keyframes pulse {
    70% {
      box-shadow: 0 0 0 10px #ff453a00;
    }
  }

  .now {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(460px, 1fr));
    gap: 16px;
  }

  .session {
    position: relative;
    display: flex;
    gap: 18px;
    padding: 16px;
    overflow: hidden;
    border: 1px solid #ffffff17;
    border-radius: 20px;
    background: var(--bg-elev);
  }

  .session-bg {
    position: absolute;
    inset: -40px;
    background-size: cover;
    background-position: center;
    filter: blur(40px) saturate(1.5);
    opacity: 0.35;
  }

  .session-poster {
    position: relative;
    display: grid;
    place-items: center;
    flex: none;
    width: 72px;
    aspect-ratio: 2 / 3;
    overflow: hidden;
    border-radius: 10px;
    background: #ffffff12;
    color: var(--text-faint);
  }

  .session-poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .session-body {
    position: relative;
    flex: 1;
    min-width: 0;
    align-self: center;
  }

  .s-title {
    font-size: 17px;
    font-weight: 700;
  }

  .s-sub {
    color: #ffffffb0;
    font-size: 13px;
  }

  .bar {
    height: 5px;
    margin: 14px 0 8px;
    overflow: hidden;
    border-radius: 3px;
    background: #ffffff1f;
  }

  .bar div {
    height: 100%;
    border-radius: 3px;
    background: var(--accent);
  }

  .s-meta {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 4px 12px;
    color: #ffffff99;
    font-size: 12px;
  }

  /* Top 10 : grand numéro en relief derrière chaque affiche. */
  .top {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 210px;
    gap: 8px;
    overflow-x: auto;
    padding: 6px 4px 16px;
    margin: -6px -4px 0;
  }

  .chart {
    position: relative;
    display: grid;
    padding: 0 0 0 58px;
    border: none;
    background: none;
    text-align: left;
  }

  .chart:disabled {
    cursor: default;
  }

  .rank {
    position: absolute;
    left: -4px;
    bottom: 40px;
    font-family: "Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif;
    font-size: 150px;
    font-weight: 900;
    line-height: 0.8;
    letter-spacing: -0.08em;
    color: var(--bg);
    /* Sur Android, la police du chiffre est faite de formes qui se chevauchent (3, 4, 6, 8) : le contour seul
       montrait leurs jointures. Le contour est peint d'abord et le chiffre plein par-dessus, qui les cache :
       il ne reste que le bord extérieur (d'où 6 px, dont la moitié visible). */
    -webkit-text-stroke: 6px #ffffff40;
    paint-order: stroke fill;
    transition: -webkit-text-stroke-color 0.2s;
  }

  :global(html:not(.tv)) .chart:not(:disabled):hover .rank {
    -webkit-text-stroke-color: var(--accent);
  }

  .chart-poster {
    position: relative;
    aspect-ratio: 2 / 3;
    overflow: hidden;
    border-radius: 12px;
    background: var(--bg-hover);
    box-shadow: -10px 10px 24px #000a;
    transition: transform 0.25s var(--ease);
  }

  :global(html:not(.tv)) .chart:not(:disabled):hover .chart-poster {
    transform: translateY(-4px);
  }

  /* Télécommande : comme pour les acteurs, la sélection entoure l'affiche seule (plus le numéro et le
     titre en couleur), plutôt qu'un cadre autour de tout le bloc. Le survol est réservé à la souris :
     Android laisse le titre ouvert « survolé » au retour. */
  :global(html.tv) .top {
    padding-top: 14px;
    margin-top: -14px;
  }

  :global(html.tv) .chart:focus {
    outline: none;
  }

  :global(html.tv) .chart:focus .chart-poster {
    outline: 3px solid var(--accent);
    outline-offset: 3px;
    transform: scale(1.04);
  }

  :global(html.tv) .chart:focus .rank {
    -webkit-text-stroke-color: var(--accent);
  }

  :global(html.tv) .chart:focus .chart-name {
    color: var(--accent);
  }

  .chart-poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .chart-name {
    margin-top: 10px;
    overflow: hidden;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chart-sub {
    color: var(--text-dim);
    font-size: 12px;
  }

  /* ---------- Depuis toujours : mosaïque de cartes ---------- */

  .bento {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    grid-auto-rows: 200px;
    gap: 16px;
  }

  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 20px 22px;
    overflow: hidden;
    border: 1px solid #ffffff14;
    border-radius: 22px;
    background: var(--bg-elev);
    text-align: left;
  }

  .tile-label {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #ffffffb8;
    font-size: 13px;
    font-weight: 600;
  }

  .tile-foot {
    color: #ffffffa0;
    font-size: 13px;
  }

  .huge {
    font-family: "Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif;
    font-size: 64px;
    font-weight: 900;
    letter-spacing: -0.04em;
    line-height: 1;
  }

  .big {
    font-family: "Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif;
    font-size: 48px;
    font-weight: 900;
    letter-spacing: -0.03em;
    line-height: 1;
  }

  .big small {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0;
    color: #ffffffc0;
  }

  .total {
    grid-column: span 2;
    background:
      radial-gradient(120% 140% at 100% 0%, color-mix(in srgb, var(--accent) 33%, transparent) 0%, transparent 55%),
      linear-gradient(135deg, #2a2216, #1a1b1f);
  }

  .total .huge {
    background: linear-gradient(90deg, #fff, #ffd27a);
    background-clip: text;
    -webkit-background-clip: text;
    color: transparent;
  }

  .streak {
    background:
      radial-gradient(120% 120% at 0% 100%, #ff6b3555 0%, transparent 60%),
      linear-gradient(160deg, #2a1a17, #1a1b1f);
  }

  .flame {
    font-size: 15px;
  }

  .genres {
    grid-row: span 2;
    justify-content: flex-start;
    gap: 18px;
    background: linear-gradient(180deg, #1f2230, #1a1b1f);
  }

  .bars {
    display: grid;
    gap: 16px;
  }

  .bar-head {
    display: flex;
    justify-content: space-between;
    margin-bottom: 6px;
    font-size: 14px;
    font-weight: 600;
  }

  .dim {
    color: var(--text-dim);
    font-weight: 500;
  }

  .bar-track {
    height: 8px;
    overflow: hidden;
    border-radius: 4px;
    background: #ffffff12;
  }

  .bar-fill {
    height: 100%;
    border-radius: 4px;
    background: linear-gradient(90deg, hsl(calc(220 + var(--i) * 28) 80% 62%), hsl(calc(250 + var(--i) * 28) 80% 70%));
    animation: grow 0.9s var(--ease) both;
    animation-delay: calc(var(--i) * 0.08s);
    transform-origin: left;
  }

  @keyframes grow {
    from {
      transform: scaleX(0);
    }
  }

  /* Cartes illustrées par la scène du titre. */
  .art {
    justify-content: flex-end;
    padding: 0;
    border: none;
  }

  .art:not(:disabled) {
    cursor: pointer;
  }

  .binge {
    grid-column: span 2;
  }

  .art-bg {
    position: absolute;
    inset: 0;
    background-size: cover;
    background-position: center 25%;
    transition: transform 0.6s var(--ease);
  }

  .art:not(:disabled):hover .art-bg {
    transform: scale(1.05);
  }

  .art-shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, #000000e6 0%, #00000080 45%, #00000030 100%);
  }

  .art-body {
    position: relative;
    display: grid;
    gap: 4px;
    padding: 20px 22px;
  }

  .art-title {
    font-size: 22px;
    font-weight: 800;
    letter-spacing: -0.01em;
    line-height: 1.15;
  }

  @media (max-width: 1250px) {
    .bento {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .genres {
      grid-row: span 1;
      grid-column: span 2;
    }
  }

  @media (max-width: 1000px) {
    .kpis {
      grid-template-columns: 1fr;
    }
  }

  /* ---------- Téléphone : l'en-tête est opaque, le bandeau reste en place ---------- */

  :global(html.phone) .hero {
    margin-top: 0;
    padding: 16px 16px 24px;
  }

  :global(html.phone) .now {
    grid-template-columns: minmax(0, 1fr);
  }

  :global(html.phone) .bento {
    grid-template-columns: minmax(0, 1fr);
    grid-auto-rows: auto;
  }

  :global(html.phone) .bento > :global(*) {
    grid-column: auto !important;
    grid-row: auto !important;
    min-height: 160px;
  }
</style>
