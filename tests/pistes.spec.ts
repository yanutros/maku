// Son et sous-titres préférés : la préférence des Réglages est gardée, et un choix fait sur un titre (dans le lecteur
// ou sur sa fiche) est confié à Maku, qui le reprendra à la lecture suivante.
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

/** Note chaque choix de piste confié à Maku (commande `remember_track`). */
async function noterChoix(page: Page) {
  await page.evaluate(() => {
    const w = window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown, o: unknown) => Promise<unknown> }; choix: unknown[] };
    const invoke = w.__TAURI_INTERNALS__.invoke;
    w.choix = [];
    w.__TAURI_INTERNALS__.invoke = (c, a, o) => {
      if (c === "remember_track") w.choix.push(a);
      return invoke(c, a, o);
    };
  });
  return () => page.evaluate(() => (window as unknown as { choix: unknown[] }).choix);
}

const prefs = (page: Page) => page.evaluate(() => ({ son: localStorage.getItem("maku.audioLang"), sousTitres: localStorage.getItem("maku.subtitles") }));

test("Réglages → Lecteur : la langue du son et les sous-titres préférés sont gardés", async ({ page }, info) => {
  test.skip(appareil(info) === "tv" || appareil(info) === "safari", "listes du système : ordinateur et téléphone");
  await ouvrir(page, info, "reglages");
  const lien = appareil(info) === "telephone" ? page.locator(".settings .menu button", { hasText: "Lecteur" }) : page.getByRole("tab", { name: "Lecteur" });
  await lien.first().click();
  const son = page.locator('[data-pref="audio"] select');
  const sousTitres = page.locator('[data-pref="subtitles"] select');
  // Réglages jamais touchés : le son dans la langue de l'interface (ici le français), aucun sous-titre.
  await expect(son).toHaveValue("fr");
  await expect(sousTitres).toHaveValue("off");
  expect(await prefs(page)).toEqual({ son: null, sousTitres: null });
  await son.selectOption({ label: "Anglais" });
  await sousTitres.selectOption({ label: "Français" });
  expect(await prefs(page)).toEqual({ son: "en", sousTitres: "fr" });
  await sousTitres.selectOption({ label: "Aucun" });
  expect((await prefs(page)).sousTitres).toBe("off");
  await sousTitres.selectOption({ label: "Si le son est dans une autre langue" });
  expect((await prefs(page)).sousTitres).toBe("auto");
  await sousTitres.selectOption({ label: "Anglais" });
  expect((await prefs(page)).sousTitres).toBe("en");
  // « Choix du serveur » est gardé comme les autres : un réglage vide redeviendrait « français, aucun sous-titre ».
  await son.selectOption({ label: "Choix du serveur" });
  await sousTitres.selectOption({ label: "Choix du serveur" });
  expect(await prefs(page)).toEqual({ son: "server", sousTitres: "server" });
});

test("Réglages → Lecteur : la préférence gardée est celle affichée au retour", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même page sur tous les appareils");
  await ouvrir(page, info, "accueil");
  await page.evaluate(() => {
    localStorage.setItem("maku.audioLang", "server");
    localStorage.setItem("maku.subtitles", "auto");
  });
  await page.goto("/?demo=reglages");
  await page.getByRole("tab", { name: "Lecteur" }).click();
  await expect(page.locator('[data-pref="audio"] select')).toHaveValue("server");
  await expect(page.locator('[data-pref="subtitles"] select')).toHaveValue("auto");
});

test("fiche : le son ou les sous-titres choisis sont retenus pour ce titre", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "listes du système");
  await ouvrir(page, info, "fiche");
  const choix = await noterChoix(page);
  const cle = await page.evaluate(async () => (await import(/* @vite-ignore */ "/src/lib/nav.svelte.ts" as string)).nav.view.ratingKey);
  const listes = page.locator(".panel.media label.track select");
  await expect(listes).toHaveCount(2);
  // Les sous-titres : « Aucun » (0) au départ dans la démo, puis la piste française (3).
  await listes.nth(1).selectOption("3");
  expect(await choix()).toEqual([{ ratingKey: cle, kind: "subtitle", stream: 3, lang: null }]);
  await listes.nth(1).selectOption("0");
  expect((await choix())[1]).toEqual({ ratingKey: cle, kind: "subtitle", stream: 0, lang: null });
  const son = await listes.nth(0).locator("option").nth(1).getAttribute("value");
  await listes.nth(0).selectOption(son!);
  expect((await choix())[2]).toEqual({ ratingKey: cle, kind: "audio", stream: Number(son), lang: null });
});

test("lecteur : la piste choisie est retenue par sa langue", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "menus à la souris");
  await ouvrir(page, info, "lecteur");
  const choix = await noterChoix(page);
  // Les pistes du fichier, telles que le lecteur les annonce (leurs numéros ne sont pas ceux du serveur).
  await page.evaluate(async () => {
    const { player } = await import(/* @vite-ignore */ "/src/lib/player.svelte.ts" as string);
    player.ratingKey = "42";
    player.tracks = [
      { id: 1, type: "audio", lang: "eng", title: "Original" },
      { id: 2, type: "audio", lang: "fre", title: "VF" },
      { id: 1, type: "sub", lang: "fre", title: "Complets" },
      { id: 2, type: "sub", title: "Sans langue" },
    ];
    player.aid = 1;
    player.sid = 1;
  });
  const menu = async (titre: string, ligne: string) => {
    await page.locator(`.overlay .pill[aria-label="${titre}"]`).click();
    await page.locator(".overlay .menu button", { hasText: ligne }).first().click();
  };
  await menu("Pistes audio", "VF");
  expect(await choix()).toEqual([{ ratingKey: "42", kind: "audio", stream: null, lang: "fre" }]);
  await menu("Sous-titres", "Aucun");
  expect((await choix())[1]).toEqual({ ratingKey: "42", kind: "subtitle", stream: null, lang: "off" });
  await menu("Sous-titres", "Complets");
  expect((await choix())[2]).toEqual({ ratingKey: "42", kind: "subtitle", stream: null, lang: "fre" });
  // Une piste sans langue connue : rien à retenir.
  await menu("Sous-titres", "Sans langue");
  expect(await choix()).toHaveLength(3);
});
