// Barre de navigation simplifiée (TV) : une pilule qui ne se déplie pas, des icônes qui ne se touchent pas.
import { expect, test } from "@playwright/test";
import { appareil, ouvrir, touche } from "./outils";

test("TV : la pilule ne se déplie pas, ses icônes restent espacées", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "téléviseur");
  await ouvrir(page, info, "vide", { simple: true });
  await expect(page.locator("aside.pill")).toBeVisible();
  await touche(page, "ArrowLeft", 4);
  const largeur = (await page.locator("aside.pill").boundingBox())!.width;
  expect(largeur, "la pilule s'est dépliée").toBeLessThan(80);
  await expect(page).toHaveScreenshot("pilule.png");
  // Entre deux icônes : au moins la place du cadre de sélection (contour de 3 px décalé de 2 px, de chaque côté).
  const ecarts = await page.evaluate(() => {
    const items = [...document.querySelectorAll<HTMLElement>("aside.pill nav .item")].map((e) => e.getBoundingClientRect());
    return items.slice(1).map((r, i) => r.top - items[i].bottom);
  });
  for (const e of ecarts) expect(e, "deux icônes de la pilule se touchent").toBeGreaterThanOrEqual(10);
});
