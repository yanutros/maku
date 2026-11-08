<script lang="ts">
  import type { Genre } from "../lib/api";
  import { genreHue, genreLabel } from "../lib/genres";
  import { t } from "../lib/i18n";

  let { genre, onclick }: { genre: Genre; onclick: () => void } = $props();

  const hue = $derived(genreHue(genre.title));
  // Trois affiches en éventail, la mieux notée au premier plan.
  const fan = $derived(genre.posters.slice(0, 3));
</script>

<button class="tile" style:--hue={hue} {onclick} aria-label={t("genreTile.ariaLabel", { genre: genreLabel(genre.title), titles: t("genreTile.titles", { count: genre.count }) })}>
  {#if genre.backdrop}<span class="bg" style:background-image={`url("${genre.backdrop}")`}></span>{/if}
  <span class="tint"></span>
  <span class="fan">
    {#each fan as poster, i (poster)}
      <img src={poster} alt="" loading="lazy" style:--i={i - (fan.length - 1) / 2} style:z-index={i === 1 ? 3 : 2 - i} />
    {/each}
  </span>
  <span class="label">
    <span class="name">{genreLabel(genre.title)}</span>
    <span class="count">{t("genreTile.titles", { count: genre.count })}</span>
  </span>
</button>

<style>
  .tile {
    position: relative;
    aspect-ratio: 16 / 9;
    padding: 0;
    overflow: hidden;
    border: none;
    border-radius: 18px;
    background: hsl(var(--hue) 45% 16%);
    box-shadow:
      0 10px 30px #0007,
      inset 0 0 0 1px #ffffff14;
    text-align: left;
    transition:
      transform 0.35s var(--ease),
      box-shadow 0.35s var(--ease);
  }

  .tile:hover,
  .tile:focus-visible {
    transform: translateY(-6px) scale(1.015);
    box-shadow:
      0 24px 50px #000a,
      0 0 0 2px hsl(var(--hue) 80% 60% / 0.8);
    outline: none;
  }

  /* Fond : la scène d'un des titres, très floutée, comme un halo de couleur. */
  .bg {
    position: absolute;
    inset: -40px;
    background-size: cover;
    background-position: center;
    filter: blur(22px) saturate(1.6) brightness(0.7);
    /* Safari (iPad, Mac) ne coupe pas un calque flouté selon les coins arrondis de son cadre : il le découpe
       lui-même, au contour exact du cadre (son débord, plus le bord de 1 px s'il y en a un). */
    clip-path: inset(40px round 18px);
  }

  .tint {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(120% 90% at 0% 100%, hsl(var(--hue) 70% 12% / 0.95) 0%, transparent 60%),
      linear-gradient(135deg, hsl(var(--hue) 70% 45% / 0.55), hsl(calc(var(--hue) + 40) 70% 20% / 0.6));
    mix-blend-mode: normal;
  }

  /* Éventail d'affiches sur la droite, qui s'ouvre au survol. */
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
    transition: transform 0.45s var(--ease);
  }

  .tile:hover .fan img {
    transform: translateX(calc(var(--i) * 48%)) rotate(calc(var(--i) * 13deg)) scale(calc(1.04 - (var(--i) * var(--i)) * 0.1));
  }

  .label {
    position: absolute;
    left: 22px;
    bottom: 18px;
    display: grid;
    max-width: 55%;
  }

  .name {
    font-family: "Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif;
    font-size: 26px;
    font-weight: 800;
    letter-spacing: -0.02em;
    line-height: 1.1;
    text-shadow: 0 2px 12px #0009;
  }

  .count {
    margin-top: 2px;
    color: #ffffffb3;
    font-size: 13px;
    font-weight: 600;
  }
</style>
