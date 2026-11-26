<script lang="ts">
  // Lecteur de livres EPUB (Kavita). Kavita découpe un livre en sections (les « pages » de sa progression) et sert
  // le HTML de chacune, styles du livre compris. Maku l'affiche à part, dans un shadow DOM (les styles du livre ne
  // touchent pas l'interface), nettoyé (ni script ni formulaire), en pages tournées (une ou deux colonnes) ou en
  // défilement, en plein écran si l'on veut. Police, taille du texte, interligne, marges, thème et présentation, comme
  // dans le lecteur de Kavita, sont retenus par bibliothèque ; Kavita ne gardant que la section, l'endroit dans la
  // section est retenu sur l'appareil.
  import { onMount, tick, untrack } from "svelte";
  import { fly } from "svelte/transition";
  import "./readerBars.css";
  import { invoke } from "@tauri-apps/api/core";
  import { api, errorText, type BookChapterInfo, type BookTocItem } from "../lib/api";
  import {
    books,
    cleanBookHtml,
    EPUB_FONTS,
    EPUB_LINE_HEIGHTS,
    EPUB_MARGINS,
    EPUB_SIZES,
    epubPosition,
    idleBars,
    epubSettings,
    saveEpubPosition,
    saveEpubSettings,
    setReaderFullscreen,
    type EpubFont,
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
  let fullscreen = $state(false);
  let loading = $state(true);
  /** Barres rentrées faute de mouvement de la souris (le curseur s'efface aussi). */
  let idle = $state(false);
  let overBars = false;
  const bars = idleBars(
    () => {
      controls = true;
      idle = false;
    },
    () => {
      controls = false;
      idle = true;
    },
    () => panel !== null || overBars || !!document.querySelector(".rb :focus-visible"),
  );
  let error = $state<string | null>(null);

  let host = $state<HTMLElement>();
  let viewport: HTMLElement | undefined;
  /** Les pages du lecteur : colonnes, taille du texte. Le livre (`content`, ses styles) est dedans, sans prise sur elles. */
  let pages: HTMLElement | undefined;
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
      /* Marges à gauche et à droite, réglables comme dans le lecteur de Kavita : le texte ne touche jamais le bord. */
      .viewport { --side: max(8px, ${s.margin}vw); }
      .viewport {
        box-sizing: border-box; width: 100%; height: 100%; overflow: hidden; background: ${c.bg}; color: ${c.fg};
        padding: calc(64px + var(--inset-top, 0px)) var(--side) calc(64px + var(--inset-bottom, 0px));
      }
      /* Les pages sont coupées au bord du texte, marges comprises : la colonne de l'écran suivant, déjà à côté, ne
         déborde plus dans la marge de droite. */
      .clip { height: 100%; overflow: hidden; }
      .scroll .clip { height: auto; overflow: visible; }
      .scroll { overflow-y: auto; overscroll-behavior: contain; }
      .scroll .pages { max-width: 46em; margin: 0 auto; }
      .pages {
        box-sizing: border-box; font-family: Georgia, "Times New Roman", serif; font-size: ${s.size}%;
        line-height: ${s.lineHeight}; overflow-wrap: break-word; hyphens: auto; color: ${c.fg};
      }
      /* Interligne choisi, même si le livre en impose un. */
      .book-content { line-height: ${s.lineHeight} !important; }
      .book-content * { line-height: inherit !important; }
      ${s.font === "book" ? "" : `.book-content, .book-content * { font-family: ${EPUB_FONTS[s.font]} !important; }`}
      /* Le livre ne règle ni la taille de son texte ni sa place : Kavita y reporte les marges et la taille du « body »
         de l'EPUB, qui écrasaient la taille choisie et décalaient les colonnes hors de l'écran. */
      .book-content {
        margin: 0 !important; padding: 0 !important; width: auto !important; max-width: none !important;
        font-size: 100% !important; box-sizing: border-box;
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
    pages = document.createElement("div");
    pages.className = "pages";
    content = document.createElement("div");
    content.className = "book-content";
    pages.append(content);
    const clip = document.createElement("div");
    clip.className = "clip";
    clip.append(pages);
    viewport.append(clip);
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
      bars.stop();
      flush();
      if (fullscreen) setReaderFullscreen(false);
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
    const library = untrack(() => info?.libraryId);
    if (library !== undefined) saveEpubSettings(library, s);
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
    if (!viewport || !pages || !content) return;
    lastTarget = target;
    if (settings.layout === "scroll") {
      pages.style.cssText = "";
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
    // Deux colonnes choisies, sur un écran assez large pour les lire.
    const perScreen = settings.columns === 2 && width >= 700 ? 2 : 1;
    const gap = perScreen === 2 ? 72 : 40;
    step = width + gap;
    Object.assign(pages.style, {
      width: `${width}px`,
      height: `${height}px`,
      columnCount: String(perScreen),
      columnGap: `${gap}px`,
      columnFill: "auto",
    });
    pages.style.setProperty("--page-h", `${height}px`);
    viewport.scrollTop = 0;
    const columnWidth = (width - gap * (perScreen - 1)) / perScreen;
    const columns = Math.max(1, Math.round((pages.scrollWidth + gap) / (columnWidth + gap)));
    screens = Math.max(1, Math.ceil(columns / perScreen));
    if ("end" in target) screen = screens - 1;
    else if ("at" in target) screen = Math.round(target.at * (screens - 1));
    else {
      const el = partElement(target.part);
      const x = el ? el.getBoundingClientRect().left - pages.getBoundingClientRect().left : 0;
      screen = Math.min(screens - 1, Math.max(0, Math.floor((x + 1) / step)));
    }
    place();
  }

  function place() {
    if (pages && settings.layout === "paged") pages.style.transform = `translateX(${-screen * step}px)`;
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
      // Les réglages de cette bibliothèque (sépia pour les romans, par exemple), retenus pour elle.
      settings = epubSettings(next.libraryId);
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
    if (fullscreen) setReaderFullscreen(false);
    books.reading = null;
  }

  /** Plein écran : rien autour du livre ; les commandes reviennent d'un appui au milieu de la page. */
  async function toggleFullscreen() {
    fullscreen = await setReaderFullscreen(!fullscreen);
    if (fullscreen) {
      panel = null;
      controls = false;
    } else {
      // En sortant du plein écran, la barre du haut et la progression reviennent.
      controls = true;
    }
  }

  /** Présentation, comme dans Kavita : défilement continu, une colonne, ou deux côte à côte. */
  type Presentation = "scroll" | "one" | "two";
  /** Écran trop étroit pour deux colonnes (téléphone) : le choix n'est pas proposé, une seule colonne s'affiche. */
  let narrow = $state(window.innerWidth < 700);
  const presentation = $derived<Presentation>(settings.layout === "scroll" ? "scroll" : settings.columns === 1 || narrow ? "one" : "two");
  function setPresentation(p: Presentation) {
    if (p === "scroll") settings.layout = "scroll";
    else {
      settings.layout = "paged";
      settings.columns = p === "one" ? 1 : 2;
    }
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
      else if (fullscreen && k === "Escape") toggleFullscreen();
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
  const presentations: [Presentation, string][] = [
    ["scroll", "books.presentScroll"],
    ["one", "books.presentOne"],
    ["two", "books.presentTwo"],
  ];
  const fonts: [EpubFont, string][] = [
    ["book", "books.fontBook"],
    ["serif", "books.fontSerif"],
    ["sans", "books.fontSans"],
    ["legible", "books.fontLegible"],
  ];
  const themes: [EpubTheme, string][] = [
    ["dark", "books.themeDark"],
    ["light", "books.themeLight"],
    ["sepia", "books.themeSepia"],
  ];
</script>

<svelte:window onkeydown={onKey} onpointermove={bars.wake} onresize={() => (narrow = window.innerWidth < 700)} />

<div
  class="reader"
  class:rb-idle={idle && !controls}
  role="dialog"
  aria-modal="true"
  aria-label={title || t("books.reader")}
  style:background={THEMES[settings.theme].bg}
>
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
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="rb rb-top" transition:fly={{ y: -64, duration: 150 }} onpointerenter={() => (overBars = true)} onpointerleave={() => (overBars = false)}>
      <button class="icon-btn" onclick={close} aria-label={t("common.c1.close")} title={t("common.c1.close")}><Icon name="x" size={22} /></button>
      <div class="rb-heading"><strong>{info.seriesName}</strong>{#if info.title}<span>{info.title}</span>{/if}</div>
      {#if toc.length}
        <button class="icon-btn" class:on={panel === "toc"} onclick={() => (panel = panel === "toc" ? null : "toc")} aria-label={t("books.toc")} title={t("books.toc")} aria-expanded={panel === "toc"}>
          <Icon name="queue" size={20} />
        </button>
      {/if}
      <button
        class="icon-btn"
        onclick={toggleFullscreen}
        aria-label={fullscreen ? t("books.exitFullscreen") : t("books.fullscreen")}
        title={fullscreen ? t("books.exitFullscreen") : t("books.fullscreen")}
        aria-pressed={fullscreen}
      >
        <Icon name={fullscreen ? "exitFullscreen" : "fullscreen"} size={20} />
      </button>
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
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="rb rb-bottom" transition:fly={{ y: 64, duration: 150 }} onpointerenter={() => (overBars = true)} onpointerleave={() => (overBars = false)}>
      {#if info.prevChapter > 0}<button class="rb-chip" onclick={() => goChapter(info!.prevChapter)} aria-label={t("books.prevChapter")} title={t("books.prevChapter")}><Icon name="skipPrev" size={16} /><span class="rb-label">{t("books.prevShort")}</span></button>{/if}
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
      {#if info.nextChapter > 0}<button class="rb-chip" onclick={() => goChapter(info!.nextChapter)} aria-label={t("books.nextChapter")} title={t("books.nextChapter")}><span class="rb-label">{t("books.nextShort")}</span><Icon name="skipNext" size={16} /></button>{/if}
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
        <label for="epub-font">{t("books.font")}</label>
        <select id="epub-font" class="control" bind:value={settings.font}>
          {#each fonts as [id, label] (id)}<option value={id}>{t(label)}</option>{/each}
        </select>
      </div>
      <div class="setting">
        <span>{t("books.textSize")}</span>
        <div class="size">
          <button class="icon-btn small" onclick={() => (settings.size = Math.max(EPUB_SIZES.min, settings.size - EPUB_SIZES.step))} disabled={settings.size <= EPUB_SIZES.min} aria-label={t("books.smaller")} title={t("books.smaller")}>A−</button>
          <output>{settings.size} %</output>
          <button class="icon-btn small" onclick={() => (settings.size = Math.min(EPUB_SIZES.max, settings.size + EPUB_SIZES.step))} disabled={settings.size >= EPUB_SIZES.max} aria-label={t("books.larger")} title={t("books.larger")}>A+</button>
        </div>
      </div>
      <div class="setting">
        <label for="epub-line">{t("books.lineHeight")}</label>
        <div class="slider control">
          <input id="epub-line" type="range" min={EPUB_LINE_HEIGHTS.min} max={EPUB_LINE_HEIGHTS.max} step={EPUB_LINE_HEIGHTS.step} bind:value={settings.lineHeight} />
          <output>{settings.lineHeight.toLocaleString(undefined, { minimumFractionDigits: 1, maximumFractionDigits: 1 })}</output>
        </div>
      </div>
      <div class="setting">
        <label for="epub-margin">{t("books.margin")}</label>
        <div class="slider control">
          <input id="epub-margin" type="range" min={EPUB_MARGINS.min} max={EPUB_MARGINS.max} step={EPUB_MARGINS.step} bind:value={settings.margin} />
          <output>{settings.margin} %</output>
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
        <span>{t("books.presentation")}</span>
        <div class="modes" role="radiogroup" aria-label={t("books.presentation")}>
          {#each presentations.filter(([id]) => !(narrow && id === "two")) as [id, label] (id)}
            <button role="radio" aria-checked={presentation === id} class:on={presentation === id} onclick={() => setPresentation(id)}>{t(label)}</button>
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
    width: min(420px, calc(100% - 24px));
    max-height: calc(100% - 150px - var(--inset-top, 0px) - var(--inset-bottom, 0px));
    overflow-y: auto;
    padding: 16px;
    border: 1px solid #ffffff1f;
    border-radius: 14px;
    background: #111e;
    backdrop-filter: blur(14px);
    box-shadow: 0 12px 40px #0008;
  }

  /* Affichage : la largeur du réglage le plus large, comme dans le lecteur de BD. */
  .panel.display {
    width: max-content;
    min-width: min(360px, calc(100% - 24px));
    max-width: calc(100% - 24px);
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

  /* Toujours à droite, même renvoyés sous leur libellé (écran étroit). */
  .size,
  .control,
  .modes {
    flex: 0 1 auto;
    margin-left: auto;
  }

  /* Police : une liste, comme dans Kavita. Interligne et marge : des curseurs, avec leur valeur. */
  select.control {
    padding: 7px 12px;
    border: 1px solid #ffffff2a;
    border-radius: 999px;
    background: #ffffff14;
    color: #fff;
    font: inherit;
    font-size: 13px;
    font-weight: 600;
  }

  select.control option {
    background: #1b1b1b;
    color: #fff;
  }

  .slider {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .slider input {
    width: 150px;
    max-width: 40vw;
    accent-color: var(--accent);
  }

  .slider output {
    min-width: 44px;
    font-variant-numeric: tabular-nums;
    text-align: right;
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

  /* Jamais plus large que le panneau : sur un écran très étroit, les boutons rétrécissent (avec « … » au besoin)
     au lieu de déborder. */
  .modes {
    max-width: 100%;
  }

  .modes button {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
