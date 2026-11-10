// Demandes Seerr : la fenêtre de demande s'atteint et se valide à la télécommande.
import { expect, test } from "@playwright/test";
import { appareil, ouvrir, touche } from "./outils";

test("TV : la fenêtre de demande prend la sélection, la page derrière ne bouge plus", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "télécommande : téléviseur");
  await ouvrir(page, info, "seerr");
  const demander = page.locator(".detail .btn.big").filter({ hasText: /Demander/ }).first();
  await demander.focus();
  await touche(page, "Enter");
  const fenetre = page.getByRole("dialog");
  await expect(fenetre).toBeVisible();
  // La sélection est dans la fenêtre, sur « Envoyer la demande »…
  await expect(fenetre.locator(":focus")).toHaveCount(1);
  // … et y reste quand on se déplace (elle ne repart pas parcourir la page derrière).
  for (const dir of ["ArrowUp", "ArrowLeft", "ArrowDown", "ArrowRight"] as const) {
    await touche(page, dir);
    await expect(fenetre.locator(":focus")).toHaveCount(1);
  }
});
