<script lang="ts">
  // Série Kavita : couverture, résumé, « Lire » ou « Continuer », puis ses volumes, ses chapitres hors volume et ses
  // hors-séries. Les livres EPUB s'ouvrent dans leur propre lecteur (EpubReader), les autres dans celui des images.
  // La série, chaque volume et chaque chapitre ont leur menu « ⋮ », comme dans Kavita : lire, marquer comme lu ou
  // non lu, détails.
  import { onMount } from "svelte";
  import { api, errorText, type BookChapter, type BookSeriesDetail, type BookVolume } from "../lib/api";
  import { books } from "../lib/books.svelte";
  import { toast } from "../lib/nav.svelte";
  import { t } from "../lib/i18n";
  import ErrorCard from "../components/ErrorCard.svelte";
  import Icon from "../components/Icon.svelte";
  import BookMenu, { type BookMenuEntry } from "../components/BookMenu.svelte";
  import BookMore from "../components/BookMore.svelte";
  import BookDetails from "../components/BookDetails.svelte";

  let { id }: { id: number } = $props();

  let detail = $state<BookSeriesDetail | null>(null);
  let error = $state<string | null>(null);

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
</script>

<div class="backdrop" style:background-image={detail ? `url("${detail.series.cover}")` : undefined}></div>

<div class="page detail">
  {#if error}<ErrorCard message={error} />{/if}
  {#if detail}
    <div class="head">
      <button class="cover" onclick={() => read(detail!.continueChapter ?? first?.id)} disabled={!readable} title={t("books.read")} tabindex="-1" aria-hidden="true">
        <img src={detail.series.cover} alt="" />
      </button>
      <div class="info">
        <div class="eyebrow">{kindLabel}</div>
        <h1>{detail.series.name}</h1>
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
        </div>
        {#if detail.summary}<p class="summary">{detail.summary}</p>{/if}
      </div>
    </div>

    {#if detail.volumes.length}
      <h2>{t("books.volumes")}</h2>
      <div class="grid">
        {#each detail.volumes as v (v.id)}
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

    {#each [["chapters", detail.chapters], ["specials", detail.specials]] as [key, list] (key)}
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
  /* Jusqu'au bord de la fenêtre, derrière le menu translucide, comme la page d'un album (MusicDetail) : posé sous le
     fond de la fenêtre (z-index négatif), il s'arrêtait net au bord du menu. */
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 0;
    background-position: center;
    background-size: cover;
    filter: blur(60px) saturate(1.4);
    opacity: 0.35;
    mask-image: linear-gradient(#000 0%, transparent 70%);
    pointer-events: none;
  }

  :global(html.tv) .backdrop {
    display: none;
  }

  .detail {
    position: relative;
    z-index: 1;
  }

  .head {
    display: flex;
    gap: 32px;
    align-items: flex-end;
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
