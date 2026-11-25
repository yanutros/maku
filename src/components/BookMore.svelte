<script lang="ts">
  // Bouton « ⋮ » rond posé en bas à droite d'une couverture Kavita, comme sur les jaquettes (PosterCard) : visible au
  // survol de l'élément qui porte la classe `has-more`, toujours sur un écran tactile, jamais sur un téléphone (un
  // appui long ouvre le même menu). La zone suit la couverture (format 2/3) en haut de l'élément.
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n";

  let { onmenu }: { onmenu: (e: MouseEvent) => void } = $props();
</script>

<span class="zone">
  <button class="more" onclick={onmenu} aria-label={t("common.c1.moreActions")} title={t("common.c1.moreActions")}>
    <Icon name="more" size={18} />
  </button>
</span>

<style>
  .zone {
    position: absolute;
    top: 0;
    right: 0;
    left: 0;
    aspect-ratio: 2 / 3;
    pointer-events: none;
  }

  .more {
    position: absolute;
    right: 6px;
    bottom: 8px;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: #000a;
    color: #fff;
    opacity: 0;
    pointer-events: auto;
    transition:
      opacity 0.2s,
      background 0.15s;
  }

  :global(.has-more:hover) .more,
  .more:focus-visible {
    opacity: 1;
  }

  .more:hover {
    background: var(--accent);
    color: var(--accent-text);
  }
</style>
