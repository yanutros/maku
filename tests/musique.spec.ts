// Musique : boutons Aléatoire et Répéter du lecteur, « Écouter tout » et « Mix of Three » dans l'onglet Playlists.
import { expect, test } from "@playwright/test";
import { appareil, ouvrir, retour, touche } from "./outils";

test("lecteur : Aléatoire mélange la suite, Répéter passe de la file au titre puis à rien", async ({ page }, info) => {
  test.skip(appareil(info) !== "telephone", "lecteur intégré (la démo n'a pas mpv)");
  await ouvrir(page, info, "ecoute"); // « En cours de lecture » ouvert
  const lecteur = page.locator(".now-playing");
  const repeter = lecteur.getByRole("button", { name: /^Répéter/ });
  await expect(repeter).toHaveAccessibleName("Répéter : non");
  await repeter.click();
  await expect(repeter).toHaveAccessibleName("Répéter la file");
  await repeter.click();
  await expect(repeter).toHaveAccessibleName("Répéter le titre");
  await repeter.click();
  await expect(repeter).toHaveAccessibleName("Répéter : non");

  const aleatoire = lecteur.getByRole("button", { name: "Aléatoire" });
  await aleatoire.click();
  await expect(aleatoire).toHaveAttribute("aria-pressed", "true");
  await aleatoire.click();
  await expect(aleatoire).toHaveAttribute("aria-pressed", "false");
});

test("Mix of Three : trois playlists choisies lancent le mix, qui reste dans la rangée des playlists", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "souris et doigt (sur TV, les mêmes actions sont des jaquettes)");
  await ouvrir(page, info, "musique");
  // Chaque lecture lancée : ce qui part au lecteur (la démo ne lit rien).
  await page.evaluate(() => {
    const w = window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown, o: unknown) => Promise<unknown> }; launched: unknown[] };
    const invoke = w.__TAURI_INTERNALS__.invoke;
    w.launched = [];
    w.__TAURI_INTERNALS__.invoke = (c, a, o) => {
      if (c === "play_music" || c === "music_queue") w.launched.push(a);
      return invoke(c, a, o);
    };
  });
  const launched = () => page.evaluate(() => (window as unknown as { launched: { ratingKey: string; keys: string[] | null; shuffle: boolean }[] }).launched);

  await page.getByRole("tab", { name: "Playlists" }).click();
  await page.getByRole("button", { name: "Écouter tout" }).click();
  await expect.poll(async () => (await launched()).length).toBe(1);
  const all = (await launched())[0];
  expect(all.keys?.length, "toutes les playlists").toBe(4);
  expect(all.shuffle).toBe(true);

  await page.getByRole("button", { name: "Mix of Three" }).click();
  const picks = page.locator("button.pick");
  await picks.nth(0).click();
  await picks.nth(2).click();
  await expect(page.getByText("Choisis 3 playlists : 2 sur 3")).toBeVisible();
  await picks.nth(3).click();
  await expect.poll(async () => (await launched()).length).toBe(2);
  const mix = (await launched())[1];
  expect(mix.ratingKey).toMatch(/^mix:/);
  expect(mix.keys?.length).toBe(3);
  expect(mix.shuffle).toBe(true);

  // Le mix est dans la rangée des playlists de l'onglet Recommandé.
  await page.getByRole("tab", { name: "Recommandé" }).click();
  await expect(page.locator('[data-key^="mix-"]')).toHaveCount(1);
  // Comme une playlist, sa pochette ouvre sa fiche : les titres de ses trois playlists.
  await page.locator('[data-key^="mix-"] .meta').click(); // (la pochette se soulève au survol : le titre, lui, reste en place)
  await expect(page.getByRole("heading", { name: "Mix of Three" })).toBeVisible();
  await expect(page.locator(".tracks .row:not(.header)").first()).toBeVisible();
});

test("TV : le petit lecteur de la musique en cours se rejoint par bas, se commande, et rend la sélection", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "téléviseur");
  await ouvrir(page, info);
  // Sur TV, la musique lancée ouvre le lecteur plein écran : Retour le referme, le petit lecteur reste.
  await retour(page);
  const mini = page.getByRole("toolbar", { name: "En cours de lecture" });
  await expect(mini).toBeVisible();
  await expect(mini).toContainText(/\d+:\d\d \/ \d+:\d\d/);

  // Bas jusqu'en bas de l'accueil : la sélection entre dans le petit lecteur, sur Pause.
  await touche(page, "ArrowDown", 8);
  await expect(mini.getByRole("button", { name: "Pause" })).toBeFocused();
  await touche(page, "ArrowRight");
  await expect(mini.getByRole("button", { name: "Suivant" })).toBeFocused();
  await touche(page, "ArrowRight");
  await expect(mini.getByRole("button", { name: "Arrêter" })).toBeFocused();
  await touche(page, "ArrowLeft", 3);
  await expect(mini.getByRole("button", { name: "En cours de lecture" })).toBeFocused();

  // Haut rend la sélection à la jaquette d'où l'on venait ; Retour aussi.
  const jaquette = () => page.evaluate(() => (document.activeElement as HTMLElement | null)?.dataset.cell ?? null);
  await touche(page, "ArrowUp");
  const depart = await jaquette();
  expect(depart).not.toBeNull();
  await touche(page, "ArrowDown");
  await expect(mini.getByRole("button", { name: "Pause" })).toBeFocused();
  await retour(page);
  expect(await jaquette()).toBe(depart);

  // OK sur le titre ouvre le lecteur plein écran ; Arrêter coupe la musique et le petit lecteur disparaît.
  await touche(page, "ArrowDown");
  await touche(page, "ArrowLeft");
  await touche(page, "Enter");
  const plein = page.getByRole("dialog", { name: "En cours de lecture" });
  await expect(plein).toBeVisible();
  await retour(page);
  await expect(plein).toHaveCount(0);
  await touche(page, "ArrowDown");
  await touche(page, "ArrowRight", 2);
  await touche(page, "Enter");
  await expect(mini).toHaveCount(0);
  expect(await jaquette()).toBe(depart);
});
