<script lang="ts">
  import { api, errorText, type Pref } from "../lib/api";
  import { toast } from "../lib/nav.svelte";
  import { t } from "../lib/i18n";

  // Formulaire d'un groupe de réglages du serveur Plex (Général, Réseau, Transcodeur…).
  let { prefs, advanced, onSaved }: { prefs: Pref[]; advanced: boolean; onSaved: () => void } = $props();

  let edits = $state<Record<string, string>>({});
  let saving = $state(false);

  const visible = $derived(prefs.filter((p) => advanced || !p.advanced));
  const dirty = $derived(Object.keys(edits).filter((id) => edits[id] !== prefs.find((p) => p.id === id)?.value));

  function value(p: Pref) {
    return edits[p.id] ?? p.value;
  }

  function set(p: Pref, v: string) {
    edits[p.id] = v;
  }

  async function save() {
    saving = true;
    try {
      await api.setPrefs(Object.fromEntries(dirty.map((id) => [id, edits[id]])));
      toast(t("serverPrefs.saved"));
      edits = {};
      onSaved();
    } catch (e) {
      toast(errorText(e), true);
    } finally {
      saving = false;
    }
  }
</script>

<div class="prefs">
  {#each visible as p (p.id)}
    <div class="pref" class:changed={dirty.includes(p.id)}>
      {#if p.kind === "bool"}
        <label class="switch-row">
          <span class="text">
            <span class="label">{p.label}{#if p.advanced}<span class="adv">{t("serverPrefs.advanced")}</span>{/if}</span>
            {#if p.summary}<span class="summary">{p.summary}</span>{/if}
          </span>
          <input
            type="checkbox"
            class="switch"
            checked={value(p) === "true" || value(p) === "1"}
            onchange={(e) => set(p, e.currentTarget.checked ? "true" : "false")}
          />
        </label>
      {:else}
        <label class="text" for="pref-{p.id}">
          <span class="label">{p.label}{#if p.advanced}<span class="adv">{t("serverPrefs.advanced")}</span>{/if}</span>
          {#if p.summary}<span class="summary">{p.summary}</span>{/if}
        </label>
        {#if p.options.length}
          <select id="pref-{p.id}" value={value(p)} onchange={(e) => set(p, e.currentTarget.value)}>
            {#each p.options as [v, l] (v)}<option value={v}>{l}</option>{/each}
          </select>
        {:else}
          <input
            id="pref-{p.id}"
            type={p.kind === "int" || p.kind === "double" ? "number" : "text"}
            step={p.kind === "double" ? "any" : "1"}
            value={value(p)}
            placeholder={p.default}
            oninput={(e) => set(p, e.currentTarget.value)}
          />
        {/if}
      {/if}
    </div>
  {:else}
    <p class="muted">{advanced ? t("serverPrefs.empty") : t("serverPrefs.emptyTryAdvanced")}</p>
  {/each}
</div>

{#if dirty.length}
  <div class="savebar">
    <span>{t("serverPrefs.changed", { count: dirty.length })}</span>
    <button class="btn" onclick={() => (edits = {})}>{t("serverPrefs.cancel")}</button>
    <button class="btn primary" disabled={saving} onclick={save}>{saving ? t("serverPrefs.saving") : t("serverPrefs.save")}</button>
  </div>
{/if}

<style>
  .prefs {
    display: grid;
    gap: 2px;
  }

  .pref {
    display: grid;
    gap: 8px;
    padding: 14px 16px;
    border-radius: 12px;
    transition: background 0.15s;
  }

  .pref:hover {
    background: #ffffff08;
  }

  .pref.changed {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    box-shadow: inset 3px 0 0 var(--accent);
  }

  .switch-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    cursor: pointer;
  }

  .text {
    display: grid;
    gap: 3px;
  }

  .label {
    font-weight: 600;
  }

  .adv {
    margin-left: 8px;
    padding: 1px 6px;
    border-radius: 4px;
    background: #ffffff12;
    color: var(--text-faint);
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .summary {
    color: var(--text-dim);
    font-size: 13px;
    line-height: 1.5;
  }

  .pref select,
  .pref input:not(.switch) {
    max-width: 460px;
  }

  .savebar {
    position: sticky;
    bottom: 16px;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 16px;
    padding: 12px 16px;
    border: 1px solid #ffffff1a;
    border-radius: 14px;
    background: #1d1f23f2;
    backdrop-filter: blur(14px);
    box-shadow: 0 14px 34px #000a;
  }

  .savebar span {
    margin-right: auto;
    color: var(--text-dim);
  }
</style>
