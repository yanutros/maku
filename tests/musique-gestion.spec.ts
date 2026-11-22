// Musique, administrateur du serveur Plex : « Corriger l'association » et « Dissocier » sur un album ou un artiste
// (avec l'artiste de l'album dans la recherche), pochette qui lance la lecture, et droit d'administration redemandé
// quand plex.tv n'a pas répondu au lancement.
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

type Spy = { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown, o: unknown) => Promise<unknown> }; calls: [string, Record<string, unknown>][] };

/** Note chaque commande envoyée au moteur, avec ses arguments. */
async function espionner(page: Page) {
  await page.evaluate(() => {
    const w = window as unknown as Spy;
    const invoke = w.__TAURI_INTERNALS__.invoke;
    w.calls = [];
    w.__TAURI_INTERNALS__.invoke = (c, a, o) => {
      w.calls.push([c, (a ?? {}) as Record<string, unknown>]);
      return invoke(c, a, o);
    };
  });
}
const appels = (page: Page, cmd: string) =>
  page.evaluate((cmd) => (window as unknown as Spy).calls.filter(([c]) => c === cmd).map(([, a]) => a), cmd);

/** La page du premier album de la démo. */
async function album(page: Page) {
  await page.evaluate(async () => {
    const nav = await import("/src/lib/nav.svelte.ts");
    const music = (await import("/src/lib/api.ts")).api;
    const hubs = await music.sectionHubs("3");
    const first = hubs.flatMap((h) => h.items).find((i) => i.kind === "album")!;
    nav.go({ name: "item", ratingKey: first.ratingKey });
  });
  await expect(page.locator(".page.detail h1")).toBeVisible();
}

test("album : Corriger l'association avec son artiste, et Dissocier", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de menu ⋮ d'administration au téléviseur");
  await ouvrir(page, info, "musique&admin");
  await album(page);
  const titre = (await page.locator(".page.detail h1").textContent())!.trim();
  const artiste = (await page.locator(".page.detail .info .artist").first().textContent())!.trim();
  await espionner(page);

  await page.locator(".page.detail .actions").getByRole("button", { name: "Plus d'actions" }).click();
  await expect(page.getByRole("menuitem", { name: "Dissocier" })).toBeVisible();
  await page.getByRole("menuitem", { name: /Corriger l.association/ }).click();

  const dialog = page.getByRole("dialog");
  await expect(dialog.getByLabel("Artiste")).toHaveValue(artiste);
  // Propositions de Plex : l'artiste de chaque album proposé est affiché.
  await expect(dialog.getByRole("option").first()).toContainText(artiste);
  await dialog.getByLabel("Artiste").fill("Un autre artiste");
  await dialog.getByRole("button", { name: "Rechercher" }).click();
  await expect.poll(() => appels(page, "plex_matches")).toContainEqual(expect.objectContaining({ title: titre, artist: "Un autre artiste" }));
  await expect(dialog.getByRole("option").first()).toContainText("Un autre artiste");
  await dialog.getByRole("option").first().click();
  await dialog.getByRole("button", { name: "Associer" }).click();
  await expect.poll(() => appels(page, "plex_fix_match")).toHaveLength(1);
});

test("artiste : Corriger l'association, sans champ d'artiste", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de menu ⋮ d'administration au téléviseur");
  await ouvrir(page, info, "musique&admin");
  await album(page);
  await page.locator(".page.detail .info .artist").first().click();
  await expect(page.locator(".page.detail .cover.round")).toBeVisible();
  await page.locator(".page.detail .actions").getByRole("button", { name: "Plus d'actions" }).click();
  await page.getByRole("menuitem", { name: /Corriger l.association/ }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("option").first()).toBeVisible();
  await expect(dialog.getByLabel("Artiste")).toHaveCount(0);
});

test("sans être administrateur : ni association ni dissociation", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de menu ⋮ d'administration au téléviseur");
  await ouvrir(page, info, "musique");
  await album(page);
  await page.locator(".page.detail .actions").getByRole("button", { name: "Plus d'actions" }).click();
  await expect(page.getByRole("menuitem", { name: "Lire ensuite" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: /Corriger l.association/ })).toHaveCount(0);
});

test("la pochette de l'album lance la lecture", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "au téléviseur, le bouton « Lire » reste le premier choix");
  await ouvrir(page, info, "musique");
  await album(page);
  await espionner(page);
  await page.locator(".page.detail .cover").hover();
  await page.locator(".page.detail .cover-play").click();
  await expect.poll(() => appels(page, "play_music")).toHaveLength(1);
});

test("plex.tv muet au lancement : l'administration est redemandée, puis apparaît", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même logique partout ; une fois suffit");
  await page.addInitScript(() => {
    localStorage.clear();
    // Les deux premières demandes échouent (plex.tv injoignable), la suivante répond.
    let fails = 2;
    (window as unknown as { __demoHooks: unknown }).__demoHooks = { plex_is_admin: { error: () => (fails-- > 0 ? "plex.tv ne répond pas" : null) } };
  });
  await page.clock.install({ time: new Date("2026-11-08T20:00:00+01:00") });
  await page.goto("/?demo=accueil&admin");
  await page.locator("main").waitFor({ state: "attached" });
  const admin = () => page.evaluate(async () => (await import("/src/lib/nav.svelte.ts")).nav.admin);
  await expect.poll(admin).toBe(false);
  // 30 s : nouvel échec ; 60 s plus tard : la réponse arrive.
  await page.clock.runFor(31_000);
  expect(await admin()).toBe(false);
  await page.clock.runFor(61_000);
  await expect.poll(admin).toBe(true);
  // L'horloge repart, pour que la page se redessine.
  await page.clock.resume();
  await expect(page.getByRole("button", { name: "Dashboard" })).toBeVisible();
});

test("Musique lue récemment : pas de bouton ⋮ sur les portraits d'artistes, le clic droit ouvre le menu", async ({ page }, info) => {
  test.skip(appareil(info) === "tv" || appareil(info) === "telephone", "rangées de l'ordinateur");
  await ouvrir(page, info, "musique");
  const recentes = page.locator("section", { has: page.getByRole("heading", { name: "Musique écoutée récemment" }) });
  const ajouts = page.locator("section", { has: page.getByRole("heading", { name: "Nouveautés" }) });
  await expect(recentes.locator(".card").first()).toBeVisible();
  await expect(recentes.getByRole("button", { name: "Plus d'actions" })).toHaveCount(0);
  // Les autres rangées gardent le leur.
  expect(await ajouts.getByRole("button", { name: "Plus d'actions" }).count()).toBeGreaterThan(0);
  await recentes.locator(".card").first().click({ button: "right" });
  await expect(page.getByRole("menu")).toBeVisible();
});
