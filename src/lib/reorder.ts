// Réordonner une liste au doigt ou à la souris : maintenir un élément, puis le glisser vers le haut
// ou le bas. Les lignes portent `data-index` (leur position dans la file) ; un simple clic reste un clic.
// La liste elle-même défile, et défile toute seule quand on approche de son bord en glissant.

const HOLD_MS = 280;
/** Au-delà de ce déplacement avant la fin de l'appui, c'est un défilement : pas de glisser. */
const SLOP = 8;
const EDGE = 48;

export interface ReorderOptions {
  onmove: (from: number, to: number) => void;
}

export function reorder(list: HTMLElement, options: ReorderOptions) {
  let opts = options;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let start: { x: number; y: number; id: number } | null = null;
  let drag: {
    row: HTMLElement;
    rows: HTMLElement[];
    /** Position de la ligne saisie parmi les lignes affichées. */
    from: number;
    to: number;
    pitch: number;
    /** Écart entre le haut de la ligne et le doigt. */
    grab: number;
    y: number;
  } | null = null;
  let raf = 0;

  // Les positions des lignes (offsetTop) se mesurent dans la liste.
  if (getComputedStyle(list).position === "static") list.style.position = "relative";
  list.classList.add("reorderable");

  const rowsOf = () => [...list.querySelectorAll<HTMLElement>("[data-index]")];

  function begin(row: HTMLElement) {
    const rows = rowsOf();
    const from = rows.indexOf(row);
    if (from < 0 || !start) return;
    const r = row.getBoundingClientRect();
    const pitch = rows.length > 1 ? rows[1].offsetTop - rows[0].offsetTop : r.height;
    drag = { row, rows, from, to: from, pitch: pitch || r.height, grab: start.y - r.top, y: start.y };
    row.classList.add("reorder-lifted");
    list.classList.add("reorder-active");
    try {
      list.setPointerCapture(start.id);
    } catch {
      // Pointeur déjà relâché : le glisser s'arrêtera au prochain événement.
    }
    navigator.vibrate?.(12);
    layout();
    raf = requestAnimationFrame(autoScroll);
  }

  /** Place la ligne saisie sous le doigt et écarte les autres pour lui faire de la place. */
  function layout() {
    if (!drag) return;
    const { row, rows, from, pitch } = drag;
    const top = rows[0].offsetTop;
    const listTop = list.getBoundingClientRect().top;
    // Position de la ligne saisie dans le contenu de la liste (défilement compris).
    const y = drag.y - listTop + list.scrollTop - drag.grab;
    const to = Math.max(0, Math.min(rows.length - 1, Math.round((y - top) / pitch)));
    drag.to = to;
    row.style.transform = `translateY(${y - row.offsetTop}px)`;
    rows.forEach((r, i) => {
      if (r === row) return;
      const shift = from < to && i > from && i <= to ? -pitch : from > to && i >= to && i < from ? pitch : 0;
      r.style.transform = shift ? `translateY(${shift}px)` : "";
    });
  }

  /** Ce qui défile : la liste elle-même, ou la page qui la contient (téléphone). */
  function scroller(): HTMLElement {
    for (let el: HTMLElement | null = list; el; el = el.parentElement) {
      const o = getComputedStyle(el).overflowY;
      if ((o === "auto" || o === "scroll") && el.scrollHeight > el.clientHeight) return el;
    }
    return document.scrollingElement as HTMLElement;
  }

  function autoScroll() {
    if (!drag) return;
    const box = scroller();
    const r = box === document.scrollingElement ? { top: 0, bottom: innerHeight } : box.getBoundingClientRect();
    const top = Math.max(r.top, 0) + EDGE;
    const bottom = Math.min(r.bottom, innerHeight) - EDGE;
    const speed = drag.y < top ? -(top - drag.y) / 4 : drag.y > bottom ? (drag.y - bottom) / 4 : 0;
    if (speed) {
      box.scrollTop += speed;
      layout();
    }
    raf = requestAnimationFrame(autoScroll);
  }

  function finish(commit: boolean) {
    clearTimeout(timer);
    timer = undefined;
    start = null;
    if (!drag) return;
    cancelAnimationFrame(raf);
    const { row, rows, from, to } = drag;
    drag = null;
    for (const r of rows) r.style.transform = "";
    row.classList.remove("reorder-lifted");
    list.classList.remove("reorder-active");
    // Le clic qui suit le relâchement ne doit pas lancer le titre.
    const swallow = (e: Event) => (e.stopPropagation(), e.preventDefault());
    list.addEventListener("click", swallow, { capture: true, once: true });
    setTimeout(() => list.removeEventListener("click", swallow, { capture: true }), 0);
    if (commit && from !== to) {
      const base = Number(rows[0].dataset.index);
      opts.onmove(base + from, base + to);
    }
  }

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    const row = (e.target as HTMLElement).closest<HTMLElement>("[data-index]");
    if (!row || !list.contains(row)) return;
    start = { x: e.clientX, y: e.clientY, id: e.pointerId };
    clearTimeout(timer);
    timer = setTimeout(() => begin(row), HOLD_MS);
  }

  function move(e: PointerEvent) {
    if (drag) {
      drag.y = e.clientY;
      layout();
      return;
    }
    if (start && Math.hypot(e.clientX - start.x, e.clientY - start.y) > SLOP) finish(false);
  }

  // Pendant le glisser, le doigt ne fait plus défiler la page.
  function touchmove(e: TouchEvent) {
    if (drag) e.preventDefault();
  }

  // Appui long sur Android : pas de menu du système ni de sélection de texte.
  function contextmenu(e: Event) {
    if (drag || timer) e.preventDefault();
  }

  const up = () => finish(true);
  const cancel = () => finish(false);
  list.addEventListener("pointerdown", down);
  list.addEventListener("pointermove", move);
  list.addEventListener("pointerup", up);
  list.addEventListener("pointercancel", cancel);
  list.addEventListener("touchmove", touchmove, { passive: false });
  list.addEventListener("contextmenu", contextmenu);
  return {
    update(o: ReorderOptions) {
      opts = o;
    },
    destroy() {
      finish(false);
      list.removeEventListener("pointerdown", down);
      list.removeEventListener("pointermove", move);
      list.removeEventListener("pointerup", up);
      list.removeEventListener("pointercancel", cancel);
      list.removeEventListener("touchmove", touchmove);
      list.removeEventListener("contextmenu", contextmenu);
    },
  };
}
