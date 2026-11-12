// Réglages : plus aucune case à cocher, et sur TV haut / bas passent par chaque ligne, dans l'ordre.
import { expect, test } from "@playwright/test";
import { appareil, ouvrir, selectionVisible, touche } from "./outils";

const RUBRIQUES = ["Compte", "Lecteur", "Interface", "Services"];

test("aucune case à cocher : des interrupteurs partout", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "les rubriques s'ouvrent au clic : ordinateur, téléphone");
  await ouvrir(page, info, "reglages");
  for (const nom of RUBRIQUES) {
    const lien = page.locator(".settings .menu button", { hasText: nom });
    if (!(await lien.count())) continue;
    await lien.first().click();
    await page.waitForTimeout(200);
    const cases = await page.locator('input[type="checkbox"]:not(.switch)').count();
    expect(cases, `case à cocher dans ${nom}`).toBe(0);
  }
});

test("les réglages s'affichent", async ({ page }, info) => {
  await ouvrir(page, info, "reglages-interface");
  await expect(page.locator(".settings .panel h1")).toBeVisible();
  await expect(page).toHaveScreenshot("reglages-interface.png");
});

test("TV : haut / bas passent par chaque ligne des Réglages, sans en sauter", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "télécommande : téléviseur");
  await ouvrir(page, info, "reglages-interface");
  // Entrer dans le panneau depuis le rail (la rubrique Interface reste ouverte).
  await page.locator(".settings .menu button", { hasText: "Interface" }).focus();
  await page.waitForTimeout(400);
  await touche(page, "ArrowRight");
  for (let i = 0; i < 10; i++) {
    const avant = await page.evaluate(() => (document.activeElement as HTMLElement).getBoundingClientRect().bottom);
    await touche(page, "ArrowDown");
    const saut = await page.evaluate((avantBas) => {
      const el = document.activeElement as HTMLElement;
      const top = el.getBoundingClientRect().top;
      if (top <= avantBas - 8) return null; // plus rien en dessous
      // Un élément sélectionnable du panneau, entre l'ancienne et la nouvelle ligne : il a été sauté.
      const panneau = document.querySelector(".settings .panel")!;
      const saute = [...panneau.querySelectorAll<HTMLElement>("button:not(:disabled), input, [tabindex='0']")].find((x) => {
        const r = x.getBoundingClientRect();
        return r.height > 4 && r.top >= avantBas - 8 && r.bottom <= top + 8 && x !== el && !el.contains(x);
      });
      return saute ? saute.outerHTML.slice(0, 120) : "";
    }, avant);
    if (saut === null) break;
    expect(saut, "ligne sautée à la flèche bas").toBe("");
    await selectionVisible(page);
  }
});

test("TV : pas de barres de défilement", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "téléviseur");
  await ouvrir(page, info, "reglages-interface");
  const largeur = await page.evaluate(() => getComputedStyle(document.querySelector(".content")!).scrollbarWidth);
  expect(largeur).toBe("none");
});
