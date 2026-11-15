// « Corriger l'association » et « Dissocier » (administrateur du serveur Plex), comme dans Plex Web : sur un film
// ou une série, jamais sur une saison ni un épisode (ils suivent leur série), dans le menu « ⋮ » d'une jaquette ou
// d'une fiche. Le téléviseur n'a pas ce menu.
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

const CORRIGER = /Corriger l.association/;

/** Ouvre le menu « ⋮ » de la première jaquette de la grille. */
async function menuJaquette(page: Page) {
  const carte = page.locator(".card").first();
  await carte.locator(".open").waitFor();
  await carte.click({ button: "right" });
  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  return { menu, titre: (await carte.locator(".open").getAttribute("title")) ?? "" };
}

test("association : sur un film et une série pour l'administrateur, pas sur un épisode", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "le menu « ⋮ » n'existe pas au téléviseur");
  await ouvrir(page, info, "films&admin");
  const { menu } = await menuJaquette(page);
  await expect(menu.getByRole("menuitem", { name: CORRIGER })).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "Dissocier" })).toBeVisible();
  await page.keyboard.press("Escape");

  await ouvrir(page, info, "serie&admin");
  await page.getByRole("button", { name: "Plus d'actions" }).first().click();
  const menuSerie = page.getByRole("menu");
  await expect(menuSerie.getByRole("menuitem", { name: CORRIGER })).toBeVisible();
  await expect(menuSerie.getByRole("menuitem", { name: "Dissocier" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(menuSerie).toHaveCount(0);

  // Une saison, puis un épisode : leur menu a les actions de l'administrateur, mais pas l'association.
  const saison = page.locator(".card", { has: page.getByRole("button", { name: "Ouvrir Saison 1" }) });
  await saison.click({ button: "right" });
  const menuSaison = page.getByRole("menu");
  await expect(menuSaison.getByRole("menuitem", { name: "Actualiser les métadonnées" })).toBeVisible();
  await expect(menuSaison.getByRole("menuitem", { name: CORRIGER })).toHaveCount(0);
  await expect(menuSaison.getByRole("menuitem", { name: "Dissocier" })).toHaveCount(0);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Ouvrir Saison 1" }).click();
  await page.locator(".episode").first().click({ button: "right" });
  const menuEpisode = page.getByRole("menu");
  await expect(menuEpisode.getByRole("menuitem", { name: "Actualiser les métadonnées" })).toBeVisible();
  await expect(menuEpisode.getByRole("menuitem", { name: CORRIGER })).toHaveCount(0);
  await expect(menuEpisode.getByRole("menuitem", { name: "Dissocier" })).toHaveCount(0);
});

test("association : absente pour un utilisateur qui n'administre pas le serveur", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "le menu « ⋮ » n'existe pas au téléviseur");
  await ouvrir(page, info, "films");
  const { menu } = await menuJaquette(page);
  await expect(menu.getByRole("menuitem", { name: "Lire" })).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: CORRIGER })).toHaveCount(0);
  await expect(menu.getByRole("menuitem", { name: "Dissocier" })).toHaveCount(0);
});

test("association : les titres proposés, une recherche, puis Associer renomme le film", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "le menu « ⋮ » n'existe pas au téléviseur");
  await ouvrir(page, info, "films&admin");
  const { menu, titre } = await menuJaquette(page);
  await menu.getByRole("menuitem", { name: CORRIGER }).click();

  const fenetre = page.getByRole("dialog", { name: CORRIGER });
  await expect(fenetre).toBeVisible();
  // À l'ouverture : les propositions de Plex d'après le titre actuel, la plus sûre en tête.
  const resultats = fenetre.getByRole("option");
  await expect(resultats).toHaveCount(4);
  await expect(resultats.first()).toContainText(titre);
  await expect(resultats.first()).toContainText("98 %");
  await expect(fenetre.getByLabel("Titre", { exact: true })).toHaveValue(titre);
  const associer = fenetre.getByRole("button", { name: "Associer" });
  await expect(associer).toBeDisabled();

  // Une recherche par titre et année.
  await fenetre.getByLabel("Titre", { exact: true }).fill("La Comète de Verre");
  await fenetre.getByLabel("Année", { exact: true }).fill("1999");
  await fenetre.getByRole("button", { name: "Rechercher" }).click();
  await expect(resultats.first()).toContainText("La Comète de Verre (1999)");

  await resultats.first().click();
  await expect(resultats.first()).toHaveAttribute("aria-selected", "true");
  await associer.click();
  await expect(fenetre).toHaveCount(0);
  await expect(page.locator(".toast").filter({ hasText: "Associé à « La Comète de Verre »" })).toBeVisible();
  // La page se recharge : la jaquette porte le nouveau titre.
  await expect(page.locator(".card .open[title='La Comète de Verre']").first()).toBeVisible();
});

test("association : Échap ferme la fenêtre sans rien changer", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "le menu « ⋮ » n'existe pas au téléviseur");
  await ouvrir(page, info, "films&admin");
  const { menu, titre } = await menuJaquette(page);
  await menu.getByRole("menuitem", { name: CORRIGER }).click();
  const fenetre = page.getByRole("dialog", { name: CORRIGER });
  await expect(fenetre.getByRole("option")).toHaveCount(4);
  await page.keyboard.press("Escape");
  await expect(fenetre).toHaveCount(0);
  await expect(page.locator(".card .open").first()).toHaveAttribute("title", titre);
});

test("association : Dissocier, depuis la fiche d'un film", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "le menu « ⋮ » n'existe pas au téléviseur");
  await ouvrir(page, info, "fiche&admin");
  await page.getByRole("button", { name: "Plus d'actions" }).first().click();
  await page.getByRole("menu").getByRole("menuitem", { name: "Dissocier" }).click();
  await expect(page.locator(".toast").filter({ hasText: "dissocié" })).toBeVisible();
});

test("association : la fenêtre s'affiche", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "le menu « ⋮ » n'existe pas au téléviseur");
  await ouvrir(page, info, "films&admin");
  const { menu } = await menuJaquette(page);
  await menu.getByRole("menuitem", { name: CORRIGER }).click();
  const fenetre = page.getByRole("dialog", { name: CORRIGER });
  await expect(fenetre.getByRole("option")).toHaveCount(4);
  await fenetre.getByRole("option").first().click();
  await fenetre.locator("img").first().waitFor();
  await expect(page).toHaveScreenshot("association.png");
});
