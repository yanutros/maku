// Préférences de l'interface (couleur, fond, notes, accueil, menu, lecteur…) : écrites comme avant dans le stockage du
// navigateur intégré (localStorage), et recopiées dans la configuration de Maku (config.json, `ui`). Au démarrage,
// celles de la configuration y sont remises : elles ne se perdent plus quand ce stockage est vidé, n'a pas été écrit
// sur le disque à temps, ou n'est pas celui de cette version de Maku (installée ou de développement).
import { invoke } from "@tauri-apps/api/core";

/** Préférences de Maku : celles-là seulement sont recopiées. */
export function isPref(key: string): boolean {
  return key.startsWith("plexr.") || key.startsWith("maku.");
}

/** Écritures en attente, par clé (null : retirée) : une rafale de changements part en un seul envoi par clé. */
const pending = new Map<string, string | null>();
let timer: ReturnType<typeof setTimeout> | null = null;

function flush() {
  timer = null;
  for (const [key, value] of pending) invoke("ui_pref_set", { key, value }).catch(() => {});
  pending.clear();
}

function queue(key: string, value: string | null) {
  pending.set(key, value);
  timer ??= setTimeout(flush, 400);
}

/**
 * Remet les préférences gardées par Maku dans le stockage du navigateur, puis recopie chaque changement. Sans
 * Maku derrière (démo, tests), rien ne change.
 */
export async function restorePrefs(): Promise<void> {
  let saved: Record<string, string> | null = null;
  try {
    saved = await invoke<Record<string, string> | null>("ui_prefs");
  } catch {
    return;
  }
  try {
    const store = window.localStorage;
    if (saved && typeof saved === "object") {
      for (const [key, value] of Object.entries(saved)) {
        if (isPref(key) && typeof value === "string" && store.getItem(key) !== value) store.setItem(key, value);
      }
      // Préférences présentes ici mais pas encore dans la configuration (versions d'avant) : recopiées.
      for (let i = 0; i < store.length; i++) {
        const key = store.key(i);
        if (key && isPref(key) && !(key in saved)) queue(key, store.getItem(key));
      }
    }
  } catch {
    // Stockage indisponible : les préférences ne valent que pour cette session.
  }
  mirror();
}

/** Recopie dans la configuration chaque écriture du stockage du navigateur (préférences de Maku seulement). */
function mirror() {
  const proto = Storage.prototype;
  const setItem = proto.setItem;
  const removeItem = proto.removeItem;
  proto.setItem = function (this: Storage, key: string, value: string) {
    setItem.call(this, key, value);
    if (this === window.localStorage && isPref(key)) queue(key, String(value));
  };
  proto.removeItem = function (this: Storage, key: string) {
    removeItem.call(this, key);
    if (this === window.localStorage && isPref(key)) queue(key, null);
  };
  // Fermeture juste après un changement : l'envoi part tout de suite.
  window.addEventListener("pagehide", () => timer && (clearTimeout(timer), flush()));
}
