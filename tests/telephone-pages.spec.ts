// Téléphone : rien ne dépasse de l'écran dans les Réglages, les onglets de l'administration tiennent sur une ligne qui
// défile, le bouton « Télécharger » a la taille de ses voisins, et les téléchargements sont rangés par dossier.
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

const nav = (page: Page, code: string, arg: unknown = null) =>
  page.evaluate(
    async ([code, arg]) => {
      const nav = await import("/src/lib/nav.svelte.ts");
      return new Function("nav", "arg", code as string)(nav, arg);
    },
    [code, arg],
  );

/** La page ne défile pas de côté, et les éléments `selector` sont entièrement dans l'écran. */
async function dansLEcran(page: Page, selector: string) {
  const r = await page.evaluate((selector) => {
    const els = [...document.querySelectorAll(selector)].map((el) => el.getBoundingClientRect()).filter((b) => b.width > 0);
    return { count: els.length, right: Math.max(...els.map((b) => b.right)), left: Math.min(...els.map((b) => b.left)), w: innerWidth, scroll: document.documentElement.scrollWidth };
  }, selector);
  expect(r.count, `aucun élément ${selector}`).toBeGreaterThan(0);
  expect(r.right, `${selector} dépasse à droite`).toBeLessThanOrEqual(r.w);
  expect(r.left, `${selector} dépasse à gauche`).toBeGreaterThanOrEqual(0);
  expect(r.scroll, "la page défile de côté").toBeLessThanOrEqual(r.w + 1);
}

test("téléphone : dans Réglages → Interface, les rangées de l'accueil et leurs flèches tiennent dans l'écran", async ({ page }, info) => {
  test.skip(appareil(info) !== "telephone", "téléphone");
  // Un téléphone étroit, comme avec des noms de bibliothèque plus longs que ceux de la démo.
  await page.setViewportSize({ width: 340, height: 800 });
  await ouvrir(page, info, "reglages-interface");
  await page.locator(".rows .row-line").first().scrollIntoViewIfNeeded();
  await dansLEcran(page, ".rows .row-line .tool");
  await dansLEcran(page, ".rows .row-line");
  // Les flèches restent dans la carte, sans en sortir par la droite.
  const sorties = await page.evaluate(() => {
    const carte = document.querySelector(".rows")!.parentElement!.getBoundingClientRect();
    return [...document.querySelectorAll(".rows .tool")].filter((el) => el.getBoundingClientRect().right > carte.right + 0.5).length;
  });
  expect(sorties, "flèches hors de la carte").toBe(0);
  // Le nom d'une rangée s'affiche en entier, sans être coupé.
  const coupes = await page.locator(".rows .check span:not(.muted)").evaluateAll((els) => els.filter((el) => el.scrollWidth > el.clientWidth + 1).map((el) => el.textContent));
  expect(coupes).toEqual([]);
});

for (const [vue, pageAdmin, onglets] of [
  ["demandes-admin&admin", "requests", 4],
  ["vide&admin", "settings", 2],
] as const) {
  test(`téléphone : les onglets de l'administration (${pageAdmin}) tiennent sur une ligne qui défile`, async ({ page }, info) => {
    test.skip(appareil(info) !== "telephone", "téléphone");
    await ouvrir(page, info, vue);
    await nav(page, `nav.go({ name: "admin", page: arg });`, pageAdmin);
    const tabs = page.locator(".admin-tabs [role=tab]");
    await expect(tabs.first()).toBeVisible();
    expect(await tabs.count()).toBeGreaterThanOrEqual(onglets);
    // Une seule ligne : tous les onglets à la même hauteur, et aucun libellé sur deux lignes.
    const hauts = await tabs.evaluateAll((els) => els.map((el) => Math.round(el.getBoundingClientRect().top)));
    expect(new Set(hauts).size, `onglets sur plusieurs lignes : ${hauts}`).toBe(1);
    const hauteurs = await tabs.evaluateAll((els) => els.map((el) => Math.round(el.getBoundingClientRect().height)));
    expect(Math.max(...hauteurs)).toBeLessThan(48);
    // La bande va d'un bord de l'écran à l'autre.
    const bande = (await page.locator(".admin-tabs").boundingBox())!;
    expect(Math.round(bande.x)).toBe(0);
    expect(Math.round(bande.x + bande.width)).toBe(page.viewportSize()!.width);
    // La page, elle, ne défile pas de côté ; un onglet hors de l'écran y revient quand on le choisit.
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)).toBe(true);
    await tabs.last().click();
    await expect(tabs.last()).toBeInViewport({ ratio: 1 });
  });
}

test("téléphone : le bouton Télécharger a la hauteur des boutons voisins", async ({ page }, info) => {
  test.skip(appareil(info) !== "telephone", "téléchargements : téléphone Android");
  await ouvrir(page, info, "telechargements");
  // Le compte peut télécharger : le bouton apparaît sur la fiche d'un album.
  await page.evaluate(async () => {
    const offline = await import("/src/lib/offline.svelte.ts");
    await offline.checkOfflineAllowed();
  });
  const album = await page.evaluate(async () => {
    const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown) => Promise<unknown> } }).__TAURI_INTERNALS__.invoke;
    // Le premier album du premier artiste de la bibliothèque musicale.
    const artists = (await invoke("plex_section_items", { key: "3", start: 0, size: 1 })) as { items: { ratingKey: string }[] };
    const albums = (await invoke("plex_children", { ratingKey: artists.items[0].ratingKey })) as { ratingKey: string }[];
    return albums[0].ratingKey;
  });
  await nav(page, `nav.go({ name: "item", ratingKey: arg });`, album);
  const lire = page.locator(".actions > .btn.primary");
  const telecharger = page.locator(".actions .download > .btn");
  await expect(telecharger).toBeVisible();
  const a = (await lire.boundingBox())!;
  const b = (await telecharger.boundingBox())!;
  expect(Math.round(b.height)).toBe(Math.round(a.height));
  // Et il occupe sa place dans la rangée, au lieu de laisser « ⋮ » s'étirer à sa place.
  const plus = (await page.locator(".actions > .round-more").boundingBox())!;
  expect(b.width).toBeGreaterThan(plus.width);
});

test("téléchargements : une playlist et chaque saison sont un dossier, qui s'ouvre sur son contenu", async ({ page }, info) => {
  test.skip(appareil(info) !== "telephone", "téléchargements : téléphone Android");
  await ouvrir(page, info, "telechargements");
  const dossiers = page.locator(".groups .group");
  await expect(dossiers).toHaveCount(3);
  // À l'arrivée : les dossiers seulement, aucun de leurs titres (le film, lui, est à part).
  await expect(page.locator(".groups .row")).toHaveCount(0);
  await expect(page.locator(".list .row")).toHaveCount(1);
  const saison1 = dossiers.filter({ hasText: "Les Marées · Saison 1" });
  const saison2 = dossiers.filter({ hasText: "Les Marées · Saison 2" });
  const playlist = dossiers.filter({ hasText: "Sur la route" });
  await expect(saison1).toContainText("6 épisodes");
  await expect(saison2).toContainText("2 sur 6 téléchargés");
  await expect(playlist).toContainText("24 titres");
  // Un appui ouvre la playlist sur ses titres, un autre la referme ; les autres dossiers restent fermés.
  await playlist.locator(".group-toggle").tap();
  await expect(playlist.locator(".row")).toHaveCount(24);
  await expect(page.locator(".groups .row")).toHaveCount(24);
  await playlist.locator(".group-toggle").tap();
  await expect(page.locator(".groups .row")).toHaveCount(0);
  // Une saison ne montre que ses propres épisodes.
  await saison2.locator(".group-toggle").tap();
  await expect(saison2.locator(".row")).toHaveCount(6);
  await expect(saison2.locator(".row").first()).toContainText("S2 · E1");
  // « Tout supprimer » demande d'abord confirmation : rien n'est effacé à ce stade.
  await saison2.getByRole("button", { name: "Tout supprimer" }).tap();
  const question = saison2.getByRole("alertdialog");
  await expect(question).toContainText("Supprimer tout ce dossier de l'appareil ?");
  await expect(question).toContainText("6 épisodes");
  await expect(saison2.locator(".row")).toHaveCount(6);
  // « Annuler » ne touche à rien, et le bouton revient.
  await question.getByRole("button", { name: "Annuler" }).tap();
  await expect(question).toHaveCount(0);
  await expect(dossiers).toHaveCount(3);
  await expect(saison2.locator(".row")).toHaveCount(6);
  // Confirmé : le dossier se vide et disparaît, les autres restent.
  await saison2.getByRole("button", { name: "Tout supprimer" }).tap();
  await saison2.getByRole("alertdialog").getByRole("button", { name: "Supprimer", exact: true }).tap();
  await expect(dossiers).toHaveCount(2);
  await expect(dossiers.filter({ hasText: "Les Marées · Saison 1" })).toHaveCount(1);
});
