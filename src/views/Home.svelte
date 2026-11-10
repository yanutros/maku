<script lang="ts">
  import { offline, offlineAvailable } from "../lib/offline.svelte";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, errorText, type Home, type Item, type SeerrMedia, type Section } from "../lib/api";
  import { go } from "../lib/nav.svelte";
  import { arrange, availableRows, shownByDefault } from "../lib/homeRows.svelte";
  import { fromItem, fromSeerr } from "../lib/tvcards";
  import PosterCard from "../components/PosterCard.svelte";
  import SeerrCard from "../components/SeerrCard.svelte";
  import Row from "../components/Row.svelte";
  import HeroCarousel from "../components/HeroCarousel.svelte";
  import ContinueRow from "../components/ContinueRow.svelte";
  import TvHome from "../components/TvHome.svelte";
  import Icon from "../components/Icon.svelte";
  import { device } from "../lib/device";
  import { t } from "../lib/i18n";

  let home = $state<Home | null>(null);
  let trending = $state<SeerrMedia[]>([]);
  let sections = $state<Section[]>([]);
  // Derniers ajouts des bibliothèques épinglées, chargés à la demande.
  let libItems = $state<Record<string, Item[]>>({});
  let error = $state<string | null>(null);

  // Rangées choisies (Réglages → Interface), dans leur ordre.
  const rows = $derived(arrange(availableRows(sections), shownByDefault(sections)).filter((r) => r.on));

  // Bannière : jusqu'à 5 éléments « À suivre », sinon les derniers ajouts.
  const slides = $derived(
    home ? (home.onDeck.length ? home.onDeck : home.recentlyAdded.filter((i) => i.backdrop)) : [],
  );

  async function load() {
    try {
      home = await api.home();
    } catch (e) {
      error = errorText(e);
    }
  }

  const section = (id: string) => sections.find((s) => `lib:${s.key}` === id);

  $effect(() => {
    for (const r of rows) {
      const s = section(r.id);
      if (!s || libItems[s.key]) continue;
      libItems[s.key] = [];
      api
        .sectionItems(s.key, 0, 24, s.kind === "artist" ? 9 : null, null, "addedAt:desc")
        .then((p) => (libItems[s.key] = p?.items ?? []), () => {});
    }
  });

  onMount(() => {
    load();
    api.sections().then((s) => (sections = s ?? []), () => {});
    // Seerr est facultatif : la rangée n'apparaît que s'il répond.
    api.seerrTrending().then((r) => (trending = r.filter((m) => m.status !== 5)), () => {});
    const unlisten = listen("player-stopped", load);
    return () => unlisten.then((f) => f());
  });

  // Téléviseur : les mêmes rangées, en cartes TV.
  const tvRows = $derived.by(() => {
    if (!home) return [];
    return rows
      .map((r) => {
        if (r.id === "continue") return { title: t("tv.continueWatching"), cards: home!.onDeck.map((i) => fromItem(i, t("tv.resume"))) };
        if (r.id === "recent") return { title: t("tv.recentlyAdded"), cards: home!.recentlyAdded.map((i) => fromItem(i, t("tv.recentlyAddedOne"))) };
        if (r.id === "trending") return { title: t("tv.trendingToRequest"), cards: trending.map(fromSeerr) };
        const s = section(r.id);
        return { title: t("home.recentIn", { library: s?.title ?? "" }), cards: (s ? (libItems[s.key] ?? []) : []).map((i) => fromItem(i, t("tv.recentlyAddedOne"))) };
      })
      .filter((r) => r.cards.length);
  });
</script>

{#if error}
  <div class="page">
    <p class="error">{error}</p>
    <!-- Sans réseau (train, avion) : les téléchargements restent lisibles. -->
    {#if offlineAvailable() && offline.items.length}
      <button class="btn primary" onclick={() => go({ name: "downloads" })}><Icon name="download" size={16} /> {t("offline.open")}</button>
    {/if}
  </div>
{:else if !home}
  <div class="hero-skeleton skeleton"></div>
{:else if device.tv}
  <TvHome rows={tvRows} />
{:else}
  <HeroCarousel items={slides} bleed eyebrow={() => (home?.onDeck.length ? t("home.upNext") : t("home.new"))} />

  <div class="page rows">
    {#each rows as r (r.id)}
      {#if r.id === "continue"}
        {#if home.onDeck.length}<ContinueRow title={t("home.continueWatching")} items={home.onDeck} />{/if}
      {:else if r.id === "recent"}
        <Row title={t("home.recentlyAdded")}>
          {#each home.recentlyAdded as item (item.ratingKey)}<PosterCard {item} />{/each}
        </Row>
      {:else if r.id === "trending"}
        {#if trending.length}
          <Row title={t("home.trendingToRequest")}>
            {#each trending as media (media.mediaType + media.tmdbId)}<SeerrCard {media} />{/each}
          </Row>
        {/if}
      {:else}
        {@const s = section(r.id)}
        {#if s && libItems[s.key]?.length}
          <Row title={t("home.recentIn", { library: s.title })}>
            {#each libItems[s.key] as item (item.ratingKey)}<PosterCard {item} />{/each}
          </Row>
        {/if}
      {/if}
    {/each}
    <div class="customize">
      <button class="btn" onclick={() => go({ name: "settings", page: "interface" })}>
        <Icon name="sliders" size={16} /> {t("home.customize")}
      </button>
    </div>
  </div>
{/if}

<style>
  /* Chargement de l'accueil (une seconde au plus) : une lueur discrète qui se fond dans la page, au lieu d'un
     grand rectangle gris aux bords nets, qui se voyait sur tous les appareils. Sur téléviseur, fixée
     à la fenêtre, elle passe derrière le menu jusqu'au bord gauche de l'écran. */
  /* Ordinateur : à la place exacte du bandeau qui va arriver (sous l'en-tête, à droite du menu), pour que rien
     ne change de teinte quand il s'affiche ; une lueur fixée derrière le menu y faisait une marque en haut. */
  .hero-skeleton {
    height: min(62vh, 560px);
    margin-top: -68px;
    border-radius: 0;
    opacity: 0.5;
    pointer-events: none;
    -webkit-mask-image: linear-gradient(180deg, #000 0%, transparent 85%);
    mask-image: linear-gradient(180deg, #000 0%, transparent 85%);
  }

  /* Téléviseur : la page est le repère de ce fond fixé (App.svelte) et commence après la colonne de 72 px du menu.
     Le fond est ramené au bord de l'écran, sur toute sa largeur ; menu déplié, la page glisse de 164 px et il
     repart d'autant vers la gauche, au même rythme : il ne bouge pas à l'écran. */
  :global(html.tv) .hero-skeleton {
    position: fixed;
    top: 0;
    z-index: 0;
    margin-top: 0;
    right: auto;
    left: -72px;
    width: 100vw;
    transition: transform 0.25s var(--ease);
  }

  :global(html.tv .shell:not(.collapsed)) .hero-skeleton {
    transform: translateX(-164px);
  }

  .rows {
    padding-top: 0;
  }

  .customize {
    display: flex;
    justify-content: center;
    margin: 8px 0 24px;
  }

  :global(html.phone) .hero-skeleton {
    height: 40vh;
    margin-top: 0;
  }
</style>
