// Notifications en cartes : titre, explication, geste et détails techniques ; trois au plus, elles partent seules.
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

/** Le module des notifications, tel que l'application le charge (même instance). */
const notify = (page: Page, script: string) =>
  page.evaluate(async (code) => {
    const nav = await import("/src/lib/nav.svelte.ts");
    new Function("nav", code)(nav);
  }, script);

test("notifications : une carte par message, l'erreur réseau avec son explication, ses détails et Réessayer", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc" && appareil(info) !== "telephone", "souris et doigt ; le téléviseur a son propre test");
  await ouvrir(page, info, "reglages");
  await notify(page, `
    window.retried = false;
    nav.notify({ kind: "success", title: "Maku est à jour", text: "Version 0.27.0, la plus récente." });
    nav.toast("Seerr injoignable : error sending request for url (https://seerr.exemple.fr)", true);
  `);
  const cartes = page.locator(".toasts .toast");
  await expect(cartes).toHaveCount(2);
  await expect(cartes.first()).toContainText("Maku est à jour");
  await expect(cartes.first()).toContainText("Version 0.27.0, la plus récente.");

  // Erreur réseau : un titre lisible, quoi faire, et le message technique replié.
  const erreur = cartes.nth(1);
  await expect(erreur.locator(".title")).toHaveText("Seerr injoignable");
  await expect(erreur.locator(".text")).toHaveText("Vérifiez qu'il est allumé et connecté.");
  await expect(erreur.locator(".details")).toHaveCount(0);
  await erreur.getByRole("button", { name: "Détails" }).click();
  await expect(erreur.locator(".details")).toContainText("error sending request");

  // La croix ferme une carte.
  await cartes.first().getByRole("button", { name: "Fermer" }).click();
  await expect(cartes).toHaveCount(1);

  // Un geste proposé ferme la carte et s'exécute.
  await notify(page, `nav.notify({ kind: "error", title: "Échec", action: { label: "Réessayer", run: () => (window.retried = true) } });`);
  await page.locator(".toasts .toast", { hasText: "Échec" }).getByRole("button", { name: "Réessayer" }).click();
  expect(await page.evaluate(() => (window as unknown as { retried: boolean }).retried)).toBe(true);
  await expect(page.locator(".toasts .toast", { hasText: "Échec" })).toHaveCount(0);
});

test("notifications : trois au plus, et un succès s'en va seul", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même comportement partout");
  await ouvrir(page, info, "reglages");
  await notify(page, `for (const n of ["Un", "Deux", "Trois", "Quatre"]) nav.toast(n);`);
  const cartes = page.locator(".toasts .toast");
  await expect(cartes).toHaveText(["Deux", "Trois", "Quatre"]);
  await expect(cartes).toHaveCount(0, { timeout: 7000 });
});

test("notifications : sur la télé, ni bouton ni croix, rien que la télécommande puisse sélectionner", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "réservé au téléviseur");
  await ouvrir(page, info, "reglages");
  await notify(page, `nav.notify({ kind: "error", title: "Échec", details: "détail", action: { label: "Réessayer", run: () => {} } });`);
  const carte = page.locator(".toasts .toast");
  await expect(carte).toContainText("Échec");
  await expect(carte.locator("button")).toHaveCount(0);
});
