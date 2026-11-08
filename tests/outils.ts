// Outils communs aux tests : ouvrir Maku en démo sur le bon appareil, piloter la télécommande, et les règles
// d'affichage tirées des bugs déjà corrigés.
import { expect, type Page, type TestInfo } from "@playwright/test";

export type Appareil = "pc" | "tv" | "telephone" | "safari";

export function appareil(info: TestInfo): Appareil {
  return info.project.name as Appareil;
}

/**
 * Ouvre Maku en démo sur une vue (`accueil`, `fiche`, `series`, `stats`, `lecteur`…). Le téléviseur et le
 * téléphone sont imposés par `?device=` ; le reste vient de la taille de la fenêtre.
 */
export async function ouvrir(page: Page, info: TestInfo, vue = "accueil", options: { simple?: boolean } = {}) {
  const device = { tv: "tv", telephone: "phone" }[appareil(info) as string];
  await page.addInitScript(
    ({ simple }) => {
      // Préférences remises à zéro à chaque test (thème jaune, barre de navigation choisie).
      localStorage.clear();
      if (simple) localStorage.setItem("plexr.sidebar", JSON.stringify({ collapsed: true, simple: true, order: [], hidden: [] }));
    },
    { simple: options.simple ?? false },
  );
  // Heure figée : « Se termine à … » et les dates restent les mêmes d'un passage à l'autre (les minuteries, elles,
  // tournent normalement).
  await page.clock.setFixedTime(new Date("2026-11-08T20:00:00+01:00"));
  await page.goto(`/?demo=${vue}${device ? `&device=${device}` : ""}`);
  // Page montée (pas forcément visible : pendant une vidéo, elle est masquée derrière le lecteur).
  await page.locator("main").waitFor({ state: "attached" });
  // Données de la démo affichées, images comprises.
  await page.waitForLoadState("networkidle");
}

/** Une touche de télécommande (flèches, OK = Entrée), puis le temps du défilement. */
export async function touche(page: Page, key: "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" | "Enter", fois = 1) {
  for (let i = 0; i < fois; i++) {
    await page.keyboard.press(key);
    await page.waitForTimeout(120);
  }
  await page.waitForTimeout(350);
}

/** Bouton Retour de la télécommande (Android : retour dans l'historique). */
export async function retour(page: Page) {
  await page.goBack();
  await page.waitForTimeout(500);
}

/** L'élément sélectionné est entièrement à l'écran, cadre compris (8 px autour). */
export async function selectionVisible(page: Page) {
  const r = await page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body) return null;
    const b = el.getBoundingClientRect();
    return { top: b.top, bottom: b.bottom, left: b.left, right: b.right, w: innerWidth, h: innerHeight, tag: el.outerHTML.slice(0, 120) };
  });
  expect(r, "un élément doit être sélectionné").not.toBeNull();
  expect(r!.top, `haut coupé : ${r!.tag}`).toBeGreaterThanOrEqual(0);
  expect(r!.bottom, `bas coupé : ${r!.tag}`).toBeLessThanOrEqual(r!.h);
  expect(r!.left, `gauche coupé : ${r!.tag}`).toBeGreaterThanOrEqual(0);
  expect(r!.right, `droite coupé : ${r!.tag}`).toBeLessThanOrEqual(r!.w);
}

/** Couleur d'un pixel de la page (capture), pour vérifier qu'un fond va bien jusqu'au bord. */
export async function pixel(page: Page, x: number, y: number) {
  const png = await page.screenshot({ clip: { x, y, width: 1, height: 1 } });
  // PNG d'un pixel : décodé par le navigateur lui-même.
  return page.evaluate(async (b64) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const c = document.createElement("canvas");
    c.width = c.height = 1;
    const ctx = c.getContext("2d")!;
    ctx.drawImage(img, 0, 0);
    const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data;
    return { r, g, b };
  }, png.toString("base64"));
}

/** Écart entre deux couleurs (0 = identiques). */
export function ecart(a: { r: number; g: number; b: number }, b: { r: number; g: number; b: number }) {
  return Math.abs(a.r - b.r) + Math.abs(a.g - b.g) + Math.abs(a.b - b.b);
}
