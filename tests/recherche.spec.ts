// Recherche : les résultats rangés par type, une rangée chacun (et pas de rangée vide).
import { expect, test } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

test("recherche : une rangée Films, une rangée Séries, pas de rangée Musique sans musique", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même page partout ; la saisie se fait ici dans le champ du haut");
  await ouvrir(page, info, "vide");
  const champ = page.getByPlaceholder(/Rechercher un film/);
  await champ.fill("le"); // (la recherche part à partir de deux lettres)
  const titres = page.locator("section h2");
  await expect(titres.filter({ hasText: /^Films$/ }).first()).toBeVisible();
  await expect(titres.filter({ hasText: /^Séries$/ }).first()).toBeVisible();
  await expect(titres.filter({ hasText: /^Musique$/ })).toHaveCount(0);
});
