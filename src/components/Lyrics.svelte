<script lang="ts">
  // Paroles du morceau en cours : la ligne chantée ressort et reste au milieu ; un clic sur une ligne y saute.
  // Après un défilement à la main, le suivi reprend au bout de quelques secondes.
  import { t } from "../lib/i18n";
  import { lineAt, lyrics } from "../lib/lyrics.svelte";
  import { mpv, player } from "../lib/player.svelte";

  let { tv = false }: { tv?: boolean } = $props();

  let box = $state<HTMLElement>();
  let manualUntil = 0;

  const lines = $derived(lyrics.state.status === "synced" ? lyrics.state.lines : []);
  const active = $derived(lineAt(lines, player.position));

  $effect(() => {
    const i = active;
    if (!box || i < 0 || Date.now() < manualUntil) return;
    const el = box.querySelector<HTMLElement>(`[data-line="${i}"]`);
    if (!el) return;
    box.scrollTo({ top: el.offsetTop - box.clientHeight / 2 + el.clientHeight / 2, behavior: "smooth" });
  });

  // Retour en haut à chaque nouveau morceau.
  $effect(() => {
    void lyrics.key;
    box?.scrollTo({ top: 0 });
  });

  function manual() {
    manualUntil = Date.now() + 4000;
  }
</script>

<div class="lyrics" class:tv role="region" aria-label={t("lyrics.title")} bind:this={box} onwheel={manual} ontouchmove={manual}>
  {#if lyrics.state.status === "loading"}
    <div class="ph">
      {#each [70, 55, 80, 45, 65] as w, i (i)}<span class="skeleton" style:width="{w}%"></span>{/each}
    </div>
  {:else if lyrics.state.status === "synced"}
    <div class="pad"></div>
    {#each lines as line, i (i)}
      {#if tv}
        <p class="line" class:on={i === active} class:past={i < active} data-line={i}>{line.text || "♪"}</p>
      {:else}
        <button class="line" class:on={i === active} class:past={i < active} data-line={i} onclick={() => mpv("seek", line.at, "absolute")}>
          {line.text || "♪"}
        </button>
      {/if}
    {/each}
    <div class="pad"></div>
  {:else if lyrics.state.status === "plain"}
    <p class="plain">{lyrics.state.text}</p>
  {:else}
    <p class="empty">{lyrics.state.status === "instrumental" ? t("lyrics.instrumental") : t("lyrics.none")}</p>
  {/if}
  <p class="source">{t("lyrics.source")}</p>
</div>

<style>
  .lyrics {
    height: 100%;
    overflow-y: auto;
    scrollbar-width: none;
    mask-image: linear-gradient(transparent, #000 12%, #000 88%, transparent);
  }

  .lyrics::-webkit-scrollbar {
    display: none;
  }

  .pad {
    height: 35%;
  }

  .line {
    display: block;
    width: 100%;
    margin: 0;
    padding: 6px 0;
    border: none;
    background: none;
    color: #ffffff59;
    font: inherit;
    font-size: 22px;
    font-weight: 700;
    line-height: 1.35;
    text-align: left;
    transition: color 0.3s ease, transform 0.3s ease;
    transform-origin: left center;
  }

  button.line {
    cursor: pointer;
  }

  button.line:hover {
    color: #ffffffa6;
  }

  .line.past {
    color: #ffffff80;
  }

  .line.on {
    color: #fff;
    transform: scale(1.04);
  }

  .plain {
    margin: 0;
    padding: 24px 0;
    color: #ffffffd9;
    font-size: 18px;
    line-height: 1.6;
    white-space: pre-line;
  }

  .empty {
    margin: 0;
    padding: 30% 0 0;
    color: #ffffff8c;
    font-size: 16px;
  }

  .source {
    margin: 0;
    padding: 16px 0 24px;
    color: #ffffff59;
    font-size: 11px;
  }

  .ph {
    display: grid;
    gap: 16px;
    padding-top: 30%;
  }

  .ph .skeleton {
    height: 22px;
    border-radius: 6px;
  }

  /* Téléviseur : plus grand, à lire du canapé. */
  .tv .line {
    font-size: 34px;
    padding: 8px 0;
  }

  .tv .plain {
    font-size: 26px;
  }

  :global(html.phone) .line {
    font-size: 20px;
  }
</style>
