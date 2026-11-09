<script lang="ts">
  import { onMount, type ComponentProps } from "svelte";
  import { api, errorText, formatDuration, isMusic } from "../lib/api";
  import { go, nav, reload, toast } from "../lib/nav.svelte";
  import { addToQueue, play, playMusic } from "../lib/player.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n";

  /**
   * Menu d'un titre sur téléviseur, comme dans Plex : appui long sur OK (ou touche Menu) sur une
   * jaquette. Panneau à droite, actions selon le titre (commencé ou non, vidéo ou musique) ;
   * Retour le ferme et rend le focus à la jaquette.
   */
  const menu = $derived(nav.tvMenu!);
  // Gardé à part : le panneau se ferme avant que l'action ne s'exécute.
  const item = nav.tvMenu!.item;
  const music = isMusic(item.kind);
  const started = !!item.viewOffsetMs && !item.watched;
  const remaining = item.viewOffsetMs && item.durationMs ? formatDuration(item.durationMs - item.viewOffsetMs) : null;
  // Liste de visionnage du compte : films et séries seulement (identifiant plex:// du titre).
  const watchGuid = item.kind === "movie" || item.kind === "show" ? item.logoGuid : null;

  let panel = $state<HTMLElement>();
  let rating = $state(false);
  let watchlisted = $state<boolean | null>(null);
  const returnTo = document.activeElement as HTMLElement | null;

  type Action = { icon: ComponentProps<typeof Icon>["name"]; label: string; run: () => unknown; filled?: boolean };

  function close() {
    nav.tvMenu = null;
    returnTo?.focus();
  }

  /**
   * `after` : ce que l'action change, appliqué au titre sur place (vu, note…) : seule sa jaquette se met à
   * jour, comme dans Plex, sans recharger la page. `true` : la page se recharge (le titre doit disparaître).
   */
  async function run(action: () => unknown, done?: string, after?: (() => void) | true) {
    close();
    try {
      await action();
      if (done) toast(done);
      if (after === true) reload();
      else after?.();
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  // Vu / non vu : la progression repart de zéro, et une série n'a plus (ou a de nouveau) d'épisodes à voir.
  function setWatched(watched: boolean) {
    item.watched = watched;
    item.viewOffsetMs = null;
    // Le nombre d'épisodes à voir n'est connu qu'après rechargement : on l'efface plutôt que d'en afficher un faux.
    if (item.unwatchedCount !== null) item.unwatchedCount = watched ? 0 : null;
  }

  const actions = $derived.by<Action[]>(() => {
    const a: Action[] = [];
    if (music) {
      const artist = item.kind === "track" ? item.subtitle : item.showTitle;
      if (item.kind === "album" && item.showKey) a.push({ icon: "user", label: t("tvMenu.goTo", { name: artist ?? "" }), run: () => run(() => go({ name: "item", ratingKey: item.showKey! })) });
      a.push({ icon: "play", filled: true, label: t("tvMenu.play"), run: () => run(() => playMusic(item.ratingKey)) });
      if (item.kind !== "track") a.push({ icon: "shuffle", label: t("tvMenu.shuffle"), run: () => run(() => playMusic(item.ratingKey, { shuffle: true })) });
      a.push({ icon: "playNext", label: t("queue.playNext"), run: () => run(() => addToQueue(item.ratingKey, true)) });
      a.push({ icon: "queue", label: t("queue.add"), run: () => run(() => addToQueue(item.ratingKey, false)) });
      if (item.kind === "artist") a.push({ icon: "radio", label: t("music.detail.radio"), run: () => run(() => playMusic(item.ratingKey, { radio: true })) });
      if (item.kind === "track" && item.albumKey) a.push({ icon: "music", label: t("tvMenu.goToAlbum"), run: () => run(() => go({ name: "item", ratingKey: item.albumKey! })) });
      if (item.kind === "track" && item.showKey) a.push({ icon: "user", label: t("tvMenu.goTo", { name: artist ?? "" }), run: () => run(() => go({ name: "item", ratingKey: item.showKey! })) });
    } else {
      if (item.playable && started) {
        a.push({ icon: "play", filled: true, label: t("tvMenu.resume", { time: remaining ?? "" }), run: () => run(() => play(item.ratingKey)) });
        a.push({ icon: "restart", label: t("tvMenu.restart"), run: () => run(() => play(item.ratingKey, { fromStart: true })) });
      } else if (item.playable) {
        a.push({ icon: "play", filled: true, label: t("tvMenu.play"), run: () => run(() => play(item.ratingKey)) });
      }
      a.push({ icon: "chevronRight", label: t("tvMenu.moreInfo"), run: () => run(() => go({ name: "item", ratingKey: item.ratingKey })) });
      if ((item.kind === "episode" || item.kind === "season") && item.showKey) {
        a.push({ icon: "tv", label: t("tvMenu.goToShow"), run: () => run(() => go({ name: "item", ratingKey: item.showKey! })) });
      }
      if (watchGuid && watchlisted !== null) {
        a.push({
          icon: "pin",
          label: watchlisted ? t("tvMenu.removeWatchlist") : t("tvMenu.addWatchlist"),
          run: () => run(() => api.setWatchlist(watchGuid, !watchlisted), watchlisted ? t("tvMenu.watchlistRemoved") : t("tvMenu.watchlistAdded")),
        });
      }
    }
    if (nav.admin && (item.kind === "movie" || item.kind === "show")) {
      a.push({ icon: "library", label: t("collection.addTo"), run: () => ((nav.tvMenu = null), (nav.collectionFor = item)) });
    }
    a.push({ icon: "star", label: item.userRating ? t("tvMenu.rated", { stars: Math.round(item.userRating / 2) }) : t("tvMenu.rate"), run: () => (rating = true) });
    if (!music) {
      if (started) a.push({ icon: "x", label: t("tvMenu.removeContinue"), run: () => run(() => api.removeFromContinue(item.ratingKey), t("tvMenu.removedContinue"), true) });
      if (!item.watched) a.push({ icon: "checkCircle", label: t("cardMenu.markWatched"), run: () => run(() => api.setWatched(item.ratingKey, true), t("cardMenu.markedWatched"), () => setWatched(true)) });
      if (item.watched || started) a.push({ icon: "restart", label: t("cardMenu.markUnwatched"), run: () => run(() => api.setWatched(item.ratingKey, false), t("cardMenu.markedUnwatched"), () => setWatched(false)) });
    }
    return a;
  });

  function rate(stars: number | null) {
    run(() => api.rate(item.ratingKey, stars === null ? null : stars * 2), stars === null ? t("tvMenu.ratingCleared") : t("tvMenu.ratingSaved", { stars }), () => (item.userRating = stars === null ? null : stars * 2));
  }

  // Haut / bas d'une action à l'autre ; les autres flèches restent dans le panneau.
  function onKey(e: KeyboardEvent) {
    const buttons = [...(panel?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
    const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      e.stopPropagation();
      const next = buttons[Math.min(buttons.length - 1, Math.max(0, i + (e.key === "ArrowDown" ? 1 : -1)))];
      next?.focus();
    } else if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      e.stopPropagation();
    }
  }

  $effect(() => {
    void rating;
    void actions.length;
    // Le premier choix prend le focus (et le garde quand la liste s'allonge).
    if (!panel?.contains(document.activeElement)) panel?.querySelector<HTMLButtonElement>("button")?.focus();
  });

  onMount(() => {
    if (watchGuid) api.watchlisted(watchGuid).then((w) => (watchlisted = w), () => {});
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="tv-menu-shade" onclick={close}></div>
<div class="tv-menu" bind:this={panel} onkeydowncapture={onKey} role="menu" aria-label={menu.item.title}>
  <h2>{item.kind === "episode" ? (item.showTitle ?? item.title) : item.title}</h2>
  {#if rating}
    {#each [5, 4, 3, 2, 1] as stars (stars)}
      <button role="menuitem" onclick={() => rate(stars)}>
        <span class="stars">{"★".repeat(stars)}<span class="off">{"★".repeat(5 - stars)}</span></span>
      </button>
    {/each}
    {#if item.userRating}
      <button role="menuitem" onclick={() => rate(null)}><Icon name="x" size={26} /> {t("tvMenu.clearRating")}</button>
    {/if}
  {:else}
    {#each actions as action (action.label)}
      <button role="menuitem" onclick={action.run}><Icon name={action.icon} size={26} filled={action.filled} /> {action.label}</button>
    {/each}
  {/if}
</div>

<style>
  .tv-menu-shade {
    position: fixed;
    inset: 0;
    z-index: 70;
    background: #0009;
  }

  /* Panneau noir à droite, comme dans Plex. */
  .tv-menu {
    position: fixed;
    top: 24px;
    right: 24px;
    bottom: 24px;
    z-index: 71;
    width: 400px;
    padding: 40px 32px;
    overflow-y: auto;
    border: 1px solid #ffffff1a;
    border-radius: 20px;
    background: #000;
  }

  h2 {
    margin: 0 0 36px;
    overflow: hidden;
    font-size: 24px;
    font-weight: 700;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  button {
    display: flex;
    align-items: center;
    gap: 20px;
    width: 100%;
    margin-bottom: 6px;
    padding: 12px 18px;
    border: none;
    border-radius: 28px;
    background: none;
    color: #fff;
    font-size: 19px;
    text-align: left;
  }

  button:focus {
    outline: none;
    background: #fff;
    color: #000;
  }

  .stars {
    color: var(--accent);
    font-size: 24px;
    letter-spacing: 4px;
  }

  .stars .off {
    color: #ffffff33;
  }

  button:focus .stars .off {
    color: #0003;
  }
</style>
