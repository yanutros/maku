// Pont entre la page et Android. Android montre l'objet du pont (`MakuBridge`) à toute page affichée dans le WebView,
// y compris la bande-annonce d'un autre site, dans son cadre. Chacune de ses méthodes demande donc une clé : la page
// de Maku la tire au hasard et la présente ici, en tout premier, avant qu'aucun cadre n'existe ; Android la retient
// et n'en acceptera pas d'autre (voir BridgeGate, PlaybackRules.kt).
//
// Le reste de l'interface continue d'appeler `window.PlexrDevice`, qui joint la clé à chaque appel. La clé ne quitte
// pas ce module, sauf pour le stockage de session, où la page la retrouve après un rechargement (Android, lui, l'a
// gardée) : un cadre d'une autre origine ne lit ni l'un ni l'autre.

type Bridge = Record<string, ((...args: unknown[]) => unknown) | undefined> & { claim?(key: string): boolean };

const STORE = "maku.bridgeKey";

function savedKey(): string | null {
  try {
    const key = sessionStorage.getItem(STORE);
    return key && /^[0-9a-f]{32}$/.test(key) ? key : null;
  } catch {
    return null;
  }
}

function newKey(): string {
  const key = Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join("");
  try {
    sessionStorage.setItem(STORE, key);
  } catch {
    // Stockage indisponible : la clé vaut jusqu'au prochain rechargement de la page.
  }
  return key;
}

const w = window as unknown as { MakuBridge?: Bridge; PlexrDevice?: unknown };
const raw = w.MakuBridge;
if (raw?.claim) {
  const key = savedKey() ?? newKey();
  if (!raw.claim(key)) console.warn("Maku : Android ne reconnaît pas la clé de la page ; ses fonctions ne répondront pas.");
  w.PlexrDevice = new Proxy(
    {},
    {
      // Seules les méthodes du pont, et pas `claim` : la clé se présente une fois, ici.
      get: (_, name) =>
        typeof name === "string" && name !== "claim" && typeof raw[name] === "function" ? (...args: unknown[]) => raw[name]!(key, ...args) : undefined,
    },
  );
}

export {};
