<!-- Fiche : deux panneaux, comme Jellyfin, l'équipe à gauche (réalisation, scénario, studio, genres) et les pistes à
     lire à droite (version, audio, sous-titres). -->
<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  import TrackSelect from "../../components/TrackSelect.svelte";
  import type { ItemDetail, Version } from "../../lib/api";
  import { t } from "../../lib/i18n";

  /** `media` : la version choisie (ou la fiche elle-même), avec ses pistes ; les pistes choisies remontent à la fiche. */
  let {
    view,
    media,
    version,
    chooseVersion,
    audioStream = $bindable(),
    subtitleStream = $bindable(),
  }: {
    view: ItemDetail;
    media: Version | ItemDetail | null;
    version: number;
    chooseVersion: (v: number) => void;
    audioStream: number | null;
    subtitleStream: number;
  } = $props();

  const hasCredits = $derived(!!view && (view.directors.length + view.writers.length + view.genres.length > 0 || !!view.studio));
  const hasMedia = $derived(!!view && (!!view.video || view.audioTracks.length > 0 || view.playable));
</script>

<!-- Deux panneaux, comme Jellyfin : l'équipe à gauche, les pistes à lire à droite. -->
{#if hasCredits || hasMedia}
  <div class="panels" class:single={!hasMedia || !hasCredits}>
    {#if hasCredits}
      <dl class="panel credits">
        {#if view.directors.length}<div><dt>{t("item.facts.director")}</dt><dd>{view.directors.join(", ")}</dd></div>{/if}
        {#if view.writers.length}<div><dt>{t("item.facts.writer")}</dt><dd>{view.writers.slice(0, 3).join(", ")}</dd></div>{/if}
        {#if view.studio}<div><dt>{t("item.facts.studio")}</dt><dd>{view.studio}</dd></div>{/if}
        {#if view.genres.length}
          <div><dt>{t("item.facts.genres")}</dt><dd class="genres">{#each view.genres as g}<span>{g}</span>{/each}</dd></div>
        {/if}
      </dl>
    {/if}
    {#if hasMedia}
      <div class="panel media">
        <!-- Plusieurs versions (4K, 1080p…) : la ligne vidéo devient un choix, comme dans Plex. -->
        {#if view.versions?.length > 1}
          <label class="track" data-nav-row>
            <span class="ico" title={t("item.facts.version")}><Icon name="film" size={18} /></span>
            <TrackSelect
              value={version}
              options={view.versions.map((v, i) => ({ value: i, label: v.label }))}
              label={t("item.facts.version")}
              onchange={(v) => chooseVersion(Number(v))}
            />
          </label>
        {:else if media?.video}
          <div class="track"><span class="ico" title={t("item.facts.video")}><Icon name="film" size={18} /></span><span class="video">{media.video}</span></div>
        {/if}
        {#if media?.audioTracks.length}
          <label class="track" data-nav-row>
            <span class="ico" title={t("item.facts.audio")}><Icon name="audio" size={18} /></span>
            <TrackSelect
              bind:value={audioStream}
              options={media.audioTracks.map((tr) => ({ value: tr.id, label: tr.label }))}
              label={t("item.facts.audio")}
            />
          </label>
        {/if}
        {#if view.playable}
          <label class="track" data-nav-row>
            <span class="ico" title={t("item.facts.subtitles")}><Icon name="subtitles" size={18} /></span>
            <TrackSelect
              bind:value={subtitleStream}
              options={[{ value: 0, label: t("item.facts.none") }, ...(media?.subtitleTracks ?? []).map((tr) => ({ value: tr.id, label: tr.label }))]}
              label={t("item.facts.subtitles")}
            />
          </label>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  /* Un seul panneau (série, saison : pas de pistes à choisir) : à la même largeur, centré. */
  .panels.single {
    grid-template-columns: minmax(0, calc((100% - 20px) / 2));
    justify-content: center;
  }

  /* ---------- Panneaux : équipe et pistes ---------- */

  .panels {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 20px;
    margin-bottom: 8px;
  }

  .panel {
    margin: 0;
    padding: 6px 20px;
    border: 1px solid #ffffff14;
    border-radius: var(--radius-lg);
    background: #16171bcc;
    backdrop-filter: blur(16px);
  }

  .credits > div {
    display: grid;
    grid-template-columns: 130px minmax(0, 1fr);
    gap: 16px;
    padding: 12px 0;
  }

  .credits > div + div {
    border-top: 1px solid #ffffff10;
  }

  dt {
    color: var(--text-dim);
    font-size: 13px;
    font-weight: 700;
  }

  dd {
    margin: 0;
    font-size: 14px;
    text-align: right;
  }

  .genres {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 6px;
  }

  .genres span {
    padding: 2px 10px;
    border: 1px solid var(--border);
    border-radius: 12px;
    color: var(--text-dim);
    font-size: 12px;
  }

  .media {
    display: grid;
    align-content: start;
    gap: 10px;
    padding: 16px 20px;
  }

  .track {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }

  .ico {
    flex: none;
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    background: #ffffff0f;
    color: var(--accent);
  }

  .video {
    font-size: 14px;
    font-weight: 600;
  }

  :global(html.phone) .panels {
    grid-template-columns: minmax(0, 1fr);
    gap: 12px;
  }

  :global(html.phone) .credits > div {
    grid-template-columns: 96px minmax(0, 1fr);
  }
</style>
