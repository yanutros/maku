<script lang="ts">
  import { api, type Account, type Section } from "../lib/api";
  import { go, nav, type View } from "../lib/nav.svelte";
  import { ordered, saveSidebar, sidebar } from "../lib/sidebar.svelte";
  import Icon, { type IconName } from "./Icon.svelte";
  import Logo from "./Logo.svelte";
  import Wordmark from "./Wordmark.svelte";
  import { device } from "../lib/device";
  import { current, player } from "../lib/player.svelte";
  import { t } from "../lib/i18n";
  import { showDownloads } from "../lib/offline.svelte";

  let { sections, serverName }: { sections: Section[]; serverName: string | null } = $props();

  const list = $derived(ordered(sections));
  const shown = $derived(list.filter((s) => !sidebar.hidden.includes(s.key)));
  const hidden = $derived(list.filter((s) => sidebar.hidden.includes(s.key)));

  let dragged = $state<string | null>(null);

  // Ordinateur et téléviseur : panneau flottant, carte du profil en bas (photo, nom, serveur) qui ouvre les Réglages.
  const desk = !device.phone;
  /** Téléviseur, barre simplifiée (Réglages → Interface) : une pilule d'icônes qui ne se déplie jamais. */
  const pill = $derived(device.tv && sidebar.simple);
  let account = $state<Account | null>(null);
  let serverKind = $state<"plex" | "jellyfin">("plex");
  let serverBrand = $state("Plex");
  $effect(() => {
    if (!desk || !serverName) return;
    api.account().then((a) => (account = a), () => (account = null));
    api.session().then((s) => ((serverKind = s.serverKind), (serverBrand = s.serverBrand)), () => {});
  });

  function icon(kind: string): IconName {
    return kind === "movie" ? "film" : kind === "artist" ? "music" : "tv";
  }

  function isActive(name: string, key?: string) {
    const v = nav.view;
    return v.name === name && (key === undefined || (v.name === "section" && v.key === key));
  }

  function open(view: View) {
    go(view);
  }

  function toggleCollapsed() {
    sidebar.collapsed = !sidebar.collapsed;
    if (sidebar.collapsed) sidebar.editing = false;
    saveSidebar();
  }

  function toggleHidden(key: string) {
    sidebar.hidden = sidebar.hidden.includes(key) ? sidebar.hidden.filter((k) => k !== key) : [...sidebar.hidden, key];
    saveSidebar();
  }

  // Glisser-déposer : la bibliothèque déplacée prend la place de celle survolée.
  function onDragOver(e: DragEvent, target: string) {
    e.preventDefault();
    if (!dragged || dragged === target) return;
    const keys = list.map((s) => s.key);
    const to = keys.indexOf(target);
    keys.splice(keys.indexOf(dragged), 1);
    keys.splice(to, 0, dragged);
    sidebar.order = keys;
  }

  function onDragEnd() {
    dragged = null;
    saveSidebar();
  }

  function move(key: string, delta: number) {
    const keys = list.map((s) => s.key);
    const i = keys.indexOf(key);
    const j = Math.max(0, Math.min(keys.length - 1, i + delta));
    keys.splice(i, 1);
    keys.splice(j, 0, key);
    sidebar.order = keys;
    saveSidebar();
  }
</script>

<!-- Sur téléviseur, le menu reste replié et ne se déplie que lorsqu'on y entre (sauf en barre simplifiée). -->
<aside
  class:collapsed={sidebar.collapsed || pill}
  class:desk
  class:pill
  data-nav-zone
  onfocusin={() => device.tv && !pill && (sidebar.collapsed = false)}
  onfocusout={(e) => device.tv && !e.currentTarget.contains(e.relatedTarget as Node) && (sidebar.collapsed = true)}
>
  <div class="brand">
    <button class="toggle" onclick={toggleCollapsed} aria-label={sidebar.collapsed ? t("sidebar.expand") : t("sidebar.collapse")} title={sidebar.collapsed ? t("sidebar.expand") : t("sidebar.collapse")}>
      <Icon name="menu" />
    </button>
    {#if !sidebar.collapsed}
      <Logo size={34} bare />
      <Wordmark size={32} />
    {/if}
  </div>

  <nav>
    <!-- Téléviseur : la musique en cours, en tête du menu (le lecteur plein écran s'ouvre). -->
    {#if device.tv && player.active && !player.video && current()}
      <button class="item playing" onclick={() => (player.expanded = true)} title={t("common.c1.nowPlaying")}>
        <Icon name="music" /> <span class="label">{t("common.c1.nowPlaying")}</span>
      </button>
    {/if}
    {#if !device.phone}
      <button class="item" class:active={isActive("search")}
            aria-current={isActive("search") ? "page" : undefined} onclick={() => open({ name: "search", query: "" })} title={t("sidebar.search")}>
        <Icon name="search" /> <span class="label">{t("sidebar.search")}</span>
      </button>
    {/if}
    <button class="item" class:active={isActive("home")}
            aria-current={isActive("home") ? "page" : undefined} onclick={() => open({ name: "home" })} title={t("sidebar.home")}>
      <Icon name="home" /> <span class="label">{t("sidebar.home")}</span>
    </button>
    <!-- Tablette Android : les téléchargements, lisibles hors ligne. -->
    {#if showDownloads()}
      <button class="item" class:active={isActive("downloads")}
              aria-current={isActive("downloads") ? "page" : undefined} onclick={() => open({ name: "downloads" })} title={t("offline.title")}>
        <Icon name="download" /> <span class="label">{t("offline.title")}</span>
      </button>
    {/if}

    {#if sections.length}
      <div class="group">
        <span>{t("sidebar.libraries")}</span>
        {#if !sidebar.collapsed && !device.tv}
          <button
            class="customize"
            class:on={sidebar.editing}
            onclick={() => (sidebar.editing = !sidebar.editing)}
            title={sidebar.editing ? t("sidebar.done") : t("sidebar.reorder")}
            aria-label={t("sidebar.reorder")}
          >
            <Icon name={sidebar.editing ? "check" : "edit"} size={13} />
          </button>
        {/if}
      </div>

      {#if sidebar.editing}
        <p class="hint">{t("sidebar.reorderHint")}</p>
        {#each list as s (s.key)}
          <div
            class="item edit"
            class:dragging={dragged === s.key}
            class:off={sidebar.hidden.includes(s.key)}
            draggable="true"
            ondragstart={(e) => {
              dragged = s.key;
              e.dataTransfer?.setData("text/plain", s.key);
            }}
            ondragover={(e) => onDragOver(e, s.key)}
            ondragend={onDragEnd}
            role="listitem"
          >
            <span class="grip"><Icon name="grip" size={16} /></span>
            <Icon name={icon(s.kind)} />
            <span class="label">{s.title}</span>
            <button class="mini" onclick={() => move(s.key, -1)} aria-label={t("sidebar.moveUp", { title: s.title })}><Icon name="chevronDown" size={14} /></button>
            <button class="mini" onclick={() => toggleHidden(s.key)} aria-label={t("sidebar.toggleHidden", { title: s.title })}>
              <Icon name={sidebar.hidden.includes(s.key) ? "eyeOff" : "eye"} size={15} />
            </button>
          </div>
        {/each}
      {:else}
        {#each shown as s (s.key)}
          <button
            class="item"
            class:active={isActive("section", s.key)}
            aria-current={isActive("section", s.key) ? "page" : undefined}
            onclick={() => open({ name: "section", key: s.key, title: s.title, kind: s.kind })}
            title={s.title}
          >
            <Icon name={icon(s.kind)} />
            <span class="label">{s.title}</span>
          </button>
        {/each}

        {#if hidden.length}
          <button class="item more" onclick={() => (sidebar.showHidden = !sidebar.showHidden)} title={t("sidebar.more")}>
            <Icon name={sidebar.showHidden ? "chevronDown" : "chevronRight"} />
            <span class="label">{t("sidebar.moreCount", { n: hidden.length })}</span>
          </button>
          {#if sidebar.showHidden}
            {#each hidden as s (s.key)}
              <button
                class="item sub"
                class:active={isActive("section", s.key)}
            aria-current={isActive("section", s.key) ? "page" : undefined}
                onclick={() => open({ name: "section", key: s.key, title: s.title, kind: s.kind })}
                title={s.title}
              >
                <Icon name={icon(s.kind)} />
                <span class="label">{s.title}</span>
              </button>
            {/each}
          {/if}
        {/if}
      {/if}
    {/if}

    <div class="group"><span>{t("sidebar.requests")}</span></div>
    <button class="item" class:active={isActive("discover") || isActive("seerr")}
            aria-current={(isActive("discover") || isActive("seerr")) ? "page" : undefined} onclick={() => open({ name: "discover" })} title={t("sidebar.discover")}>
      <Icon name="compass" /> <span class="label">{t("sidebar.discover")}</span>
    </button>
    <div class="group"><span>Tracearr</span></div>
    <button class="item" class:active={isActive("stats")}
            aria-current={isActive("stats") ? "page" : undefined} onclick={() => open({ name: "stats" })} title={t("sidebar.stats")}>
      <Icon name="stats" /> <span class="label">{t("sidebar.stats")}</span>
    </button>
  </nav>

  <div class="spacer"></div>
  {#if desk && nav.homeProfiles > 1 && serverKind === "plex"}
    <!-- Plex Home : la carte ouvre « Qui regarde ? », la roue dentée les Réglages. -->
    <div class="profile-card">
      <button class="profile" onclick={() => (nav.profiles = true)} title={t("sidebar.switchUser")}>
        {#if account?.thumb}<img class="avatar" src={account.thumb} alt="" />{:else}<span class="avatar"><Icon name="user" size={16} /></span>{/if}
        <span class="label">
          {account?.username ?? t("sidebar.switchUser")}
          <small>{t("sidebar.switchUser")}</small>
        </span>
      </button>
      <button class="gear-btn" class:active={isActive("settings")}
              aria-current={isActive("settings") ? "page" : undefined} onclick={() => open({ name: "settings" })} title={t("sidebar.settings")}>
        <Icon name="settings" size={17} />
      </button>
    </div>
  {:else if desk}
    <button class="profile" class:active={isActive("settings")}
            aria-current={isActive("settings") ? "page" : undefined} onclick={() => open({ name: "settings" })} title={t("sidebar.settings")}>
      {#if account?.thumb}<img class="avatar" src={account.thumb} alt="" />{:else}<span class="avatar"><Icon name="user" size={16} /></span>{/if}
      <span class="label">
        {account?.username ?? t("sidebar.settings")}
        <small>{serverBrand} · {serverName}</small>
      </span>
      <span class="gear"><Icon name="settings" size={17} /></span>
    </button>
  {:else}
    <button class="item settings" class:active={isActive("settings")}
              aria-current={isActive("settings") ? "page" : undefined} onclick={() => open({ name: "settings" })} title={t("sidebar.settings")}>
      <Icon name="settings" />
      <span class="label">
        {t("sidebar.settings")}
        <small>{serverName}</small>
      </span>
    </button>
  {/if}
</aside>

<style>
  aside {
    display: flex;
    flex-direction: column;
    padding: 14px 12px 16px;
    overflow-x: hidden;
    overflow-y: auto;
    background: var(--bg-sidebar);
    border-right: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 4px 16px;
    font-size: 21px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .toggle {
    display: grid;
    place-items: center;
    flex: none;
    width: 40px;
    height: 40px;
    border: none;
    border-radius: 10px;
    background: none;
    color: var(--text-dim);
  }

  .toggle:hover {
    background: #ffffff12;
    color: var(--text);
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    min-height: 40px;
    padding: 8px 12px;
    border: none;
    border-radius: var(--radius);
    background: none;
    color: var(--text-dim);
    text-align: left;
    font-weight: 500;
    white-space: nowrap;
    transition:
      background 0.15s,
      color 0.15s;
  }

  .item :global(svg) {
    flex: none;
  }

  button.item:hover {
    background: #ffffff0d;
    color: var(--text);
  }

  .item.active {
    background: #ffffff12;
    color: var(--text);
    box-shadow: inset 3px 0 0 var(--accent);
  }

  .item.active :global(svg) {
    color: var(--accent);
  }

  .item.playing :global(svg) {
    color: var(--accent);
  }

  .item.sub {
    padding-left: 24px;
  }

  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .settings small {
    display: block;
    font-size: 11px;
    color: var(--text-faint);
  }

  .group {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 12px 6px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.09em;
    color: var(--text-faint);
  }

  .customize {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--text-faint);
  }

  .customize:hover,
  .customize.on {
    background: #ffffff14;
    color: var(--accent);
  }

  .hint {
    margin: 0 12px 6px;
    color: var(--text-faint);
    font-size: 11px;
    white-space: normal;
  }

  .item.edit {
    gap: 8px;
    padding-left: 6px;
    border: 1px dashed #ffffff1a;
    cursor: grab;
  }

  .item.edit.dragging {
    opacity: 0.4;
  }

  .item.edit.off .label {
    color: var(--text-faint);
    text-decoration: line-through;
  }

  .grip {
    color: var(--text-faint);
  }

  .mini {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--text-dim);
  }

  .mini:first-of-type :global(svg) {
    transform: rotate(180deg);
  }

  .mini:hover {
    background: #ffffff14;
    color: var(--text);
  }

  .spacer {
    flex: 1;
  }

  /* ---------- Menu replié : seulement les icônes ---------- */

  .collapsed {
    padding: 14px 10px 16px;
  }

  .collapsed .brand {
    justify-content: center;
    padding: 0 0 16px;
  }

  .collapsed .item {
    justify-content: center;
    padding: 8px 0;
  }

  .collapsed .label,
  .collapsed .group span,
  .collapsed .hint {
    display: none;
  }

  .collapsed .group {
    padding: 10px 8px;
  }

  .collapsed .group::before {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--border);
  }

  .collapsed .item.sub {
    padding-left: 0;
  }

  /* ---------- Ordinateur : panneau flottant (Apple), page active en barre lumineuse (Netflix) ---------- */

  /* Au-dessus de « En cours de lecture », dont le fond passe derrière le panneau translucide. */
  .desk {
    position: relative;
    z-index: 26;
    margin: 10px 0 10px 10px;
    padding: 12px 8px 10px;
    border: 1px solid #ffffff10;
    border-radius: 18px;
    background: color-mix(in srgb, var(--bg-sidebar) 82%, transparent);
    backdrop-filter: blur(24px) saturate(1.4);
  }

  /* Téléviseur : pas de flou d'arrière-plan, coûteux pour son processeur graphique. */
  :global(html.tv) .desk {
    background: color-mix(in srgb, var(--bg-sidebar) 72%, transparent);
    backdrop-filter: none;
  }

  .desk .item {
    position: relative;
    min-height: 38px;
    border-radius: 10px;
  }

  .desk .item.active {
    background: linear-gradient(90deg, color-mix(in srgb, var(--accent) 20%, transparent), transparent 85%);
    box-shadow: none;
    font-weight: 600;
  }

  .desk .item.active::before {
    content: "";
    position: absolute;
    left: 0;
    top: 9px;
    bottom: 9px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--accent);
    box-shadow: 0 0 12px var(--accent);
  }

  .desk .group {
    padding: 16px 12px 4px;
    font-size: 12px;
    text-transform: none;
    letter-spacing: 0;
  }

  .profile {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    margin-top: 10px;
    padding: 8px;
    border: none;
    border-radius: 14px;
    background: #ffffff0a;
    color: var(--text);
    text-align: left;
    font-weight: 600;
    white-space: nowrap;
    transition: background 0.15s;
  }

  .profile:hover,
  .profile.active {
    background: #ffffff14;
  }

  .profile small {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 11px;
    font-weight: 500;
    color: var(--text-faint);
  }

  .avatar {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    object-fit: cover;
    background: #ffffff14;
    color: var(--text-dim);
  }

  .gear {
    display: grid;
    flex: none;
    color: var(--text-faint);
    transition: color 0.15s, transform 0.3s var(--ease);
  }

  .profile:hover .gear,
  .profile.active .gear {
    color: var(--accent);
    transform: rotate(45deg);
  }

  .collapsed.desk {
    padding: 12px 6px 10px;
  }

  .collapsed .profile {
    justify-content: center;
    padding: 6px 0;
    background: none;
  }

  .collapsed .profile .gear {
    display: none;
  }

  /* Plex Home : la carte du profil et la roue dentée des Réglages, côte à côte (l'une sous l'autre,
     menu replié). */
  .profile-card {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 10px;
    border-radius: 14px;
    background: #ffffff0a;
  }

  .profile-card .profile {
    flex: 1;
    min-width: 0;
    margin-top: 0;
    background: none;
  }

  .profile-card .profile:hover {
    background: #ffffff14;
  }

  .profile-card .label {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .gear-btn {
    display: grid;
    place-items: center;
    flex: none;
    width: 36px;
    height: 36px;
    margin-right: 6px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--text-faint);
    transition: color 0.15s, background 0.15s, transform 0.3s var(--ease);
  }

  .gear-btn:hover,
  .gear-btn:focus-visible,
  .gear-btn.active {
    background: #ffffff14;
    color: var(--accent);
    transform: rotate(45deg);
  }

  .collapsed .profile-card {
    flex-direction: column;
    background: none;
  }

  .collapsed .profile-card .gear-btn {
    margin: 0;
  }

  /* Téléviseur : sa colonne garde la largeur du menu replié (App.svelte) ; déplié, le menu s'élargit
     par-dessus et la page glisse d'autant vers la droite, comme dans Wholphin. Seul le menu change de
     taille : la page n'est jamais remise en page pendant l'animation (c'est ce qui saccadait). */
  :global(html.tv) aside {
    position: relative;
    z-index: 30;
    width: 226px;
    transition: width 0.25s var(--ease);
  }

  :global(html.tv) aside.collapsed {
    width: 62px;
  }
  /* ---------- Téléviseur, barre simplifiée : pilule flottante, centrée en hauteur (façon tvOS) ---------- */

  :global(html.tv) aside.pill {
    align-self: center;
    width: 58px;
    max-height: calc(100% - 32px);
    margin: 0 0 0 10px;
    padding: 12px 0;
    border-radius: 29px;
  }

  .pill .brand,
  .pill .group,
  .pill .spacer {
    display: none;
  }

  /* Espacées pour que le cadre de sélection (contour décalé de 2 px) ne touche jamais le rond voisin. */
  .pill nav {
    align-items: center;
    gap: 12px;
  }

  .pill .item,
  .pill .profile,
  .pill .gear-btn {
    justify-content: center;
    width: 42px;
    min-height: 42px;
    height: 42px;
    margin: 0 auto;
    padding: 0;
    border-radius: 50%;
  }

  /* La rubrique en cours : un rond plein de la couleur du thème. */
  .pill .item.active {
    background: var(--accent);
    box-shadow: none;
    color: var(--accent-text);
  }

  /* Le rond suffit : pas de barre lumineuse sur le côté. */
  .pill .item.active::before {
    display: none;
  }

  .pill .item.active :global(svg) {
    color: var(--accent-text);
  }

  .pill .profile-card {
    gap: 12px;
    margin-top: 12px;
  }

  .pill > .profile,
  .pill > .item.settings {
    margin-top: 12px;
  }

  :global(html.tv) .pill :focus {
    outline-offset: 2px;
  }

</style>
