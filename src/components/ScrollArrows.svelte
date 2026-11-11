<script lang="ts">
  import Icon from "./Icon.svelte";
  import { glideTo } from "../lib/spatial";
  import { t } from "../lib/i18n";

  // Flèches ‹ › d'une rangée qui défile de côté (Top 10 des statistiques, distribution d'une fiche), comme
  // celles des autres rangées : les barres de défilement étant masquées, c'est ainsi qu'on la parcourt à la
  // souris. Absentes au téléviseur et au téléphone (télécommande, doigt), et quand tout tient à l'écran.
  let { target }: { target: HTMLElement | undefined } = $props();

  let atStart = $state(true);
  let atEnd = $state(true);

  function update() {
    if (!target) return;
    atStart = target.scrollLeft <= 4;
    atEnd = target.scrollLeft + target.clientWidth >= target.scrollWidth - 4;
  }

  $effect(() => {
    const el = target;
    if (!el) return;
    update();
    el.addEventListener("scroll", update, { passive: true });
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", update);
      ro.disconnect();
    };
  });

  function move(dir: number) {
    if (target) glideTo(target, { left: target.scrollLeft + dir * target.clientWidth * 0.85 });
  }
</script>

{#if !(atStart && atEnd)}
  <div class="arrows">
    <button class="btn round" disabled={atStart} onclick={() => move(-1)} aria-label={t("common.c1.previous")}>
      <Icon name="chevronLeft" size={18} />
    </button>
    <button class="btn round" disabled={atEnd} onclick={() => move(1)} aria-label={t("common.c1.next")}>
      <Icon name="chevronRight" size={18} />
    </button>
  </div>
{/if}

<style>
  .arrows {
    display: flex;
    align-self: center;
    gap: 6px;
    margin-left: auto;
  }

  .arrows .btn {
    width: 32px;
    height: 32px;
  }

  :global(html.tv) .arrows,
  :global(html.phone) .arrows {
    display: none;
  }
</style>
