<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, errorText, type Item } from "../lib/api";
  import { go, nav, reload, toast } from "../lib/nav.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n";

  /**
   * « Ajouter à la collection », comme dans Plex (administrateur) : les collections de la bibliothèque
   * du titre, avec une recherche, ou une nouvelle collection dont le nom part du titre. Depuis la barre de
   * la bibliothèque (`batch`), tous les titres qu'elle affiche, filtre compris.
   */
  const item = nav.collectionFor;
  const batch = nav.collectionBatch;

  let collections = $state<Item[] | null>(null);
  let search = $state("");
  // Comme Plex, le nom d'une nouvelle collection part du titre (« Primetime » → « Primetime »).
  let name = $state(!item ? "" : item.kind === "episode" || item.kind === "season" ? (item.showTitle ?? item.title) : item.title);
  let busy = $state(false);
  let box = $state<HTMLElement>();

  const shown = $derived.by(() => {
    const q = search.trim().toLocaleLowerCase();
    return (collections ?? []).filter((c) => !q || c.title.toLocaleLowerCase().includes(q));
  });

  onMount(async () => {
    try {
      // Une collection intelligente se remplit d'elle-même : on n'y ajoute rien.
      collections = batch ? (await api.collections(batch.sectionKey)).filter((c) => !c.smart) : await api.collectionChoices(item!.ratingKey);
    } catch (e) {
      toast(errorText(e), true);
      collections = [];
    }
    await tick();
    box?.querySelector<HTMLElement>("input")?.focus();
  });

  function close() {
    nav.collectionFor = null;
    nav.collectionBatch = null;
  }

  async function addTo(c: Item) {
    busy = true;
    try {
      if (batch) {
        const keys = await batch.keys();
        await api.addManyToCollection(keys, c.ratingKey);
        toast(t("collection.addedMany", { count: keys.length, collection: c.title }));
        close();
        return;
      }
      await api.addToCollection(item!.ratingKey, c.ratingKey);
      toast(t("collection.added", { title: item!.title, collection: c.title }));
      close();
      reload();
    } catch (e) {
      toast(errorText(e), true);
    } finally {
      busy = false;
    }
  }

  async function create() {
    const title = name.trim();
    if (!title) return;
    busy = true;
    try {
      if (batch) {
        const id = await api.createCollectionWith(batch.sectionKey, batch.kind, title, await batch.keys());
        toast(t("collection.created", { collection: title }));
        close();
        if (id) go({ name: "item", ratingKey: id });
        return;
      }
      await api.createCollection(item!.ratingKey, title);
      toast(t("collection.created", { collection: title }));
      close();
      reload();
    } catch (e) {
      toast(errorText(e), true);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={close}></div>
<div class="dialog" role="dialog" aria-modal="true" aria-label={t("collection.title")} bind:this={box}>
  <div class="top">
    <h2>{batch ? t("collection.titleMany", { count: batch.count }) : t("collection.title")}</h2>
    <button class="close" onclick={close} aria-label={t("collection.cancel")}><Icon name="x" size={20} /></button>
  </div>
  <input class="search" bind:value={search} placeholder={t("collection.search")} aria-label={t("collection.search")} />

  <div class="lists">
    {#if !collections}
      <p class="muted">{t("common.c1.loading")}</p>
    {:else if !shown.length}
      <p class="muted">{collections.length ? t("collection.noMatch") : t("collection.none")}</p>
    {/if}
    {#each shown as c (c.ratingKey)}
      <button class="list" disabled={busy} onclick={() => addTo(c)}>
        <span class="cover">{#if c.poster}<img src={c.poster} alt="" loading="lazy" decoding="async" />{:else}<Icon name="library" size={20} />{/if}</span>
        <span class="name">{c.title}</span>
        {#if c.subtitle}<span class="muted count">{c.subtitle}</span>{/if}
      </button>
    {/each}
  </div>

  <form class="new" onsubmit={(e) => (e.preventDefault(), create())}>
    <input bind:value={name} maxlength="120" placeholder={t("collection.newName")} aria-label={t("collection.newName")} />
    <button class="btn primary" type="submit" disabled={busy || !name.trim()}>{t("collection.create")}</button>
  </form>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 70;
    background: #000a;
  }

  .dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    z-index: 71;
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: min(540px, calc(100vw - 32px));
    max-height: min(700px, calc(100vh - 48px));
    padding: 22px;
    border: 1px solid #ffffff1a;
    border-radius: 18px;
    background: #1c1e23f7;
    box-shadow: 0 24px 60px #000c;
    transform: translate(-50%, -50%);
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h2 {
    margin: 0;
    font-size: 21px;
  }

  .close {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--text-dim);
  }

  .close:hover,
  .close:focus-visible {
    background: #ffffff1a;
    color: var(--text);
  }

  input {
    min-width: 0;
    padding: 10px 12px;
    border: 1px solid #ffffff26;
    border-radius: 10px;
    background: #0005;
    color: var(--text);
  }

  input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .lists {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-height: 120px;
    overflow-y: auto;
  }

  .list {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
    border: none;
    border-radius: 10px;
    background: none;
    color: var(--text);
    text-align: left;
  }

  .list:hover:not(:disabled),
  .list:focus-visible {
    outline: none;
    background: #ffffff14;
  }

  .cover {
    display: grid;
    place-items: center;
    flex: none;
    width: 36px;
    height: 54px;
    overflow: hidden;
    border-radius: 6px;
    background: #ffffff10;
    color: var(--text-faint);
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .name {
    flex: 1;
    overflow: hidden;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    font-size: 13px;
  }

  .new {
    display: flex;
    gap: 8px;
  }

  .new input {
    flex: 1;
  }
</style>
