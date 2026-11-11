<script lang="ts" module>
  import { onBack } from "../lib/nav.svelte";

  /** Menu ouvert (un seul à la fois) : Retour revient d'abord à la liste des filtres, puis le referme. */
  let backOpen: (() => void) | null = null;
  onBack(() => {
    if (!backOpen) return false;
    backOpen();
    return true;
  });
</script>

<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, type FilterValue, type SectionFilters } from "../lib/api";
  import { device } from "../lib/device";
  import { reveal } from "../lib/spatial";
  import { find, t } from "../lib/i18n";
  import type { LibraryView } from "../lib/libraryView.svelte";
  import Icon from "./Icon.svelte";

  // Barre de l'onglet Bibliothèque (films et séries), comme dans Plex : « Tout » ou un filtre (non vus,
  // en cours, HDR… ou une valeur : genre, année, acteur…), puis l'ordre de tri et son sens.
  let {
    sectionKey,
    kind,
    view = $bindable(),
    onchange,
  }: { sectionKey: string; kind: string; view: LibraryView; onchange: () => void } = $props();

  let defs = $state<SectionFilters | null>(null);
  onMount(() => {
    api.sectionFilters(sectionKey, kind).then(
      (d) => (defs = d),
      () => (defs = { filters: [], sorts: [] }),
    );
  });

  const flags = $derived(defs?.filters.filter((f) => f.boolean) ?? []);
  const fields = $derived(defs?.filters.filter((f) => !f.boolean) ?? []);
  const sorts = $derived(defs?.sorts ?? []);

  /** Nom traduit d'un filtre ou d'un tri ; celui du serveur pour ce que Maku ne connaît pas. */
  const filterName = (id: string, fallback = id) => find(`library.filters.${id}`) ?? fallback;
  const sortName = (key: string, fallback = key) => find(`library.sorts.${key}`) ?? fallback;
  const fieldTitle = (id: string) => filterName(id, fields.find((f) => f.id === id)?.title ?? id);

  const filterLabel = $derived(
    view.flag ? filterName(view.flag, flags.find((f) => f.id === view.flag)?.title) : t("library.filters.all"),
  );
  const currentSort = $derived(view.sort ?? "titleSort");
  const sortLabel = $derived(sortName(currentSort, sorts.find((s) => s.key === currentSort)?.title));

  // --- Menus ---
  let open = $state<"filter" | "sort" | null>(null);
  /** Filtre dont on parcourt les valeurs (genre, année…), sinon la liste des filtres. */
  let field = $state<string | null>(null);
  let values = $state<FilterValue[] | null>(null);
  let query = $state("");
  let menu = $state<HTMLElement>();
  let filterBtn = $state<HTMLButtonElement>();
  let sortBtn = $state<HTMLButtonElement>();

  const shownValues = $derived.by(() => {
    const q = query.trim().toLocaleLowerCase();
    return q ? (values ?? []).filter((v) => v.title.toLocaleLowerCase().includes(q)) : (values ?? []);
  });

  async function focusMenu() {
    await tick();
    // Le menu reste dans l'écran (téléphone : le tri, à droite, dépasserait du bord).
    if (menu && !menu.style.left) {
      const over = menu.getBoundingClientRect().right - (window.innerWidth - 12);
      if (over > 0) menu.style.left = `${-over}px`;
    }
    const items = [...(menu?.querySelectorAll<HTMLElement>("[role=menuitemradio], [role=menuitem], input") ?? [])];
    const target = items.find((el) => el.getAttribute("aria-checked") === "true") ?? items[0];
    if (!target) return;
    target.focus({ preventScroll: true });
    reveal(target);
  }

  function show(which: "filter" | "sort") {
    if (open === which) return close();
    open = which;
    field = null;
    backOpen = back;
    focusMenu();
  }

  function close() {
    const from = open;
    open = null;
    field = null;
    if (backOpen === back) backOpen = null;
    (from === "sort" ? sortBtn : filterBtn)?.focus({ preventScroll: true });
  }

  /** Retour : de la liste des valeurs à celle des filtres, puis menu fermé. */
  function back() {
    if (field) {
      field = null;
      focusMenu();
    } else close();
  }

  function apply(next: Partial<LibraryView>) {
    view = { ...view, ...next };
    onchange();
  }

  function pickFlag(id: string | null) {
    apply({ flag: id, ...(id ? {} : { value: null }) });
    close();
  }

  async function openField(id: string) {
    field = id;
    values = null;
    query = "";
    focusMenu();
    try {
      const list = await api.filterValues(sectionKey, kind, id);
      if (field === id) values = list;
    } catch {
      if (field === id) values = [];
    }
    focusMenu();
  }

  function pickValue(v: FilterValue) {
    apply({ value: { field: field!, id: v.id, title: v.title } });
    close();
  }

  /** Un ordre de tri ; le même une seconde fois inverse son sens, comme dans Plex. */
  function pickSort(key: string, desc: boolean) {
    if (key === currentSort) apply({ sort: key, desc: !view.desc });
    else apply({ sort: key, desc });
    close();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }
</script>

<svelte:window onresize={() => open && close()} />

<div class="bar" class:tv={device.tv}>
  <div class="drop">
    <button bind:this={filterBtn} class="pick" class:on={!!view.flag} aria-haspopup="menu" aria-expanded={open === "filter"} onclick={() => show("filter")}>
      <Icon name="sliders" size={15} />
      <span>{filterLabel}</span>
      <Icon name="chevronDown" size={14} />
    </button>
    {#if open === "filter"}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="menu" role="menu" tabindex="-1" bind:this={menu} onkeydown={onKey}>
        {#if !field}
          <button role="menuitemradio" aria-checked={!view.flag && !view.value} onclick={() => pickFlag(null)}>
            <span class="text">{t("library.filters.all")}</span>
            {#if !view.flag && !view.value}<Icon name="check" size={15} />{/if}
          </button>
          {#each flags as f (f.id)}
            <button role="menuitemradio" aria-checked={view.flag === f.id} onclick={() => pickFlag(f.id)}>
              <span class="text">{filterName(f.id, f.title)}</span>
              {#if view.flag === f.id}<Icon name="check" size={15} />{/if}
            </button>
          {/each}
          {#if fields.length}<div class="sep"></div>{/if}
          {#each fields as f (f.id)}
            <button role="menuitem" class:on={view.value?.field === f.id} onclick={() => openField(f.id)}>
              <span class="text">{filterName(f.id, f.title)}</span>
              {#if view.value?.field === f.id}<span class="value">{view.value.title}</span>{/if}
              <Icon name="chevronRight" size={15} />
            </button>
          {/each}
          {#if !defs}<p class="muted note">{t("library.loading")}</p>{/if}
        {:else}
          <button role="menuitem" class="head" onclick={back}>
            <Icon name="chevronLeft" size={15} />
            <span class="text">{fieldTitle(field)}</span>
          </button>
          <!-- Une longue liste (acteurs, studios…) se filtre au clavier ; au téléviseur, on la parcourt. -->
          {#if !device.tv && (values?.length ?? 0) > 12}
            <input class="search" type="search" placeholder={t("library.filters.search")} bind:value={query} />
          {/if}
          <div class="values">
            {#if !values}
              <p class="muted note">{t("library.loading")}</p>
            {:else if !shownValues.length}
              <p class="muted note">{t("library.filters.none")}</p>
            {:else}
              {#each shownValues as v (v.id)}
                {@const on = view.value?.field === field && view.value.id === v.id}
                <button role="menuitemradio" aria-checked={on} onclick={() => pickValue(v)}>
                  <span class="text">{v.title}</span>
                  {#if on}<Icon name="check" size={15} />{/if}
                </button>
              {/each}
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>

  {#if sorts.length}
    <div class="drop">
      <button bind:this={sortBtn} class="pick" aria-haspopup="menu" aria-expanded={open === "sort"} onclick={() => show("sort")}>
        <span>{t("library.filters.sortBy", { field: sortLabel })}</span>
        <Icon name={view.desc ? "chevronDown" : "chevronUp"} size={14} />
      </button>
      {#if open === "sort"}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="menu" role="menu" tabindex="-1" bind:this={menu} onkeydown={onKey}>
          {#each sorts as s (s.key)}
            {@const on = s.key === currentSort}
            <button role="menuitemradio" aria-checked={on} onclick={() => pickSort(s.key, s.desc)}>
              <span class="text">{sortName(s.key, s.title)}</span>
              {#if on}
                <span class="value">{view.desc ? t("library.filters.descending") : t("library.filters.ascending")}</span>
                <Icon name={view.desc ? "chevronDown" : "chevronUp"} size={15} />
              {/if}
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  {#if view.value}
    <span class="chip">
      <span>{fieldTitle(view.value.field)} : <strong>{view.value.title}</strong></span>
      <button class="chip-x" onclick={() => apply({ value: null })} aria-label={t("library.removeFilter")}><Icon name="x" size={13} /></button>
    </span>
  {/if}
</div>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="backdrop" onclick={close}></div>
{/if}

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin-bottom: 20px;
  }

  .bar.tv {
    margin-bottom: 0;
  }

  .drop {
    position: relative;
  }

  .pick {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px;
    border: 1px solid var(--border);
    border-radius: 20px;
    background: var(--bg-elev);
    color: var(--text);
    font: inherit;
    font-weight: 600;
    white-space: nowrap;
  }

  .pick:hover,
  .pick[aria-expanded="true"] {
    border-color: var(--text-faint);
  }

  .pick.on {
    border-color: var(--accent);
    color: var(--accent);
  }

  .pick :global(svg) {
    color: var(--text-dim);
  }

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
  }

  .menu {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 61;
    display: flex;
    flex-direction: column;
    width: min(280px, calc(100vw - 24px));
    max-height: min(480px, 70vh);
    padding: 6px;
    border: 1px solid #ffffff1a;
    border-radius: 12px;
    background: #1c1e23f2;
    backdrop-filter: blur(18px);
    box-shadow: 0 18px 44px #000b;
    overflow-y: auto;
    animation: pop 0.12s var(--ease);
  }

  .values {
    display: flex;
    flex-direction: column;
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.96);
    }
  }

  .menu button {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border: none;
    border-radius: 8px;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 13px;
    text-align: left;
  }

  .menu button:hover {
    background: #ffffff14;
  }

  .menu button[aria-checked="true"],
  .menu button.on {
    color: var(--accent);
  }

  .menu button.head {
    color: var(--text-dim);
    font-weight: 600;
  }

  .text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .value {
    max-width: 45%;
    overflow: hidden;
    color: var(--text-faint);
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sep {
    flex-shrink: 0;
    height: 1px;
    margin: 4px 6px;
    background: var(--border);
  }

  .note {
    margin: 8px 10px;
    font-size: 13px;
  }

  .search {
    flex-shrink: 0;
    margin: 2px 4px 6px;
    padding: 7px 10px;
    font-size: 13px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 5px 6px 5px 14px;
    border: 1px solid var(--border);
    border-radius: 20px;
    background: var(--bg-elev);
  }

  .chip-x {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: #ffffff14;
    color: var(--text);
  }

  .chip-x:hover {
    background: var(--accent);
    color: var(--accent-text);
  }

  /* Téléviseur : la sélection se voit en couleur du thème, comme les autres listes. */
  .tv .menu {
    width: 340px;
  }

  .tv .menu button {
    font-size: 16px;
    padding: 10px 12px;
  }

  .tv .menu button:focus,
  .tv .pick:focus,
  .tv .chip-x:focus {
    outline: none;
    background: var(--accent);
    color: var(--accent-text);
  }

  .tv .menu button:focus :global(svg),
  .tv .pick:focus :global(svg),
  .tv .menu button:focus .value {
    color: var(--accent-text);
  }
</style>
