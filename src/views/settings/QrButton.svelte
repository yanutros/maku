<!-- Bouton qui déplie (ou replie) le QR code d'une ligne des réglages. -->
<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  import { t } from "../../lib/i18n";

  let { open, onclick }: { open: boolean; onclick: () => void } = $props();
</script>

<button class="btn" {onclick}>
  <Icon name="image" size={16} />
  {open ? t("settings.hideQr") : t("settings.showQr")}
</button>
