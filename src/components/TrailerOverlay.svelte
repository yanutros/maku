<script lang="ts">
  // Bande-annonce YouTube (fiche d'un film ou d'une série), lue par-dessus la page ; Retour ou
  // Échap la ferme. Les bandes-annonces du serveur, elles, passent par le lecteur habituel.
  import { nav } from "../lib/nav.svelte";
  import { locale, t } from "../lib/i18n";
  import Icon from "./Icon.svelte";

  const src = $derived(
    nav.trailer
      ? `https://www.youtube-nocookie.com/embed/${nav.trailer}?autoplay=1&rel=0&modestbranding=1&hl=${locale}`
      : null,
  );

  function onKey(e: KeyboardEvent) {
    if (nav.trailer && e.key === "Escape") nav.trailer = null;
  }
</script>

<svelte:window onkeydown={onKey} />

{#if src}
  <div class="trailer" role="dialog" aria-label={t("item.trailer")}>
    <button class="icon-btn close" onclick={() => (nav.trailer = null)} aria-label={t("player.close")} data-nav-default>
      <Icon name="x" size={26} />
    </button>
    <iframe {src} title={t("item.trailer")} allow="autoplay; encrypted-media; fullscreen; picture-in-picture" allowfullscreen></iframe>
  </div>
{/if}

<style>
  .trailer {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    background: #000;
  }

  iframe {
    width: min(100vw, calc(100vh * 16 / 9));
    aspect-ratio: 16 / 9;
    border: none;
  }

  .close {
    position: absolute;
    top: 18px;
    left: 18px;
    z-index: 1;
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: #000000aa;
    color: #fff;
  }
</style>
