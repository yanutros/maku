// Gestion des demandes Seerr par son administrateur (Admin › Demandes) : qui y a droit, et combien de demandes
// attendent une validation (la pastille du menu).
import { api, type SeerrRequest } from "./api";
import { nav } from "./nav.svelte";
import { device } from "./device";
import { t } from "./i18n";

/** Vérifie les droits du compte Seerr connecté (`hasSeerr` : une adresse de Seerr est enregistrée). */
export async function checkSeerrManager(hasSeerr: boolean) {
  // Téléviseur : pas de gestion des demandes, inutile de demander à Seerr.
  nav.seerrManager = hasSeerr && !device.tv ? await api.seerrCanManage().catch(() => false) : false;
  await refreshPending();
}

/** Recompte les demandes en attente (après une validation, un refus, une suppression…). */
export async function refreshPending() {
  if (!nav.seerrManager) {
    nav.pendingRequests = 0;
    return;
  }
  const counts = await api.seerrRequestCounts().catch(() => null);
  if (counts) nav.pendingRequests = counts.pending;
}

export type RequestState = { label: string; tone: "pending" | "processing" | "available" | "partial" | "declined" | "failed" };

/** État affiché d'une demande : celui de la demande, puis celui du titre une fois validée. */
export function requestState(r: SeerrRequest): RequestState {
  if (r.status === 1) return { label: t("requests.state.pending"), tone: "pending" };
  if (r.status === 3) return { label: t("requests.state.declined"), tone: "declined" };
  if (r.status === 4) return { label: t("requests.state.failed"), tone: "failed" };
  if (r.media.status === 5 || r.status === 5) return { label: t("requests.state.available"), tone: "available" };
  if (r.media.status === 4) return { label: t("requests.state.partial"), tone: "partial" };
  return { label: t("requests.state.processing"), tone: "processing" };
}

/** « il y a 14 heures », « avant-hier »… dans la langue de l'interface. */
export function ago(iso: string, locale: string): string {
  const then = Date.parse(iso);
  if (Number.isNaN(then)) return "";
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  const s = Math.round((then - Date.now()) / 1000);
  const steps: [number, Intl.RelativeTimeFormatUnit][] = [
    [60, "second"],
    [60, "minute"],
    [24, "hour"],
    [30, "day"],
    [12, "month"],
  ];
  let value = s;
  for (const [size, unit] of steps) {
    if (Math.abs(value) < size) return rtf.format(value, unit);
    value = Math.round(value / size);
  }
  return rtf.format(value, "year");
}

/** Espace libre d'un dossier : « 22,2 To » (unités traduites par le navigateur). */
export function bytes(n: number, locale: string): string {
  const units = ["byte", "kilobyte", "megabyte", "gigabyte", "terabyte", "petabyte"];
  let i = 0;
  let v = n;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  return new Intl.NumberFormat(locale, { style: "unit", unit: units[i], unitDisplay: "short", maximumFractionDigits: v < 10 ? 1 : 0 }).format(v);
}
