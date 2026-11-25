<script lang="ts" module>
  /** « # » puis A à Z, comme dans Plex. */
  export const LETTERS = ["#", ..."ABCDEFGHIJKLMNOPQRSTUVWXYZ"];

  /** Lettre d'un titre dans le bandeau : accents retirés ; chiffres et signes sous « # ». */
  export function letterOf(title: string): string {
    const c = title.normalize("NFD").replace(/[̀-ͯ]/g, "").trim().charAt(0).toUpperCase();
    return c >= "A" && c <= "Z" ? c : "#";
  }
</script>

<script lang="ts">
  // Bandeau des lettres (ordinateur), collé au bord droit : une lettre sans titre est éteinte, les autres mènent au
  // premier titre qui commence par elle. Les bibliothèques du serveur multimédia et celles de Kavita le partagent.
  import { t } from "../lib/i18n";

  let { has, jump }: { has: (letter: string) => boolean; jump: (letter: string) => void } = $props();
</script>

<nav class="letters" aria-label={t("library.letters")}>
  {#each LETTERS as l (l)}
    <button disabled={!has(l)} onclick={() => jump(l)}>{l}</button>
  {/each}
</nav>

<style>
  /* Lettres collées au bord droit, sur toute la hauteur utile, comme dans Plex. */
  .letters {
    position: fixed;
    top: 50%;
    right: 10px;
    z-index: 5;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 6px 2px;
    border-radius: 12px;
    background: #0008;
    backdrop-filter: blur(10px);
    transform: translateY(-50%);
  }

  .letters button {
    width: 24px;
    height: min(22px, calc((100vh - 220px) / 27));
    padding: 0;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--text-dim);
    font-size: 11px;
    font-weight: 700;
    line-height: 1;
  }

  .letters button:hover:not(:disabled) {
    background: var(--accent);
    color: var(--accent-text);
  }

  .letters button:disabled {
    color: var(--text-faint);
    opacity: 0.4;
  }
</style>
