<!-- Bandeau d'une rubrique des réglages, façon Netflix : grande icône dans la couleur du thème, titre et une phrase. -->
<script lang="ts">
  import Icon, { type IconName } from "../../components/Icon.svelte";

  let { icon, title, text }: { icon: IconName; title: string; text: string } = $props();
</script>

  <header class="banner">
    <span class="banner-icon"><Icon name={icon} size={30} /></span>
    <div>
      <h2>{title}</h2>
      <p>{text}</p>
    </div>
  </header>
