<!-- Réglages → Interface : couleur, langue, barre du lecteur, menu, notes des sites, accueil, bibliothèques (TV). -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import TrackSelect from "../../components/TrackSelect.svelte";
  import RatingLogo from "../../components/RatingLogo.svelte";
  import HomeEditor from "../../components/HomeEditor.svelte";
  import Icon from "../../components/Icon.svelte";
  import Banner from "./Banner.svelte";
  import { ratingPrefs, setRatingSite, type RatingSite } from "../../lib/ratings.svelte";
  import { playerBar, setPlayerBar, type PlayerBar } from "../../lib/playerBar.svelte";
  import { api, type Section } from "../../lib/api";
  import { nav, toast } from "../../lib/nav.svelte";
  import { ordered, saveSidebar, sidebar } from "../../lib/sidebar.svelte";
  import { device } from "../../lib/device";
  import { applyTheme, savedTheme, THEME_COLORS, THEMES, type Theme } from "../../lib/theme";
  import { LANGUAGES, savedLanguage, setLanguage, t } from "../../lib/i18n";

  let { desk }: { desk: boolean } = $props();

  function resetSidebar() {
    sidebar.order = [];
    sidebar.hidden = [];
    // Sur téléviseur, le menu reste replié hors sélection.
    sidebar.collapsed = device.tv;
    saveSidebar();
    toast(t("settings.interface.menuReset"));
  }

  // Téléviseur : ordre et visibilité des bibliothèques du menu, aux flèches de la télécommande.
  let sections = $state<Section[]>([]);
  const libraries = $derived(ordered(sections));

  async function moveLibrary(key: string, delta: number) {
    const keys = libraries.map((s) => s.key);
    const i = keys.indexOf(key);
    keys.splice(i, 1);
    keys.splice(i + delta, 0, key);
    sidebar.order = keys;
    saveSidebar();
    // La ligne a bougé : la sélection la suit (ou passe à l'autre flèche en bout de liste).
    await tick();
    const own = document.querySelector<HTMLButtonElement>(`[data-lib="${key}"][data-dir="${delta}"]`);
    const other = document.querySelector<HTMLButtonElement>(`[data-lib="${key}"][data-dir="${-delta}"]`);
    (own && !own.disabled ? own : other)?.focus();
  }

  function toggleLibrary(key: string) {
    sidebar.hidden = sidebar.hidden.includes(key) ? sidebar.hidden.filter((k) => k !== key) : [...sidebar.hidden, key];
    saveSidebar();
  }

  let theme = $state<Theme>(savedTheme());
  const THEME_LABELS: Record<Theme, string> = {
    jaune: t("settings.interface.themes.jaune"),
    rouge: t("settings.interface.themes.rouge"),
    vert: t("settings.interface.themes.vert"),
    violet: t("settings.interface.themes.violet"),
  };

  // Langue choisie (« auto » : celle du système) ; en changer recharge l'interface.
  const language = savedLanguage();

  function chooseTheme(th: Theme) {
    theme = th;
    applyTheme(th, true);
  }

  onMount(() => {
    if (device.tv) api.sections().then((s) => (sections = s), () => {});
  });
</script>

<h1 class="title">{t("settings.menu.interface")}</h1>
{#if desk}<Banner icon="menu" title={t("settings.menu.interface")} text={t("settings.banner.interface")} />{/if}
<section class="card list">
  <div class="line">
    <div class="line-text"><strong>{t("settings.interface.color")}</strong></div>
    <div class="themes" role="radiogroup" aria-label={t("settings.interface.color")}>
      {#each THEMES as th (th)}
        <button
          class="theme"
          class:on={theme === th}
          role="radio"
          aria-checked={theme === th}
          aria-label={THEME_LABELS[th]}
          title={THEME_LABELS[th]}
          style:--swatch={THEME_COLORS[th]}
          onclick={() => chooseTheme(th)}
        >
          <span class="swatch">{#if theme === th}<Icon name="check" size={18} />{/if}</span>
        </button>
      {/each}
    </div>
  </div>
  <div class="line">
    <div class="line-text"><strong>{t("settings.interface.language")}</strong></div>
    <div class="field-slot">
      <TrackSelect
        value={language}
        options={[{ value: "auto", label: t("settings.interface.languageAuto") }, ...LANGUAGES.map((l) => ({ value: l.id, label: l.label }))]}
        label={t("settings.interface.language")}
        onchange={(id) => id !== language && setLanguage(id as Parameters<typeof setLanguage>[0])}
      />
    </div>
  </div>
  <!-- Barre du lecteur vidéo : la capsule complète, ou la rangée minimaliste. -->
  <div class="line">
    <div class="line-text">
      <strong>{t("settings.interface.playerBar")}</strong>
      <span class="muted small">{t("settings.interface.playerBarHint")}</span>
    </div>
    <div class="field-slot">
      <TrackSelect
        value={playerBar.style}
        options={[
          { value: "full", label: t("settings.interface.playerBarFull") },
          { value: "minimal", label: t("settings.interface.playerBarMinimal") },
        ]}
        label={t("settings.interface.playerBar")}
        onchange={(v) => setPlayerBar(v as PlayerBar)}
      />
    </div>
  </div>
  {#if device.tv}
    <label class="line">
      <div class="line-text">
        <strong>{t("settings.interface.simpleBar")}</strong>
        <span class="muted small">{t("settings.interface.simpleBarHint")}</span>
      </div>
      <input type="checkbox" class="switch" bind:checked={sidebar.simple} onchange={saveSidebar} />
    </label>
  {:else}
    <label class="line">
      <div class="line-text">
        <strong>{t("settings.interface.collapsed")}</strong>
        <span class="muted small">{t("settings.interface.collapsedHint")}</span>
      </div>
      <input type="checkbox" class="switch" bind:checked={sidebar.collapsed} onchange={saveSidebar} />
    </label>
    <div class="line">
      <div class="line-text">
        <strong>{t("settings.interface.resetMenu")}</strong>
        <span class="muted small">{t("settings.interface.reorderHint")}</span>
      </div>
      <button class="btn" onclick={resetSidebar}>{t("settings.interface.resetMenu")}</button>
    </div>
  {/if}
</section>

<!-- Notes des sites sur les fiches des films et des séries : celles que le serveur connaît, et Allociné. -->
<div class="tile">
<h2 class="group-title">{t("settings.ratings.title")}</h2>
<section class="card list">
  {#each (nav.serverKind === "plex" ? ["imdb", "rtCritic", "rtAudience", "tmdb", "allocinePress", "allocineUsers"] : ["community", "rtCritic", "allocinePress", "allocineUsers"]) as RatingSite[] as site (site)}
    <label class="line">
      <span class="rating-logo"><RatingLogo source={site} /></span>
      <span class="line-text">
        <strong>{t(`settings.ratings.${site}`)}</strong>
        {#if site === "allocinePress" || site === "allocineUsers"}<span class="muted small">{t("settings.ratings.allocineHint")}</span>{/if}
      </span>
      <input type="checkbox" class="switch" checked={ratingPrefs[site]} onchange={(e) => setRatingSite(site, e.currentTarget.checked)} />
    </label>
  {/each}
</section>
</div>

<div class="tile">
<h2 class="group-title">{t("settings.interface.home")}</h2>
<section class="card">
  <p class="muted">{t("settings.interface.homeHint")}</p>
  <HomeEditor />
</section>
</div>

{#if device.tv}
  <div class="tile">
  <h2 class="group-title">{t("settings.interface.libraries")}</h2>
  <section class="card">
    <div class="libs">
      {#each libraries as s, i (s.key)}
        {@const off = sidebar.hidden.includes(s.key)}
        <div class="lib" class:off>
          <Icon name={s.kind === "movie" ? "film" : s.kind === "artist" ? "music" : "tv"} />
          <span class="lib-name">{s.title}</span>
          <button class="btn round" data-lib={s.key} data-dir="-1" disabled={i === 0} onclick={() => moveLibrary(s.key, -1)} aria-label={t("settings.interface.moveUp", { title: s.title })}>
            <Icon name="chevronUp" size={18} />
          </button>
          <button class="btn round" data-lib={s.key} data-dir="1" disabled={i === libraries.length - 1} onclick={() => moveLibrary(s.key, 1)} aria-label={t("settings.interface.moveDown", { title: s.title })}>
            <Icon name="chevronDown" size={18} />
          </button>
          <button class="btn round" onclick={() => toggleLibrary(s.key)} aria-label={off ? t("settings.interface.show", { title: s.title }) : t("settings.interface.hide", { title: s.title })}>
            <Icon name={off ? "eyeOff" : "eye"} size={18} />
          </button>
        </div>
      {/each}
    </div>
    <div><button class="btn" onclick={resetSidebar}>{t("settings.interface.resetMenu")}</button></div>
  </section>
  </div>
{/if}
