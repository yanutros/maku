// Accueil : il s'affiche, sans grand rectangle gris ni marque en haut à gauche pendant le chargement.
import { expect, test } from "@playwright/test";
import { appareil, ecart, ouvrir, pixel } from "./outils";

test("l'accueil s'affiche avec ses rangées", async ({ page }, info) => {
  await ouvrir(page, info, "vide");
  await expect(page.getByText(/Continuer à regarder/i).first()).toBeVisible();
  // Le carrousel change de diapositive tout seul : il est masqué dans la capture.
  await expect(page).toHaveScreenshot("accueil.png", { mask: [page.locator(".hero")] });
});

test("le chargement de l'accueil ne fait pas de rectangle gris ni de marque en haut à gauche", async ({ page }, info) => {
  test.skip(appareil(info) === "telephone", "pas de menu à gauche sur téléphone");
  // Accueil retenu en chargement : on voit la lueur d'attente.
  await page.route("**/*", (route) => route.continue());
  await ouvrir(page, info, "reglages");
  await page.evaluate(() => {
    // La démo répond tout de suite : on retarde la réponse de l'accueil pour observer le chargement.
    const w = window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown, o: unknown) => Promise<unknown> } };
    const invoke = w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke = (c, a, o) => (c === "plex_home" ? new Promise(() => {}) : invoke(c, a, o));
  });
  await page.getByRole("button", { name: "Accueil" }).first().click();
  await expect(page.locator(".hero-skeleton")).toBeVisible();
  await page.waitForTimeout(300);
  // En haut, à gauche de la page et juste à droite du menu : même teinte (pas de bande).
  const menu = await page.locator("aside").boundingBox();
  const gauche = await pixel(page, Math.max(2, Math.round(menu!.x + menu!.width) + 4), 6);
  const droite = await pixel(page, Math.round(menu!.x + menu!.width) + 80, 6);
  expect(ecart(gauche, droite), "bande de couleur en haut, contre le menu").toBeLessThan(18);
});
