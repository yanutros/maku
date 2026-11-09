<script lang="ts">
  import type { RatingSource } from "../lib/api";

  /**
   * Pastille d'un site d'évaluation, dessinée ici (pas les logos des sites) : sur les fiches à côté de la note,
   * et dans Réglages → Interface à côté du nom du site. `good` choisit la tomate fraîche ou écrasée, le pop-corn
   * plein ou renversé. La presse d'Allociné est en négatif de ses spectateurs.
   */
  let { source, good = true }: { source: RatingSource; good?: boolean } = $props();
</script>

{#if source === "imdb"}
  <span class="tag imdb">IMDb</span>
{:else if source === "tmdb"}
  <span class="tag tmdb">TMDB</span>
{:else if source === "rtCritic"}
  <svg viewBox="0 0 24 24" aria-hidden="true">
    {#if good}
      <circle cx="12" cy="13.5" r="8" fill="#fa320a" /><path d="M9 4.5c1.5.3 2.5 1.2 3 2.5.6-1.4 1.8-2.3 3.4-2.4-.8 1.3-2 2.3-3.4 2.6-1.4-.2-2.4-1.3-3-2.7z" fill="#00912d" />
    {:else}
      <path d="M12 3.5l2 4 4.4-1.6-1.6 4.4 4 2-4 2 1.6 4.4L14 17.1l-2 4-2-4-4.4 1.6 1.6-4.4-4-2 4-2-1.6-4.4L10 7.5z" fill="#0ac855" />
    {/if}
  </svg>
{:else if source === "rtAudience"}
  <svg viewBox="0 0 24 24" aria-hidden="true" class:spilled={!good}>
    <path d="M6 9h12l-1.6 12H7.6z" fill={good ? "#fa320a" : "#0ac855"} /><path d="M9.5 9l.5 12M14.5 9l-.5 12" stroke="#fff" stroke-width="1.4" />
    <circle cx="8" cy="7.5" r="2.4" fill="#fff6dc" /><circle cx="12" cy="6" r="2.8" fill="#fff6dc" /><circle cx="16" cy="7.5" r="2.4" fill="#fff6dc" />
  </svg>
{:else if source === "allocinePress" || source === "allocineUsers"}
  <span class="tag ac" class:press={source === "allocinePress"}>Allo</span>
{:else}
  <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 2.8l2.8 5.8 6.3.9-4.6 4.4 1.1 6.3L12 17.2l-5.6 3 1.1-6.3L2.9 9.5l6.3-.9z" fill="var(--accent)" /></svg>
{/if}

<style>
  svg {
    flex: none;
    width: 18px;
    height: 18px;
  }

  svg.spilled {
    transform: rotate(-35deg);
  }

  .tag {
    flex: none;
    padding: 1px 4px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 800;
    line-height: 1.3;
  }

  .imdb {
    background: #f5c518;
    color: #000;
  }

  .tmdb {
    background: linear-gradient(90deg, #90cea1, #01b4e4);
    color: #0d253f;
  }

  .ac {
    background: #fecc00;
    color: #1a1a1a;
  }

  /* La presse en négatif (gris foncé, lettres jaunes) pour la distinguer des spectateurs. */
  .ac.press {
    background: #2b2b2b;
    color: #fecc00;
    box-shadow: inset 0 0 0 1px #fecc0066;
  }
</style>
