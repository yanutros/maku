// Traductions de l'interface. Format i18next (comme Tracearr) : un fichier JSON par langue dans
// src/locales, clés imbriquées, variables {{nom}}, pluriels en `_one` / `_other` (avec `count`).
// L'anglais sert de langue de repli, puis le français (langue d'origine de Maku).
import { invoke } from "@tauri-apps/api/core";
import fr from "../locales/fr.json";
import en from "../locales/en.json";
import es from "../locales/es.json";
import de from "../locales/de.json";
import nl from "../locales/nl.json";

type Dict = { [key: string]: string | Dict };

export const LANGUAGES = [
  { id: "fr", label: "Français", locale: "fr-FR" },
  { id: "en", label: "English", locale: "en-GB" },
  { id: "es", label: "Español", locale: "es-ES" },
  { id: "de", label: "Deutsch", locale: "de-DE" },
  { id: "nl", label: "Nederlands", locale: "nl-NL" },
] as const;
export type Language = (typeof LANGUAGES)[number]["id"];

const DICTS: Record<Language, Dict> = { fr, en, es, de, nl };
const KEY = "plexr.language";

/** Choix enregistré : une langue, ou « auto » (celle du système). */
export function savedLanguage(): Language | "auto" {
  try {
    const v = localStorage.getItem(KEY);
    return v && v in DICTS ? (v as Language) : "auto";
  } catch {
    return "auto";
  }
}

/** Langue du système si Maku la connaît, sinon l'anglais. */
function systemLanguage(): Language {
  for (const tag of navigator.languages ?? [navigator.language]) {
    const id = tag.slice(0, 2).toLowerCase();
    if (id in DICTS) return id as Language;
  }
  return "en";
}

const choice = savedLanguage();
/** Langue de l'interface, fixée au lancement (en changer recharge l'interface). */
export const language: Language = choice === "auto" ? systemLanguage() : choice;
/** Code régional pour les dates et les nombres (Intl, toLocaleString…). */
export const locale: string = LANGUAGES.find((l) => l.id === language)!.locale;

const plurals = new Intl.PluralRules(locale);

function lookup(dict: Dict, key: string): string | undefined {
  let node: string | Dict | undefined = dict;
  for (const part of key.split(".")) {
    if (typeof node !== "object") return undefined;
    node = node[part];
  }
  return typeof node === "string" ? node : undefined;
}

export function find(key: string): string | undefined {
  return lookup(DICTS[language], key) ?? lookup(DICTS.en, key) ?? lookup(DICTS.fr, key);
}

/** Texte traduit. `params.count` choisit la forme du pluriel (`clé_one`, `clé_other`). */
export function t(key: string, params?: Record<string, string | number>): string {
  let text: string | undefined;
  if (params && typeof params.count === "number") {
    text = find(`${key}_${plurals.select(params.count)}`) ?? find(`${key}_other`);
  }
  text ??= find(key);
  if (text === undefined) {
    if (import.meta.env.DEV) console.warn(`[i18n] clé manquante : ${key}`);
    return key;
  }
  return params ? text.replace(/\{\{\s*(\w+)\s*\}\}/g, (m, name) => (name in params ? String(params[name]) : m)) : text;
}

/** Communique la langue au backend (messages d'erreur, page du téléphone). */
export function initLanguage() {
  document.documentElement.lang = language;
  invoke("set_language", { language }).catch(() => {});
}

/** Change de langue (« auto » : celle du système), puis recharge l'interface dans cette langue. */
export function setLanguage(choice: Language | "auto") {
  try {
    if (choice === "auto") localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, choice);
  } catch {
    // Stockage indisponible : la langue du système reste utilisée.
  }
  location.reload();
}
