// Statistiques : le Top 10 se parcourt avec ses flèches sur ordinateur (barres de défilement masquées).
import { expect, test } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

test("statistiques : les flèches du Top 10 le font défiler (ordinateur), absentes au téléphone et à la TV", async ({ page }, info) => {
  await ouvrir(page, info, "stats");
  const top = page.locator(".top");
  await expect(top).toBeVisible();
  const fleches = page.locator(".section .arrows button");
  if (appareil(info) === "tv" || appareil(info) === "telephone") {
    await expect(fleches.first()).toBeHidden();
    return;
  }
  await expect(fleches.first()).toBeDisabled();
  await fleches.last().click();
  await expect.poll(() => top.evaluate((el) => el.scrollLeft)).toBeGreaterThan(100);
  await expect(fleches.first()).toBeEnabled();
});
