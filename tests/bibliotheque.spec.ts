// Onglet Bibliothèque des films et des séries : filtre (« Tout », non vus, genre…) et tri, comme dans Plex.
import { expect, test } from "@playwright/test";
import { appareil, ouvrir, touche } from "./outils";

test("bibliothèque : filtrer les non vus, choisir un genre, trier par année", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "mêmes menus partout ; le téléviseur a son propre test");
  await ouvrir(page, info, "films");
  await page.getByRole("tab", { name: "Bibliothèque" }).click();
  const nombre = page.locator("h1 .muted");
  await expect(nombre).toHaveText("24");

  await page.getByRole("button", { name: "Tout" }).click();
  await page.getByRole("menuitemradio", { name: "Non vu" }).click();
  await expect(nombre).toHaveText("20");
  await expect(page.getByRole("button", { name: "Non vu" })).toBeVisible();

  await page.getByRole("button", { name: "Non vu" }).click();
  await page.getByRole("menuitem", { name: "Genre" }).click();
  await page.getByRole("menuitemradio", { name: "Drame" }).click();
  await expect(page.getByText("Genre : Drame")).toBeVisible();

  await page.getByRole("button", { name: "Par Titre" }).click();
  await page.getByRole("menuitemradio", { name: "Année" }).click();
  await expect(page.getByRole("button", { name: "Par Année" })).toBeVisible();
  // (La démo répète sa liste : la première moitié suffit.)
  const années = await page.locator(".grid .sub").allTextContents();
  const nombres = années.slice(0, années.length / 2).map(Number);
  expect(nombres).toEqual([...nombres].sort((a, b) => b - a));

  // Le choix est gardé pour la visite suivante (le rechargement des tests efface le stockage : on le lit).
  const gardé = await page.evaluate(() => JSON.parse(localStorage.getItem("maku.libraryView") ?? "{}")["1"]);
  expect(gardé).toMatchObject({ flag: "unwatched", sort: "year", desc: true, value: { field: "genre", id: "Drame" } });
});

test("bibliothèque TV : haut depuis la grille mène au filtre, le menu se parcourt à la télécommande", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "navigation à la télécommande");
  await ouvrir(page, info, "films");
  await page.getByRole("tab", { name: "Bibliothèque" }).click();
  await page.locator("[data-index='0']").focus();
  await touche(page, "ArrowUp");
  await expect(page.locator(":focus")).toHaveText(/Par Titre/);
  await touche(page, "ArrowLeft");
  await expect(page.locator(":focus")).toHaveText(/Tout/);
  await touche(page, "Enter");
  await touche(page, "ArrowDown");
  await expect(page.locator(":focus")).toHaveText("Non vu");
  await touche(page, "Enter");
  await expect(page.getByRole("button", { name: "Non vu" })).toBeFocused();
  await expect(page.getByText("20 titres")).toBeVisible();
});
