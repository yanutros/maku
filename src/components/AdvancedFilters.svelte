<script lang="ts">
  import { api, errorText, type FilterValue, type SectionFilters } from "../lib/api";
  import { go, toast } from "../lib/nav.svelte";
  import { find, t } from "../lib/i18n";
  import { completeRules, smartPairs, type Rule, type SmartFilter } from "../lib/libraryView.svelte";
  import Icon from "./Icon.svelte";
  import TrackSelect from "./TrackSelect.svelte";

  // Nouvelle collection intelligente (administrateur, sur ordinateur), comme dans Plex : des règles « champ,
  // opérateur, valeur » qui doivent toutes (ou une seule) être vraies, et un nombre de titres au plus. Plex la
  // tient ensuite à jour à chaque ajout dans la bibliothèque. « Aperçu » montre dans la grille les titres
  // qu'elle contiendra. Depuis la fiche d'une collection (`editing`), on en change les règles.
  let {
    sectionKey,
    kind,
    defs,
    filter,
    sort,
    editing = null,
    onapply,
    onclose,
  }: {
    sectionKey: string;
    kind: string;
    defs: SectionFilters;
    filter: SmartFilter | null;
    sort: string | null;
    editing?: { ratingKey: string; title: string } | null;
    onapply: (filter: SmartFilter | null) => void;
    onclose: () => void;
  } = $props();

  const fieldOf = (key: string) => defs.fields.find((f) => f.key === key);
  const opsOf = (key: string) => defs.operators.find((o) => o.kind === fieldOf(key)?.kind)?.ops ?? [];

  /** Une règle vide : le premier champ (souvent le titre) et son premier opérateur. */
  function blank(): Rule {
    const field = defs.fields[0]?.key ?? "title";
    return { field, op: opsOf(field)[0]?.key ?? "=", value: "" };
  }

  // svelte-ignore state_referenced_locally
  let any = $state(filter?.any ?? false);
  // svelte-ignore state_referenced_locally
  let rules = $state<Rule[]>(filter?.rules.length ? filter.rules.map((r) => ({ ...r })) : [blank()]);
  // svelte-ignore state_referenced_locally
  let limit = $state<string>(filter?.limit ? String(filter.limit) : "");
  let name = $state("");
  let busy = $state(false);

  const current = $derived<SmartFilter>({ any, rules, limit: Number(limit) > 0 ? Math.floor(Number(limit)) : null });
  const ready = $derived(completeRules(current).length > 0);

  // --- Valeurs des champs « étiquette » (genre, acteur…), chargées une fois par champ ---
  let values = $state<Record<string, FilterValue[] | undefined>>({});
  function loadValues(field: string) {
    if (fieldOf(field)?.kind !== "tag" || values[field]) return;
    values[field] = [];
    api.filterValues(sectionKey, kind, field).then(
      (list) => (values[field] = list),
      () => {},
    );
  }
  $effect(() => {
    for (const r of rules) loadValues(r.field);
  });

  /** Nom traduit d'un champ ; celui du serveur pour ce que Maku ne connaît pas. */
  const fieldName = (key: string) =>
    find(`library.fields.${key}`) ?? find(`library.filters.${key}`) ?? find(`library.sorts.${key}`) ?? fieldOf(key)?.title ?? key;

  const OPS: Record<string, string> = { "=": "is", "!=": "isNot", "==": "is", "!==": "isNot", "<=": "beginsWith", ">=": "endsWith", ">>=": "greater", "<<=": "less" };
  function opName(field: string, op: { key: string; title: string }) {
    const type = fieldOf(field)?.kind;
    let id = OPS[op.key];
    if (type === "string" && op.key === "=") id = "contains";
    else if (type === "string" && op.key === "!=") id = "notContains";
    else if (type === "date" && op.key === ">>=") id = "after";
    else if (type === "date" && op.key === "<<=") id = "before";
    return (id && find(`library.ops.${id}`)) ?? op.title;
  }

  function setField(r: Rule, field: string) {
    r.field = field;
    if (!opsOf(field).some((o) => o.key === r.op)) r.op = opsOf(field)[0]?.key ?? "=";
    r.value = fieldOf(field)?.kind === "boolean" ? "1" : "";
  }

  function apply() {
    onapply(ready ? $state.snapshot(current) : null);
  }

  async function create() {
    const title = name.trim();
    if (!title || !ready) return;
    busy = true;
    try {
      const key = await api.createSmartCollection(sectionKey, kind, title, smartPairs(current), sort);
      toast(t("library.advanced.created", { title }));
      name = "";
      onapply(null);
      if (key) go({ name: "item", ratingKey: key });
    } catch (e) {
      toast(errorText(e), true);
    } finally {
      busy = false;
    }
  }

  async function update() {
    if (!editing || !ready) return;
    busy = true;
    try {
      await api.updateSmartCollection(editing.ratingKey, sectionKey, kind, smartPairs(current), sort);
      toast(t("library.advanced.updated", { title: editing.title }));
      go({ name: "item", ratingKey: editing.ratingKey });
    } catch (e) {
      toast(errorText(e), true);
    } finally {
      busy = false;
    }
  }

  const onEnter = (e: KeyboardEvent) => e.key === "Enter" && apply();
</script>

<div class="adv">
  <div class="top">
    <h3>{editing ? t("library.advanced.editTitle", { title: editing.title }) : t("library.advanced.title")}</h3>
    <button class="btn round" onclick={onclose} title={t("collection.cancel")} aria-label={t("collection.cancel")}><Icon name="x" size={16} /></button>
  </div>

  <div class="line">
    {#if !editing}
      <input class="name" bind:value={name} placeholder={t("library.advanced.name")} aria-label={t("library.advanced.name")} onkeydown={(e) => e.key === "Enter" && create()} />
    {/if}
    <div class="match">
      <TrackSelect
        value={any ? "any" : "all"}
        label={t("library.advanced.match")}
        options={[
          { value: "all", label: t("library.advanced.all") },
          { value: "any", label: t("library.advanced.any") },
        ]}
        onchange={(v) => (any = v === "any")}
      />
    </div>
  </div>

  {#each rules as r, i (i)}
    {@const type = fieldOf(r.field)?.kind}
    <div class="rule">
      <div class="cell field">
        <TrackSelect value={r.field} label={t("library.advanced.field")} options={defs.fields.map((f) => ({ value: f.key, label: fieldName(f.key) }))} onchange={(v) => setField(r, String(v))} />
      </div>
      <div class="cell op">
        <TrackSelect bind:value={r.op} label={t("library.advanced.operator")} options={opsOf(r.field).map((o) => ({ value: o.key, label: opName(r.field, o) }))} />
      </div>
      <div class="cell value">
        {#if type === "boolean"}
          <TrackSelect
            bind:value={r.value}
            label={t("library.advanced.value")}
            options={[
              { value: "1", label: t("library.advanced.yes") },
              { value: "0", label: t("library.advanced.no") },
            ]}
          />
        {:else if type === "tag"}
          <TrackSelect
            bind:value={r.value}
            label={t("library.advanced.value")}
            options={[{ value: "", label: values[r.field]?.length ? t("library.advanced.choose") : t("library.loading") }, ...(values[r.field] ?? []).map((v) => ({ value: v.id, label: v.title }))]}
          />
        {:else}
          <input
            type={type === "integer" ? "number" : "text"}
            bind:value={r.value}
            placeholder={type === "date" ? t("library.advanced.datePlaceholder") : t("library.advanced.value")}
            aria-label={t("library.advanced.value")}
            onkeydown={onEnter}
          />
        {/if}
      </div>
      <button
        class="btn round"
        onclick={() => (rules = rules.length > 1 ? rules.filter((_, j) => j !== i) : [blank()])}
        title={t("library.advanced.remove")}
        aria-label={t("library.advanced.remove")}
      >
        <Icon name="x" size={14} />
      </button>
    </div>
  {/each}

  <div class="bottom">
    <button class="btn" onclick={() => rules.push(blank())}><Icon name="plus" size={15} /> {t("library.advanced.add")}</button>
    <label class="limit">
      <span>{t("library.advanced.limit")}</span>
      <input type="number" min="1" bind:value={limit} placeholder="—" onkeydown={onEnter} />
    </label>
    <span class="grow"></span>
    <button class="btn" disabled={!ready} onclick={apply}><Icon name="eye" size={15} /> {t("library.advanced.apply")}</button>
    {#if editing}
      <button class="btn primary" disabled={!ready || busy} onclick={update}>{t("library.advanced.update")}</button>
    {:else}
      <button class="btn primary" disabled={!ready || !name.trim() || busy} onclick={create}>
        <Icon name="library" size={15} /> {t("library.advanced.create")}
      </button>
    {/if}
  </div>
</div>

<style>
  .adv {
    display: grid;
    gap: 10px;
    margin: 0 0 20px;
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--bg-elev);
  }

  .top,
  .line,
  .rule,
  .bottom {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }

  h3 {
    margin: 0 8px 0 0;
    font-size: 17px;
  }

  .match {
    display: flex;
    width: 340px;
  }

  .top .btn.round {
    margin-left: auto;
  }

  .name {
    flex: 1;
    max-width: 420px;
  }

  .cell {
    display: flex;
    min-width: 0;
  }

  .field {
    width: 200px;
  }

  .op {
    width: 190px;
  }

  .value {
    flex: 1;
    min-width: 200px;
  }

  .value input {
    flex: 1;
  }

  input {
    min-width: 0;
    padding: 8px 10px;
  }

  .limit {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--text-dim);
    font-size: 13px;
    font-weight: 600;
    text-transform: uppercase;
  }

  .limit input {
    width: 90px;
  }

  .grow {
    flex: 1;
  }

  /* Téléphone : une règle par bloc, chaque liste sur toute la largeur. */
  @media (max-width: 640px) {
    .field,
    .op,
    .value,
    .match {
      width: 100%;
      min-width: 0;
    }
  }
</style>
