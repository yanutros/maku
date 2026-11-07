<script lang="ts">
  import { menuable } from "../lib/longpress";
  import type { Snippet } from "svelte";
  import { focusTabs, moveFocus } from "../lib/spatial";
  import TitleLogo from "./TitleLogo.svelte";
  import Icon from "./Icon.svelte";
  import type { TvRow } from "../lib/tvcards";
  import { useAmbient } from "../lib/ambient.svelte";

  /**
   * Rangées des téléviseurs, à la manière des applications TV : en haut, la fiche de l'élément
   * sélectionné (fond, titre, résumé) ; en dessous, des rangées de jaquettes de taille fixe. La
   * rangée active remonte toujours au même endroit et la jaquette active reste à gauche : tout
   * se déplace par translation, sans redimensionnement, pour rester fluide sur les petites TV.
   * `header` (onglets d'une bibliothèque) s'affiche au-dessus et se rejoint avec la flèche haut.
   * `blur` (musique) : au lieu d'une image de fond, les couleurs floutées de la pochette sélectionnée,
   * sur toute la page (une pochette carrée agrandie en fond 16/9 serait floue et coupée).
   */
  let { rows, header, blur = false }: { rows: TvRow[]; header?: Snippet; blur?: boolean } = $props();

  // Position de la sélection, et colonne retenue pour chaque rangée.
  let row = $state(0);
  let cols = $state<number[]>([]);
  const col = (r: number) => Math.min(cols[r] ?? 0, rows[r].cards.length - 1);
  const current = $derived(rows[row]?.cards[col(row)] ?? rows[0]?.cards[0]);

  // Le fond suit la sélection avec un léger délai, pour ne pas charger chaque image survolée.
  let backdrop = $state<string | null>(null);
  $effect(() => {
    const next = (blur ? current?.image : null) ?? current?.backdrop ?? null;
    const t = setTimeout(() => (backdrop = next), backdrop ? 280 : 0);
    return () => clearTimeout(t);
  });

  // Musique : la pochette floutée sur toute la fenêtre, menu compris (dessinée par App.svelte).
  $effect(() => (blur ? useAmbient(backdrop) : undefined));

  let root = $state<HTMLElement>();

  function focusCard(r: number, c: number) {
    root?.querySelector<HTMLElement>(`[data-cell="${r}-${c}"]`)?.focus({ preventScroll: true });
  }

  function onFocusIn(e: FocusEvent) {
    const cell = (e.target as HTMLElement).dataset.cell;
    if (!cell) return;
    // Tout bouge par translation : un défilement natif (scrollIntoView venu d'ailleurs) décalerait les
    // rangées hors de l'écran, on l'annule.
    for (const el of [root, ...(root?.querySelectorAll<HTMLElement>(".rows, .viewport") ?? [])]) {
      if (!el) continue;
      el.scrollTop = 0;
      el.scrollLeft = 0;
    }
    const [r, c] = cell.split("-").map(Number);
    row = r;
    cols[r] = c;
  }

  function onKey(e: KeyboardEvent) {
    if (!(e.target as HTMLElement).dataset.cell) return;
    const c = col(row);
    let r2 = row;
    let c2 = c;
    if (e.key === "ArrowRight") c2 = Math.min(c + 1, rows[row].cards.length - 1);
    else if (e.key === "ArrowLeft") {
      // Au bord gauche, on rejoint le menu.
      if (c === 0) {
        e.preventDefault();
        moveFocus("ArrowLeft");
        return;
      }
      c2 = c - 1;
    } else if (e.key === "ArrowDown") r2 = Math.min(row + 1, rows.length - 1);
    else if (e.key === "ArrowUp") {
      // Tout en haut, on rejoint les onglets.
      if (row === 0) {
        e.preventDefault();
        if (!focusTabs()) moveFocus("ArrowUp");
        return;
      }
      r2 = row - 1;
    } else return;
    e.preventDefault();
    if (r2 !== row) c2 = col(r2);
    focusCard(r2, c2);
  }
</script>

<div class="tv-rows" class:with-header={!!header} bind:this={root} role="presentation" onfocusin={onFocusIn} onkeydown={onKey}>
  {#key backdrop}
    {#if backdrop && !blur}<img class="backdrop" src={backdrop} alt="" decoding="async" />{/if}
  {/key}
  {#if !blur}<div class="shade"></div>{/if}

  {#if header}<div class="header">{@render header()}</div>{/if}

  {#if current}
    <div class="spotlight">
      <div class="eyebrow">{current.eyebrow}</div>
      <!-- Musique (pochettes carrées, artistes ronds) : pas de logo officiel, le titre en texte simple. -->
      <h1>
        {#if current.shape === "square" || current.shape === "round"}{current.title}{:else}<TitleLogo logo={current.logo} guid={current.logoGuid} genres={current.genres} title={current.title} />{/if}
      </h1>
      {#if current.line}<div class="line">{current.line}</div>{/if}
      {#if current.summary}<p>{current.summary}</p>{/if}
    </div>
  {/if}

  <div class="rows">
    <div class="column" style:transform="translateY({-row * 100}%)">
      {#each rows as r, ri (ri)}
        {@const shape = r.cards[0]?.shape ?? "poster"}
        <section class:active={ri === row} class={shape}>
          <h2>{r.title}</h2>
          <div class="viewport">
            <div class="track" style:transform="translateX(calc({-col(ri)} * var(--step)))" data-nav-grid>
              {#each r.cards as card, ci (card.key)}
                <button
                  class="card"
                  data-cell="{ri}-{ci}"
                  data-key={card.key}
                  tabindex={ri === row ? 0 : -1}
                  data-nav-default={ri === row && ci === col(ri) ? "" : undefined}
                  onclick={card.open}
                  use:menuable={card.item}
                  aria-label={card.title}
                >
                  {#if card.image}<img src={card.image} alt="" loading="lazy" decoding="async" />{/if}
                  {#if card.progress > 0}<div class="progress"><div style:width="{card.progress}%"></div></div>{/if}
                  {#if card.watched}<span class="watched"><Icon name="check" size={14} /></span>{/if}
                </button>
              {/each}
            </div>
          </div>
        </section>
      {/each}
    </div>
  </div>
</div>

<style>
  .tv-rows {
    --row-h: 326px;
    --rows-top: 300px;
    position: relative;
    height: 100vh;
    margin-top: -68px;
    overflow: hidden;
    contain: strict;
  }

  .tv-rows.with-header {
    --rows-top: 336px;
  }

  .backdrop {
    position: absolute;
    top: 0;
    right: 0;
    width: 72%;
    height: 72%;
    object-fit: cover;
    animation: fade 0.35s ease-out;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  /* Le fond se fond dans la page vers la gauche et vers le bas. */
  .shade {
    position: absolute;
    top: 0;
    right: 0;
    width: 72%;
    height: 72%;
    background:
      linear-gradient(90deg, var(--bg) 0%, #14151899 30%, transparent 60%),
      linear-gradient(0deg, var(--bg) 0%, transparent 45%);
  }

  .header {
    position: absolute;
    z-index: 1;
    top: 28px;
    left: 48px;
    right: 48px;
  }

  .spotlight {
    position: absolute;
    top: 44px;
    left: 48px;
    width: 52%;
  }

  .with-header .spotlight {
    top: 96px;
  }

  .eyebrow {
    color: var(--accent);
    font-size: 13px;
    font-weight: 800;
    letter-spacing: 0.16em;
    text-transform: uppercase;
  }

  h1 {
    margin: 6px 0 10px;
    overflow: hidden;
    font-size: 40px;
    font-weight: 800;
    line-height: 1.1;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .with-header h1 {
    font-size: 34px;
  }

  .line {
    color: #ffffffd8;
    font-size: 16px;
    font-weight: 600;
  }

  .spotlight p {
    display: -webkit-box;
    margin: 12px 0 0;
    overflow: hidden;
    color: var(--text-dim);
    font-size: 15px;
    line-height: 1.5;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
  }

  .with-header .spotlight p {
    -webkit-line-clamp: 2;
  }

  /* Fenêtre des rangées : seule la rangée active et le titre de la suivante sont visibles. */
  .rows {
    position: absolute;
    top: var(--rows-top);
    left: 0;
    right: 0;
    bottom: 0;
    overflow: hidden;
  }

  .column {
    height: var(--row-h);
    transition: transform 0.3s var(--ease);
    will-change: transform;
  }

  section {
    --card-w: 168px;
    --ratio: 2 / 3;
    --gap: 20px;
    --step: calc(var(--card-w) + var(--gap));
    height: var(--row-h);
    opacity: 0.55;
    transition: opacity 0.3s;
  }

  section.square,
  section.round {
    --card-w: 236px;
    --ratio: 1;
  }

  section.wide {
    --card-w: 400px;
    --ratio: 16 / 9;
  }

  section.active {
    opacity: 1;
  }

  h2 {
    margin: 0 0 0 48px;
    font-size: 20px;
    font-weight: 700;
  }

  /* Les jaquettes déjà passées disparaissent à gauche ; la marge laisse place au liseré. */
  .viewport {
    margin-left: 36px;
    padding: 14px 0 14px 12px;
    overflow: hidden;
  }

  .track {
    display: flex;
    gap: var(--gap);
    transition: transform 0.3s var(--ease);
    will-change: transform;
  }

  .card {
    position: relative;
    flex: none;
    width: var(--card-w);
    aspect-ratio: var(--ratio);
    padding: 0;
    overflow: hidden;
    border: none;
    border-radius: 10px;
    background: var(--bg-elev);
    transition: transform 0.15s;
  }

  .round .card {
    border-radius: 50%;
  }

  .card img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .card:focus {
    outline: 3px solid var(--accent);
    outline-offset: 3px;
    transform: scale(1.05);
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

  .progress div {
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
  }

  /* Logo du titre dans la fiche du haut. */
  h1 {
    --logo-h: 104px;
    --logo-w: 70%;
  }
</style>
