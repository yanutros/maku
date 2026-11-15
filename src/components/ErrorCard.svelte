<script lang="ts">
  // Erreur dans une page ou un formulaire, au style des notifications (Toasts.svelte) : une icône, un titre lisible,
  // ce qu'il faut faire, le message technique replié (« Détails ») et, quand on peut, « Réessayer ». Elle reste à sa
  // place, là où le contenu manque, au lieu de partir comme une notification.
  import Icon, { type IconName } from "./Icon.svelte";
  import { describeError } from "../lib/nav.svelte";
  import { device } from "../lib/device";
  import { t } from "../lib/i18n";

  let { message, onretry }: { message: string; onretry?: () => void } = $props();

  const info = $derived(describeError(message));
  let open = $state(false);
</script>

<div class="notice is-error error-card" role="alert">
  <span class="icon"><Icon name={(info.icon as IconName | undefined) ?? "alert"} size={19} /></span>
  <div class="body">
    <div class="title">{info.title}</div>
    {#if info.text}<div class="text">{info.text}</div>{/if}
    {#if info.details && open}<pre class="details">{info.details}</pre>{/if}
    {#if onretry || (info.details && !device.tv)}
      <div class="actions">
        {#if onretry}<button class="btn primary" onclick={onretry}>{t("toast.retry")}</button>{/if}
        {#if info.details && !device.tv}
          <button class="btn" aria-expanded={open} onclick={() => (open = !open)}>{open ? t("toast.hideDetails") : t("toast.details")}</button>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .error-card {
    max-width: 560px;
    margin: 12px 0;
  }
</style>
