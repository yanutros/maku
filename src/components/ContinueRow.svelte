<script lang="ts">
  import { episodeTitle, errorText, formatDuration, type Item } from "../lib/api";
  import { go, nav, toast } from "../lib/nav.svelte";
  import { play } from "../lib/player.svelte";
  import Icon from "./Icon.svelte";
  import TitleLogo from "./TitleLogo.svelte";
  import { t } from "../lib/i18n";

  /**
   * « Continuer à regarder » façon plateforme de streaming : de grandes affiches, et celle
   * survolée (la première au départ) s'ouvre en grand format avec sa scène, son épisode,
   * sa progression et le bouton pour reprendre.
   */
  let { title, items }: { title: string; items: Item[] } = $props();

  let active = $state(0);
  let scroller = $state<HTMLElement>();
  let atStart = $state(true);
  let atEnd = $state(false);

  function update() {
    if (!scroller) return;
    atStart = scroller.scrollLeft <= 4;
    atEnd = scroller.scrollLeft + scroller.clientWidth >= scroller.scrollWidth - 4;
  }

  function scrollBy(dir: number) {
    scroller?.scrollBy({ left: dir * scroller.clientWidth * 0.8, behavior: "smooth" });
  }

  $effect(() => {
    void active;
    update();
  });

  function scene(item: Item) {
    return item.kind === "episode" ? (item.thumb ?? item.backdrop) : (item.backdrop ?? item.thumb);
  }

  function progress(item: Item) {
    return item.viewOffsetMs && item.durationMs ? Math.min(100, (item.viewOffsetMs / item.durationMs) * 100) : 0;
  }

  function remaining(item: Item) {
    return item.viewOffsetMs && item.durationMs ? formatDuration(item.durationMs - item.viewOffsetMs) : null;
  }

  function resume(item: Item) {
    play(item.ratingKey).catch((e) => toast(errorText(e), true));
  }

  function openMenu(e: MouseEvent, item: Item) {
    e.preventDefault();
    nav.menu = { item, x: e.clientX, y: e.clientY };
  }
</script>

<section>
  <div class="head">
    <h2>{title}</h2>
    <div class="arrows">
      <button class="btn round" disabled={atStart} onclick={() => scrollBy(-1)} aria-label={t("common.c1.previous")}><Icon name="chevronLeft" size={18} /></button>
      <button class="btn round" disabled={atEnd} onclick={() => scrollBy(1)} aria-label={t("common.c1.next")}><Icon name="chevronRight" size={18} /></button>
    </div>
  </div>

  <div class="track" bind:this={scroller} onscroll={update}>
    {#each items as item, i (item.ratingKey)}
      {@const open = active === i}
      {@const pct = progress(item)}
      <article
        class="card"
        class:open
        onmouseenter={() => (active = i)}
        onfocusin={() => (active = i)}
        oncontextmenu={(e) => openMenu(e, item)}
      >
        <!-- Affiche, visible quand la carte est fermée. -->
        <button class="poster" onclick={() => (open ? go({ name: "item", ratingKey: item.ratingKey }) : (active = i))} aria-label="{item.showTitle ?? item.title}" tabindex={open ? -1 : 0}>
          {#if item.poster}<img src={item.poster} alt="" loading="lazy" />{/if}
        </button>

        <!-- Grand format : scène, titre, épisode, progression et reprise. -->
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="wide" aria-hidden={!open} onclick={() => go({ name: "item", ratingKey: item.ratingKey })}>
          {#if scene(item)}<img class="scene" src={scene(item)} alt="" loading="lazy" />{/if}
          <div class="shade"></div>
          <div class="info">
            {#if item.kind === "episode"}<div class="eyebrow">{t("common.c1.show")}</div>{:else}<div class="eyebrow">{t("common.c1.movie")}</div>{/if}
            <h3><TitleLogo logo={item.logo} guid={item.logoGuid} genres={item.genres} title={item.showTitle ?? item.title} /></h3>
            <div class="sub">
              {#if item.kind === "episode"}
                <span class="ep">{t("common.c1.episodeCode", { season: item.parentIndex ?? "", episode: item.index ?? "" })}</span>{#if episodeTitle(item.title)} · {episodeTitle(item.title)}{/if}
              {:else if item.year}
                {item.year}{#if item.genres.length} · {item.genres.slice(0, 2).join(" · ")}{/if}
              {/if}
            </div>
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <div class="actions" onclick={(e) => e.stopPropagation()}>
              <button class="resume" onclick={() => resume(item)} tabindex={open ? 0 : -1}>
                <Icon name="play" size={20} filled />
                {item.viewOffsetMs ? t("common.c1.resume") : t("common.c1.play")}
              </button>
              <button class="ghost" onclick={() => go({ name: "item", ratingKey: item.ratingKey })} tabindex={open ? 0 : -1} aria-label={t("common.c1.moreInfo")} title={t("common.c1.moreInfo")}>
                <Icon name="chevronDown" size={20} />
              </button>
              <button class="ghost" onclick={(e) => openMenu(e, item)} tabindex={open ? 0 : -1} aria-label={t("common.c1.moreActions")} title={t("common.c1.moreActions")}>
                <Icon name="more" size={20} />
              </button>
              {#if remaining(item)}<span class="left">{t("common.c1.remaining", { time: remaining(item) ?? "" })}</span>{/if}
            </div>
          </div>
        </div>

        {#if pct > 0}<div class="progress"><div style:width="{pct}%"></div></div>{/if}
      </article>
    {/each}
  </div>
</section>

<style>
  section {
    --h: clamp(250px, 26vw, 360px);
    margin-top: 28px;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 14px;
  }

  h2 {
    margin: 0;
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }

  .arrows {
    display: flex;
    gap: 6px;
  }

  .arrows .btn {
    width: 32px;
    height: 32px;
  }

  .track {
    display: flex;
    gap: 14px;
    overflow-x: auto;
    padding: 14px 8px 18px;
    margin: -14px -8px -10px;
    scrollbar-width: none;
  }

  .track::-webkit-scrollbar {
    display: none;
  }

  /* Fermée : une affiche ; ouverte : le grand format en 16:9, à la même hauteur. */
  .card {
    position: relative;
    flex: none;
    width: calc(var(--h) * 2 / 3);
    height: var(--h);
    overflow: hidden;
    border-radius: 14px;
    background: var(--bg-elev);
    box-shadow: 0 10px 28px #0008;
    transition:
      width 0.5s var(--ease),
      box-shadow 0.3s var(--ease),
      transform 0.3s var(--ease);
  }

  .card.open {
    width: calc(var(--h) * 16 / 9);
    box-shadow:
      0 0 0 2px #ffffffd0,
      0 22px 50px #000c;
  }

  .card:not(.open):hover {
    transform: translateY(-4px);
  }

  .poster,
  .wide {
    position: absolute;
    inset: 0;
    transition: opacity 0.4s var(--ease);
  }

  .poster {
    padding: 0;
    border: none;
    background: none;
  }

  .poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .card.open .poster {
    opacity: 0;
    pointer-events: none;
  }

  .wide {
    opacity: 0;
    pointer-events: none;
  }

  .card.open .wide {
    opacity: 1;
    pointer-events: auto;
    cursor: pointer;
  }

  .actions button {
    cursor: pointer;
  }

  .scene {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    transform: scale(1.06);
    transition: transform 6s ease-out;
  }

  .card.open .scene {
    transform: scale(1);
  }

  .shade {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(0deg, #000000f2 0%, #000000a0 35%, transparent 70%),
      linear-gradient(90deg, #00000080 0%, transparent 55%);
  }

  .info {
    position: absolute;
    left: 24px;
    right: 24px;
    bottom: 24px;
    /* Le texte apparaît une fois la carte ouverte. */
    opacity: 0;
    transform: translateY(10px);
    transition:
      opacity 0.35s 0.15s,
      transform 0.45s 0.15s var(--ease);
  }

  .card.open .info {
    opacity: 1;
    transform: none;
  }

  .eyebrow {
    color: var(--accent);
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.18em;
    text-transform: uppercase;
  }

  h3 {
    margin: 4px 0 6px;
    overflow: hidden;
    font-family: "Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif;
    font-size: clamp(24px, 2.6vw, 38px);
    font-weight: 900;
    letter-spacing: -0.03em;
    line-height: 1.02;
    text-overflow: ellipsis;
    text-shadow: 0 4px 24px #000;
    white-space: nowrap;
  }

  .sub {
    overflow: hidden;
    color: #ffffffd0;
    font-size: 14px;
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ep {
    color: #fff;
    font-weight: 800;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 16px;
  }

  .resume {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 42px;
    padding: 0 20px 0 16px;
    border: none;
    border-radius: 21px;
    background: #fff;
    color: #000;
    font-weight: 700;
    transition:
      transform 0.15s,
      background 0.15s;
  }

  .resume:hover {
    background: var(--accent);
    transform: scale(1.04);
  }

  .ghost {
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    padding: 0;
    border: 2px solid #ffffff66;
    border-radius: 50%;
    background: #00000040;
    color: #fff;
    backdrop-filter: blur(8px);
    transition:
      border-color 0.15s,
      background 0.15s;
  }

  .ghost:hover {
    border-color: #fff;
    background: #ffffff26;
  }

  .left {
    margin-left: 6px;
    color: #ffffffb0;
    font-size: 13px;
    font-weight: 600;
  }

  /* Barre de progression, en bas de la carte dans les deux formats. */
  .progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 5px;
    background: #ffffff33;
  }

  .progress div {
    height: 100%;
    background: var(--accent);
    box-shadow: 0 0 12px var(--accent);
  }

  /* ---------- Téléphone : cartes plus basses, la carte ouverte laisse voir la suivante ---------- */

  :global(html.phone) section {
    --h: 210px;
    margin-top: 22px;
  }

  :global(html.phone) h2 {
    font-size: 17px;
  }

  :global(html.phone) .arrows {
    display: none;
  }

  :global(html.phone) .track {
    gap: 10px;
    padding: 10px 16px 14px;
    margin: -10px -16px -6px;
  }

  :global(html.phone) .card.open {
    width: min(calc(var(--h) * 16 / 9), calc(100vw - 32px - 56px));
  }

  :global(html.phone) .info {
    right: 14px;
    bottom: 14px;
    left: 14px;
  }

  :global(html.phone) h3 {
    font-size: 22px;
  }

  :global(html.phone) .sub {
    font-size: 12px;
  }

  :global(html.phone) .actions {
    gap: 8px;
    margin-top: 10px;
  }

  :global(html.phone) .resume {
    height: 36px;
    padding: 0 14px 0 12px;
    font-size: 14px;
  }

  :global(html.phone) .ghost {
    width: 36px;
    height: 36px;
  }

  :global(html.phone) .left {
    display: none;
  }

  /* Logo du titre dans la carte ouverte. */
  h3 {
    --logo-h: 72px;
    --logo-w: 62%;
  }

  :global(html.phone) h3 {
    --logo-h: 48px;
  }
</style>
