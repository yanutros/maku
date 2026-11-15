<script lang="ts">
  // Notifications en cartes, empilées dans le coin de la fenêtre : une icône teintée selon le type (succès, erreur,
  // information), un titre, une phrase d'explication, et au besoin un geste (« Réessayer ») et le message technique
  // (« Détails »). Elles partent seules ; la souris dessus, ou les détails ouverts, les retiennent. Sur téléviseur,
  // elles n'ont ni bouton ni croix : la télécommande ne doit pas y être attirée, elles s'effacent seules.
  import { flip } from "svelte/animate";
  import { fade, fly } from "svelte/transition";
  import Icon, { type IconName } from "./Icon.svelte";
  import { dismissToast, holdToast, nav, releaseToast, type Toast } from "../lib/nav.svelte";
  import { device } from "../lib/device";
  import { t } from "../lib/i18n";

  /** Au-dessus de la barre musique, quand elle est affichée. */
  let { raised = false }: { raised?: boolean } = $props();

  const ICONS: Record<Toast["kind"], IconName> = { success: "checkCircle", error: "alert", info: "info" };
  /** Détails techniques dépliés, par notification. */
  let open = $state<Record<number, boolean>>({});

  function toggleDetails(id: number) {
    open[id] = !open[id];
    if (open[id]) holdToast(id);
    else releaseToast(id);
  }

  function act(n: Toast) {
    dismissToast(n.id);
    n.action?.run();
  }

</script>

<div class="toasts" class:raised role="region" aria-label={t("toast.region")}>
  {#each nav.toasts as n (n.id)}
    <div
      class="toast notice is-{n.kind}"
      role={n.kind === "error" ? "alert" : "status"}
      onmouseenter={() => holdToast(n.id)}
      onmouseleave={() => !open[n.id] && releaseToast(n.id)}
      in:fly={{ x: device.phone ? 0 : 24, y: device.phone ? 16 : 0, duration: 220 }}
      out:fade={{ duration: 160 }}
      animate:flip={{ duration: 200 }}
    >
      <span class="icon"><Icon name={(n.icon as IconName | undefined) ?? ICONS[n.kind]} size={19} /></span>
      <div class="body">
        <div class="title">{n.title}</div>
        {#if n.text}<div class="text">{n.text}</div>{/if}
        {#if n.details && open[n.id]}<pre class="details">{n.details}</pre>{/if}
        {#if !device.tv && (n.action || n.details)}
          <div class="actions">
            {#if n.action}<button class="btn primary" onclick={() => act(n)}>{n.action.label}</button>{/if}
            {#if n.details}
              <button class="btn" aria-expanded={!!open[n.id]} onclick={() => toggleDetails(n.id)}>
                {open[n.id] ? t("toast.hideDetails") : t("toast.details")}
              </button>
            {/if}
          </div>
        {/if}
      </div>
      {#if !device.tv}
        <button class="close" aria-label={t("toast.close")} onclick={() => dismissToast(n.id)}><Icon name="x" size={16} /></button>
      {/if}
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 24px;
    bottom: 24px;
    z-index: 100;
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: min(380px, calc(100vw - 32px));
    pointer-events: none;
  }

  /* Au-dessus de la barre musique. */
  .toasts.raised {
    bottom: 124px;
  }

  .toast {
    box-shadow: 0 16px 40px #0009;
    pointer-events: auto;
  }

  /* Téléphone : en bas, sur toute la largeur, au-dessus des onglets (et du mini-lecteur). */
  :global(html.phone) .toasts {
    right: 12px;
    bottom: calc(var(--safe-bottom) + 76px);
    left: 12px;
    width: auto;
  }

  :global(html.phone) .toasts.raised {
    bottom: calc(var(--safe-bottom) + 148px);
  }

  /* Téléviseur : un peu plus loin des bords de l'écran. */
  :global(html.tv) .toasts {
    right: 40px;
    bottom: 40px;
  }
</style>
