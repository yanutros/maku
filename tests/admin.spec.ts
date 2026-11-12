// Catégorie Admin du menu : Dashboard (tableau de bord, utilisateurs) et Réglages serveur, sur ordinateur et pour
// l'administrateur seulement, avec la barre de sections des bibliothèques.
import { expect, test } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

test("admin : Dashboard et Réglages serveur dans le menu, avec leurs sections (ordinateur, administrateur)", async ({ page }, info) => {
  await ouvrir(page, info, "vide&admin");
  const dashboard = page.locator("button.item", { hasText: "Dashboard" });
  if (appareil(info) !== "pc" && appareil(info) !== "safari") {
    await expect(page.locator("main")).toBeAttached();
    await expect(dashboard).toHaveCount(0);
    return;
  }
  await dashboard.click();
  await expect(page.locator("h1")).toHaveText("Dashboard");
  await expect(page.getByRole("tab", { name: "Tableau de bord" })).toHaveAttribute("aria-selected", "true");
  await page.getByRole("tab", { name: "Utilisateurs" }).click();
  await expect(page.getByRole("tab", { name: "Utilisateurs" })).toHaveAttribute("aria-selected", "true");

  await page.locator("button.item", { hasText: "Réglages serveur" }).click();
  await expect(page.locator("h1")).toHaveText("Réglages serveur");
});

test("admin : absent pour un utilisateur qui n'est pas administrateur, et plus dans les Réglages", async ({ page }, info) => {
  await ouvrir(page, info, "reglages");
  await expect(page.locator(".settings")).toBeVisible();
  await expect(page.locator("button.item", { hasText: "Dashboard" })).toHaveCount(0);
  await expect(page.locator(".settings .menu button", { hasText: "Réglages du serveur" })).toHaveCount(0);
});

test("admin : tableau de bord — arrêter une lecture avec un message, débit, top des utilisateurs, historique, les plus lus", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "réservé à l'ordinateur");
  await page.setViewportSize({ width: 1400, height: 2600 });
  await ouvrir(page, info, "vide&admin");
  await page.locator("button.item", { hasText: "Dashboard" }).click();
  // Jaquettes à leurs proportions : affiche pour un film, carrée pour la musique ; les boutons d'arrêt alignés.
  const posters = page.locator(".session .poster");
  const [film, album] = [await posters.nth(0).boundingBox(), await posters.nth(1).boundingBox()];
  expect(Math.round(film!.height / film!.width * 10)).toBe(15);
  expect(Math.round(album!.height)).toBe(Math.round(album!.width));
  const stops = page.getByRole("button", { name: "Arrêter la lecture" });
  expect(Math.round((await stops.nth(0).boundingBox())!.y)).toBe(Math.round((await stops.nth(1).boundingBox())!.y));
  await stops.first().click();
  const fenetre = page.getByRole("dialog");
  await expect(fenetre).toContainText("Arrêter la lecture de Hugo ?");
  await fenetre.locator("textarea").fill("Le serveur redémarre");
  await fenetre.getByRole("button", { name: "Arrêter", exact: true }).click();
  await expect(page.getByText("Lecture de Hugo arrêtée")).toBeVisible();

  await expect(page.getByRole("heading", { name: "Débit" })).toBeVisible();
  await expect(page.locator(".legend").first()).toContainText("Distant");
  await expect(page.locator(".user").first()).toContainText("Camille");
  await expect(page.getByRole("heading", { name: "Historique de lecture" })).toBeVisible();
  await expect(page.locator(".most-col")).toHaveCount(3);
  await page.screenshot({ path: "test-results/dashboard-admin.png", fullPage: true });
});

test("admin : la position d'une lecture en cours avance en direct, sans attendre le relevé suivant", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "réservé à l'ordinateur");
  await ouvrir(page, info, "vide&admin");
  await page.locator("button.item", { hasText: "Dashboard" }).click();
  const temps = page.locator(".session .meta span").first();
  await expect(temps).toHaveText(/^41:00 /);
  // L'heure des tests est figée : on l'avance de 5 s, la position suit sans nouveau relevé du serveur.
  await page.clock.setFixedTime(new Date("2026-11-08T20:00:05+01:00"));
  await expect(temps).toHaveText(/^41:0[4-6] /, { timeout: 4000 });
  // Le serveur (la démo) renvoie toujours 41:00, comme un lecteur qui n'a pas encore donné sa nouvelle
  // position : après le relevé suivant (toutes les 3 s), le temps affiché ne recule pas.
  await page.clock.setFixedTime(new Date("2026-11-08T20:00:08+01:00"));
  await expect(temps).toHaveText(/^41:0[7-9] /, { timeout: 4000 });
  await page.waitForTimeout(4000);
  await expect(temps).toHaveText(/^41:0[7-9] /);
});
