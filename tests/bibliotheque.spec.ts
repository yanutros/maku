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
  // Haut depuis la barre : retour à l'onglet actif (il était devenu inatteignable).
  await touche(page, "ArrowUp");
  await expect(page.locator(":focus")).toHaveText("Bibliothèque");
  await touche(page, "ArrowDown");
  await expect(page.locator(":focus")).toHaveText(/Tout|Par Titre/);
  if (!/Tout/.test((await page.locator(":focus").textContent()) ?? "")) await touche(page, "ArrowLeft");
  await expect(page.locator(":focus")).toHaveText(/Tout/);
  await touche(page, "Enter");
  await touche(page, "ArrowDown");
  await expect(page.locator(":focus")).toHaveText("Non vu");
  await touche(page, "Enter");
  await expect(page.getByRole("button", { name: "Non vu" })).toBeFocused();
  await expect(page.getByText("20 titres")).toBeVisible();
});

test("bibliothèque : nouvelle collection intelligente (administrateur, ordinateur), avec un aperçu dans la grille", async ({ page }, info) => {
  await ouvrir(page, info, "films&admin");
  await page.getByRole("tab", { name: "Bibliothèque" }).click();
  const bouton = page.getByRole("button", { name: "Collection", exact: true });
  if (appareil(info) === "tv" || appareil(info) === "telephone") {
    // Réservé à l'ordinateur.
    await expect(page.locator(".bar .pick").first()).toBeVisible();
    await expect(bouton).toHaveCount(0);
    return;
  }
  await bouton.click();
  await page.getByRole("menuitem", { name: "Nouvelle collection intelligente" }).click();
  await expect(page.getByRole("button", { name: "Créer la collection" })).toBeDisabled();
  await page.getByRole("textbox", { name: "Valeur" }).fill("or");
  await page.getByRole("button", { name: "Aperçu" }).click();
  await expect(page.locator("h1 .muted")).toHaveText("6");
  await expect(page.locator(".bar .pick").first()).toHaveText(/Aperçu de la collection/);
  await page.getByRole("textbox", { name: "Nom de la collection" }).fill("Orbites");
  await expect(page.getByRole("button", { name: "Créer la collection" })).toBeEnabled();
  // Fermer le panneau retire l'aperçu.
  await page.getByRole("button", { name: "Fermer" }).click();
  await expect(page.locator("h1 .muted")).toHaveText("24");
});

test("bibliothèque : ranger les titres affichés dans une collection ordinaire (administrateur, ordinateur)", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "réservé à l'ordinateur (vérifié dans le test précédent)");
  await ouvrir(page, info, "films&admin");
  await page.getByRole("tab", { name: "Bibliothèque" }).click();
  await page.getByRole("button", { name: "Tout" }).click();
  await page.getByRole("menuitemradio", { name: "Non vu" }).click();
  await expect(page.locator("h1 .muted")).toHaveText("20");
  await page.getByRole("button", { name: "Collection", exact: true }).click();
  await page.getByRole("menuitem", { name: /Ajouter à une collection/ }).click();
  const fenêtre = page.getByRole("dialog");
  await expect(fenêtre.getByRole("heading")).toHaveText("Ajouter 20 titres à la collection");
  // Les collections intelligentes se remplissent seules : seules les ordinaires sont proposées.
  await expect(fenêtre.getByText("Soirées d'été")).toBeVisible();
  await expect(fenêtre.getByText("Science-fiction")).toHaveCount(0);
  await fenêtre.getByText("Soirées d'été").click();
  await expect(page.getByText("20 titres ajoutés à « Soirées d'été »")).toBeVisible();
});

test("bibliothèque TV : appuis rapides vers le bas, la grille glisse en continu jusqu'à la case visée", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "navigation à la télécommande");
  await ouvrir(page, info, "films");
  await page.getByRole("tab", { name: "Bibliothèque" }).click();
  await page.locator("[data-index='0']").focus();
  // Relevé du défilement à chaque image pendant une rafale d'appuis (un toutes les 50 ms, comme une flèche maintenue).
  await page.evaluate(() => {
    const el = document.querySelector<HTMLElement>(".content")!;
    const w = window as unknown as { __pos: number[] };
    w.__pos = [];
    const t0 = performance.now();
    const loop = () => {
      w.__pos.push(el.scrollTop);
      if (performance.now() - t0 < 1500) requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
  });
  for (let i = 0; i < 3; i++) {
    await page.keyboard.press("ArrowDown");
    await page.waitForTimeout(50);
  }
  await page.waitForTimeout(1500);
  const pos = await page.evaluate(() => (window as unknown as { __pos: number[] }).__pos);
  // Jamais de recul (une animation relancée repartait de sa position de départ).
  for (let i = 1; i < pos.length; i++) expect(pos[i]).toBeGreaterThanOrEqual(pos[i - 1] - 1);
  expect(pos[pos.length - 1]).toBeGreaterThan(0);
  const box = await page.locator(":focus").boundingBox();
  const vh = page.viewportSize()!.height;
  expect(box!.y).toBeGreaterThanOrEqual(0);
  expect(box!.y + box!.height).toBeLessThanOrEqual(vh);
});

test("collection : ordre personnalisé en glissant les jaquettes, conversion d'une intelligente (administrateur, ordinateur)", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "réservé à l'ordinateur");
  await ouvrir(page, info, "films&admin");
  await page.getByRole("tab", { name: "Collections" }).click();
  await page.locator(".card", { hasText: "Soirées d'été" }).locator(".open").click();
  // Les réglages sont derrière la roue dentée.
  await expect(page.locator(".order")).toHaveCount(0);
  await page.getByRole("button", { name: "Réglages de la collection" }).click();
  await expect(page.locator(".order")).toBeVisible();
  await page.locator(".settings-backdrop").click({ position: { x: 5, y: 5 } });
  const titres = page.locator(".grid .title");
  const avant = await titres.allTextContents();
  await page.locator(".sortable .slot").nth(0).dragTo(page.locator(".sortable .slot").nth(2));
  await expect(titres.nth(2)).toHaveText(avant[0]);

  // Une collection intelligente : pas d'ordre à la main, mais la conversion (confirmée par un second clic).
  await page.goBack();
  await page.locator(".card", { hasText: "Science-fiction" }).locator(".open").click();
  await page.getByRole("button", { name: "Réglages de la collection" }).click();
  await expect(page.locator(".order")).toHaveCount(0);
  await page.getByRole("button", { name: "Convertir en collection ordinaire" }).click();
  await expect(page.getByRole("button", { name: "Cliquer à nouveau pour confirmer" })).toBeVisible();
  await page.getByRole("button", { name: "Cliquer à nouveau pour confirmer" }).click();
  await expect(page.getByText("Collection convertie")).toBeVisible();
  // Retour ne ramène pas à l'ancienne collection (supprimée) : on revient à la liste des collections.
  await page.goBack();
  await expect(page.getByRole("tab", { name: "Collections" })).toHaveAttribute("aria-selected", "true");
});
