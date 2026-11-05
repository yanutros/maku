<script lang="ts">
  import { t } from "../lib/i18n";
  import Icon from "./Icon.svelte";

  /** Champ de mot de passe, avec un bouton pour l'afficher en clair (et le masquer à nouveau). */
  let { value = $bindable(""), autocomplete = "current-password" }: { value?: string; autocomplete?: AutoFill } = $props();
  let shown = $state(false);
</script>

<span class="password">
  <input type={shown ? "text" : "password"} bind:value {autocomplete} spellcheck="false" autocapitalize="off" />
  <button
    type="button"
    class="toggle"
    onclick={() => (shown = !shown)}
    aria-label={shown ? t("login.hidePassword") : t("login.showPassword")}
    aria-pressed={shown}
    title={shown ? t("login.hidePassword") : t("login.showPassword")}
  >
    <Icon name={shown ? "eyeOff" : "eye"} size={18} />
  </button>
</span>

<style>
  .password {
    position: relative;
    display: block;
  }

  .password input {
    width: 100%;
    padding-right: 48px;
  }

  .toggle {
    position: absolute;
    top: 50%;
    right: 6px;
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--text-dim);
    transform: translateY(-50%);
  }

  .toggle:hover,
  .toggle:focus-visible {
    background: #ffffff14;
    color: var(--text);
  }
</style>
