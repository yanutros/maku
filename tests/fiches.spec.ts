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

test("série → saison : les épisodes de haut en bas, l'épisode à reprendre en avant, retour à la série", async ({ page }, info) => {
  await ouvrir(page, info, "serie");
  await page.getByText("Saison 2", { exact: true }).first().click();
  // Tous les épisodes, chacun avec son titre et son résumé ; celui à reprendre mis en avant, et repris
  // par le bouton principal.
  await expect(page.locator(".episode").first()).toBeVisible();
  await expect(page.locator(".episode.target")).toHaveCount(1);
  await expect(page.locator(".episode.target .ep-title")).toContainText("Aurore");
  await expect(page.locator(".actions .btn.primary")).toContainText(/S2 · E4/);
  if (appareil(info) === "tv") {
    // Télécommande : les saisons en onglets, puis la série.
    const tabs = page.getByRole("tablist");
    await expect(tabs.getByRole("tab", { name: "Saison 2" })).toHaveAttribute("aria-selected", "true");
    await tabs.getByRole("tab", { name: "Saison 1" }).click();
    await expect(tabs.getByRole("tab", { name: "Saison 1" })).toHaveAttribute("aria-selected", "true");
    await tabs.getByRole("tab", { name: "Série" }).click();
  } else {
    // Souris et doigt : pas d'onglets, le logo de la série y ramène.
    await expect(page.getByRole("tablist")).toHaveCount(0);
    await page.locator(".show-logo").click();
  }
  await expect(page.getByText("Saison 1", { exact: true }).first()).toBeVisible();
});

test("TV : une saison s'ouvre sur l'épisode à reprendre, les onglets sont au-dessus", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "télécommande : téléviseur");
  await ouvrir(page, info, "serie");
  await page.getByRole("button", { name: /Saison 2/ }).first().focus();
  await touche(page, "Enter");
  // La sélection arrive sur la vignette de l'épisode à reprendre, pas sur le bouton du haut.
  await expect(page.locator(".episode.target .thumb")).toBeFocused();
  await touche(page, "ArrowDown");
  await expect(page.locator(".episode.target + .episode .thumb")).toBeFocused();
  // En remontant : les boutons de la saison, puis les onglets, sur la saison ouverte.
  for (let i = 0; i < 8 && !(await page.evaluate(() => document.activeElement?.getAttribute("role") === "tab")); i++) {
    await touche(page, "ArrowUp");
  }
  await expect(page.getByRole("tab", { name: "Saison 2" })).toBeFocused();
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

test("TV : après la lecture d'un épisode lancé depuis sa vignette, la sélection revient sur cette vignette", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "télécommande : téléviseur");
  await ouvrir(page, info, "serie");
  await page.getByRole("button", { name: /Saison 2/ }).first().focus();
  await touche(page, "Enter");
  await expect(page.locator(".episode.target .thumb")).toBeFocused();
  // L'épisode suivant, lancé depuis sa vignette.
  await touche(page, "ArrowDown");
  const vignette = page.locator(".episode.target + .episode .thumb");
  await expect(vignette).toBeFocused();
  const lance = await vignette.getAttribute("data-key");
  await touche(page, "Enter");
  await expect(page.locator(".overlay")).toBeVisible();
  // Arrêt de la lecture (bouton ■) : retour sur la fiche, sur la même vignette, à l'écran.
  await page.locator(".overlay").getByRole("button", { name: "Arrêter", exact: true }).first().click({ force: true });
  await expect(page.locator(".overlay")).toHaveCount(0);
  await expect(page.locator(`[data-key="${lance}"]`)).toBeFocused();
  await selectionVisible(page);
  // Et les flèches repartent de là, sans remonter au bouton « Lire ».
  await touche(page, "ArrowDown");
  await expect(page.locator(`.episode:has([data-key="${lance}"]) + .episode .thumb`)).toBeFocused();
});

test("TV : OK maintenu sur la vignette d'un épisode ouvre son menu (vu / non vu)", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "télécommande : téléviseur");
  await ouvrir(page, info, "serie");
  await page.getByRole("button", { name: /Saison 2/ }).first().focus();
  await touche(page, "Enter");
  const vignette = page.locator(".episode.target .thumb");
  await expect(vignette).toBeFocused();
  const episode = page.locator(".episode.target");
  await expect(episode.locator(".badge.ok")).toHaveCount(0);
  // OK maintenu : le menu de l'épisode, sans lancer la lecture.
  await page.keyboard.down("Enter");
  await page.waitForTimeout(800);
  await page.keyboard.up("Enter");
  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  await expect(page.locator(".overlay")).toHaveCount(0);
  await menu.getByRole("menuitem", { name: "Marquer comme vu" }).click();
  await expect(menu).toHaveCount(0);
  await expect(episode.locator(".badge.ok")).toBeVisible();
  // Un appui bref, lui, lance toujours l'épisode.
  await vignette.focus();
  await touche(page, "Enter");
  await expect(page.locator(".overlay")).toBeVisible();
});
