<script lang="ts">
  import type { TvCard } from "../lib/tvcards";
  import Icon from "./Icon.svelte";

  /**
   * Image d'une jaquette TV qui n'a pas de pochette à elle : une mosaïque de pochettes (trois : la première en
   * grand ; quatre : en carré), une grande icône au milieu, une étiquette en haut à gauche (« Mix »). Sans tout
   * cela, l'image de la jaquette.
   */
  let { card }: { card: TvCard } = $props();

  const tiles = $derived((card.mosaic ?? []).slice(0, 4));
</script>

{#if tiles.length}
  <span class="mosaic" class:three={tiles.length === 3} class:four={tiles.length === 4}>
    {#each tiles as src, i (i)}<span class="tile" class:lead={tiles.length === 3 && i === 0}>{#if src}<img {src} alt="" loading="lazy" decoding="async" />{/if}</span>{/each}
  </span>
{:else if card.image}
  <img class="single" src={card.image} alt="" loading="lazy" decoding="async" />
{/if}
{#if card.icon}<span class="icon" class:bare={!tiles.length && !card.image}><Icon name={card.icon} size={52} /></span>{/if}
{#if card.badge}<span class="badge"><Icon name="shuffle" size={14} /> {card.badge}</span>{/if}

<style>
  .mosaic {
    position: absolute;
    inset: 0;
    display: grid;
    gap: 2px;
  }

  .mosaic.three {
    grid-template-columns: 2fr 1fr;
    grid-template-rows: 1fr 1fr;
  }

  .mosaic.four {
    grid-template-columns: 1fr 1fr;
    grid-template-rows: 1fr 1fr;
  }

  .tile {
    overflow: hidden;
    background: #ffffff12;
  }

  .tile.lead {
    grid-row: 1 / 3;
  }

  img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .single {
    position: absolute;
    inset: 0;
  }

  /* L'icône de l'action (aléatoire, nouveau mix…) sur un voile, au-dessus des pochettes. */
  .icon {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: #0000008c;
    color: #fff;
  }

  /* Sans pochette (« Annuler le mix ») : l'icône sur la couleur du thème. */
  .icon.bare {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    color: var(--accent);
  }

  .badge {
    position: absolute;
    top: 8px;
    left: 8px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: 6px;
    background: var(--accent);
    color: var(--accent-text);
    font-size: 13px;
    font-weight: 800;
  }
</style>
