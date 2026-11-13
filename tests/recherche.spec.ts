// Recherche : les résultats rangés par type, une rangée chacun (et pas de rangée vide).
import { expect, test } from "@playwright/test";
import { appareil, ouvrir, touche } from "./outils";

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

test("TV : la recherche reste en revenant sur sa page, et OK sur le champ le vide pour en taper une nouvelle", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "téléviseur : le champ s'ouvre avec OK");
  await ouvrir(page, info, "vide");
  const rechercher = page.locator("aside nav .item", { hasText: "Rechercher" });
  await rechercher.focus();
  await touche(page, "Enter");
  const champ = page.getByPlaceholder(/Rechercher un film/);
  await expect(champ).toBeFocused();
  // OK ouvre le champ (clavier de la TV), puis la saisie.
  await touche(page, "Enter");
  await champ.pressSequentially("le");
  const films = page.locator("section h2").filter({ hasText: /^Films$/ }).first();
  await expect(films).toBeVisible();

  // On consulte un résultat : le champ garde la recherche.
  await touche(page, "ArrowDown");
  await expect(champ).not.toBeFocused();
  await expect(champ).toHaveValue("le");

  // De retour sur le champ, OK le vide ; les résultats restent jusqu'à la nouvelle saisie.
  await champ.focus();
  await touche(page, "Enter");
  await expect(champ).toHaveValue("");
  await expect(films).toBeVisible();
  await champ.pressSequentially("ma");
  await expect(champ).toHaveValue("ma");
});
