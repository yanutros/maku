<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type EditInfo, type ImageChoice } from "../lib/api";
  import { genreLabel } from "../lib/genres";
  import { nav, reload, toast } from "../lib/nav.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n";
  import { device } from "../lib/device";

  // Fenêtre « Modifier » d'un film, d'une série, d'un épisode ou d'un élément musical.
  const item = nav.editing!;

  type Tab = "general" | "genres" | "posters" | "arts";
  let tab = $state<Tab>("general");
  let info = $state<EditInfo | null>(null);
  let values = $state<Record<string, string>>({});
  let locks = $state<Record<string, boolean>>({});
  let genres = $state<string[]>([]);
  let newGenre = $state("");
  let images = $state<Record<string, ImageChoice[] | null>>({ posters: null, arts: null });
  let imageUrl = $state("");
  let saving = $state(false);
  let error = $state<string | null>(null);

  const isMusicItem = $derived(["artist", "album", "track"].includes(item.kind));
  const tabs = $derived(
    [
      { id: "general" as Tab, label: t("editDialog.tabs.general"), icon: "edit" as const },
      ...(info?.genres ? [{ id: "genres" as Tab, label: t("editDialog.tabs.genres"), icon: "tag" as const }] : []),
      { id: "posters" as Tab, label: isMusicItem ? t("editDialog.tabs.cover") : t("editDialog.tabs.poster"), icon: "image" as const },
      ...(item.kind !== "track" ? [{ id: "arts" as Tab, label: t("editDialog.tabs.background"), icon: "image" as const }] : []),
    ],
  );

  // Une image changée (choisie, ajoutée par adresse ou envoyée) : la page se met à jour à la fermeture, même
  // sans « Enregistrer », pour montrer la nouvelle affiche.
  let imageChanged = false;

  function close() {
    nav.editing = null;
    if (imageChanged) reload();
  }

  function onInput(name: string, value: string) {
    values[name] = value;
    // Comme dans Plex, un champ modifié est verrouillé pour ne plus être écrasé par l'agent.
    locks[name] = true;
  }

  function addGenre() {
    const g = newGenre.trim();
    if (g && !genres.includes(g)) genres = [...genres, g];
    newGenre = "";
  }

  async function loadImages(kind: "posters" | "arts") {
    if (images[kind]) return;
    try {
      images[kind] = await api.images(item.ratingKey, kind);
    } catch (e) {
      error = errorText(e);
      images[kind] = [];
    }
  }

  async function chooseImage(kind: "posters" | "arts", key: string) {
    try {
      await api.setImage(item.ratingKey, kind, key);
      images[kind] = (images[kind] ?? []).map((i) => ({ ...i, selected: i.key === key }));
      imageChanged = true;
      toast(kind === "posters" ? t("editDialog.posterUpdated") : t("editDialog.backgroundUpdated"));
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  // --- Image de l'ordinateur : bouton « Choisir une image » ou fichier glissé sur la fenêtre, comme dans Plex ---
  let uploading = $state(false);
  let dragOver = $state(false);
  let picker = $state<HTMLInputElement>();

  async function upload(file: { data?: string; path?: string }) {
    if (tab !== "posters" && tab !== "arts") return;
    const kind = tab;
    uploading = true;
    try {
      await api.uploadImage(item.ratingKey, kind, file);
      imageChanged = true;
      toast(kind === "posters" ? t("editDialog.posterUpdated") : t("editDialog.backgroundUpdated"));
      images[kind] = null;
      await loadImages(kind);
    } catch (e) {
      toast(errorText(e), true);
    } finally {
      uploading = false;
    }
  }

  /** Fichier choisi (ou déposé dans un navigateur) : envoyé en base64. */
  function uploadFile(file: File | undefined) {
    if (!file) return;
    if (!/^image\/(jpeg|png|webp)$/.test(file.type)) return toast(t("editDialog.imageType"), true);
    const reader = new FileReader();
    reader.onload = () => upload({ data: String(reader.result).split(",")[1] ?? "" });
    reader.readAsDataURL(file);
  }

  // Application : un fichier glissé sur la fenêtre arrive par Tauri (la page n'en reçoit pas le contenu).
  onMount(() => {
    let stop: (() => void) | undefined;
    import("@tauri-apps/api/webview")
      .then(({ getCurrentWebview }) =>
        getCurrentWebview().onDragDropEvent((e) => {
          const images = tab === "posters" || tab === "arts";
          if (e.payload.type === "over" || e.payload.type === "enter") dragOver = images;
          else if (e.payload.type === "leave") dragOver = false;
          else if (e.payload.type === "drop") {
            dragOver = false;
            const path = e.payload.paths.find((p) => /\.(jpe?g|png|webp)$/i.test(p));
            if (!images) return;
            if (path) upload({ path });
            else toast(t("editDialog.imageType"), true);
          }
        }),
      )
      .then((unlisten) => (stop = unlisten), () => {});
    return () => stop?.();
  });

  async function addImageFromUrl(kind: "posters" | "arts") {
    const url = imageUrl.trim();
    if (!/^https?:\/\//.test(url)) {
      toast(t("editDialog.invalidImageUrl"), true);
      return;
    }
    await chooseImage(kind, url);
    imageUrl = "";
    images[kind] = null;
    loadImages(kind);
  }

  async function save() {
    if (!info) return;
    saving = true;
    error = null;
    try {
      const changes = info.fields
        .filter((f) => values[f.name] !== f.value || locks[f.name] !== f.locked)
        .map((f) => ({ name: f.name, value: values[f.name], locked: locks[f.name] }));
      const genresChanged = info.genres !== null && genres.join("\n") !== info.genres.join("\n");
      await api.edit(item.ratingKey, changes, genresChanged ? genres : null);
      toast(t("editDialog.saved"));
      imageChanged = true;
      close();
    } catch (e) {
      error = errorText(e);
    } finally {
      saving = false;
    }
  }

  $effect(() => {
    if (tab === "posters" || tab === "arts") loadImages(tab);
  });

  onMount(async () => {
    try {
      const i = await api.editInfo(item.ratingKey);
      info = i;
      values = Object.fromEntries(i.fields.map((f) => [f.name, f.value]));
      locks = Object.fromEntries(i.fields.map((f) => [f.name, f.locked]));
      genres = [...(i.genres ?? [])];
    } catch (e) {
      error = errorText(e);
    }
  });
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="overlay" onclick={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("editDialog.title", { title: item.title })}>
    <header>
      <Icon name="edit" size={18} />
      <h2>{t("editDialog.title", { title: info?.title ?? item.title })}</h2>
      <button class="x" onclick={close} aria-label={t("common.c1.close")}><Icon name="x" size={18} /></button>
    </header>

    <div class="body">
      <nav class="tabs">
        {#each tabs as tb (tb.id)}
          <button class:active={tab === tb.id} onclick={() => (tab = tb.id)}><Icon name={tb.icon} size={16} /> {tb.label}</button>
        {/each}
      </nav>

      <div class="panel">
        {#if error}<p class="error">{error}</p>{/if}
        {#if !info}
          <div class="skeleton" style:height="220px"></div>
        {:else if tab === "general"}
          {#each info.fields as f (f.name)}
            <div class="field">
              <label for="f-{f.name}">{f.label}</label>
              <div class="input-row" class:long={f.input === "long"}>
                <button
                  class="lock"
                  class:on={locks[f.name]}
                  onclick={() => (locks[f.name] = !locks[f.name])}
                  title={locks[f.name] ? t("editDialog.lockedHint") : t("editDialog.unlockedHint")}
                  aria-label={locks[f.name] ? t("editDialog.unlock") : t("editDialog.lock")}
                >
                  <Icon name={locks[f.name] ? "lock" : "unlock"} size={15} />
                </button>
                {#if f.input === "long"}
                  <textarea id="f-{f.name}" rows="6" value={values[f.name]} oninput={(e) => onInput(f.name, e.currentTarget.value)}></textarea>
                {:else}
                  <input
                    id="f-{f.name}"
                    type={f.input === "date" ? "date" : "text"}
                    value={values[f.name]}
                    oninput={(e) => onInput(f.name, e.currentTarget.value)}
                  />
                {/if}
              </div>
            </div>
          {/each}
        {:else if tab === "genres"}
          <p class="hint">{t("editDialog.genresHint")}</p>
          <div class="chips">
            {#each genres as g (g)}
              <span class="chip">
                {genreLabel(g)}
                <button onclick={() => (genres = genres.filter((x) => x !== g))} aria-label={t("editDialog.removeGenre", { genre: g })}><Icon name="x" size={12} /></button>
              </span>
            {:else}
              <span class="hint">{t("editDialog.noGenres")}</span>
            {/each}
          </div>
          <form class="add" onsubmit={(e) => (e.preventDefault(), addGenre())}>
            <input bind:value={newGenre} placeholder={t("editDialog.addGenrePlaceholder")} />
            <button class="btn" type="submit"><Icon name="plus" size={16} /> {t("editDialog.add")}</button>
          </form>
        {:else}
          {@const kind = tab}
          <!-- Comme Plex : « Choisir une image » sur l'ordinateur, ou la glisser ici. -->
          {#if !device.tv}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="drop"
              class:over={dragOver}
              ondragover={(e) => (e.preventDefault(), (dragOver = true))}
              ondragleave={() => (dragOver = false)}
              ondrop={(e) => (e.preventDefault(), (dragOver = false), uploadFile(e.dataTransfer?.files[0]))}
            >
              <Icon name="image" size={18} />
              {#if uploading}
                <span>{t("editDialog.uploading")}</span>
              {:else}
                <button class="link" onclick={() => picker?.click()}>{t("editDialog.chooseImage")}</button>
                <span>{t("editDialog.orDrop")}</span>
              {/if}
              <input bind:this={picker} type="file" accept="image/jpeg,image/png,image/webp" hidden onchange={(e) => (uploadFile(e.currentTarget.files?.[0]), (e.currentTarget.value = ""))} />
            </div>
          {/if}
          <div class="images" class:wide={kind === "arts"}>
            {#if !images[kind]}
              {#each Array(8) as _, i (i)}<div class="skeleton img-skeleton"></div>{/each}
            {:else}
              {#each images[kind]! as img (img.key)}
                <button class="choice" class:selected={img.selected} onclick={() => chooseImage(kind, img.key)} title={img.provider ?? ""}>
                  <img src={img.preview} alt="" loading="lazy" />
                  {#if img.selected}<span class="tick"><Icon name="check" size={14} /></span>{/if}
                  {#if img.provider}<span class="provider">{img.provider}</span>{/if}
                </button>
              {:else}
                <p class="hint">{t("editDialog.noImages")}</p>
              {/each}
            {/if}
          </div>
          <form class="add" onsubmit={(e) => (e.preventDefault(), addImageFromUrl(kind))}>
            <input bind:value={imageUrl} placeholder={t("editDialog.imageUrlPlaceholder")} />
            <button class="btn" type="submit"><Icon name="plus" size={16} /> {t("editDialog.use")}</button>
          </form>
        {/if}
      </div>
    </div>

    <footer>
      {#if tab === "posters" || tab === "arts"}
        <span class="hint">{t("editDialog.clickToApply")}</span>
      {/if}
      <button class="btn" onclick={close}>{t("common.c1.cancel")}</button>
      <button class="btn primary" disabled={saving || !info} onclick={save}>{saving ? t("common.c1.saving") : t("common.c1.save")}</button>
    </footer>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 70;
    display: grid;
    place-items: center;
    background: #000a;
    backdrop-filter: blur(4px);
    animation: fade 0.15s;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  .dialog {
    display: flex;
    flex-direction: column;
    width: min(920px, 94vw);
    height: min(680px, 88vh);
    overflow: hidden;
    border: 1px solid #ffffff1a;
    border-radius: 18px;
    background: var(--bg-elev);
    box-shadow: 0 30px 80px #000c;
  }

  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
    color: var(--accent);
  }

  h2 {
    flex: 1;
    margin: 0;
    overflow: hidden;
    color: var(--text);
    font-size: 18px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .x {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--text-dim);
  }

  .x:hover {
    background: #ffffff14;
  }

  .body {
    display: grid;
    grid-template-columns: 190px minmax(0, 1fr);
    flex: 1;
    min-height: 0;
  }

  .tabs {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px;
    border-right: 1px solid var(--border);
    background: #00000026;
  }

  .tabs button {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border: none;
    border-radius: 8px;
    background: none;
    color: var(--text-dim);
    font-weight: 500;
    text-align: left;
  }

  .tabs button:hover {
    color: var(--text);
  }

  .tabs button.active {
    background: #ffffff12;
    color: var(--accent);
  }

  .panel {
    padding: 18px 22px;
    overflow-y: auto;
  }

  .field {
    margin-bottom: 14px;
  }

  label {
    display: block;
    margin-bottom: 5px;
    color: var(--text-dim);
    font-size: 13px;
    font-weight: 600;
  }

  .input-row {
    display: flex;
    align-items: stretch;
  }

  .input-row input,
  .input-row textarea {
    flex: 1;
    min-width: 0;
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
  }

  textarea {
    resize: vertical;
    font: inherit;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 9px 12px;
    outline: none;
    line-height: 1.5;
  }

  textarea:focus {
    border-color: var(--accent);
  }

  .lock {
    display: grid;
    place-items: center;
    width: 38px;
    border: 1px solid var(--border);
    border-right: none;
    border-radius: var(--radius) 0 0 var(--radius);
    background: #ffffff08;
    color: var(--text-faint);
  }

  .lock.on {
    color: var(--accent);
  }

  .hint {
    color: var(--text-faint);
    font-size: 13px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 12px 0 18px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 6px 5px 12px;
    border-radius: 16px;
    background: #ffffff14;
    font-size: 13px;
  }

  .chip button {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: #ffffff1a;
  }

  .chip button:hover {
    background: var(--danger);
  }

  /* Zone « Choisir une image · ou la glisser ici » : en pointillés, dorée quand un fichier la survole. */
  .drop {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin-bottom: 14px;
    padding: 14px;
    border: 1.5px dashed var(--border);
    border-radius: 12px;
    color: var(--text-dim);
    font-size: 13px;
    transition: border-color 0.15s, background 0.15s;
  }

  .drop.over {
    border-color: var(--accent);
    background: #ffffff0a;
    color: var(--text);
  }

  .link {
    padding: 0;
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }

  .link:hover {
    text-decoration: underline;
  }

  .add {
    display: flex;
    gap: 8px;
    margin-top: 14px;
  }

  .add input {
    flex: 1;
  }

  .images {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(118px, 1fr));
    gap: 12px;
  }

  .images.wide {
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  }

  .choice,
  .img-skeleton {
    position: relative;
    aspect-ratio: 2 / 3;
    padding: 0;
    overflow: hidden;
    border: 2px solid transparent;
    border-radius: 10px;
    background: var(--bg);
  }

  .wide .choice,
  .wide .img-skeleton {
    aspect-ratio: 16 / 9;
  }

  .choice:hover {
    border-color: #ffffff55;
  }

  .choice.selected {
    border-color: var(--accent);
  }

  .choice img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .tick {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-text);
  }

  .provider {
    position: absolute;
    left: 6px;
    bottom: 6px;
    padding: 1px 6px;
    border-radius: 4px;
    background: #000b;
    font-size: 10px;
    text-transform: uppercase;
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
    padding: 14px 20px;
    border-top: 1px solid var(--border);
  }

  footer .hint {
    margin-right: auto;
  }
</style>
