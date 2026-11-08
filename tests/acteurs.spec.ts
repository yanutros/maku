// Acteurs : ouvrir sa fiche puis revenir ramène sur lui, avec un seul cercle doré.
import { expect, test } from "@playwright/test";
import { appareil, ouvrir, retour, touche } from "./outils";

test("TV : retour de la fiche d'un acteur, la sélection revient sur lui", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "télécommande : téléviseur");
  await ouvrir(page, info, "fiche");
  // Descendre jusqu'à la distribution, puis deux acteurs plus loin.
  for (let i = 0; i < 8 && !(await page.evaluate(() => document.activeElement?.classList.contains("person"))); i++) {
    await touche(page, "ArrowDown");
  }
  await touche(page, "ArrowRight", 2);
  const avant = await page.evaluate(() => (document.activeElement as HTMLElement).dataset.key);
  expect(avant).toBeTruthy();
  await touche(page, "Enter");
  await expect(page.locator(".person-head")).toBeVisible();
  await retour(page);
  await expect
    .poll(() => page.evaluate(() => (document.activeElement as HTMLElement | null)?.dataset.key), { timeout: 3000 })
    .toBe(avant);
  // Un seul acteur porte le cercle de sélection.
  const cercles = await page.evaluate(
    () => [...document.querySelectorAll(".person .avatar")].filter((a) => getComputedStyle(a).boxShadow !== "none").length,
  );
  expect(cercles, "plusieurs acteurs paraissent sélectionnés").toBe(1);
});

test("TV : depuis les titres d'un acteur, la flèche haut remonte sur sa photo", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "télécommande : téléviseur");
  await ouvrir(page, info, "fiche");
  for (let i = 0; i < 8 && !(await page.evaluate(() => document.activeElement?.classList.contains("person"))); i++) {
    await touche(page, "ArrowDown");
  }
  await touche(page, "Enter");
  await expect(page.locator(".person-head")).toBeVisible();
  await touche(page, "ArrowUp");
  await expect(page.locator(".person-head .avatar")).toBeFocused();
});
