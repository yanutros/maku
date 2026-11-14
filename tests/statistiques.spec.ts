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

test("statistiques : une connexion à Tracearr faite dans les Réglages suffit, sans la refaire dans Mes statistiques", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même parcours partout ; l'ordinateur suffit");
  await ouvrir(page, info, "reglages&tracearrlogin");
  await page.getByRole("tab", { name: "Services" }).click();
  const tracearr = page.locator("h2.group-title", { hasText: "Tracearr" }).locator("xpath=following-sibling::section[1]");
  await tracearr.getByRole("button", { name: "Se connecter avec Plex" }).click();
  // Les Réglages ouvrent Mes statistiques une fois la connexion validée : le tableau, pas une nouvelle demande.
  await expect(page.locator(".top")).toBeVisible({ timeout: 10_000 });
  await expect(page.getByRole("button", { name: "Se connecter avec Plex" })).toHaveCount(0);
});
