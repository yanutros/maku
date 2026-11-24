<script lang="ts">
  // Lecteur de BD et de mangas (Kavita), sur le modèle des lecteurs les plus répandus (Kavita, Komga, Mihon) :
  // - la page entière dans l'écran, tournée par un clic sur un côté, les flèches ou un glissé du doigt ; de droite
  //   à gauche pour un manga ;
  // - deux pages côte à côte sur un écran plus large que haut (couverture et doubles pages seules) ;
  // - zoom : double-clic (ou double appui), pincer, Ctrl + molette ; zoomé, on se déplace en glissant ou à la
  //   molette, et les côtés ne tournent plus la page ;
  // - « Largeur » : la page prend toute la largeur et défile ; arrivé en bas, on passe à la suivante ;
  // - « Vertical » : toutes les pages les unes sous les autres (webtoon).
  // Sens, pages simples ou doubles et ajustement sont retenus par série. Les PDF passent par le même lecteur
  // (Kavita en sert les pages en images). La progression part à Kavita à chaque page (avec un court délai) et à la
  // fermeture.
  import { onMount } from "svelte";
  import { api, errorText, type BookChapterInfo } from "../lib/api";
  import {
    books,
    directionFor,
    layoutFor,
    pageUrl,
    setDirection,
    setLayout,
    spreads,
    type Direction,
    type Fit,
    type PageLayout,
    type Spread,
  } from "../lib/books.svelte";
  import { t } from "../lib/i18n";
  import ErrorCard from "./ErrorCard.svelte";
  import Icon from "./Icon.svelte";

  let info = $state<BookChapterInfo | null>(null);
  /** Page affichée (la première, en double page), à partir de 0. */
  let page = $state(0);
  let direction = $state<Direction>("ltr");
  let layout = $state<PageLayout>({ spread: "auto", fit: "screen" });
  let controls = $state(true);
  let panel = $state(false);
  let error = $state<string | null>(null);
  let scroller = $state<HTMLElement>();
  let stage = $state<HTMLElement>();
  let landscape = $state(window.innerWidth > window.innerHeight);

  async function open(chapterId: number) {
    error = null;
    try {
      const next = await api.kavitaChapter(chapterId);
      info = next;
      direction = directionFor(next.seriesId, next.libraryKind);
      layout = layoutFor(next.seriesId);
      page = next.resumePage;
      // En double page, la reprise se fait sur la paire qui contient la page.
      page = shown[0] ?? page;
      saved = readCount();
      if (direction === "vertical") requestAnimationFrame(() => scrollToPage(page));
    } catch (e) {
      error = errorText(e);
    }
  }
  onMount(() => {
    if (books.reading) open(books.reading.chapterId);
    return () => flush();
  });

  // --- Pages affichées : une seule, ou une paire en double page ---

  const double = $derived(
    !!info && direction !== "vertical" && layout.fit === "screen" && (layout.spread === "double" || (layout.spread === "auto" && landscape)),
  );
  const views = $derived(info ? (double ? spreads(info.pages, info.wide) : Array.from({ length: info.pages }, (_, i) => [i])) : []);
  const at = $derived(Math.max(0, views.findIndex((v) => v.includes(page))));
  const shown = $derived(views[at] ?? [page]);
  /** Dans l'ordre de l'écran : en manga, la première page est à droite. */
  const onScreen = $derived(direction === "rtl" ? [...shown].reverse() : shown);

  // --- Progression : envoyée 800 ms après le dernier changement de page, et à la fermeture ---

  let saved = -1;
  let timer: ReturnType<typeof setTimeout> | undefined;

  /** Pages lues : la page en cours, ou toutes quand la dernière est à l'écran (chapitre fini). */
  function readCount() {
    if (!info) return 0;
    return shown[shown.length - 1] >= info.pages - 1 ? info.pages : page;
  }

  function flush() {
    clearTimeout(timer);
    const i = info;
    if (!i) return;
    const count = readCount();
    if (count === saved) return;
    saved = count;
    api.kavitaProgress({ chapterId: i.chapterId, volumeId: i.volumeId, seriesId: i.seriesId, libraryId: i.libraryId, page: count }).catch(() => {});
  }

  $effect(() => {
    void page;
    void double;
    if (!info) return;
    clearTimeout(timer);
    timer = setTimeout(flush, 800);
  });

  // Pages voisines chargées d'avance, pour tourner sans attendre.
  $effect(() => {
    if (!info || direction === "vertical") return;
    for (const v of [views[at + 1], views[at + 2], views[at - 1]]) for (const p of v ?? []) new Image().src = pageUrl(info.pageUrl, p);
  });

  function close() {
    flush();
    books.reading = null;
  }

  async function goChapter(id: number) {
    flush();
    info = null;
    resetZoom();
    await open(id);
  }

  // --- Tourner les pages ---

  /** Largeur : où se placer dans la page qu'on ouvre (en haut en avançant, en bas en reculant). */
  let edge: "top" | "bottom" = "top";

  function turn(delta: 1 | -1) {
    if (!info) return;
    const target = at + delta;
    if (target >= views.length) {
      // Dernière page : le chapitre suivant, ou la fin.
      if (info.nextChapter > 0) goChapter(info.nextChapter);
      else close();
    } else if (target < 0) {
      if (info.prevChapter > 0) goChapter(info.prevChapter);
    } else {
      page = views[target][0];
      resetZoom();
      edge = delta > 0 ? "top" : "bottom";
      requestAnimationFrame(placeEdge);
    }
  }

  const fitWidth = () => layout.fit === "width" && direction !== "vertical";
  const atBottom = () => !!stage && stage.scrollTop + stage.clientHeight >= stage.scrollHeight - 2;

  function next() {
    // Largeur : on descend d'abord dans la page.
    if (fitWidth() && stage && !atBottom()) stage.scrollBy({ top: stage.clientHeight * 0.85, behavior: "smooth" });
    else turn(1);
  }

  function prev() {
    if (fitWidth() && stage && stage.scrollTop > 0) stage.scrollBy({ top: -stage.clientHeight * 0.85, behavior: "smooth" });
    else turn(-1);
  }

  function placeEdge() {
    if (!stage || !fitWidth()) return;
    stage.scrollTop = edge === "top" ? 0 : stage.scrollHeight;
  }

  /** Côté gauche ou droit de l'écran : en manga (droite à gauche), la gauche avance. */
  const leftward = () => (direction === "rtl" ? next() : prev());
  const rightward = () => (direction === "rtl" ? prev() : next());

  function onKey(e: KeyboardEvent) {
    if (!books.reading) return;
    const k = e.key;
    if (k === "Escape" || k === "Backspace" || k === "GoBack" || k === "BrowserBack") {
      if (panel) panel = false;
      else if (zoom > 1) resetZoom();
      else close();
    } else if (direction === "vertical") return;
    else if (k === "ArrowLeft") leftward();
    else if (k === "ArrowRight") rightward();
    else if (k === " " || k === "PageDown" || k === "ArrowDown") next();
    else if (k === "PageUp" || k === "ArrowUp") prev();
    else if (k === "Home") page = 0;
    else if (k === "End" && info) page = views[views.length - 1]?.[0] ?? info.pages - 1;
    else if ((k === "+" || k === "=") && layout.fit === "screen") zoomAtCenter(zoom * 1.5);
    else if (k === "-" && layout.fit === "screen") zoomAtCenter(zoom / 1.5);
    else return;
    e.preventDefault();
  }

  // --- Zoom : double-clic, pincer, Ctrl + molette ; zoomé, on se déplace en glissant ---

  let zoom = $state(1);
  let tx = $state(0);
  let ty = $state(0);
  const MAX_ZOOM = 4;

  function resetZoom() {
    zoom = 1;
    tx = 0;
    ty = 0;
  }

  /** Taille de la page (ou de la paire) sans zoom, et celle de l'écran. */
  function sizes() {
    const box = stage!.getBoundingClientRect();
    let [left, top, right, bottom] = [Infinity, Infinity, -Infinity, -Infinity];
    for (const img of stage!.querySelectorAll("img")) {
      const r = img.getBoundingClientRect();
      [left, top, right, bottom] = [Math.min(left, r.left), Math.min(top, r.top), Math.max(right, r.right), Math.max(bottom, r.bottom)];
    }
    const w = right > left ? (right - left) / zoom : box.width;
    const h = bottom > top ? (bottom - top) / zoom : box.height;
    return { box, w, h };
  }

  /** Déplacement borné : la page ne quitte jamais l'écran. */
  function clampPan(z: number, x: number, y: number): [number, number] {
    const { box, w, h } = sizes();
    const mx = Math.max(0, (w * z - box.width) / 2);
    const my = Math.max(0, (h * z - box.height) / 2);
    return [Math.min(mx, Math.max(-mx, x)), Math.min(my, Math.max(-my, y))];
  }

  /** Zoom `z` en gardant immobile le point (cx, cy) de l'écran. */
  function zoomAt(cx: number, cy: number, z: number) {
    if (!stage) return;
    const z1 = Math.min(MAX_ZOOM, Math.max(1, z));
    if (z1 <= 1.02) return resetZoom();
    const box = stage.getBoundingClientRect();
    const dx = cx - (box.left + box.width / 2);
    const dy = cy - (box.top + box.height / 2);
    const k = z1 / zoom;
    [tx, ty] = clampPan(z1, dx - (dx - tx) * k, dy - (dy - ty) * k);
    zoom = z1;
  }

  function zoomAtCenter(z: number) {
    if (!stage) return;
    const box = stage.getBoundingClientRect();
    zoomAt(box.left + box.width / 2, box.top + box.height / 2, z);
  }

  function pan(dx: number, dy: number) {
    [tx, ty] = clampPan(zoom, tx + dx, ty + dy);
  }

  // Clic : un côté tourne la page, le milieu montre ou cache les commandes. Un double-clic zoome : le clic simple
  // attend donc un instant qu'un second clic ne suive pas (comme Mihon).
  let tapTimer: ReturnType<typeof setTimeout> | undefined;
  let lastTap = { t: 0, x: 0, y: 0 };
  /** Un glissé (page zoomée) vient de finir : le clic qui suit n'en est pas un. */
  let dragged = false;

  function onStageClick(e: MouseEvent) {
    if (dragged) {
      dragged = false;
      return;
    }
    if (panel) {
      panel = false;
      return;
    }
    if (layout.fit !== "screen") return tapAction(e.clientX);
    const now = Date.now();
    if (tapTimer && now - lastTap.t < 300 && Math.hypot(e.clientX - lastTap.x, e.clientY - lastTap.y) < 40) {
      clearTimeout(tapTimer);
      tapTimer = undefined;
      if (zoom > 1) resetZoom();
      else zoomAt(e.clientX, e.clientY, 2.5);
      return;
    }
    lastTap = { t: now, x: e.clientX, y: e.clientY };
    const x = e.clientX;
    tapTimer = setTimeout(() => {
      tapTimer = undefined;
      tapAction(x);
    }, 300);
  }

  function tapAction(clientX: number) {
    const x = clientX / window.innerWidth;
    // Zoomé, les côtés ne tournent pas la page : on lit, on se déplace.
    if (zoom > 1 || (x >= 1 / 3 && x <= 2 / 3)) controls = !controls;
    else if (x < 1 / 3) leftward();
    else rightward();
  }

  // Souris : glisser déplace la page zoomée.
  let drag: { x: number; y: number } | null = null;
  function onPointerDown(e: PointerEvent) {
    dragged = false;
    if (e.pointerType !== "mouse" || zoom <= 1) return;
    drag = { x: e.clientX, y: e.clientY };
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
  }
  function onPointerMove(e: PointerEvent) {
    if (!drag) return;
    const [dx, dy] = [e.clientX - drag.x, e.clientY - drag.y];
    if (Math.abs(dx) + Math.abs(dy) > 3) dragged = true;
    pan(dx, dy);
    drag = { x: e.clientX, y: e.clientY };
  }
  function onPointerUp() {
    drag = null;
  }

  // Doigts : glisser tourne la page (ou déplace la page zoomée), pincer zoome.
  let touchStart: { x: number; y: number } | null = null;
  let touchLast: { x: number; y: number } | null = null;
  let pinch: { d: number } | null = null;
  const distance = (a: Touch, b: Touch) => Math.hypot(a.clientX - b.clientX, a.clientY - b.clientY);

  function onTouchStart(e: TouchEvent) {
    dragged = false;
    const [a, b] = [e.touches[0], e.touches[1]];
    if (a && b && layout.fit === "screen") {
      pinch = { d: distance(a, b) };
      touchStart = null;
    } else if (a && e.touches.length === 1) {
      touchStart = touchLast = { x: a.clientX, y: a.clientY };
    }
  }
  function onTouchMove(e: TouchEvent) {
    const [a, b] = [e.touches[0], e.touches[1]];
    if (pinch && a && b) {
      const d = distance(a, b);
      zoomAt((a.clientX + b.clientX) / 2, (a.clientY + b.clientY) / 2, zoom * (d / pinch.d));
      pinch.d = d;
      dragged = true;
    } else if (a && touchLast && zoom > 1) {
      pan(a.clientX - touchLast.x, a.clientY - touchLast.y);
      touchLast = { x: a.clientX, y: a.clientY };
      dragged = true;
    }
  }
  function onTouchEnd(e: TouchEvent) {
    if (pinch) {
      if (e.touches.length < 2) pinch = null;
      touchStart = null;
      return;
    }
    const end = e.changedTouches[0];
    const start = touchStart;
    touchStart = touchLast = null;
    if (!start || !end || zoom > 1) return;
    const [dx, dy] = [end.clientX - start.x, end.clientY - start.y];
    if (Math.abs(dx) < 50 || Math.abs(dx) < Math.abs(dy)) return;
    // Vers la gauche, comme tourner la page de droite.
    if (dx < 0) rightward();
    else leftward();
  }

  // Molette : Ctrl + molette zoome ; zoomé, elle déplace la page ; sinon elle tourne les pages (un cran par geste).
  // En largeur, elle fait défiler la page, puis la tourne si l'on insiste une fois arrivé au bout.
  let lastWheel = 0;
  let lastScroll = 0;
  function onWheel(e: WheelEvent) {
    if (direction === "vertical") return;
    const now = Date.now();
    if (e.ctrlKey) {
      e.preventDefault();
      if (layout.fit === "screen") zoomAt(e.clientX, e.clientY, zoom * Math.exp(-e.deltaY * 0.01));
      return;
    }
    if (zoom > 1) {
      e.preventDefault();
      pan(-e.deltaX, -e.deltaY);
      return;
    }
    if (layout.fit === "width") {
      const stuck = (e.deltaY > 0 && atBottom()) || (e.deltaY < 0 && (stage?.scrollTop ?? 0) <= 0);
      if (stuck && now - lastScroll > 400) {
        e.preventDefault();
        lastScroll = now;
        if (e.deltaY > 0) turn(1);
        else turn(-1);
      }
      return;
    }
    e.preventDefault();
    const gap = now - lastWheel;
    lastWheel = now;
    if (gap < 250 || Math.abs(e.deltaY) < 1) return;
    if (e.deltaY > 0) next();
    else prev();
  }

  // La molette doit pouvoir empêcher le zoom de la fenêtre entière : écouteur non passif.
  $effect(() => {
    const el = stage;
    if (!el) return;
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  });

  // --- Réglages : sens, pages simples ou doubles, ajustement ---

  function changeDirection(d: Direction) {
    direction = d;
    resetZoom();
    if (info) setDirection(info.seriesId, d);
    if (d === "vertical") requestAnimationFrame(() => scrollToPage(page));
  }

  function changeLayout(patch: Partial<PageLayout>) {
    layout = { ...layout, ...patch };
    resetZoom();
    if (info) setLayout(info.seriesId, layout);
    page = shown[0] ?? page;
    edge = "top";
    requestAnimationFrame(placeEdge);
  }

  // --- Défilement vertical : la page la plus visible est la page en cours ---

  function scrollToPage(p: number) {
    scroller?.querySelector<HTMLElement>(`[data-page="${p}"]`)?.scrollIntoView({ block: "start" });
  }

  function onScroll() {
    if (!scroller) return;
    const top = scroller.scrollTop + scroller.clientHeight / 3;
    let current = 0;
    for (const el of scroller.querySelectorAll<HTMLElement>("[data-page]")) {
      if (el.offsetTop <= top) current = Number(el.dataset.page);
      else break;
    }
    if (current !== page) page = current;
  }

  const title = $derived(info ? [info.seriesName, info.title].filter(Boolean).join(" — ") : "");
  const pageLabel = $derived(shown.length > 1 ? `${shown[0] + 1}-${shown[shown.length - 1] + 1}` : String(page + 1));
  const directions: [Direction, string][] = [
    ["ltr", "books.ltr"],
    ["rtl", "books.rtl"],
    ["vertical", "books.vertical"],
  ];
  const fits: [Fit, string][] = [
    ["screen", "books.fitScreen"],
    ["width", "books.fitWidth"],
  ];
  const spreadModes: [Spread, string][] = [
    ["auto", "books.spreadAuto"],
    ["single", "books.spreadSingle"],
    ["double", "books.spreadDouble"],
  ];
</script>

<svelte:window onkeydown={onKey} onresize={() => (landscape = window.innerWidth > window.innerHeight)} />

<div class="reader" role="dialog" aria-modal="true" aria-label={title || t("books.reader")}>
  {#if error}
    <div class="center"><ErrorCard message={error} /><button class="btn" onclick={close}>{t("common.c1.close")}</button></div>
  {:else if !info}
    <div class="center"><div class="spinner"></div></div>
  {:else if direction === "vertical"}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="strip" bind:this={scroller} onscroll={onScroll} onclick={() => (panel ? (panel = false) : (controls = !controls))}>
      {#each Array.from({ length: info.pages }, (_, i) => i) as i (i)}
        <img data-page={i} src={pageUrl(info.pageUrl, i)} alt={t("books.pageAlt", { page: i + 1 })} loading="lazy" />
      {/each}
      {#if info.nextChapter > 0}
        <button class="btn primary next-chapter" onclick={(e) => (e.stopPropagation(), goChapter(info!.nextChapter))}>{t("books.nextChapter")}</button>
      {/if}
    </div>
  {:else}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
      class="stage"
      class:width={layout.fit === "width"}
      class:zoomed={zoom > 1}
      bind:this={stage}
      onclick={onStageClick}
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      ontouchstart={onTouchStart}
      ontouchmove={onTouchMove}
      ontouchend={onTouchEnd}
      onscroll={() => (lastScroll = Date.now())}
    >
      <div class="view" class:double={shown.length > 1} style:transform={zoom > 1 ? `translate(${tx}px, ${ty}px) scale(${zoom})` : undefined}>
        {#each onScreen as p (p)}
          <img src={pageUrl(info.pageUrl, p)} alt={t("books.pageAlt", { page: p + 1 })} draggable="false" onload={() => edge === "bottom" && placeEdge()} />
        {/each}
      </div>
    </div>
  {/if}

  {#if info && controls}
    <div class="bar top">
      <button class="icon-btn" onclick={close} aria-label={t("common.c1.close")} title={t("common.c1.close")}><Icon name="x" size={22} /></button>
      <span class="title">{title}</span>
      <button class="icon-btn" class:on={panel} onclick={() => (panel = !panel)} aria-label={t("books.display")} title={t("books.display")} aria-expanded={panel}>
        <Icon name="sliders" size={20} />
      </button>
    </div>
    <div class="bar bottom">
      {#if info.prevChapter > 0}<button class="icon-btn" onclick={() => goChapter(info!.prevChapter)} aria-label={t("books.prevChapter")} title={t("books.prevChapter")}><Icon name="skipPrev" size={20} /></button>{/if}
      <input
        type="range"
        min="0"
        max={Math.max(0, info.pages - 1)}
        value={page}
        dir={direction === "rtl" ? "rtl" : "ltr"}
        aria-label={t("books.page")}
        oninput={(e) => {
          const v = Number((e.target as HTMLInputElement).value);
          page = (views.find((x) => x.includes(v)) ?? [v])[0];
          resetZoom();
          if (direction === "vertical") scrollToPage(page);
        }}
      />
      <span class="count">{t("books.pageOf", { page: pageLabel, pages: info.pages })}</span>
      {#if info.nextChapter > 0}<button class="icon-btn" onclick={() => goChapter(info!.nextChapter)} aria-label={t("books.nextChapter")} title={t("books.nextChapter")}><Icon name="skipNext" size={20} /></button>{/if}
    </div>
  {/if}

  {#if info && panel}
    <div class="panel" role="group" aria-label={t("books.display")}>
      <h2>{t("books.display")}</h2>
      <div class="setting">
        <span>{t("books.direction")}</span>
        <div class="modes" role="radiogroup" aria-label={t("books.direction")}>
          {#each directions as [d, label] (d)}
            <button role="radio" aria-checked={direction === d} class:on={direction === d} onclick={() => changeDirection(d)}>{t(label)}</button>
          {/each}
        </div>
      </div>
      {#if direction !== "vertical"}
        <div class="setting">
          <span>{t("books.fit")}</span>
          <div class="modes" role="radiogroup" aria-label={t("books.fit")}>
            {#each fits as [f, label] (f)}
              <button role="radio" aria-checked={layout.fit === f} class:on={layout.fit === f} onclick={() => changeLayout({ fit: f })}>{t(label)}</button>
            {/each}
          </div>
        </div>
        {#if layout.fit === "screen"}
          <div class="setting">
            <span>{t("books.spread")}</span>
            <div class="modes" role="radiogroup" aria-label={t("books.spread")}>
              {#each spreadModes as [s, label] (s)}
                <button role="radio" aria-checked={layout.spread === s} class:on={layout.spread === s} onclick={() => changeLayout({ spread: s })}>{t(label)}</button>
              {/each}
            </div>
          </div>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .reader {
    position: fixed;
    inset: 0;
    z-index: 80;
    background: #000;
    color: #fff;
    user-select: none;
  }

  .center {
    display: grid;
    place-items: center;
    gap: 16px;
    height: 100%;
    padding: 24px;
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

  /* Page entière : la page (ou la paire) tient dans l'écran, centrée. */
  .stage {
    width: 100%;
    height: 100%;
    overflow: hidden;
    cursor: pointer;
    touch-action: none;
  }

  .stage.zoomed {
    cursor: grab;
  }

  .view {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    transform-origin: center;
  }

  .view img {
    flex: 0 1 auto;
    min-width: 0;
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }

  .view.double img {
    max-width: 50%;
  }

  /* Largeur : la page prend toute la largeur et défile. */
  .stage.width {
    overflow-y: auto;
    overscroll-behavior: contain;
    touch-action: pan-y;
  }

  .stage.width .view {
    display: block;
    height: auto;
  }

  .stage.width .view img {
    display: block;
    width: 100%;
    max-height: none;
  }

  .strip {
    height: 100%;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .strip img {
    display: block;
    width: min(100%, 900px);
    min-height: 200px;
    margin: 0 auto;
  }

  .next-chapter {
    display: block;
    margin: 32px auto 64px;
  }

  .bar {
    position: absolute;
    right: 0;
    left: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: #000c;
    backdrop-filter: blur(10px);
  }

  .top {
    top: 0;
    padding-top: calc(12px + var(--inset-top, 0px));
  }

  .bottom {
    bottom: 0;
    padding-bottom: calc(12px + var(--inset-bottom, 0px));
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

  .panel {
    position: absolute;
    top: calc(68px + var(--inset-top, 0px));
    right: 12px;
    /* Assez large pour chaque réglage sur une ligne (libellé à gauche, boutons à droite). */
    width: min(500px, calc(100% - 24px));
    padding: 16px;
    border: 1px solid #ffffff1f;
    border-radius: 14px;
    background: #111e;
    backdrop-filter: blur(14px);
    box-shadow: 0 12px 40px #0008;
  }

  .panel h2 {
    margin: 0 0 8px;
    font-size: 15px;
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
  .modes {
    display: flex;
    flex: none;
    margin-left: auto;
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

  :global(html.phone) .modes button {
    padding: 7px 9px;
    font-size: 12px;
  }

  input[type="range"] {
    flex: 1;
    min-width: 0;
    accent-color: var(--accent);
  }

  .count {
    min-width: 72px;
    color: #fffc;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    text-align: right;
    white-space: nowrap;
  }
</style>
