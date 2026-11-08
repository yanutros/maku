// Lecteur vidéo : lecture / pause à la taille des autres boutons ; sur TV, pas de boutons du haut.
import { expect, test } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

test("le bouton lecture / pause a la taille des autres boutons", async ({ page }, info) => {
  await ouvrir(page, info, "lecteur");
  const main = await page.locator(".overlay .icon-btn.main").boundingBox();
  const autre = await page.locator(".overlay .transport .icon-btn:not(.main)").first().boundingBox();
  expect(main!.width).toBe(autre!.width);
  expect(main!.height).toBe(autre!.height);
});

test("TV : pas de boutons retour, volume ni plein écran en haut", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "téléviseur");
  await ouvrir(page, info, "lecteur");
  await expect(page.locator(".overlay .icon-btn.main")).toBeVisible();
  await expect(page.locator(".overlay .top")).toHaveCount(0);
});
