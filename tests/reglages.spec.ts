// Réglages : plus aucune case à cocher, et sur TV haut / bas passent par chaque ligne, dans l'ordre.
import { expect, test } from "@playwright/test";
import { appareil, ouvrir, selectionVisible, touche } from "./outils";

const RUBRIQUES = ["Compte", "Lecteur", "Interface", "Services"];

test("aucune case à cocher : des interrupteurs partout", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "les rubriques s'ouvrent au clic : ordinateur, téléphone");
  await ouvrir(page, info, "reglages");
  for (const nom of RUBRIQUES) {
    // Ordinateur : la barre de sections ; téléphone : la bande des rubriques.
    const lien = appareil(info) === "telephone" ? page.locator(".settings .menu button", { hasText: nom }) : page.getByRole("tab", { name: nom });
    if (!(await lien.count())) continue;
    await lien.first().click();
    await page.waitForTimeout(200);
    const cases = await page.locator('input[type="checkbox"]:not(.switch)').count();
    expect(cases, `case à cocher dans ${nom}`).toBe(0);
  }
});

test("les réglages s'affichent", async ({ page }, info) => {
  await ouvrir(page, info, "reglages-interface");
  await expect(page.locator(".settings h1:visible").first()).toBeVisible();
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

test("services : le relais Maku Server trouvé a son panneau, comme Seerr et Tracearr", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc" && appareil(info) !== "telephone", "même page sur les autres appareils");
  await ouvrir(page, info, "reglages&relay");
  if (appareil(info) === "telephone") await page.locator(".settings .menu button", { hasText: "Services" }).click();
  else await page.getByRole("tab", { name: "Services" }).click();
  const titre = page.locator("h2.group-title", { hasText: "Maku Server local détecté" });
  await expect(titre).toContainText("Connecté");
  await expect(titre.locator("xpath=following-sibling::section[1]")).toContainText("Maku Server trouvé sur http://serveur.local:32700");
});

test("interface : le fond se choisit parmi cinq teintes, et reste sur l'appareil", async ({ page }, info) => {
  await ouvrir(page, info, "reglages-interface");
  // Téléviseur : la rubrique suit la sélection du menu des réglages ; on la pose sur Interface.
  if (appareil(info) === "tv") {
    await page.locator(".settings .menu button", { hasText: "Interface" }).focus();
    await page.waitForTimeout(400);
  }
  const fond = page.getByRole("radiogroup", { name: "Fond" });
  const teinte = () => page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  // Télécommande au téléviseur (sélection puis OK), clic ailleurs.
  const choisir = async (nom: string) => {
    const pastille = fond.getByRole("radio", { name: nom, exact: true });
    if (appareil(info) === "tv") {
      await pastille.focus();
      await touche(page, "Enter");
    } else await pastille.click();
  };
  // Par défaut : la nuit de Maku.
  await expect(fond.getByRole("radio", { name: "Nuit", exact: true })).toHaveAttribute("aria-checked", "true");
  expect(await teinte()).toBe("rgb(20, 21, 24)");

  await choisir("Ardoise");
  await expect(fond.getByRole("radio", { name: "Ardoise", exact: true })).toHaveAttribute("aria-checked", "true");
  expect(await teinte()).toBe("rgb(47, 56, 62)");
  await choisir("Minuit");
  expect(await teinte()).toBe("rgb(17, 24, 39)");
  await choisir("Marine");
  expect(await teinte()).toBe("rgb(22, 30, 45)");
  await choisir("Graphite");
  expect(await teinte()).toBe("rgb(26, 26, 26)");

  // Gardé sur l'appareil (les tests effacent les préférences à chaque chargement : on vérifie ce qui est enregistré,
  // puis qu'il est appliqué au démarrage).
  expect(await page.evaluate(() => localStorage.getItem("plexr.background"))).toBe("graphite");
  await page.evaluate(async () => {
    delete document.documentElement.dataset.background;
    const { applyBackground, savedBackground } = await import("/src/lib/theme.ts");
    applyBackground(savedBackground());
  });
  expect(await teinte()).toBe("rgb(26, 26, 26)");
});
