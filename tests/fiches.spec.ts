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

test("série → saison : un épisode choisi en haut, les autres en rangée, les saisons en onglets", async ({ page }, info) => {
  await ouvrir(page, info, "serie");
  await page.getByText("Saison 2", { exact: true }).first().click();
  // La saison s'ouvre sur un épisode : sa ligne en haut, la rangée de la saison dessous.
  const tabs = page.getByRole("tablist");
  await expect(tabs.getByRole("tab", { name: "Saison 2" })).toHaveAttribute("aria-selected", "true");
  await expect(page.locator(".ep-thumb").first()).toBeVisible();
  // Choisir un autre épisode change le haut de la page, sans la quitter ni lancer la lecture.
  const second = page.locator(".ep-thumb").nth(1);
  await second.click({ position: { x: 12, y: 12 } }); // à côté du bouton ▶ du milieu, qui lance l'épisode
  await expect(second).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".ep.sel .ep-name")).toHaveText("Sous la glace");
  // Sur TV, la sélection choisit déjà l'épisode : OK, comme ce clic, le lance (le lecteur couvre alors la page).
  if (appareil(info) === "tv") return;
  await expect(page.locator(".overlay")).toHaveCount(0);
  // Onglet d'une autre saison, puis celui de la série.
  await tabs.getByRole("tab", { name: "Saison 1" }).click();
  await expect(tabs.getByRole("tab", { name: "Saison 1" })).toHaveAttribute("aria-selected", "true");
  await tabs.getByRole("tab", { name: "Série" }).click();
  await expect(page.getByText("Saison 1", { exact: true }).first()).toBeVisible();
});

test("TV : une saison s'ouvre sur l'épisode choisi, la rangée ne bouge pas, les onglets sont au-dessus", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "télécommande : téléviseur");
  await ouvrir(page, info, "serie");
  await page.getByRole("button", { name: /Saison 2/ }).first().focus();
  await touche(page, "Enter");
  // La sélection arrive sur la vignette de l'épisode choisi, pas sur « Lire ».
  await expect(page.locator(".ep-thumb[aria-pressed='true']")).toBeFocused();
  // D'un épisode à l'autre, la rangée reste à la même hauteur.
  const haut = () => page.locator(".ep-row").evaluate((el) => Math.round(el.getBoundingClientRect().top));
  const avant = await haut();
  for (const dir of ["ArrowLeft", "ArrowLeft", "ArrowRight"] as const) {
    await touche(page, dir);
    await page.waitForTimeout(400);
    expect(await haut(), "la rangée des épisodes s'est déplacée").toBe(avant);
  }
  // Haut : les onglets, sur la saison ouverte ; gauche : la série.
  await touche(page, "ArrowUp");
  await expect(page.getByRole("tab", { name: "Saison 2" })).toBeFocused();
  await touche(page, "ArrowLeft", 2);
  await expect(page.getByRole("tab", { name: "Série" })).toBeFocused();
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
