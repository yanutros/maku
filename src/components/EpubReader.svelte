<script lang="ts">
  // Lecteur de livres EPUB (Kavita). Kavita découpe un livre en sections (les « pages » de sa progression) et sert
  // le HTML de chacune, styles du livre compris. Maku l'affiche à part, dans un shadow DOM (les styles du livre ne
  // touchent pas l'interface), nettoyé (ni script ni formulaire), en pages tournées (colonnes de la largeur de
  // l'écran, deux côte à côte sur un grand écran) ou en défilement. Taille du texte, thème et mise en page sont
  // retenus ; Kavita ne gardant que la section, l'endroit dans la section est retenu sur l'appareil.
  import { onMount, tick, untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { api, errorText, type BookChapterInfo, type BookTocItem } from "../lib/api";
  import {
    books,
    cleanBookHtml,
    EPUB_SIZES,
    epubPosition,
    epubSettings,
    saveEpubPosition,
    saveEpubSettings,
    type EpubLayout,
    type EpubSettings,
    type EpubTheme,
  } from "../lib/books.svelte";
  import { t } from "../lib/i18n";
  import ErrorCard from "./ErrorCard.svelte";
  import Icon from "./Icon.svelte";

  /** Où se placer dans une section qu'on ouvre. */
  type Target = { at: number } | { end: true } | { part: string };

  let info = $state<BookChapterInfo | null>(null);
  /** Section affichée (la « page » de Kavita, à partir de 0). */
  let section = $state(0);
  /** Écran affiché dans la section (pages tournées), et leur nombre. */
  let screen = $state(0);
  let screens = $state(1);
  /** Défilement : la fin de la section est visible. */
  let scrolledEnd = $state(false);
  let settings = $state<EpubSettings>(epubSettings());
  let toc = $state<BookTocItem[]>([]);
  let panel = $state<"toc" | "settings" | null>(null);
  let controls = $state(true);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let host = $state<HTMLElement>();
  let viewport: HTMLElement | undefined;
  let content: HTMLElement | undefined;
  let sheet: HTMLStyleElement | undefined;
  /** Largeur d'un écran, espace entre colonnes compris. */
  let step = 0;

  const THEMES: Record<EpubTheme, { bg: string; fg: string; link: string }> = {
    dark: { bg: "#141414", fg: "#e4e4e4", link: "#f5c451" },
    light: { bg: "#ffffff", fg: "#1b1b1b", link: "#8a5a00" },
    sepia: { bg: "#f4ecd8", fg: "#4b3a28", link: "#8a5a00" },
  };

  // --- Affichage : le livre dans un shadow DOM, avec ses styles et ceux du lecteur ---

  function readerCss(s: EpubSettings) {
    const c = THEMES[s.theme];
    return `
      :host { all: initial; display: block; width: 100%; height: 100%; }
      .viewport { --side: 56px; }
      @media (max-width: 700px) { .viewport { --side: 20px; } }
      .viewport {
        box-sizing: border-box; width: 100%; height: 100%; overflow: hidden; background: ${c.bg}; color: ${c.fg};
        padding: calc(64px + var(--inset-top, 0px)) var(--side) calc(64px + var(--inset-bottom, 0px));
      }
      .scroll { overflow-y: auto; overscroll-behavior: contain; }
      .scroll .book-content { max-width: 46em; margin: 0 auto; }
      .book-content {
        box-sizing: border-box; font-family: Georgia, "Times New Roman", serif; font-size: ${s.size}%;
        line-height: 1.6; overflow-wrap: break-word; hyphens: auto; color: ${c.fg};
      }
      .book-content * { color: ${c.fg} !important; background-color: transparent !important; }
      .book-content a, .book-content a * { color: ${c.link} !important; }
      .book-content img, .book-content svg, .book-content video {
        max-width: 100%; max-height: var(--page-h, 90vh); height: auto; object-fit: contain; break-inside: avoid;
      }
      .book-content p { orphans: 2; widows: 2; }
    `;
  }

  onMount(() => {
    const root = host!.attachShadow({ mode: "open" });
    sheet = document.createElement("style");
    viewport = document.createElement("div");
    content = document.createElement("div");
    content.className = "book-content";
    viewport.append(content);
    root.append(sheet, viewport);
    viewport.addEventListener("click", onContentClick);
    viewport.addEventListener("scroll", onScroll, { passive: true });
    // Une image chargée, un écran redimensionné : la section se remet en pages.
    root.addEventListener("load", relayout, true);
    const resize = new ResizeObserver(() => relayout());
    resize.observe(host!);
    if (books.reading) open(books.reading.chapterId);
    return () => {
      resize.disconnect();
      flush();
    };
  });

  $effect(() => {
    if (!sheet || !viewport) return;
    sheet.textContent = readerCss(settings);
    viewport.className = settings.layout === "scroll" ? "viewport scroll" : "viewport paged";
  });

  // Réglages changés : retenus, et la section remise en pages au même endroit.
  let firstSettings = true;
  $effect(() => {
    const s = $state.snapshot(settings);
    if (firstSettings) {
      firstSettings = false;
      return;
    }
    saveEpubSettings(s);
    const at = untrack(position);
    tick().then(() => layout({ at }));
  });

  /** Endroit dans la section : 0 au début, 1 à la fin. */
  function position(): number {
    if (!viewport) return 0;
    if (settings.layout === "scroll") {
      const max = viewport.scrollHeight - viewport.clientHeight;
      return max > 0 ? viewport.scrollTop / max : 0;
    }
    return screens > 1 ? screen / (screens - 1) : 0;
  }

  /** Met la section en pages (ou en défilement) et s'y place. */
  function layout(target: Target) {
    if (!viewport || !content) return;
    lastTarget = target;
    if (settings.layout === "scroll") {
      content.style.cssText = "";
      screens = 1;
      screen = 0;
      const max = viewport.scrollHeight - viewport.clientHeight;
      if ("end" in target) viewport.scrollTop = max;
      else if ("at" in target) viewport.scrollTop = target.at * max;
      else partElement(target.part)?.scrollIntoView({ block: "start" });
      onScroll();
      return;
    }
    const cs = getComputedStyle(viewport);
    const width = viewport.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
    const height = viewport.clientHeight - parseFloat(cs.paddingTop) - parseFloat(cs.paddingBottom);
    // Deux colonnes côte à côte sur un grand écran, comme un livre ouvert.
    const perScreen = width >= 900 ? 2 : 1;
    const gap = perScreen === 2 ? 72 : 40;
    step = width + gap;
    Object.assign(content.style, {
      width: `${width}px`,
      height: `${height}px`,
      columnCount: String(perScreen),
      columnGap: `${gap}px`,
      columnFill: "auto",
    });
    content.style.setProperty("--page-h", `${height}px`);
    viewport.scrollTop = 0;
    const columnWidth = (width - gap * (perScreen - 1)) / perScreen;
    const columns = Math.max(1, Math.round((content.scrollWidth + gap) / (columnWidth + gap)));
    screens = Math.max(1, Math.ceil(columns / perScreen));
    if ("end" in target) screen = screens - 1;
    else if ("at" in target) screen = Math.round(target.at * (screens - 1));
    else {
      const el = partElement(target.part);
      const x = el ? el.getBoundingClientRect().left - content.getBoundingClientRect().left : 0;
      screen = Math.min(screens - 1, Math.max(0, Math.floor((x + 1) / step)));
    }
    place();
  }

  function place() {
    if (content && settings.layout === "paged") content.style.transform = `translateX(${-screen * step}px)`;
  }
  $effect(() => {
    void screen;
    place();
  });

  /** Dernier placement demandé : refait quand une image se charge, tant qu'on n'a pas bougé. */
  let lastTarget: Target = { at: 0 };
  let moved = false;
  function relayout() {
    if (!content?.childNodes.length) return;
    layout(moved ? { at: position() } : lastTarget);
  }

  function partElement(part: string): Element | null {
    if (!part || !content) return null;
    // Ancre d'un sommaire ou d'une note : son id (ou son ancien attribut name).
    const v = part.replace(/["\\]/g, "\\$&");
    try {
      return content.querySelector(`[id="${v}"], [name="${v}"]`);
    } catch {
      return null;
    }
  }

  // --- Chargement : chapitre (le livre), puis ses sections ---

  async function open(chapterId: number) {
    error = null;
    loading = true;
    try {
      const next = await api.kavitaChapter(chapterId);
      info = next;
      saved = -1;
      api.kavitaBookToc(chapterId).then((items) => (toc = items)).catch(() => (toc = []));
      // L'endroit retenu sur l'appareil, s'il est dans la section où Kavita reprend.
      const kept = epubPosition(chapterId);
      await show(next.resumePage, kept && kept.page === next.resumePage ? { at: kept.at } : { at: 0 });
      saved = readCount();
    } catch (e) {
      error = errorText(e);
      loading = false;
    }
  }

  const cache = new Map<number, string>();
  let token = 0;

  async function fetchSection(n: number): Promise<string> {
    const key = (info?.chapterId ?? 0) * 100000 + n;
    const hit = cache.get(key);
    if (hit !== undefined) return hit;
    const html = await api.kavitaBookPage(info!.chapterId, n);
    cache.set(key, html);
    return html;
  }

  async function show(n: number, target: Target) {
    if (!info || !content) return;
    const mine = ++token;
    loading = true;
    try {
      const html = await fetchSection(n);
      if (mine !== token) return;
      section = n;
      moved = false;
      content.replaceChildren(cleanBookHtml(html));
      viewport!.scrollTop = 0;
      loading = false;
      await tick();
      layout(target);
      // La section suivante, chargée d'avance.
      if (n + 1 < info.pages) fetchSection(n + 1).catch(() => {});
    } catch (e) {
      if (mine === token) {
        error = errorText(e);
        loading = false;
      }
    }
  }

  // --- Progression : la section à Kavita (le nombre de sections à la toute fin), l'endroit sur l'appareil ---

  let saved = -1;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const atEnd = () => (settings.layout === "scroll" ? scrolledEnd : screen >= screens - 1);
  const readCount = () => (info && section >= info.pages - 1 && atEnd() ? info.pages : section);

  function flush() {
    clearTimeout(timer);
    const i = info;
    if (!i || loading) return;
    saveEpubPosition(i.chapterId, section, position());
    const count = readCount();
    if (count === saved) return;
    saved = count;
    api.kavitaProgress({ chapterId: i.chapterId, volumeId: i.volumeId, seriesId: i.seriesId, libraryId: i.libraryId, page: count }).catch(() => {});
  }

  $effect(() => {
    void [section, screen, scrolledEnd, scrollTick];
    if (!info) return;
    clearTimeout(timer);
    timer = setTimeout(flush, 800);
  });

  let scrollTick = $state(0);
  function onScroll() {
    if (!viewport || settings.layout !== "scroll") return;
    scrolledEnd = viewport.scrollTop + viewport.clientHeight >= viewport.scrollHeight - 4;
    scrollTick++;
  }

  // --- Navigation ---

  function close() {
    flush();
    books.reading = null;
  }

  async function goChapter(id: number) {
    flush();
    info = null;
    toc = [];
    content?.replaceChildren();
    await open(id);
  }

  function next() {
    if (!info || loading) return;
    moved = true;
    if (settings.layout === "scroll" && viewport && !scrolledEnd) {
      viewport.scrollBy({ top: viewport.clientHeight * 0.85 });
    } else if (settings.layout === "paged" && screen < screens - 1) {
      screen += 1;
    } else if (section < info.pages - 1) {
      show(section + 1, { at: 0 });
    } else {
      // Fin du livre : il est marqué lu, puis le suivant de la série s'ouvre (ou le lecteur se ferme).
      flush();
      if (info.nextChapter > 0) goChapter(info.nextChapter);
      else close();
    }
  }

  function prev() {
    if (!info || loading) return;
    moved = true;
    if (settings.layout === "scroll" && viewport && viewport.scrollTop > 0) {
      viewport.scrollBy({ top: -viewport.clientHeight * 0.85 });
    } else if (settings.layout === "paged" && screen > 0) {
      screen -= 1;
    } else if (section > 0) {
      show(section - 1, { end: true });
    } else if (info.prevChapter > 0) {
      goChapter(info.prevChapter);
    }
  }

  function goTo(page: number, part: string) {
    panel = null;
    if (!info) return;
    const n = Math.min(info.pages - 1, Math.max(0, page));
    if (n === section && content?.childNodes.length) layout(part ? { part } : { at: 0 });
    else show(n, part ? { part } : { at: 0 });
  }

  /** Liens du livre : internes (Kavita les marque « kavita-page »), ou vers le web, ouverts dans le navigateur. */
  function onContentClick(e: Event) {
    const a = (e.target as Element | null)?.closest?.("a");
    if (!a) return;
    e.preventDefault();
    e.stopPropagation();
    const page = a.getAttribute("kavita-page");
    if (page !== null) goTo(Number(page), a.getAttribute("kavita-part") ?? "");
    else {
      const href = a.getAttribute("href") ?? "";
      if (/^https?:\/\//i.test(href)) invoke("open_url", { url: href }).catch(() => {});
    }
  }

  function onKey(e: KeyboardEvent) {
    if (!books.reading) return;
    const k = e.key;
    if (k === "Escape" || k === "Backspace" || k === "GoBack" || k === "BrowserBack") {
      if (panel) panel = null;
      else close();
    } else if (panel) return;
    else if (k === "ArrowRight" || k === "PageDown" || k === " ") next();
    else if (k === "ArrowLeft" || k === "PageUp") prev();
    else if ((k === "ArrowDown" || k === "ArrowUp") && settings.layout === "scroll") viewport?.scrollBy({ top: k === "ArrowDown" ? 60 : -60 });
    else if (k === "ArrowDown") next();
    else if (k === "ArrowUp") prev();
    else return;
    e.preventDefault();
  }

  /** Un tiers de l'écran de chaque côté tourne la page ; le milieu montre ou cache les commandes. */
  function onTap(e: MouseEvent) {
    if (panel) {
      panel = null;
      return;
    }
    const x = e.clientX / window.innerWidth;
    if (x < 1 / 3) prev();
    else if (x > 2 / 3) next();
    else controls = !controls;
  }

  let touchX: number | null = null;
  function onTouchStart(e: TouchEvent) {
    touchX = e.touches[0]?.clientX ?? null;
  }
  function onTouchEnd(e: TouchEvent) {
    const end = e.changedTouches[0]?.clientX;
    if (touchX === null || end === undefined || settings.layout === "scroll") return;
    const dx = end - touchX;
    touchX = null;
    if (Math.abs(dx) < 50) return;
    if (dx < 0) next();
    else prev();
  }

  /** Entrée du sommaire où l'on se trouve : la dernière qui commence à la section en cours ou avant. */
  const flatToc = $derived.by(() => {
    const out: { item: BookTocItem; depth: number }[] = [];
    const walk = (items: BookTocItem[], depth: number) => {
      for (const item of items) {
        out.push({ item, depth });
        walk(item.children, depth + 1);
      }
    };
    walk(toc, 0);
    return out;
  });
  const currentToc = $derived(flatToc.reduce<BookTocItem | null>((cur, e) => (e.item.page <= section ? e.item : cur), null));

  const title = $derived(info ? [info.seriesName, info.title].filter(Boolean).join(" — ") : "");
  const layouts: [EpubLayout, string][] = [
    ["paged", "books.paged"],
    ["scroll", "books.scroll"],
  ];
  const themes: [EpubTheme, string][] = [
    ["dark", "books.themeDark"],
    ["light", "books.themeLight"],
    ["sepia", "books.themeSepia"],
  ];
</script>

<svelte:window onkeydown={onKey} />

<div class="reader" role="dialog" aria-modal="true" aria-label={title || t("books.reader")} style:background={THEMES[settings.theme].bg}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="stage" onclick={onTap} ontouchstart={onTouchStart} ontouchend={onTouchEnd}>
    <div class="book" bind:this={host}></div>
  </div>

  {#if error}
    <div class="center"><ErrorCard message={error} /><button class="btn" onclick={close}>{t("common.c1.close")}</button></div>
  {:else if loading}
    <div class="center veil"><div class="spinner"></div></div>
  {/if}

  {#if info && controls}
    <div class="bar top">
      <button class="icon-btn" onclick={close} aria-label={t("common.c1.close")} title={t("common.c1.close")}><Icon name="x" size={22} /></button>
      <span class="title">{title}</span>
      {#if toc.length}
        <button class="icon-btn" class:on={panel === "toc"} onclick={() => (panel = panel === "toc" ? null : "toc")} aria-label={t("books.toc")} title={t("books.toc")} aria-expanded={panel === "toc"}>
          <Icon name="queue" size={20} />
        </button>
      {/if}
      <button
        class="icon-btn"
        class:on={panel === "settings"}
        onclick={() => (panel = panel === "settings" ? null : "settings")}
        aria-label={t("books.display")}
        title={t("books.display")}
        aria-expanded={panel === "settings"}
      >
        <Icon name="sliders" size={20} />
      </button>
    </div>
    <div class="bar bottom">
      {#if info.prevChapter > 0}<button class="icon-btn" onclick={() => goChapter(info!.prevChapter)} aria-label={t("books.prevChapter")} title={t("books.prevChapter")}><Icon name="skipPrev" size={20} /></button>{/if}
      <input
        type="range"
        min="0"
        max={Math.max(0, info.pages - 1)}
        value={section}
        aria-label={t("books.section")}
        onchange={(e) => goTo(Number((e.target as HTMLInputElement).value), "")}
      />
      <span class="count">
        {t("books.sectionOf", { section: section + 1, sections: info.pages })}{settings.layout === "paged" && screens > 1 ? ` · ${screen + 1}/${screens}` : ""}
      </span>
      {#if info.nextChapter > 0}<button class="icon-btn" onclick={() => goChapter(info!.nextChapter)} aria-label={t("books.nextChapter")} title={t("books.nextChapter")}><Icon name="skipNext" size={20} /></button>{/if}
    </div>
  {/if}

  {#if panel === "toc"}
    <nav class="panel toc" aria-label={t("books.toc")}>
      <h2>{t("books.toc")}</h2>
      <ul>
        {#each flatToc as { item, depth }, i (i)}
          <li>
            <button style:padding-left="{12 + depth * 16}px" class:current={item === currentToc} aria-current={item === currentToc ? "location" : undefined} onclick={() => goTo(item.page, item.part)}>
              {item.title || t("books.untitled")}
            </button>
          </li>
        {/each}
      </ul>
    </nav>
  {:else if panel === "settings"}
    <div class="panel display" role="group" aria-label={t("books.display")}>
      <h2>{t("books.display")}</h2>
      <div class="setting">
        <span>{t("books.textSize")}</span>
        <div class="size">
          <button class="icon-btn small" onclick={() => (settings.size = Math.max(EPUB_SIZES.min, settings.size - EPUB_SIZES.step))} disabled={settings.size <= EPUB_SIZES.min} aria-label={t("books.smaller")} title={t("books.smaller")}>A−</button>
          <output>{settings.size} %</output>
          <button class="icon-btn small" onclick={() => (settings.size = Math.min(EPUB_SIZES.max, settings.size + EPUB_SIZES.step))} disabled={settings.size >= EPUB_SIZES.max} aria-label={t("books.larger")} title={t("books.larger")}>A+</button>
        </div>
      </div>
      <div class="setting">
        <span>{t("books.theme")}</span>
        <div class="modes" role="radiogroup" aria-label={t("books.theme")}>
          {#each themes as [id, label] (id)}
            <button role="radio" aria-checked={settings.theme === id} class:on={settings.theme === id} onclick={() => (settings.theme = id)}>{t(label)}</button>
          {/each}
        </div>
      </div>
      <div class="setting">
        <span>{t("books.layout")}</span>
        <div class="modes" role="radiogroup" aria-label={t("books.layout")}>
          {#each layouts as [id, label] (id)}
            <button role="radio" aria-checked={settings.layout === id} class:on={settings.layout === id} onclick={() => (settings.layout = id)}>{t(label)}</button>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .reader {
    position: fixed;
    inset: 0;
    z-index: 80;
    color: #fff;
    user-select: none;
  }

  .stage,
  .book {
    width: 100%;
    height: 100%;
  }

  .center {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    align-content: center;
    gap: 16px;
    padding: 24px;
    background: #000;
  }

  .veil {
    background: #0006;
    pointer-events: none;
  }

  .spinner {
    width: 40px;
    height: 40px;
    border: 3px solid #fff3;
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .bar {
    position: absolute;
    right: 0;
    left: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 16px;
    background: #000c;
    backdrop-filter: blur(10px);
  }

  .top {
    top: 0;
    padding-top: calc(10px + var(--inset-top, 0px));
  }

  .bottom {
    bottom: 0;
    padding-bottom: calc(10px + var(--inset-bottom, 0px));
  }

  .title {
    flex: 1;
    overflow: hidden;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .icon-btn {
    display: grid;
    flex-shrink: 0;
    place-items: center;
    width: 40px;
    height: 40px;
    border: none;
    border-radius: 50%;
    background: #ffffff14;
    color: #fff;
  }

  .icon-btn:hover,
  .icon-btn.on {
    background: #ffffff2a;
  }

  .icon-btn.small {
    width: 44px;
    border-radius: 999px;
    font-weight: 700;
  }

  .icon-btn:disabled {
    opacity: 0.4;
  }

  input[type="range"] {
    flex: 1;
    min-width: 0;
    accent-color: var(--accent);
  }

  .count {
    color: #fffc;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    text-align: right;
    white-space: nowrap;
  }

  .panel {
    position: absolute;
    top: calc(64px + var(--inset-top, 0px));
    right: 12px;
    width: min(360px, calc(100% - 24px));
    max-height: calc(100% - 150px - var(--inset-top, 0px) - var(--inset-bottom, 0px));
    overflow-y: auto;
    padding: 16px;
    border: 1px solid #ffffff1f;
    border-radius: 14px;
    background: #111e;
    backdrop-filter: blur(14px);
    box-shadow: 0 12px 40px #0008;
  }

  .panel h2 {
    margin: 0 0 12px;
    font-size: 15px;
  }

  .toc ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .toc button {
    width: 100%;
    padding: 9px 12px;
    border: none;
    border-radius: 8px;
    background: none;
    color: #fffd;
    font-size: 14px;
    text-align: left;
  }

  .toc button:hover {
    background: #ffffff14;
  }

  .toc button.current {
    color: var(--accent);
    font-weight: 700;
  }

  .setting {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px 12px;
    padding: 8px 0;
  }

  .size {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .size output {
    min-width: 52px;
    font-variant-numeric: tabular-nums;
    text-align: center;
  }

  .modes {
    display: flex;
    overflow: hidden;
    border: 1px solid #ffffff2a;
    border-radius: 999px;
  }

  .modes button {
    padding: 7px 12px;
    border: none;
    background: none;
    color: #fffc;
    font-size: 13px;
    font-weight: 600;
  }

  .modes button.on {
    background: var(--accent);
    color: var(--accent-text);
  }
</style>
