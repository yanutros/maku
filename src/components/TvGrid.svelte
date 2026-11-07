<script lang="ts">
  import { menuable } from "../lib/longpress";
  import { focusTabs, moveFocus, reveal } from "../lib/spatial";
  import type { TvCard } from "../lib/tvcards";
  import Icon from "./Icon.svelte";

  /**
   * Grille des téléviseurs (toute une bibliothèque, ses catégories…) : nombre de colonnes fixe,
   * flèches gérées ici sans calcul de position, et seules les lignes à l'écran sont dessinées
   * (`content-visibility`), pour garder peu de mémoire même avec des milliers de titres.
   * `onEnd` est appelé quand la sélection approche de la fin, pour charger la suite.
   */
  let { cards, onEnd, initialKey = null }: { cards: TvCard[]; onEnd?: () => void; initialKey?: string | null } = $props();

  const shape = $derived(cards[0]?.shape ?? "poster");
  const columns = $derived(shape === "wide" ? 4 : shape === "tile" ? 2 : shape === "poster" ? 6 : 5);

  let index = $state(0);
  let root = $state<HTMLElement>();

  // Au retour d'une fiche, la sélection par défaut est la jaquette qu'on avait quittée, dès que la
  // grille se dessine (voir Library.restore), pour ne pas passer par la première.
  let seeded = false;
  $effect(() => {
    if (seeded || !initialKey || !cards.length) return;
    seeded = true;
    const i = cards.findIndex((c) => c.key === initialKey);
    if (i >= 0) index = i;
  });

  /** Première rangée : la page remonte tout en haut (titre et onglets visibles) ; sinon la case se centre. */
  function show(el: HTMLElement, i: number) {
    if (i < columns) root?.closest(".content")?.scrollTo({ top: 0, behavior: "smooth" });
    else reveal(el, { center: true });
  }

  function focusAt(i: number) {
    const el = root?.querySelector<HTMLElement>(`[data-index="${i}"]`);
    if (!el) return;
    el.focus({ preventScroll: true });
    show(el, i);
  }

  function onFocusIn(e: FocusEvent) {
    const i = (e.target as HTMLElement).dataset.index;
    if (i === undefined) return;
    index = Number(i);
    // Une case de la première rangée sélectionnée alors que la page est déjà descendue (changement
    // d'onglet, retour sur la page) : la page remonte, sinon la rangée reste coupée en haut.
    if (index < columns && (root?.closest(".content")?.scrollTop ?? 0) > 0) show(e.target as HTMLElement, index);
    if (index >= cards.length - columns * 3) onEnd?.();
  }

  function onKey(e: KeyboardEvent) {
    if ((e.target as HTMLElement).dataset.index === undefined) return;
    const c = index % columns;
    let next = index;
    if (e.key === "ArrowRight") next = Math.min(index + 1, cards.length - 1);
    else if (e.key === "ArrowLeft") {
      if (c === 0) {
        e.preventDefault();
        moveFocus("ArrowLeft");
        return;
      }
      next = index - 1;
    } else if (e.key === "ArrowDown") next = Math.min(index + columns, cards.length - 1);
    else if (e.key === "ArrowUp") {
      if (index < columns) {
        e.preventDefault();
        if (!focusTabs()) moveFocus("ArrowUp");
        return;
      }
      next = index - columns;
    } else return;
    e.preventDefault();
    focusAt(next);
  }
</script>

<div
  class="tv-grid {shape}"
  style:--columns={columns}
  bind:this={root}
  data-nav-grid
  role="presentation"
  onfocusin={onFocusIn}
  onkeydown={onKey}
>
  {#each cards as card, i (card.key)}
    <button
      class="cell"
      data-index={i}
      data-key={card.key}
      data-nav-default={i === index ? "" : undefined}
      onclick={card.open}
      use:menuable={card.item}
      aria-label={card.title}
    >
      {#if shape === "tile"}
        <!-- Tuile de catégorie, comme sur ordinateur (GenreTile) : sans flou, trop coûteux pour un téléviseur. -->
        <span class="img tile" style:--hue={card.hue ?? 220}>
          {#if card.image}<span class="tile-bg" style:background-image={`url("${card.image}")`}></span>{/if}
          <span class="tile-tint"></span>
          <span class="fan">
            {#each (card.posters ?? []).slice(0, 3) as poster, i (poster)}
              <img src={poster} alt="" loading="lazy" decoding="async" style:--i={i - (Math.min(card.posters?.length ?? 0, 3) - 1) / 2} style:z-index={i === 1 ? 3 : 2 - i} />
            {/each}
          </span>
          <span class="tile-label">
            <span class="tile-name">{card.title}</span>
            <span class="tile-count">{card.line}</span>
          </span>
        </span>
      {:else}
      <span class="img">
        {#if card.image}<img src={card.image} alt="" loading="lazy" decoding="async" />{/if}
        {#if shape === "wide"}<span class="overlay">{card.label}</span>{/if}
        {#if card.progress > 0}<span class="progress"><span style:width="{card.progress}%"></span></span>{/if}
        {#if card.watched}<span class="watched"><Icon name="check" size={14} /></span>{/if}
      </span>
      {/if}
      {#if shape !== "wide" && shape !== "tile"}<span class="label">{card.label}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .tv-grid {
    --ratio: 2 / 3;
    display: grid;
    grid-template-columns: repeat(var(--columns), 1fr);
    /* L'espace entre les cases vient de leur marge intérieure (voir .cell). */
    gap: 4px 0;
    margin: 0 -12px;
    padding: 0 0 36px;
  }

  .tv-grid.square,
  .tv-grid.round {
    --ratio: 1;
  }

  .tv-grid.wide,
  .tv-grid.tile {
    --ratio: 16 / 9;
  }

  /* ---------- Tuiles de catégorie ---------- */

  /* Une grande tuile grossit moins que les jaquettes : agrandie de 5 %, elle débordait de sa case
     (qui coupe tout ce qui dépasse) et son cadre était rogné. La règle doit l'emporter sur celle des
     jaquettes (`.cell:focus .img`), d'où `.img.tile`. */
  .tv-grid.tile .cell {
    padding: 14px;
  }

  .cell:focus .img.tile {
    transform: scale(1.02);
  }

  .tile {
    border-radius: 18px;
    background: hsl(var(--hue) 45% 16%);
  }

  .tile-bg {
    position: absolute;
    inset: 0;
    background-size: cover;
    background-position: center;
    opacity: 0.35;
  }

  .tile-tint {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(120% 90% at 0% 100%, hsl(var(--hue) 70% 12% / 0.95) 0%, transparent 60%),
      linear-gradient(135deg, hsl(var(--hue) 70% 45% / 0.55), hsl(calc(var(--hue) + 40) 70% 20% / 0.6));
  }

  .fan {
    position: absolute;
    right: 9%;
    top: 14%;
    width: 28%;
    aspect-ratio: 2 / 3;
  }

  .fan img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 8px;
    box-shadow: 0 12px 28px #000b;
    transform: translateX(calc(var(--i) * 34%)) rotate(calc(var(--i) * 9deg)) scale(calc(1 - (var(--i) * var(--i)) * 0.12));
    transform-origin: 50% 110%;
    transition: transform 0.35s var(--ease);
  }

  .cell:focus .fan img {
    transform: translateX(calc(var(--i) * 48%)) rotate(calc(var(--i) * 13deg)) scale(calc(1.04 - (var(--i) * var(--i)) * 0.1));
  }

  .tile-label {
    position: absolute;
    left: 22px;
    bottom: 18px;
    display: grid;
    max-width: 55%;
    color: var(--text);
  }

  .tile-name {
    font-size: 26px;
    font-weight: 800;
    letter-spacing: -0.02em;
    line-height: 1.1;
    text-shadow: 0 2px 12px #0009;
  }

  .tile-count {
    margin-top: 2px;
    color: #ffffffb3;
    font-size: 13px;
    font-weight: 600;
  }

  /* Hors de l'écran, une case n'est ni mise en page ni dessinée. Ce mode coupe tout ce qui dépasse
     de la case : la marge intérieure laisse la place au cadre et à l'agrandissement de la sélection. */
  .cell {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    padding: 12px;
    border: none;
    background: none;
    text-align: left;
    content-visibility: auto;
    contain-intrinsic-size: auto 300px;
  }

  .img {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: var(--ratio);
    overflow: hidden;
    border-radius: 10px;
    background: var(--bg-elev);
    transition: transform 0.15s;
  }

  .round .img {
    border-radius: 50%;
  }

  .img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .cell:focus {
    outline: none;
  }

  .cell:focus .img {
    outline: 3px solid #fff;
    outline-offset: 3px;
    transform: scale(1.05);
  }

  .label {
    overflow: hidden;
    color: var(--text-dim);
    font-size: 14px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cell:focus .label {
    color: var(--text);
  }

  .round .label {
    text-align: center;
  }

  .overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: flex-end;
    padding: 14px;
    background: linear-gradient(0deg, #000000c0, transparent 60%);
    font-size: 18px;
    font-weight: 800;
  }

  /* Titre vu : une petite coche discrète dans le coin, sur un rond translucide, comme dans Plex. */
  .watched {
    position: absolute;
    top: 8px;
    right: 8px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: #00000080;
    color: #ffffffd9;
  }

  .progress {
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: 10px;
    height: 4px;
    border-radius: 2px;
    background: #ffffff40;
  }

  .progress span {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
  }
</style>
