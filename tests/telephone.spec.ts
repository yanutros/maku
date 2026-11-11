// Téléphone : bugs corrigés (onglets du bas sur Statistiques, « À suivre » du lecteur plein écran).
import { expect, test } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

test("téléphone : les onglets du bas restent visibles sur Statistiques (au-dessus du fond)", async ({ page }, info) => {
  test.skip(appareil(info) !== "telephone", "onglets du téléphone");
  await ouvrir(page, info, "stats");
  const onglets = page.locator("nav.tabs");
  await expect(onglets).toBeVisible();
  const dessus = await onglets.evaluate((el) => {
    const r = el.getBoundingClientRect();
    return !!document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)?.closest("nav.tabs");
  });
  expect(dessus).toBe(true);
});

test("téléphone : « À suivre » affiche la file, sans faire glisser l'écran de côté", async ({ page }, info) => {
  test.skip(appareil(info) !== "telephone", "lecteur plein écran du téléphone");
  await ouvrir(page, info, "ecoute");
  await page.locator(".now-playing [role=tab]", { hasText: "À suivre" }).click();
  await expect(page.locator(".now-playing .next .q").first()).toBeVisible();
  const largeur = await page.locator(".now-playing").evaluate((el) => [el.scrollWidth, el.clientWidth, el.scrollLeft]);
  expect(largeur[0]).toBeLessThanOrEqual(largeur[1]);
  expect(largeur[2]).toBe(0);
});
