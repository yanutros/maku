<script lang="ts">
  import "@fontsource/bebas-neue/400.css";
  import "@fontsource/cinzel/700.css";
  import "@fontsource/orbitron/700.css";
  import "@fontsource/fredoka/700.css";
  import "@fontsource/playfair-display/700-italic.css";
  import "@fontsource/creepster/400.css";
  import { remoteLogo, titleStyle } from "../lib/logos";

  /**
   * Titre d'un film ou d'une série, comme dans Jellyfin : son logo officiel (« clear logo »), celui du
   * serveur ou, à défaut, celui du service en ligne de Plex (`guid`) ; sans logo nulle part, le titre
   * prend une typographie de cinéma choisie d'après les genres (sauf `styled={false}`). `--logo-h` et `--logo-w` règlent la
   * taille maximale du logo.
   */
  let {
    logo,
    title,
    guid = null,
    genres = [],
    styled = true,
  }: { logo: string | null; title: string; guid?: string | null; genres?: string[]; styled?: boolean } = $props();

  let fetched = $state<string | null>(null);
  let failed = $state(false);

  // Avec un logo du serveur, on cherche seulement sa version traduite ; sans, n'importe lequel.
  $effect(() => {
    fetched = null;
    failed = false;
    if (!guid) return;
    let current = true;
    remoteLogo(guid, !logo).then((url) => current && (fetched = url));
    return () => (current = false);
  });

  const src = $derived(fetched ?? logo);
</script>

{#if src && !failed}
  <img class="title-logo" {src} alt={title} title={title} onerror={() => (failed = true)} />
{:else if styled}
  <span class="styled {titleStyle(genres)}">{title}</span>
{:else}
  {title}
{/if}

<style>
  .title-logo {
    display: block;
    max-width: var(--logo-w, min(440px, 80%));
    max-height: var(--logo-h, 120px);
    margin: var(--logo-margin, 0);
    object-fit: contain;
    object-position: var(--logo-pos, left bottom);
    filter: drop-shadow(0 2px 14px #000b);
  }

  /* Sans logo : une typographie de cinéma, sur deux lignes au plus. */
  /* Interligne serré, mais les jambages (p, q, g) et l'italique dépassent de la ligne : une marge intérieure leur laisse
     la place sans rien décaler (compensée par une marge négative). */
  .styled {
    display: -webkit-box;
    overflow: hidden;
    line-height: 1;
    margin: 0 -0.08em -0.18em;
    padding: 0 0.08em 0.18em;
    text-shadow: 0 3px 18px #000c;
    white-space: normal;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .action {
    font-family: "Bebas Neue", sans-serif;
    font-size: 1.3em;
    font-weight: 400;
    letter-spacing: 0.02em;
    line-height: 0.92;
  }

  .epic {
    font-family: "Cinzel", serif;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .scifi {
    font-family: "Orbitron", sans-serif;
    font-size: 0.86em;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .fun {
    font-family: "Fredoka", sans-serif;
    font-weight: 700;
    letter-spacing: -0.01em;
  }

  .horror {
    font-family: "Creepster", cursive;
    font-size: 1.15em;
    font-weight: 400;
    letter-spacing: 0.04em;
    text-shadow:
      0 0 18px #b00000aa,
      0 3px 14px #000;
  }

  .elegant {
    font-family: "Playfair Display", serif;
    font-style: italic;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
</style>
