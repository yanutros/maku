// Musique : « Écouter tout » et « Mix of Three » dans l'onglet Playlists, et le mix gardé dans l'onglet Recommandé.
import { expect, test } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

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
