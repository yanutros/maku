// Touche Retour de la télécommande, comme dans Plex : page précédente à l'endroit quitté ; sur une page de
// premier niveau, le menu de gauche, puis l'accueil.
import { expect, test } from "@playwright/test";
import { appareil, ouvrir, retour, touche } from "./outils";

const dansLeMenu = (page: import("@playwright/test").Page) => page.evaluate(() => !!document.activeElement?.closest("aside"));

test("TV : Retour depuis une saison revient sur cette saison dans la série, puis à l'accueil, puis au menu", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "téléviseur");
  await ouvrir(page, info, "serie");
  const saison = page.locator(".seasons .grid [data-key]").nth(1);
  const cle = await saison.getAttribute("data-key");
  await saison.click();
  await expect(page.locator(".episodes")).toBeVisible();

  await retour(page);
  await expect(page.locator(".seasons")).toBeVisible();
  await expect.poll(() => page.evaluate(() => (document.activeElement?.closest("[data-key]") as HTMLElement | null)?.dataset.key)).toBe(cle);

  // La série avait été ouverte depuis l'accueil : Retour y ramène.
  await retour(page);
  await expect(page.locator(".seasons")).toHaveCount(0);
  await expect(page.locator("aside [data-nav-home]")).toHaveAttribute("aria-current", "page");
  // Sur l'accueil, Retour passe la sélection au menu de gauche au lieu de quitter.
  await touche(page, "ArrowDown");
  expect(await dansLeMenu(page)).toBe(false);
  await retour(page);
  await expect(page.locator("aside [data-nav-home]")).toBeFocused();
});

test("TV : une bibliothèque ouverte depuis le menu : Retour passe au menu, puis à l'accueil", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "téléviseur");
  await ouvrir(page, info, "vide");
  const films = page.locator("aside nav .item", { hasText: "Films" });
  await films.focus();
  await touche(page, "Enter");
  await expect(films).toHaveAttribute("aria-current", "page");
  await touche(page, "ArrowRight");
  expect(await dansLeMenu(page)).toBe(false);

  await retour(page);
  await expect(films).toBeFocused();

  await retour(page);
  await expect(page.locator("aside [data-nav-home]")).toHaveAttribute("aria-current", "page");
  await expect(page.locator("aside [data-nav-home]")).toBeFocused();
});

test("TV : deux Retour rapides depuis une fiche ouverte de l'accueil : la sélection reste au menu", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "téléviseur");
  await ouvrir(page, info, "vide");
  // Une jaquette de l'accueil, ouverte avec OK : au retour, l'accueil la sélectionnera de nouveau.
  await touche(page, "ArrowRight");
  await touche(page, "Enter");
  await expect(page.locator("aside [data-nav-home]")).not.toHaveAttribute("aria-current", "page");
  // L'accueil met du temps à se remettre en place (comme sur un téléviseur) : ses jaquettes arrivent après 1,5 s.
  await page.evaluate(() => {
    const keep = Element.prototype.querySelectorAll;
    const until = Date.now() + 1500;
    Element.prototype.querySelectorAll = function (this: Element, sel: string) {
      return Date.now() < until && (sel === "[data-key]" || sel === "[data-cell]") ? keep.call(this, ":not(*)") : keep.call(this, sel);
    } as typeof Element.prototype.querySelectorAll;
  });
  await page.goBack();
  await page.goBack();
  await expect(page.locator("aside [data-nav-home]")).toBeFocused();
  await page.waitForTimeout(2500);
  await expect(page.locator("aside [data-nav-home]")).toBeFocused();
});
