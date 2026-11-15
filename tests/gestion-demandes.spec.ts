// Admin › Demandes : l'administrateur de Seerr valide, refuse, corrige (serveur, profil, dossier) et supprime les
// demandes de tout le monde, partout sauf au téléviseur (menu de gauche ; onglet Bibliothèques au téléphone).
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

const cartes = (page: Page) => page.locator("article.req");
const carte = (page: Page, titre: string) => cartes(page).filter({ hasText: titre });

test("demandes : dans Admin pour l'administrateur de Seerr, avec le nombre en attente (sauf au téléviseur)", async ({ page }, info) => {
  await ouvrir(page, info, "vide&admin");
  if (appareil(info) === "tv") {
    await expect(page.locator("main")).toBeAttached();
    await page.waitForTimeout(500);
    await expect(page.locator("button.item", { hasText: "Demandes" })).toHaveCount(0);
    await expect(page.locator("button.library", { hasText: "Demandes" })).toHaveCount(0);
    return;
  }
  if (appareil(info) === "telephone") {
    await page.locator("nav.tabs button", { hasText: "Bibliothèques" }).click();
    const entree = page.locator("button.library", { hasText: "Demandes" });
    await expect(entree.locator(".count")).toHaveText("2");
    await entree.click();
  } else {
    const entree = page.locator("button.item", { hasText: "Demandes" });
    await expect(entree.locator(".count")).toHaveText("2");
    await entree.click();
  }
  await expect(page.locator("h1")).toHaveText("Demandes");
  await expect(page.getByRole("tab", { name: "En attente (2)" })).toHaveAttribute("aria-selected", "true");
  await expect(cartes(page)).toHaveCount(2);
  // Rien ne déborde de l'écran.
  const deborde = await page.locator(".content").evaluate((el) => el.scrollWidth - el.clientWidth);
  expect(deborde).toBeLessThanOrEqual(0);
});

test("demandes : réservées à l'administrateur de Seerr", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de gestion des demandes au téléviseur");
  await ouvrir(page, info, "demandes-admin");
  await expect(page.getByText("Réservé à l'administrateur de Seerr.")).toBeVisible();
  await expect(cartes(page)).toHaveCount(0);
  await expect(page.locator("button.item", { hasText: "Demandes" })).toHaveCount(0);
});

test("demandes : valider, refuser, puis supprimer avec confirmation", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de gestion des demandes au téléviseur");
  await ouvrir(page, info, "demandes-admin&admin");
  await expect(cartes(page)).toHaveCount(2);

  await carte(page, "Ondes Courtes").getByRole("button", { name: "Valider" }).click();
  await expect(page.locator(".toast").filter({ hasText: "« Ondes Courtes » validée" })).toBeVisible();
  await expect(cartes(page)).toHaveCount(1);
  await expect(page.getByRole("tab", { name: "En attente (1)" })).toBeVisible();
  if (appareil(info) !== "telephone") await expect(page.locator("button.item", { hasText: "Demandes" }).locator(".count")).toHaveText("1");

  await carte(page, "Terminus Sud").getByRole("button", { name: "Refuser" }).click();
  await expect(page.getByText("Aucune demande en attente : tout est traité.")).toBeVisible();
  await expect(page.getByRole("tab", { name: "En attente", exact: true })).toBeVisible();
  if (appareil(info) !== "telephone") await expect(page.locator("button.item", { hasText: "Demandes" }).locator(".count")).toHaveCount(0);

  // Toutes : les deux demandes traitées, avec leur nouvel état.
  await page.getByRole("tab", { name: "Toutes" }).click();
  await expect(carte(page, "Ondes Courtes").locator(".state")).toHaveText("Validée");
  await expect(carte(page, "Terminus Sud").locator(".state")).toHaveText("Refusée");
  await expect(carte(page, "Terminus Sud")).toContainText("Refusée par Camille");

  // Supprimer demande une confirmation, qu'on peut annuler.
  const avant = await cartes(page).count();
  const terminus = carte(page, "Terminus Sud");
  await terminus.getByRole("button", { name: "Supprimer" }).click();
  await expect(terminus.getByText("Supprimer la demande ?")).toBeVisible();
  await terminus.getByRole("button", { name: "Annuler" }).click();
  await expect(cartes(page)).toHaveCount(avant);
  await terminus.getByRole("button", { name: "Supprimer" }).click();
  await terminus.getByRole("button", { name: "Supprimer" }).click();
  await expect(page.locator(".toast").filter({ hasText: "Demande de « Terminus Sud » supprimée" })).toBeVisible();
  await expect(cartes(page)).toHaveCount(avant - 1);
  await expect(carte(page, "Terminus Sud")).toHaveCount(0);
});

test("demandes : une demande en échec se relance", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de gestion des demandes au téléviseur");
  await ouvrir(page, info, "demandes-admin&admin");
  await page.getByRole("tab", { name: "Toutes" }).click();
  const ambre = carte(page, "La Cité d'Ambre");
  await expect(ambre.locator(".state")).toHaveText("Échec");
  await ambre.getByRole("button", { name: "Réessayer" }).click();
  await expect(ambre.locator(".state")).toHaveText("Validée");
  await expect(ambre.getByRole("button", { name: "Réessayer" })).toHaveCount(0);
});

test("demandes : corriger le profil et le dossier, puis enregistrer et valider", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de gestion des demandes au téléviseur");
  await ouvrir(page, info, "demandes-admin&admin");
  const ondes = carte(page, "Ondes Courtes");
  await expect(ondes).toContainText("/data/films-animation");

  await ondes.getByRole("button", { name: "Modifier" }).click();
  const fenetre = page.getByRole("dialog", { name: "Modifier la demande" });
  await expect(fenetre).toContainText("Ondes Courtes");
  await expect(fenetre).toContainText("Demandé par Hugo");
  // Les réglages actuels de la demande sont choisis ; le serveur 4K n'est pas proposé pour une demande normale.
  await expect(fenetre.getByLabel("Serveur de destination").locator("option")).toHaveText(["Radarr (par défaut)"]);
  await expect(fenetre.getByLabel("Profil de qualité")).toHaveValue("1");
  await expect(fenetre.getByLabel("Dossier racine")).toHaveValue("/data/films-animation");
  await expect(fenetre.getByLabel("Dossier racine").locator("option").first()).toHaveText("/data/films (8,4 To libres)");

  await fenetre.getByLabel("Profil de qualité").selectOption({ label: "Ultra HD" });
  await fenetre.getByLabel("Dossier racine").selectOption("/data/films");
  await fenetre.getByRole("button", { name: "Enregistrer", exact: true }).click();
  await expect(fenetre).toHaveCount(0);
  await expect(ondes).toContainText("Ultra HD");
  await expect(ondes).toContainText("/data/films");
  await expect(ondes).not.toContainText("/data/films-animation");
  await expect(ondes.locator(".state")).toHaveText("En attente");

  // Rouverte, la fenêtre reprend les nouveaux réglages ; « Enregistrer et valider » fait les deux.
  await ondes.getByRole("button", { name: "Modifier" }).click();
  await expect(fenetre.getByLabel("Profil de qualité")).toHaveValue("2");
  await expect(fenetre.getByLabel("Dossier racine")).toHaveValue("/data/films");
  await fenetre.getByRole("button", { name: "Enregistrer et valider" }).click();
  await expect(page.locator(".toast").filter({ hasText: "« Ondes Courtes » validée" })).toBeVisible();
  await expect(cartes(page)).toHaveCount(1);
});

test("demandes : la page s'affiche", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de gestion des demandes au téléviseur");
  await ouvrir(page, info, "demandes-admin&admin");
  await expect(cartes(page)).toHaveCount(2);
  await page.locator("article.req img").first().waitFor();
  await expect(page).toHaveScreenshot("demandes-admin.png");
});
