// Erreurs dans les pages : la carte des notifications (titre lisible, quoi faire, détails, Réessayer), à sa place.
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

/** Fait échouer des commandes de la démo, comme un serveur éteint ; rend de quoi le rallumer. */
async function panne(page: Page, cmds: string[]) {
  await page.evaluate((list) => {
    const w = window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown, o: unknown) => Promise<unknown> }; panne: string[] };
    const invoke = w.__TAURI_INTERNALS__.invoke;
    w.panne = list;
    w.__TAURI_INTERNALS__.invoke = (c, a, o) =>
      w.panne.includes(c) ? Promise.reject("error sending request for url (http://serveur.local:32400/hubs)") : invoke(c, a, o);
  }, cmds);
}
const rallumer = (page: Page) => page.evaluate(() => ((window as unknown as { panne: string[] }).panne = []));

test("erreurs : une bibliothèque injoignable affiche la carte, Réessayer la recharge", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc" && appareil(info) !== "telephone", "même carte partout");
  await ouvrir(page, info, "vide");
  await panne(page, ["plex_section_hubs"]);
  if (appareil(info) === "pc") await page.locator("button.item", { hasText: "Films" }).click();
  else {
    await page.locator("nav.tabs button", { hasText: "Bibliothèques" }).click();
    await page.locator("button.library", { hasText: "Films" }).click();
  }
  const carte = page.locator(".error-card");
  await expect(carte.locator(".title")).toHaveText("Serveur injoignable");
  await expect(carte.locator(".text")).toHaveText("Vérifiez qu'il est allumé et connecté.");
  // Le message technique est replié, et plus aucun texte rouge brut.
  await expect(carte.locator(".details")).toHaveCount(0);
  await carte.getByRole("button", { name: "Détails" }).click();
  await expect(carte.locator(".details")).toContainText("error sending request");
  await expect(page.locator("p.error")).toHaveCount(0);
  // Pas d'emplacement de chargement sous l'erreur.
  await expect(page.locator(".skeleton")).toHaveCount(0);

  await rallumer(page);
  await carte.getByRole("button", { name: "Réessayer" }).click();
  await expect(page.locator(".error-card")).toHaveCount(0);
  await expect(page.locator(".row, .tv-rows, .hero").first()).toBeVisible();
});

test("erreurs : un bloc du tableau de bord a sa carte, et Réessayer ne recharge que lui", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "réservé à l'ordinateur");
  await ouvrir(page, info, "vide&admin");
  await panne(page, ["plex_bandwidth"]);
  await page.locator("button.item", { hasText: "Dashboard" }).click();
  const carte = page.locator(".error-card");
  await expect(carte.locator(".title")).toHaveText("Statistiques indisponibles");
  await expect(page.locator(".session").first()).toBeVisible();
  await rallumer(page);
  await carte.getByRole("button", { name: "Réessayer" }).click();
  await expect(page.locator(".error-card")).toHaveCount(0);
  await expect(page.locator(".legend").first()).toContainText("Distant");
});
