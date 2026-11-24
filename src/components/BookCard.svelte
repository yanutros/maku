<script lang="ts">
  // Couverture d'une série Kavita, avec sa progression de lecture.
  import type { BookSeries } from "../lib/api";
  import Icon from "./Icon.svelte";

  let { series, onclick }: { series: BookSeries; onclick: () => void } = $props();
  const progress = $derived(series.pages > 0 ? Math.min(1, series.pagesRead / series.pages) : 0);
  let broken = $state(false);
</script>

<button class="book" {onclick} title={series.name}>
  <span class="cover">
    {#if broken}<Icon name="book" size={40} />{:else}<img src={series.cover} alt="" loading="lazy" onerror={() => (broken = true)} />{/if}
    {#if progress > 0 && progress < 1}<span class="bar"><span style:width="{progress * 100}%"></span></span>{/if}
    {#if progress >= 1}<span class="done"><Icon name="check" size={14} /></span>{/if}
  </span>
  <span class="name">{series.name}</span>
</button>

<style>
  .book {
    display: grid;
    gap: 8px;
    min-width: 0;
    padding: 0;
    border: none;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }

  .cover {
    position: relative;
    display: grid;
    place-items: center;
    aspect-ratio: 2 / 3;
    overflow: hidden;
    border-radius: var(--radius);
    background: var(--bg-elev);
    color: var(--text-faint);
    box-shadow: 0 10px 24px #0007;
    transition: transform 0.15s var(--ease), box-shadow 0.15s;
  }

  .book:hover .cover,
  .book:focus-visible .cover {
    transform: translateY(-3px);
    box-shadow: 0 0 0 2px var(--accent), 0 14px 30px #0009;
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .bar {
    position: absolute;
    right: 8px;
    bottom: 8px;
    left: 8px;
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
    top: 8px;
    right: 8px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-text);
  }

  .name {
    overflow: hidden;
    font-size: 14px;
    font-weight: 600;
    line-height: 1.3;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
</style>
