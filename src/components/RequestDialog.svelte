<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, seerrStatusLabel, type SeerrMedia, type SeerrSeason } from "../lib/api";
  import { nav, toast } from "../lib/nav.svelte";
  import { t } from "../lib/i18n";

  let { media }: { media: SeerrMedia } = $props();

  let seasons = $state<SeerrSeason[]>([]);
  let selected = $state<number[]>([]);
  let loading = $state(media.mediaType === "tv");
  let sending = $state(false);
  let error = $state<string | null>(null);

  // Saisons déjà demandées, en cours ou disponibles : non sélectionnables.
  const isLocked = (s: SeerrSeason) => s.status >= 2 && s.status !== 4;
  const available = $derived(seasons.filter((s) => s.seasonNumber > 0 && !isLocked(s)));

  onMount(async () => {
    if (media.mediaType !== "tv") return;
    try {
      seasons = await api.seerrTvSeasons(media.tmdbId);
      selected = available.map((s) => s.seasonNumber);
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  });

  function toggle(n: number) {
    selected = selected.includes(n) ? selected.filter((x) => x !== n) : [...selected, n];
  }

  function close() {
    nav.requesting = null;
  }

  async function submit() {
    sending = true;
    error = null;
    try {
      await api.seerrRequest(media.mediaType, media.tmdbId, media.mediaType === "tv" ? selected : null);
      media.status = 2;
      toast(t("request.sent", { title: media.title }));
      close();
    } catch (e) {
      error = errorText(e);
    } finally {
      sending = false;
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={close}>
  <div class="dialog" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
    {#if media.backdrop}<img class="hero" src={media.backdrop} alt="" />{/if}
    <div class="body">
      <h2>{t("request.title", { title: media.title })}</h2>
      {#if media.overview}<p class="muted overview">{media.overview}</p>{/if}

      {#if media.mediaType === "tv"}
        {#if loading}
          <p class="muted">{t("request.loadingSeasons")}</p>
        {:else}
          <div class="seasons-head">
            <strong>{t("request.seasons")}</strong>
            <button
              class="link"
              onclick={() =>
                (selected = selected.length === available.length ? [] : available.map((s) => s.seasonNumber))}
            >
              {selected.length === available.length ? t("request.uncheckAll") : t("request.checkAll")}
            </button>
          </div>
          <div class="seasons">
            {#each seasons as s (s.seasonNumber)}
              <label class:locked={isLocked(s)}>
                <input
                  type="checkbox"
                  class="switch"
                  disabled={isLocked(s)}
                  checked={selected.includes(s.seasonNumber)}
                  onchange={() => toggle(s.seasonNumber)}
                />
                <span>{s.name}</span>
                <span class="muted">{t("request.episodes", { count: s.episodeCount })}</span>
                {#if seerrStatusLabel(s.status)}<span class="badge">{seerrStatusLabel(s.status)}</span>{/if}
              </label>
            {/each}
          </div>
        {/if}
      {/if}

      {#if error}<p class="error">{error}</p>{/if}

      <div class="actions">
        <button class="btn" onclick={close}>{t("request.cancel")}</button>
        <button
          class="btn primary"
          disabled={sending || loading || (media.mediaType === "tv" && !selected.length)}
          onclick={submit}
        >
          {sending ? t("request.sending") : t("request.submit")}
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: #000a;
    display: grid;
    place-items: center;
    z-index: 50;
  }

  .dialog {
    width: min(560px, 92vw);
    max-height: 86vh;
    overflow-y: auto;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 12px;
  }

  .hero {
    width: 100%;
    height: 180px;
    object-fit: cover;
    display: block;
    mask-image: linear-gradient(#000 50%, transparent);
  }

  .body {
    padding: 4px 24px 20px;
  }

  h2 {
    margin: 12px 0 8px;
  }

  .overview {
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .seasons-head {
    display: flex;
    justify-content: space-between;
    margin: 16px 0 8px;
  }

  .link {
    background: none;
    border: none;
    color: var(--accent);
  }

  .seasons {
    display: grid;
    gap: 4px;
  }

  label {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: 6px;
  }

  label:hover:not(.locked) {
    background: var(--bg-hover);
  }

  label.locked {
    opacity: 0.6;
  }

  label span:nth-of-type(1) {
    flex: 1;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 20px;
  }
</style>
