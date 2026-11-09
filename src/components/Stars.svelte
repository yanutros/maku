<script lang="ts">
  import { api, errorText, type Item } from "../lib/api";
  import { toast } from "../lib/nav.svelte";
  import { t } from "../lib/i18n";

  /**
   * Note de l'utilisateur, de 1 à 5 étoiles (10 pour Plex = 5 étoiles), comme dans Plex : survoler
   * montre la note visée, cliquer l'enregistre, recliquer sur la note actuelle l'efface. Chaque compte
   * a ses propres notes, ami partagé compris.
   */
  let { item, size = 15 }: { item: Item; size?: number } = $props();

  let hover = $state<number | null>(null);
  let saving = $state(false);
  // Note changée ici, en attendant que la liste soit rechargée depuis le serveur.
  let mine = $state<{ key: string; rating: number | null } | null>(null);
  const rating = $derived(mine?.key === item.ratingKey ? mine.rating : (item.userRating ?? null));
  const value = $derived(rating ? Math.round(rating / 2) : 0);
  const shown = $derived(hover ?? value);

  async function rate(stars: number, e: MouseEvent) {
    e.stopPropagation();
    if (saving) return;
    const next = stars === value ? null : stars * 2;
    const before = mine;
    mine = { key: item.ratingKey, rating: next };
    saving = true;
    try {
      await api.rate(item.ratingKey, next);
    } catch (err) {
      mine = before;
      toast(errorText(err), true);
    } finally {
      saving = false;
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<span class="stars" class:rated={value > 0} onmouseleave={() => (hover = null)} role="group" aria-label={value ? t("stars.rated", { stars: value }) : t("stars.rate")}>
  {#each [1, 2, 3, 4, 5] as n (n)}
    <button
      class:on={n <= shown}
      style:--s="{size}px"
      onmouseenter={() => (hover = n)}
      onclick={(e) => rate(n, e)}
      aria-label={n === value ? t("stars.clear") : t("stars.give", { stars: n })}
      title={n === value ? t("stars.clear") : t("stars.give", { stars: n })}
    >
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 2.8l2.8 5.8 6.3.9-4.6 4.4 1.1 6.3L12 17.2l-5.6 3 1.1-6.3L2.9 9.5l6.3-.9z" /></svg>
    </button>
  {/each}
</span>

<style>
  .stars {
    display: inline-flex;
    align-items: center;
  }

  button {
    display: grid;
    place-items: center;
    padding: 2px;
    border: none;
    background: none;
    color: var(--text-faint);
    cursor: pointer;
  }

  svg {
    width: var(--s);
    height: var(--s);
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linejoin: round;
  }

  button.on {
    color: var(--accent);
  }

  button.on svg {
    fill: currentColor;
  }
</style>
