<script lang="ts">
  // Série Kavita : couverture, résumé, « Lire » ou « Continuer », puis ses volumes, ses chapitres hors volume et ses
  // hors-séries. Les livres EPUB s'ouvrent dans leur propre lecteur (EpubReader), les autres dans celui des images.
  // La série, chaque volume et chaque chapitre ont leur menu « ⋮ », comme dans Kavita : lire, marquer comme lu ou
  // non lu, détails.
  // Comme la fiche d'un film : le fond de la série en grand, qui se fond dans la page (l'illustration trouvée en
  // ligne, sinon la couverture floutée, teintée de sa couleur), et son logo à la place du titre quand il existe.
  import { onMount } from "svelte";
  import { api, errorText, type BookArt, type BookChapter, type BookSeriesDetail, type BookVolume } from "../lib/api";
  import { books, seriesArt } from "../lib/books.svelte";
  import { toast } from "../lib/nav.svelte";
  import { t } from "../lib/i18n";
  import ErrorCard from "../components/ErrorCard.svelte";
  import Icon from "../components/Icon.svelte";
  import BookMenu, { type BookMenuEntry } from "../components/BookMenu.svelte";
  import BookMore from "../components/BookMore.svelte";
  import BookDetails from "../components/BookDetails.svelte";
  import TitleLogo from "../components/TitleLogo.svelte";

  let { id }: { id: number } = $props();

  let detail = $state<BookSeriesDetail | null>(null);
  let error = $state<string | null>(null);
  let art = $state<BookArt>({ backdrop: null, logo: null });

  // Fond et logo en ligne, une fois la série connue (son nom, son type, son numéro AniList).
  let artFor = "";
  $effect(() => {
    if (!detail) return;
    const d = detail;
    const key = `${d.series.id}`;
    if (artFor === key) return;
    artFor = key;
    seriesArt(d.series.name, d.libraryKind, d.anilistId).then((a) => {
      if (detail?.series.id === d.series.id) art = a;
    });
  });

  async function load() {
    try {
      detail = await api.kavitaSeriesDetail(id);
    } catch (e) {
      error = errorText(e);
    }
  }
  onMount(load);

  // Retour du lecteur : la progression a changé.
  let wasReading = false;
  $effect(() => {
    const reading = books.reading !== null;
    if (wasReading && !reading) load();
    wasReading = reading;
  });

  const readable = $derived(detail ? detail.series.format !== "unknown" : false);
  const progress = $derived(detail && detail.series.pages > 0 ? Math.min(1, detail.series.pagesRead / detail.series.pages) : 0);
  const kindLabel = $derived(detail ? t(`books.kind.${detail.libraryKind}`) : "");

  /** Premier chapitre de la série, pour « Lire » quand Kavita ne propose pas de point de reprise. */
  const first = $derived(detail ? (detail.volumes[0]?.chapters[0] ?? detail.chapters[0] ?? detail.specials[0] ?? null) : null);

  function read(chapterId: number | undefined | null) {
    if (!chapterId || !readable) return;
    books.reading = { chapterId, seriesId: id, format: detail!.series.format };
  }

  /** Volume : son premier chapitre pas encore fini (le premier, s'il est déjà lu). */
  function readVolume(v: BookVolume) {
    read((v.chapters.find((c) => c.pagesRead < c.pages) ?? v.chapters[0])?.id);
  }

  const ratio = (x: { pages: number; pagesRead: number }) => (x.pages > 0 ? Math.min(1, x.pagesRead / x.pages) : 0);

  // --- Menus « ⋮ » : la série, un volume ou un chapitre ---
  type Target = { kind: "series" } | { kind: "volume"; volume: BookVolume } | { kind: "chapter"; chapter: BookChapter };
  let menu = $state<{ x: number; y: number; target: Target } | null>(null);
  let details = $state<{ title: string; cover: string; volumeId?: number; chapterId?: number } | null>(null);

  function openMenu(e: MouseEvent, target: Target) {
    e.preventDefault();
    e.stopPropagation();
    menu = { x: e.clientX, y: e.clientY, target };
  }

  async function markRead(target: Target, read: boolean) {
    await api.kavitaMarkRead({
      seriesId: id,
      volumeId: target.kind === "volume" ? target.volume.id : null,
      chapterId: target.kind === "chapter" ? target.chapter.id : null,
      read,
    });
    toast(read ? t("books.markedRead") : t("books.markedUnread"));
    await load();
  }

  const menuTitle = (target: Target) =>
    target.kind === "series" ? detail!.series.name : target.kind === "volume" ? target.volume.name || t("books.volume") : chapterLabel(target.chapter);

  function entries(target: Target): BookMenuEntry[] {
    const x = target.kind === "series" ? detail!.series : target.kind === "volume" ? target.volume : target.chapter;
    const list: BookMenuEntry[] = [];
    if (readable && target.kind !== "series")
      list.push({ icon: "play", label: t("books.read"), run: () => (target.kind === "volume" ? readVolume(target.volume) : read(target.chapter.id)) });
    if (x.pagesRead < x.pages) list.push({ icon: "checkCircle", label: t("books.markRead"), run: () => markRead(target, true) });
    if (x.pagesRead > 0) list.push({ icon: "restart", label: t("books.markUnread"), run: () => markRead(target, false) });
    if (target.kind === "volume")
      list.push({ icon: "info", label: t("books.details.open"), run: () => (details = { title: menuTitle(target), cover: target.volume.cover, volumeId: target.volume.id }) });
    if (target.kind === "chapter")
      list.push({ icon: "info", label: t("books.details.open"), run: () => (details = { title: menuTitle(target), cover: target.chapter.cover, chapterId: target.chapter.id }) });
    return list;
  }
  const chapterLabel = (c: BookChapter) =>
    [c.number && t("books.chapter", { number: c.number }), c.title].filter(Boolean).join(" · ") || t("books.untitled");

  // --- Informations, comme sur une fiche : auteurs · année · volumes · genres · statut ---
  const facts = $derived.by(() => {
    if (!detail) return [];
    const d = detail;
    return [
      d.writers.slice(0, 2).join(", "),
      d.year ? String(d.year) : "",
      d.volumes.length ? t("books.volumeCount", { count: d.volumes.length }) : d.chapters.length ? t("books.chapterCount", { count: d.chapters.length }) : "",
      d.genres.slice(0, 3).join(", "),
      d.status ? t(`books.status.${d.status}`) : "",
    ].filter(Boolean);
  });

  /** Où reprend « Continuer » : « vol. 2, page 6 » ou « chap. 5, page 1 » (la page qui suit la dernière lue). */
  const resumeAt = $derived.by(() => {
    if (!detail?.continueChapter || progress <= 0 || progress >= 1) return null;
    const d = detail;
    const id = d.continueChapter;
    const volume = d.volumes.find((v) => v.chapters.some((c) => c.id === id));
    const chapter = [...d.volumes.flatMap((v) => v.chapters), ...d.chapters, ...d.specials].find((c) => c.id === id);
    if (!chapter) return null;
    const page = Math.min(chapter.pages, chapter.pagesRead + 1);
    const where = volume ? t("books.atVolume", { volume: volume.name.replace(/^\D+/, "") || volume.name }) : t("books.atChapter", { chapter: chapter.number || chapterLabel(chapter) });
    return t("books.resumeAt", { where, page });
  });

  // --- Volumes et chapitres : du premier au dernier, ou l'inverse ; filtre « aller au n° » pour les longues séries ---
  let newestFirst = $state(false);
  let find = $state("");
  const ordered = <T,>(list: T[]) => (newestFirst ? [...list].reverse() : list);
  const matches = (text: string) => !find.trim() || text.toLowerCase().includes(find.trim().toLowerCase());
  const volumes = $derived(detail ? ordered(detail.volumes.filter((v) => matches(v.name) || v.chapters.some((c) => matches(c.number)))) : []);
  const chapters = $derived(detail ? ordered(detail.chapters.filter((c) => matches(chapterLabel(c)))) : []);
  const specials = $derived(detail ? ordered(detail.specials.filter((c) => matches(chapterLabel(c)))) : []);
  /** Plus de 12 volumes ou chapitres : de quoi trier et chercher. */
  const long = $derived(!!detail && detail.volumes.length + detail.chapters.length > 12);
</script>

<div class="backdrop" style:--tint={detail?.color ?? "transparent"} aria-hidden="true">
  {#if art.backdrop}
    <img class="art" src={art.backdrop} alt="" />
  {:else if detail}
    <img class="cover-art" src={detail.series.cover} alt="" />
  {/if}
</div>

<div class="page detail">
  {#if error}<ErrorCard message={error} />{/if}
  {#if detail}
    <div class="head" class:with-art={!!art.backdrop}>
      <button class="cover" onclick={() => read(detail!.continueChapter ?? first?.id)} disabled={!readable} title={t("books.read")} tabindex="-1" aria-hidden="true">
        <img src={detail.series.cover} alt="" />
      </button>
      <div class="info">
        <div class="eyebrow">{kindLabel}</div>
        <h1 class="title" style:--logo-h="150px" style:--logo-w="min(520px, 100%)"><TitleLogo logo={art.logo} title={detail.series.name} genres={detail.genres} /></h1>
        {#if facts.length}<div class="facts">{#each facts as f, i (i)}<span>{f}</span>{/each}</div>{/if}
        <div class="meta">
          {#if detail.series.pages}<span>{t("books.pages", { count: detail.series.pages })}</span>{/if}
          {#if progress > 0}<span>{t("books.readPercent", { percent: Math.round(progress * 100) })}</span>{/if}
        </div>
        <div class="actions">
          {#if readable}
            <button class="btn primary big" onclick={() => read(detail!.continueChapter ?? first?.id)} disabled={!first && !detail.continueChapter}>
              <Icon name="play" size={18} filled /> {progress > 0 && progress < 1 ? t("books.continue") : t("books.read")}
            </button>
          {/if}
          <button class="btn round big" onclick={(e) => openMenu(e, { kind: "series" })} title={t("common.c1.moreActions")} aria-label={t("common.c1.moreActions")}>
            <Icon name="more" size={20} />
          </button>
          {#if resumeAt}<span class="resume">{resumeAt}</span>{/if}
        </div>
        {#if detail.summary}<p class="summary">{detail.summary}</p>{/if}
      </div>
    </div>

    {#if long}
      <div class="toolbar">
        <label class="find">
          <Icon name="search" size={16} />
          <input type="search" bind:value={find} placeholder={t("books.findNumber")} aria-label={t("books.findNumber")} />
        </label>
        <div class="modes" role="radiogroup" aria-label={t("books.order")}>
          <button role="radio" aria-checked={!newestFirst} class:on={!newestFirst} onclick={() => (newestFirst = false)}>{t("books.oldestFirst")}</button>
          <button role="radio" aria-checked={newestFirst} class:on={newestFirst} onclick={() => (newestFirst = true)}>{t("books.newestFirst")}</button>
        </div>
      </div>
    {/if}

    {#if volumes.length}
      <h2>{t("books.volumes")}</h2>
      <div class="grid">
        {#each volumes as v (v.id)}
          <div class="tile-wrap has-more" role="group" oncontextmenu={(e) => openMenu(e, { kind: "volume", volume: v })}>
            <button class="tile" onclick={() => readVolume(v)} disabled={!readable}>
              <span class="cover-small"><img src={v.cover} alt="" loading="lazy" />{#if ratio(v) > 0 && ratio(v) < 1}<span class="bar"><span style:width="{ratio(v) * 100}%"></span></span>{/if}{#if ratio(v) >= 1}<span class="done"><Icon name="check" size={13} /></span>{/if}</span>
              <span class="name">{v.name || t("books.volume")}</span>
            </button>
            <BookMore onmenu={(e) => openMenu(e, { kind: "volume", volume: v })} />
          </div>
        {/each}
      </div>
    {/if}

    {#each [["chapters", chapters], ["specials", specials]] as [key, list] (key)}
      {#if (list as BookChapter[]).length}
        <h2>{t(`books.${key}`)}</h2>
        <div class="rows">
          {#each list as BookChapter[] as c (c.id)}
            <div class="row-wrap" role="group" oncontextmenu={(e) => openMenu(e, { kind: "chapter", chapter: c })}>
              <button class="row" onclick={() => read(c.id)} disabled={!readable}>
                <img src={c.cover} alt="" loading="lazy" />
                <span class="label">{chapterLabel(c)}</span>
                <span class="muted small">{t("books.pages", { count: c.pages })}</span>
                {#if ratio(c) >= 1}<span class="check"><Icon name="check" size={16} /></span>{:else if ratio(c) > 0}<span class="pct">{Math.round(ratio(c) * 100)} %</span>{/if}
              </button>
              <button class="dots" onclick={(e) => openMenu(e, { kind: "chapter", chapter: c })} title={t("common.c1.moreActions")} aria-label={t("common.c1.moreActions")}>
                <Icon name="more" size={18} />
              </button>
            </div>
          {/each}
        </div>
      {/if}
    {/each}
    {#if find.trim() && !volumes.length && !chapters.length && !specials.length}<p class="muted">{t("books.noMatch")}</p>{/if}
  {:else if !error}
    <div class="head"><div class="skeleton cover-skeleton"></div></div>
  {/if}
</div>

{#if menu && detail}
  <BookMenu x={menu.x} y={menu.y} title={menuTitle(menu.target)} entries={entries(menu.target)} onclose={() => (menu = null)} />
{/if}
{#if details}
  <BookDetails {...details} onclose={() => (details = null)} />
{/if}

<style>
  /* Comme la fiche d'un film : le fond jusqu'au bord de la fenêtre, derrière le menu translucide, intact en haut et
     fondu dans la page vers le bas. Sans illustration en ligne, la couverture floutée et agrandie, teintée de sa
     couleur (calculée par Kavita). */
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 0;
    overflow: hidden;
    background: radial-gradient(120% 80% at 70% 0%, color-mix(in srgb, var(--tint) 45%, transparent), transparent 70%);
    pointer-events: none;
  }

  .backdrop img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: unveil 1.2s var(--ease);
  }

  .backdrop .art {
    height: auto;
    min-height: 62%;
    max-height: 100%;
    object-position: center top;
    mask-image: linear-gradient(#000 0%, #000 50%, transparent 100%);
  }

  .backdrop .cover-art {
    height: 85%;
    object-position: center 22%;
    filter: blur(26px) saturate(1.35) brightness(0.75);
    opacity: 0.75;
    transform: scale(1.12);
    mask-image: linear-gradient(#000 0%, #000 35%, transparent 100%);
  }

  /* Le bas s'assombrit sous le titre, le haut un peu sous l'en-tête. */
  .backdrop::after {
    content: "";
    position: absolute;
    inset: 0;
    background:
      linear-gradient(0deg, var(--bg) 0%, #000000b0 30%, #00000030 60%, transparent 80%),
      linear-gradient(90deg, #000000a0 0%, transparent 55%),
      linear-gradient(180deg, #00000070 0%, transparent 14%);
  }

  @keyframes unveil {
    from {
      opacity: 0;
      transform: scale(1.08);
    }
  }

  :global(html.tv) .backdrop {
    display: none;
  }

  .detail {
    position: relative;
    z-index: 1;
  }

  /* L'en-tête occupe le haut de l'écran, comme le bandeau d'un film : le fond se voit avant les volumes. */
  .head {
    display: flex;
    gap: 32px;
    align-items: flex-end;
    min-height: calc(62vh - 68px);
    animation: rise 0.7s 0.1s var(--ease) backwards;
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(16px);
    }
  }

  :global(html.phone) .head {
    min-height: 0;
  }

  :global(html.phone) .head {
    flex-direction: column;
    align-items: center;
    text-align: center;
  }

  .cover,
  .cover-skeleton {
    flex: none;
    width: 220px;
    aspect-ratio: 2 / 3;
    padding: 0;
    overflow: hidden;
    border: none;
    border-radius: var(--radius-lg);
    background: var(--bg-elev);
    box-shadow: 0 20px 50px #000c;
    cursor: pointer;
  }

  .cover img,
  .cover-small img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .eyebrow {
    color: var(--accent);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  h1 {
    margin: 6px 0 10px;
    font-size: clamp(28px, 4vw, 44px);
    line-height: 1.1;
    text-shadow: 0 2px 18px #0009;
  }

  :global(html.phone) h1 {
    --logo-pos: center bottom;
    display: flex;
    justify-content: center;
  }

  /* Auteurs · année · volumes · genres · statut. */
  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 0;
    margin-bottom: 6px;
    color: var(--text);
    font-size: 14px;
    font-weight: 600;
  }

  .facts span:not(:last-child)::after {
    content: "·";
    margin: 0 8px;
    color: var(--text-faint);
  }

  :global(html.phone) .facts {
    justify-content: center;
  }

  .resume {
    align-self: center;
    color: var(--text-dim);
    font-size: 13px;
  }

  /* Longues séries : chercher un numéro, et l'ordre des volumes et chapitres. */
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 34px;
  }

  /* La même pilule que la recherche et le tri des bibliothèques (et leurs onglets). */
  .find,
  .modes {
    height: 44px;
    border-radius: 22px;
    background: #ffffff0d;
  }

  .find {
    display: flex;
    flex: 1 1 260px;
    align-items: center;
    gap: 8px;
    max-width: 420px;
    padding: 0 16px;
    border: 1px solid transparent;
    color: var(--text-dim);
    transition: border-color 0.15s;
  }

  .find:focus-within {
    border-color: var(--accent);
  }

  .find input {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: none;
    background: none;
    color: var(--text);
    outline: none;
  }

  .modes {
    display: flex;
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
  }

  .modes button:hover {
    color: var(--text);
  }

  .modes button.on {
    background: var(--accent);
    color: var(--accent-text);
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    color: var(--text-dim);
    font-size: 14px;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 18px;
  }

  :global(html.phone) .actions {
    justify-content: center;
  }

  .summary {
    max-width: 760px;
    margin-top: 18px;
    color: var(--text-dim);
    line-height: 1.55;
    white-space: pre-line;
  }

  h2 {
    margin: 34px 0 14px;
    font-size: 20px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
    gap: 20px 16px;
  }

  .tile-wrap {
    position: relative;
  }

  .tile {
    width: 100%;
    display: grid;
    gap: 8px;
    padding: 0;
    border: none;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }

  .cover-small {
    position: relative;
    display: block;
    aspect-ratio: 2 / 3;
    overflow: hidden;
    border-radius: var(--radius);
    background: var(--bg-elev);
  }

  .tile:hover .cover-small,
  .tile:focus-visible .cover-small {
    box-shadow: 0 0 0 2px var(--accent);
  }

  .name {
    font-size: 14px;
    font-weight: 600;
  }

  .bar {
    position: absolute;
    right: 6px;
    bottom: 6px;
    left: 6px;
    height: 4px;
    overflow: hidden;
    border-radius: 2px;
    background: #0009;
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
  }

  .done {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-text);
  }

  .rows {
    display: grid;
    gap: 4px;
  }

  .row-wrap {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .row {
    flex: 1;
    min-width: 0;
    display: grid;
    grid-template-columns: 44px 1fr auto auto;
    align-items: center;
    gap: 14px;
    padding: 6px 10px;
    border: none;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }

  .row:hover,
  .row:focus-visible {
    background: #ffffff10;
  }

  /* « ⋮ » d'un chapitre : au survol de la ligne (toujours visible sur un écran tactile). */
  .dots {
    display: grid;
    flex: none;
    place-items: center;
    width: 34px;
    height: 34px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--text-dim);
    opacity: 0;
    transition: opacity 0.15s;
  }

  .row-wrap:hover .dots,
  .dots:focus-visible {
    opacity: 1;
  }

  .dots:hover {
    background: #ffffff1a;
    color: var(--text);
  }

  .row img {
    width: 44px;
    height: 62px;
    object-fit: cover;
    border-radius: 6px;
    background: var(--bg-elev);
  }

  .label {
    font-weight: 600;
  }

  .check,
  .pct {
    color: var(--accent);
    font-size: 13px;
    font-weight: 700;
  }

  button:disabled {
    cursor: default;
  }
</style>
