<!-- Fiche : la distribution. Comme dans Plex, un acteur ouvre la liste de ses films et séries du serveur. -->
<script lang="ts">
  import ScrollArrows from "../../components/ScrollArrows.svelte";
  import type { ItemDetail } from "../../lib/api";
  import { go } from "../../lib/nav.svelte";
  import { t } from "../../lib/i18n";

  let { cast }: { cast: ItemDetail["cast"] } = $props();

  /** Rangée de la distribution, que ses flèches font défiler (ordinateur). */
  let castRow = $state<HTMLElement>();
</script>

{#if cast.length}
  <h2 class="row-title">{t("item.cast")}<ScrollArrows target={castRow} /></h2>
  <div class="cast" data-nav-row bind:this={castRow}>
    {#each cast as person (person.name + person.role)}
      <!-- Comme dans Plex : un acteur ouvre la liste de ses films et séries du serveur. -->
      <button
        class="person"
        data-key="person-{person.id ?? person.name}"
        disabled={person.id === null}
        onclick={() => person.id !== null && go({ name: "person", id: person.id, personName: person.name, photo: person.photo })}
      >
        <div class="avatar">
          {#if person.photo}<img src={person.photo} alt="" loading="lazy" />{:else}{person.name[0]}{/if}
        </div>
        <div class="name">{person.name}</div>
        {#if person.role}<div class="role">{person.role}</div>{/if}
      </button>
    {/each}
  </div>
{/if}

<style>
  .cast {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 164px;
    gap: 16px;
    overflow-x: auto;
    padding: 10px 10px 8px;
    margin: -10px -10px 0;
  }

  .person {
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    text-align: center;
  }

  .person:not(:disabled) .avatar {
    transition: transform 0.2s var(--ease), box-shadow 0.2s;
  }

  /* Souris seulement : sur téléviseur, Android laisse l'acteur ouvert « survolé » au retour, et son
     cercle doré s'ajoutait à la sélection (deux acteurs ciblés). */
  :global(html:not(.tv)) .person:not(:disabled):hover .avatar {
    transform: scale(1.05);
    box-shadow: 0 0 0 3px var(--accent);
  }

  /* Télécommande : la sélection entoure la photo (cercle doré), comme dans Plex, plutôt qu'un carré
     autour de la photo et du nom. */
  :global(html.tv) .person:focus {
    outline: none;
  }

  :global(html.tv) .person:focus .avatar {
    transform: scale(1.06);
    box-shadow: 0 0 0 4px var(--accent);
  }

  :global(html.tv) .person:focus .name {
    color: var(--accent);
  }

  .avatar {
    display: grid;
    place-items: center;
    width: 148px;
    height: 148px;
    margin: 0 auto 10px;
    overflow: hidden;
    border-radius: 50%;
    background: var(--bg-elev);
    color: var(--text-faint);
    font-size: 36px;
    font-weight: 600;
  }

  .avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .name {
    font-weight: 600;
    font-size: 14px;
  }

  .role {
    color: var(--text-dim);
    font-size: 12px;
  }

  :global(html.phone) .cast {
    grid-auto-columns: 100px;
    gap: 12px;
  }

  :global(html.phone) .avatar {
    width: 88px;
    height: 88px;
    font-size: 26px;
  }
</style>
