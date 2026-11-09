<script lang="ts">
  import { api, errorText, isMusic, type Item } from "../lib/api";
  import { go, nav, reload, toast } from "../lib/nav.svelte";
  import { addToQueue, play, playMusic } from "../lib/player.svelte";
  import { canDownload, downloadMusic } from "../lib/offline.svelte";
  import Icon from "./Icon.svelte";
  import Stars from "./Stars.svelte";
  import { t } from "../lib/i18n";

  // Menu « ⋮ » d'une jaquette ou d'un titre : actions de lecture, file d'attente et playlists (musique),
  // puis gestion (administrateur seulement : modifier, actualiser, analyser).
  // Le menu se ferme avant que l'action ne s'exécute : on garde donc le contenu visé à part.
  const menu = $derived(nav.menu!);
  let item = $state(nav.menu!.item);
  $effect.pre(() => {
    if (nav.menu) item = nav.menu.item;
  });
  const music = $derived(isMusic(item.kind));
  const editable = $derived(item.kind !== "playlist");

  let el = $state<HTMLElement>();
  // Le menu reste dans la fenêtre, même ouvert près d'un bord.
  const pos = $derived.by(() => {
    const w = 240;
    const h = el?.offsetHeight ?? 300;
    return {
      left: Math.min(menu.x, window.innerWidth - w - 12),
      top: Math.min(menu.y, window.innerHeight - h - 12),
    };
  });

  function close() {
    nav.menu = null;
  }

  async function run(action: () => Promise<unknown>, done?: string) {
    close();
    try {
      await action();
      if (done) toast(done);
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  function start(shuffle = false, radio = false) {
    run(() => (music ? playMusic(item.ratingKey, { shuffle, radio }) : play(item.ratingKey)));
  }

  // « Non vu » remet aussi à zéro une lecture en cours (le titre quitte « Continuer à regarder »).
  const started = $derived(item.watched || (item.viewOffsetMs ?? 0) > 0);

  function setWatched(watched: boolean) {
    run(async () => {
      await api.setWatched(item.ratingKey, watched);
      reload();
    }, watched ? t("cardMenu.markedWatched") : t("cardMenu.markedUnwatched"));
  }

  function openDetails() {
    close();
    go({ name: "item", ratingKey: item.ratingKey });
  }

  function edit() {
    nav.editing = item;
    close();
  }

  function queue(next: boolean) {
    run(() => addToQueue(item.ratingKey, next));
  }

  function open(ratingKey: string) {
    close();
    go({ name: "item", ratingKey });
  }

  // Plex : la note de l'utilisateur, pour ce qui se note (pas les collections ni les playlists).
  const rateable = $derived(nav.serverKind === "plex" && ["movie", "show", "season", "episode", "artist", "album", "track"].includes(item.kind));
  // Un fichier à décrire dans « Voir les informations ».
  const hasFile = $derived(item.kind === "track" || item.kind === "movie" || item.kind === "episode");

  function removeContinue() {
    run(async () => {
      await api.removeFromContinue(item.ratingKey);
      reload();
    }, t("tvMenu.removedContinue"));
  }

  // Le compte a le droit de télécharger (Android) : un titre, un album ou une playlist, pour l'hors-ligne.
  const downloadable = $derived(music && item.kind !== "artist" && canDownload());
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} onresize={close} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={close} oncontextmenu={(e) => (e.preventDefault(), close())}></div>
<div class="menu" bind:this={el} style:left="{pos.left}px" style:top="{pos.top}px" role="menu">
  <div class="head">{item.showTitle && item.kind === "episode" ? item.showTitle : item.title}</div>
  <!-- Plex : la note de l'utilisateur (Jellyfin et Emby n'ont que « J'aime »). -->
  {#if rateable}
    <div class="rating"><Stars {item} size={17} /></div>
  {/if}
  {#if item.playable || music}
    <button role="menuitem" onclick={() => start()}><Icon name="play" size={16} filled /> {t("common.c1.play")}</button>
  {/if}
  {#if music && item.kind !== "track"}
    <button role="menuitem" onclick={() => start(true)}><Icon name="shuffle" size={16} /> {t("common.c1.shuffle")}</button>
  {/if}
  {#if music && item.kind === "artist"}
    <button role="menuitem" onclick={() => start(false, true)}><Icon name="radio" size={16} /> {t("music.detail.radio")}</button>
  {/if}
  {#if !music}
    {#if !item.watched}
      <button role="menuitem" onclick={() => setWatched(true)}><Icon name="checkCircle" size={16} /> {t("cardMenu.markWatched")}</button>
    {/if}
    {#if started}
      <button role="menuitem" onclick={() => setWatched(false)}><Icon name="restart" size={16} /> {t("cardMenu.markUnwatched")}</button>
    {/if}
    {#if (item.viewOffsetMs ?? 0) > 0}
      <button role="menuitem" onclick={removeContinue}><Icon name="x" size={16} /> {t("tvMenu.removeContinue")}</button>
    {/if}
  {/if}
  {#if music}
    <button role="menuitem" onclick={() => queue(true)}><Icon name="playNext" size={16} /> {t("queue.playNext")}</button>
    <button role="menuitem" onclick={() => queue(false)}><Icon name="queue" size={16} /> {t("queue.add")}</button>
    {#if item.kind !== "playlist"}
      <button role="menuitem" onclick={() => ((nav.playlistFor = item), close())}><Icon name="playlistAdd" size={16} /> {t("playlist.addTo")}</button>
    {/if}
    {#if downloadable}
      <button role="menuitem" onclick={() => run(() => downloadMusic(item), t("cardMenu.downloadStarted"))}><Icon name="download" size={16} /> {t("cardMenu.download")}</button>
    {/if}
  {/if}
  {#if hasFile}
    <button role="menuitem" onclick={() => ((nav.infoFor = item), close())}><Icon name="info" size={16} /> {t("trackInfo.open")}</button>
  {/if}
  {#if item.kind === "track"}
    {#if item.albumKey}
      <button role="menuitem" onclick={() => open(item.albumKey!)}><Icon name="music" size={16} /> {t("tvMenu.goToAlbum")}</button>
    {/if}
    {#if item.showKey}
      <button role="menuitem" onclick={() => open(item.showKey!)}><Icon name="user" size={16} /> {t("cardMenu.goToArtist")}</button>
    {/if}
  {:else if !menu.here}
    <button role="menuitem" onclick={openDetails}>
      <Icon name="chevronRight" size={16} /> {t("cardMenu.openDetails")}
    </button>
  {/if}
  {#if nav.admin && editable}
    <div class="sep"></div>
    <button role="menuitem" onclick={edit}><Icon name="edit" size={16} /> {t("cardMenu.edit")}</button>
    {#if item.kind === "movie" || item.kind === "show"}
      <button role="menuitem" onclick={() => ((nav.collectionFor = item), close())}>
        <Icon name="library" size={16} /> {t("collection.addTo")}
      </button>
    {/if}
    <button role="menuitem" onclick={() => run(() => api.refreshMetadata(item.ratingKey), t("cardMenu.refreshStarted"))}>
      <Icon name="restart" size={16} /> {t("cardMenu.refreshMetadata")}
    </button>
    <button role="menuitem" onclick={() => run(() => api.analyze(item.ratingKey), t("cardMenu.analyzeStarted"))}>
      <Icon name="search" size={16} /> {t("cardMenu.analyze")}
    </button>
  {/if}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
  }

  .menu {
    position: fixed;
    z-index: 61;
    width: 240px;
    padding: 6px;
    border: 1px solid #ffffff1a;
    border-radius: 12px;
    background: #1c1e23f2;
    backdrop-filter: blur(18px);
    box-shadow: 0 18px 44px #000b;
    animation: pop 0.12s var(--ease);
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.96);
    }
  }

  .head {
    padding: 8px 10px 6px;
    overflow: hidden;
    color: var(--text-faint);
    font-size: 12px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border: none;
    border-radius: 8px;
    background: none;
    text-align: left;
    font-size: 13px;
  }

  button:hover {
    background: #ffffff14;
  }

  button :global(svg) {
    color: var(--text-dim);
  }

  .rating {
    padding: 0 6px 6px;
  }

  .rating :global(button) {
    width: auto;
    padding: 3px;
  }

  .sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--border);
  }
</style>
