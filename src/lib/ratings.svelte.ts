// Notes des sites affichées sur les fiches des films et des séries (Réglages → Interface) : toutes par défaut,
// chacune désactivable. Allociné est cherché à part (src-tauri/src/allocine.rs) : le couper évite aussi ces
// appels à Wikidata et à Allociné.
import type { RatingSource } from "./api";

/** Sites réglables, dans l'ordre d'affichage. « allocine » couvre la note de la presse et celle des spectateurs. */
export const RATING_SITES = ["imdb", "rtCritic", "rtAudience", "tmdb", "community", "allocine"] as const;
export type RatingSite = (typeof RATING_SITES)[number];

const KEY = "maku.ratings";

function load(): Record<RatingSite, boolean> {
  const all = Object.fromEntries(RATING_SITES.map((s) => [s, true])) as Record<RatingSite, boolean>;
  try {
    const off = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    if (Array.isArray(off)) for (const s of off) if (s in all) all[s as RatingSite] = false;
  } catch {
    // Stockage indisponible ou illisible : tout reste affiché.
  }
  return all;
}

export const ratingPrefs = $state(load());

export function setRatingSite(site: RatingSite, on: boolean) {
  ratingPrefs[site] = on;
  try {
    localStorage.setItem(KEY, JSON.stringify(RATING_SITES.filter((s) => !ratingPrefs[s])));
  } catch {
    // Stockage indisponible : réglage gardé jusqu'à la fermeture.
  }
}

/** Le réglage qui commande une note. */
export const siteOf = (source: RatingSource): RatingSite =>
  source === "allocinePress" || source === "allocineUsers" ? "allocine" : source;
