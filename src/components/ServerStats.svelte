<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, errorText, formatLongDuration, type Bandwidth, type MediaStats, type PlayHistory, type StatAccount } from "../lib/api";
  import { locale, t } from "../lib/i18n";
  import TrackSelect from "./TrackSelect.svelte";

  // Statistiques du serveur, comme le tableau de bord de Plex Web : débit (temps réel ou passé), top des
  // utilisateurs, historique des lectures par type, et les titres les plus lus. Les couleurs suivent le type
  // (films, TV, musique, photos) ou la provenance (distant, local), jamais le rang.
  const KINDS = [
    { id: "movie", plex: 1, color: "#3987e5", label: t("dashboard.movies") },
    { id: "show", plex: 4, color: "#d95926", label: t("dashboard.tv") },
    { id: "music", plex: 10, color: "#199e70", label: t("dashboard.music") },
    { id: "photo", plex: 13, color: "#c98500", label: t("dashboard.photos") },
  ] as const;
  const kindOf = (plex: number) => KINDS.find((k) => k.plex === plex)?.id ?? null;
  const REMOTE = "#3987e5";
  const LOCAL = "#d95926";

  const now = () => Math.floor(Date.now() / 1000);
  const DAY = 86400;

  /** Comptes connus (statistiques et historique), pour les noms, avatars et le choix d'utilisateur. */
  let accounts = $state<StatAccount[]>([]);
  function learn(list: StatAccount[]) {
    const known = new Map(accounts.map((a) => [a.id, a]));
    for (const a of list) if (!known.has(a.id) || (!known.get(a.id)!.thumb && a.thumb)) known.set(a.id, a);
    accounts = [...known.values()];
  }
  const nameOf = (id: number) => accounts.find((a) => a.id === id)?.name || t("dashboard.unknownUser");
  const thumbOf = (id: number) => accounts.find((a) => a.id === id)?.thumb ?? null;
  const userOptions = $derived([{ value: "all", label: t("dashboard.allUsers") }, ...accounts.filter((a) => a.name).map((a) => ({ value: String(a.id), label: a.name }))]);

  const fmtRate = (mbps: number) => (mbps >= 1 ? `${mbps.toFixed(mbps >= 10 ? 0 : 1)} Mb/s` : `${Math.round(mbps * 1000)} kb/s`);
  const fmtHours = (sec: number) => formatLongDuration(sec * 1000);

  // ---------------- Débit ----------------
  const BW = {
    live: { timespan: 6, span: 120, label: t("dashboard.live") },
    day: { timespan: 4, span: DAY, label: t("dashboard.last24h") },
    month: { timespan: 3, span: 30 * DAY, label: t("dashboard.last30d") },
  } as const;
  let bwRange = $state<keyof typeof BW>("live");
  let bwUser = $state("all");
  let bw = $state<Bandwidth | null>(null);
  let bwError = $state<string | null>(null);
  let bwTimer: ReturnType<typeof setInterval> | undefined;

  async function loadBandwidth() {
    const r = BW[bwRange];
    try {
      const data = await api.bandwidth(r.timespan, now() - r.span);
      bw = data;
      learn(data.accounts);
      bwError = null;
    } catch (e) {
      bwError = errorText(e);
    }
  }

  /** Points du graphique : débit distant et local (Mb/s) à chaque instant, pour l'utilisateur choisi. */
  const bwSeries = $derived.by(() => {
    if (!bw) return [];
    const byAt = new Map<number, { remote: number; local: number }>();
    for (const p of bw.points) {
      if (bwUser !== "all" && String(p.accountId) !== bwUser) continue;
      const b = byAt.get(p.at) ?? { remote: 0, local: 0 };
      if (p.lan) b.local += p.bytes;
      else b.remote += p.bytes;
      byAt.set(p.at, b);
    }
    const ats = [...byAt.keys()].sort((a, b) => a - b);
    // Durée d'un intervalle : le plus petit écart entre deux relevés (sinon celle de la période).
    let step = Infinity;
    for (let i = 1; i < ats.length; i++) step = Math.min(step, ats[i] - ats[i - 1]);
    if (!isFinite(step) || step <= 0) step = bwRange === "live" ? 6 : bwRange === "day" ? 3600 : DAY;
    return ats.map((at) => {
      const b = byAt.get(at)!;
      return { at, remote: (b.remote * 8) / step / 1e6, local: (b.local * 8) / step / 1e6 };
    });
  });
  const bwAverage = $derived.by(() => {
    const n = bwSeries.length || 1;
    return { remote: bwSeries.reduce((s, p) => s + p.remote, 0) / n, local: bwSeries.reduce((s, p) => s + p.local, 0) / n };
  });

  // ---------------- Top des utilisateurs, historique ----------------
  const PERIOD = {
    week: { span: 7 * DAY, timespan: 3, label: t("dashboard.last7d") },
    month: { span: 30 * DAY, timespan: 2, label: t("dashboard.last30d") },
    year: { span: 365 * DAY, timespan: 1, label: t("dashboard.last12m") },
  } as const;
  let topRange = $state<keyof typeof PERIOD>("week");
  let top = $state<MediaStats | null>(null);
  let topError = $state<string | null>(null);

  async function loadTop() {
    try {
      // Par jour : assez fin pour toutes les périodes, et le total se fait ici.
      const data = await api.mediaStats(3, now() - PERIOD[topRange].span);
      top = data;
      learn(data.accounts);
      topError = null;
    } catch (e) {
      topError = errorText(e);
    }
  }

  const topUsers = $derived.by(() => {
    const users = new Map<number, { id: number; count: number; duration: number; byKind: Record<string, number> }>();
    for (const e of top?.entries ?? []) {
      const kind = kindOf(e.kind);
      if (!kind) continue;
      const u = users.get(e.accountId) ?? { id: e.accountId, count: 0, duration: 0, byKind: {} };
      u.count += e.count;
      u.duration += e.duration;
      u.byKind[kind] = (u.byKind[kind] ?? 0) + e.duration;
      users.set(e.accountId, u);
    }
    return [...users.values()].sort((a, b) => b.duration - a.duration);
  });

  let histRange = $state<keyof typeof PERIOD>("month");
  let histUser = $state("all");
  let hist = $state<MediaStats | null>(null);
  let histError = $state<string | null>(null);

  async function loadHistory() {
    try {
      const data = await api.mediaStats(PERIOD[histRange].timespan, now() - PERIOD[histRange].span);
      hist = data;
      learn(data.accounts);
      histError = null;
    } catch (e) {
      histError = errorText(e);
    }
  }

  /** Une barre par jour, semaine ou mois : la durée de chaque type (secondes). */
  const histBars = $derived.by(() => {
    const byAt = new Map<number, Record<string, number>>();
    for (const e of hist?.entries ?? []) {
      const kind = kindOf(e.kind);
      if (!kind || (histUser !== "all" && String(e.accountId) !== histUser)) continue;
      const b = byAt.get(e.at) ?? {};
      b[kind] = (b[kind] ?? 0) + e.duration;
      byAt.set(e.at, b);
    }
    return [...byAt.entries()].sort((a, b) => a[0] - b[0]).map(([at, byKind]) => ({ at, byKind, total: Object.values(byKind).reduce((s, v) => s + v, 0) }));
  });
  const histTotals = $derived(KINDS.map((k) => ({ ...k, total: histBars.reduce((s, b) => s + (b.byKind[k.id] ?? 0), 0) })));

  // ---------------- Les plus lus ----------------
  let mostRange = $state<"week" | "month">("month");
  let mostUser = $state("all");
  let plays = $state<PlayHistory | null>(null);
  let mostError = $state<string | null>(null);

  async function loadMost() {
    try {
      const data = await api.history(now() - PERIOD[mostRange].span);
      plays = data;
      learn(data.accounts);
      mostError = null;
    } catch (e) {
      mostError = errorText(e);
    }
  }

  /** Les six titres les plus lus de chaque colonne : lectures et nombre d'utilisateurs différents. */
  const most = $derived.by(() => {
    const cols = { movie: new Map(), show: new Map(), artist: new Map() } as Record<string, Map<string, { key: string; title: string; poster: string | null; count: number; users: Set<number> }>>;
    for (const e of plays?.entries ?? []) {
      if (mostUser !== "all" && String(e.accountId) !== mostUser) continue;
      const col = e.kind === "movie" ? "movie" : e.kind === "episode" ? "show" : e.kind === "track" ? "artist" : null;
      if (!col) continue;
      const key = col === "movie" ? e.ratingKey : (e.grandparentKey ?? e.grandparentTitle ?? e.ratingKey);
      const title = col === "movie" ? e.title : (e.grandparentTitle ?? e.title);
      const poster = col === "movie" ? e.poster : (e.grandparentPoster ?? e.poster);
      const row = cols[col].get(key) ?? { key, title, poster, count: 0, users: new Set<number>() };
      row.count++;
      row.users.add(e.accountId);
      cols[col].set(key, row);
    }
    const best = (m: Map<string, { key: string; title: string; poster: string | null; count: number; users: Set<number> }>) =>
      [...m.values()].sort((a, b) => b.count - a.count).slice(0, 6);
    return [
      { id: "movie", label: t("dashboard.movies"), rows: best(cols.movie) },
      { id: "show", label: t("dashboard.tv"), rows: best(cols.show) },
      { id: "artist", label: t("dashboard.artists"), rows: best(cols.artist) },
    ];
  });

  // ---------------- Dessin des graphiques ----------------
  let lineWidth = $state(800);
  let barWidth = $state(800);
  const H = 220;
  const PAD = { left: 56, right: 12, top: 12, bottom: 28 };

  /** Haut d'échelle arrondi (1, 2, 5 × 10ⁿ), pour des graduations lisibles. */
  function niceMax(v: number) {
    if (v <= 0) return 1;
    const p = 10 ** Math.floor(Math.log10(v));
    const n = v / p;
    return (n <= 1 ? 1 : n <= 2 ? 2 : n <= 5 ? 5 : 10) * p;
  }

  const timeFmt = (at: number, range: string) =>
    new Intl.DateTimeFormat(
      locale,
      range === "live" ? { hour: "2-digit", minute: "2-digit", second: "2-digit" } : range === "day" ? { hour: "2-digit", minute: "2-digit" } : range === "year" ? { month: "short" } : { day: "numeric", month: "short" },
    ).format(new Date(at * 1000));

  const bwMax = $derived(niceMax(Math.max(0, ...bwSeries.map((p) => Math.max(p.remote, p.local)))));
  function bwX(i: number) {
    const w = lineWidth - PAD.left - PAD.right;
    return PAD.left + (bwSeries.length > 1 ? (i / (bwSeries.length - 1)) * w : w / 2);
  }
  const bwY = (v: number) => PAD.top + (1 - v / bwMax) * (H - PAD.top - PAD.bottom);
  const bwPath = (key: "remote" | "local") => bwSeries.map((p, i) => `${i ? "L" : "M"}${bwX(i).toFixed(1)},${bwY(p[key]).toFixed(1)}`).join("");
  let bwHover = $state<number | null>(null);
  function bwMove(e: PointerEvent) {
    const r = (e.currentTarget as SVGElement).getBoundingClientRect();
    const x = e.clientX - r.left;
    const w = lineWidth - PAD.left - PAD.right;
    bwHover = bwSeries.length ? Math.max(0, Math.min(bwSeries.length - 1, Math.round(((x - PAD.left) / w) * (bwSeries.length - 1)))) : null;
  }

  const histMax = $derived(niceMax(Math.max(0, ...histBars.map((b) => b.total)) / 3600));
  const barSlot = $derived(histBars.length ? (barWidth - PAD.left - PAD.right) / histBars.length : 0);
  const barW = $derived(Math.max(4, Math.min(56, barSlot * 0.6)));
  const histY = (hours: number) => PAD.top + (1 - hours / histMax) * (H - PAD.top - PAD.bottom);
  let histHover = $state<number | null>(null);

  /** Segments empilés d'une barre (du bas vers le haut), séparés par 2 px de fond. */
  function segments(b: { byKind: Record<string, number> }) {
    let base = 0;
    return KINDS.filter((k) => b.byKind[k.id]).map((k) => {
      const hours = b.byKind[k.id] / 3600;
      const y1 = histY(base);
      base += hours;
      const y2 = histY(base);
      return { ...k, y: y2, h: Math.max(0, y1 - y2 - 2) };
    });
  }

  onMount(() => {
    loadBandwidth();
    loadTop();
    loadHistory();
    loadMost();
    bwTimer = setInterval(() => bwRange === "live" && loadBandwidth(), 6000);
  });
  onDestroy(() => clearInterval(bwTimer));
</script>

<!-- ---------- Débit ---------- -->
<div class="block">
  <div class="block-head">
    <h3 class="sub">{t("dashboard.bandwidthTitle")}</h3>
    <div class="filters">
      <div class="filter"><TrackSelect bind:value={bwUser} label={t("dashboard.allUsers")} options={userOptions} /></div>
      <div class="filter">
        <TrackSelect value={bwRange} label={t("dashboard.period")} options={Object.entries(BW).map(([value, r]) => ({ value, label: r.label }))} onchange={(v) => ((bwRange = v as keyof typeof BW), (bw = null), loadBandwidth())} />
      </div>
    </div>
  </div>
  {#if bwError}
    <p class="error">{t("dashboard.statsUnavailable", { error: bwError })}</p>
  {:else if !bw}
    <div class="skeleton" style:height="{H}px"></div>
  {:else if !bwSeries.length}
    <p class="muted">{t("dashboard.noData")}</p>
  {:else}
    <div class="chart" bind:clientWidth={lineWidth}>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <svg width={lineWidth} height={H} onpointermove={bwMove} onpointerleave={() => (bwHover = null)} role="img" aria-label={t("dashboard.bandwidthTitle")}>
        {#each [0, 0.25, 0.5, 0.75, 1] as f (f)}
          <line class="grid" x1={PAD.left} x2={lineWidth - PAD.right} y1={bwY(bwMax * f)} y2={bwY(bwMax * f)} />
          <text class="axis" x={PAD.left - 8} y={bwY(bwMax * f) + 4} text-anchor="end">{fmtRate(bwMax * f)}</text>
        {/each}
        {#each [0, Math.floor((bwSeries.length - 1) / 2), bwSeries.length - 1] as i (i)}
          <text class="axis" x={bwX(i)} y={H - 8} text-anchor={i === 0 ? "start" : i === bwSeries.length - 1 ? "end" : "middle"}>{timeFmt(bwSeries[i].at, bwRange)}</text>
        {/each}
        <path d={bwPath("local")} stroke={LOCAL} />
        <path d={bwPath("remote")} stroke={REMOTE} />
        {#if bwHover !== null}
          {@const p = bwSeries[bwHover]}
          <line class="cross" x1={bwX(bwHover)} x2={bwX(bwHover)} y1={PAD.top} y2={H - PAD.bottom} />
          <circle cx={bwX(bwHover)} cy={bwY(p.remote)} r="4" fill={REMOTE} />
          <circle cx={bwX(bwHover)} cy={bwY(p.local)} r="4" fill={LOCAL} />
        {/if}
      </svg>
      {#if bwHover !== null}
        {@const p = bwSeries[bwHover]}
        <div class="tip" style:left="{Math.min(bwX(bwHover) + 12, lineWidth - 190)}px">
          <strong>{timeFmt(p.at, bwRange)}</strong>
          <span><i style:background={REMOTE}></i>{t("dashboard.remoteSeries")} <b>{fmtRate(p.remote)}</b></span>
          <span><i style:background={LOCAL}></i>{t("dashboard.localSeries")} <b>{fmtRate(p.local)}</b></span>
        </div>
      {/if}
    </div>
    <div class="legend">
      <span><i style:background={REMOTE}></i>{t("dashboard.remoteSeries")}</span>
      <span><i style:background={LOCAL}></i>{t("dashboard.localSeries")}</span>
      <span class="muted right">{t("dashboard.average", { remote: fmtRate(bwAverage.remote), local: fmtRate(bwAverage.local) })}</span>
    </div>
  {/if}
</div>

<!-- ---------- Historique de lecture ---------- -->
<div class="block">
  <div class="block-head">
    <h3 class="sub">{t("dashboard.history")}</h3>
    <div class="filters">
      <div class="filter"><TrackSelect bind:value={histUser} label={t("dashboard.allUsers")} options={userOptions} /></div>
      <div class="filter">
        <TrackSelect value={histRange} label={t("dashboard.period")} options={Object.entries(PERIOD).map(([value, r]) => ({ value, label: r.label }))} onchange={(v) => ((histRange = v as keyof typeof PERIOD), (hist = null), loadHistory())} />
      </div>
    </div>
  </div>
  {#if histError}
    <p class="error">{t("dashboard.statsUnavailable", { error: histError })}</p>
  {:else if !hist}
    <div class="skeleton" style:height="{H}px"></div>
  {:else if !histBars.length}
    <p class="muted">{t("dashboard.noData")}</p>
  {:else}
    <div class="chart" bind:clientWidth={barWidth}>
      <svg width={barWidth} height={H} role="img" aria-label={t("dashboard.history")}>
        {#each [0, 0.25, 0.5, 0.75, 1] as f (f)}
          <line class="grid" x1={PAD.left} x2={barWidth - PAD.right} y1={histY(histMax * f)} y2={histY(histMax * f)} />
          <text class="axis" x={PAD.left - 8} y={histY(histMax * f) + 4} text-anchor="end">{fmtHours(histMax * f * 3600)}</text>
        {/each}
        {#each histBars as b, i (b.at)}
          {@const x = PAD.left + i * barSlot + (barSlot - barW) / 2}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <g onpointerenter={() => (histHover = i)} onpointerleave={() => (histHover = null)}>
            <rect class="hit" x={PAD.left + i * barSlot} y={PAD.top} width={barSlot} height={H - PAD.top - PAD.bottom} />
            {#each segments(b) as s (s.id)}<rect x={x} y={s.y} width={barW} height={s.h} rx="3" fill={s.color} />{/each}
          </g>
          {#if histBars.length <= 12 || i % Math.ceil(histBars.length / 8) === 0}
            <text class="axis" x={x + barW / 2} y={H - 8} text-anchor="middle">{timeFmt(b.at, histRange)}</text>
          {/if}
        {/each}
      </svg>
      {#if histHover !== null}
        {@const b = histBars[histHover]}
        <div class="tip" style:left="{Math.min(PAD.left + histHover * barSlot + barSlot, barWidth - 190)}px">
          <strong>{timeFmt(b.at, histRange)}</strong>
          {#each KINDS.filter((k) => b.byKind[k.id]) as k (k.id)}
            <span><i style:background={k.color}></i>{k.label} <b>{fmtHours(b.byKind[k.id])}</b></span>
          {/each}
        </div>
      {/if}
    </div>
    <div class="legend">
      {#each histTotals as k (k.id)}<span><i style:background={k.color}></i>{k.label} <b>{fmtHours(k.total)}</b></span>{/each}
    </div>
  {/if}
</div>

<!-- ---------- Top des utilisateurs ---------- -->
<div class="block wide">
  <div class="block-head">
    <h3 class="sub">{t("dashboard.topUsers")}</h3>
    <div class="filters">
      <div class="filter">
        <TrackSelect value={topRange} label={t("dashboard.period")} options={Object.entries(PERIOD).map(([value, r]) => ({ value, label: r.label }))} onchange={(v) => ((topRange = v as keyof typeof PERIOD), (top = null), loadTop())} />
      </div>
    </div>
  </div>
  {#if topError}
    <p class="error">{t("dashboard.statsUnavailable", { error: topError })}</p>
  {:else if !top}
    <div class="skeleton" style:height="200px"></div>
  {:else if !topUsers.length}
    <p class="muted">{t("dashboard.noData")}</p>
  {:else}
    <div class="users">
      {#each topUsers as u (u.id)}
        <div class="user">
          <div class="user-head">
            {#if thumbOf(u.id)}<img class="avatar" src={thumbOf(u.id)} alt="" />{:else}<span class="avatar ph"></span>{/if}
            <div>
              <div class="strong">{nameOf(u.id)}</div>
              <div class="muted small">{t("dashboard.plays", { count: u.count })} · {fmtHours(u.duration)}</div>
            </div>
          </div>
          {#each KINDS as k (k.id)}
            <div class="user-row">
              <span><i style:background={k.color}></i>{k.label}</span>
              <span class="muted">{fmtHours(u.byKind[k.id] ?? 0)}</span>
            </div>
          {/each}
        </div>
      {/each}
    </div>
  {/if}
</div>

<!-- ---------- Les plus lus ---------- -->
<div class="block wide">
  <div class="block-head">
    <h3 class="sub">{t("dashboard.mostPlayed")}</h3>
    <div class="filters">
      <div class="filter"><TrackSelect bind:value={mostUser} label={t("dashboard.allUsers")} options={userOptions} /></div>
      <div class="filter">
        <TrackSelect
          value={mostRange}
          label={t("dashboard.period")}
          options={[
            { value: "week", label: PERIOD.week.label },
            { value: "month", label: PERIOD.month.label },
          ]}
          onchange={(v) => ((mostRange = v as "week" | "month"), (plays = null), loadMost())}
        />
      </div>
    </div>
  </div>
  {#if mostError}
    <p class="error">{t("dashboard.statsUnavailable", { error: mostError })}</p>
  {:else if !plays}
    <div class="skeleton" style:height="300px"></div>
  {:else}
    <div class="most">
      {#each most as col (col.id)}
        <div class="most-col">
          <h4>{col.label}</h4>
          {#each col.rows as r (r.key)}
            <div class="most-row">
              <span class="most-img" class:round={col.id === "artist"}>{#if r.poster}<img src={r.poster} alt="" loading="lazy" />{/if}</span>
              <span class="most-text">
                <span class="strong">{r.title}</span>
                <span class="muted small">{t("dashboard.plays", { count: r.count })} · {t("dashboard.users", { count: r.users.size })}</span>
              </span>
            </div>
          {:else}
            <p class="muted small">{t("dashboard.noData")}</p>
          {/each}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* Chaque bloc est une tuile ; le tableau de bord les range sur deux colonnes (voir ServerDashboard). */
  .block {
    min-width: 0;
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: #ffffff05;
  }

  .block-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px 16px;
    margin-bottom: 14px;
  }

  .sub {
    margin: 0;
    font-size: 17px;
  }

  .filters {
    display: flex;
    gap: 10px;
  }

  .filter {
    display: flex;
    width: 210px;
  }

  .chart {
    position: relative;
    padding: 6px 0;
    border-radius: 14px;
    background: #ffffff08;
  }

  svg {
    display: block;
  }

  .grid {
    stroke: #ffffff14;
    stroke-width: 1;
  }

  .axis {
    fill: var(--text-faint);
    font-size: 11px;
  }

  path {
    fill: none;
    stroke-width: 2;
    stroke-linejoin: round;
    stroke-linecap: round;
  }

  .cross {
    stroke: #ffffff40;
    stroke-width: 1;
  }

  circle {
    stroke: var(--bg);
    stroke-width: 2;
  }

  .hit {
    fill: transparent;
  }

  .tip {
    position: absolute;
    top: 12px;
    display: grid;
    gap: 4px;
    min-width: 170px;
    padding: 10px 12px;
    border: 1px solid #ffffff1a;
    border-radius: 10px;
    background: #1c1e23f2;
    box-shadow: 0 10px 30px #000a;
    font-size: 12px;
    pointer-events: none;
  }

  .tip span,
  .legend span,
  .user-row span:first-child {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .tip b {
    margin-left: auto;
    padding-left: 10px;
  }

  i {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 18px;
    margin-top: 10px;
    font-size: 12px;
  }

  .legend .right {
    margin-left: auto;
  }

  .users {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 14px;
  }

  .user {
    overflow: hidden;
    border-radius: 14px;
    background: #ffffff08;
  }

  .user-head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px;
    border-bottom: 1px solid var(--border);
  }

  .avatar {
    width: 44px;
    height: 44px;
    flex: none;
    border-radius: 50%;
    object-fit: cover;
  }

  .avatar.ph {
    background: #ffffff14;
  }

  .user-row {
    display: flex;
    justify-content: space-between;
    padding: 9px 14px;
    font-size: 13px;
  }

  .user-row:nth-child(odd) {
    background: #ffffff05;
  }

  .strong {
    overflow: hidden;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 12px;
  }

  .most {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 16px;
  }

  .most-col {
    overflow: hidden;
    border-radius: 14px;
    background: #ffffff08;
  }

  .most-col h4 {
    margin: 0;
    padding: 14px;
    border-bottom: 1px solid var(--border);
    font-size: 14px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .most-col > .small {
    padding: 0 14px;
  }

  .most-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 14px;
  }

  .most-img {
    width: 40px;
    height: 60px;
    flex: none;
    overflow: hidden;
    border-radius: 6px;
    background: #ffffff10;
  }

  .most-img.round {
    height: 40px;
    border-radius: 50%;
  }

  .most-img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .most-text {
    display: grid;
    min-width: 0;
  }
</style>
