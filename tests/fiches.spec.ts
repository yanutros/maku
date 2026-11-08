// Fiches : fond jusqu'au bord gauche (menu plié, déplié), navigation série → saison → épisode, sélection visible.
import { expect, test } from "@playwright/test";
import { appareil, ecart, ouvrir, pixel, selectionVisible, touche } from "./outils";

const FOND = { r: 0x14, g: 0x15, b: 0x18 };

test("le fond d'une fiche va jusqu'au bord gauche de l'écran", async ({ page }, info) => {
  test.skip(appareil(info) === "telephone", "pas de menu à gauche sur téléphone");
  await ouvrir(page, info, "fiche");
  await expect(page.locator(".backdrop img")).toBeVisible();
  await page.waitForTimeout(1400); // fin du fondu d'arrivée de l'image
  await expect(page).toHaveScreenshot("fiche.png");
  // Tout à gauche, sous le menu : l'image, pas le fond uni de l'application.
  const bord = await pixel(page, 3, 120);
  expect(ecart(bord, FOND), "le fond de la fiche s'arrête avant le bord gauche").toBeGreaterThan(12);
});

test("TV : le fond reste au bord quand le menu se déplie puis se replie", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "menu qui se déplie : téléviseur");
  await ouvrir(page, info, "fiche");
  await page.waitForTimeout(1400);
  const avant = await pixel(page, 3, 120);
  await touche(page, "ArrowLeft", 3); // jusqu'au menu, qui se déplie
  await expect(page.locator(".shell")).not.toHaveClass(/collapsed/);
  await page.waitForTimeout(400);
  expect(ecart(await pixel(page, 3, 120), FOND), "fond noir sous le menu déplié").toBeGreaterThan(12);
  await touche(page, "ArrowRight"); // retour dans la page : le menu se replie
  // Pendant le repli (0,25 s) : jamais le fond uni.
  for (let i = 0; i < 4; i++) {
    expect(ecart(await pixel(page, 3, 120), FOND), "fond noir pendant le repli du menu").toBeGreaterThan(12);
    await page.waitForTimeout(70);
  }
  await page.waitForTimeout(300);
  expect(ecart(await pixel(page, 3, 120), avant)).toBeLessThan(30);
});

test("série → saison → épisode, puis « Saison N » et « Toutes les saisons »", async ({ page }, info) => {
  await ouvrir(page, info, "serie");
  await page.getByText("Saison 2", { exact: true }).first().click();
  await expect(page.getByRole("button", { name: "Toutes les saisons" })).toBeVisible();
  await page.locator(".ep-title").first().click();
  const saison = page.getByRole("button", { name: /^Saison 2$/ });
  await expect(saison).toBeVisible();
  // L'ordre des boutons d'un épisode en cours : Reprendre, Saison, depuis le début.
  await saison.click();
  await expect(page.locator(".ep-title").first()).toBeVisible();
  await page.getByRole("button", { name: "Toutes les saisons" }).click();
  await expect(page.getByText("Saison 1", { exact: true }).first()).toBeVisible();
});

test("TV : en descendant une fiche, la sélection reste entière à l'écran", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "télécommande : téléviseur");
  await ouvrir(page, info, "fiche");
  for (let i = 0; i < 6; i++) {
    await touche(page, "ArrowDown");
    await selectionVisible(page);
  }
});

test("TV : un seul cadre de sélection, de la couleur du thème", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "télécommande : téléviseur");
  await ouvrir(page, info, "vide");
  await touche(page, "ArrowDown");
  const { cadre, accent } = await page.evaluate(() => {
    const el = document.activeElement as HTMLElement;
    const probe = document.createElement("span");
    probe.style.color = "var(--accent)";
    document.body.append(probe);
    const accent = getComputedStyle(probe).color;
    probe.remove();
    return { cadre: getComputedStyle(el).outlineColor, accent };
  });
  expect(cadre, "cadre de sélection blanc au lieu de la couleur du thème").toBe(accent);
});
