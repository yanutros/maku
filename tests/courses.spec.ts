// Réponses qui arrivent dans le désordre, et rechargements : la saison ouverte par un onglet reste celle de la page,
// la bibliothèque musicale ne double pas sa liste, et la progression hors ligne repart à chaque retour du réseau.
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

type Invoke = (cmd: string, args?: unknown) => Promise<unknown>;
const invoke = <T>(page: Page, cmd: string, args: unknown = {}) =>
  page.evaluate(([cmd, args]) => (window as unknown as { __TAURI_INTERNALS__: { invoke: Invoke } }).__TAURI_INTERNALS__.invoke(cmd as string, args), [cmd, args]) as Promise<T>;

/** Retarde une commande de la démo (voir `__demoHooks`, demo.ts), pour toute la commande ou par titre. */
const lent = (page: Page, cmd: string, delay: number | Record<string, number>) =>
  page.evaluate(
    ([cmd, delay]) => {
      const w = window as unknown as { __demoHooks?: Record<string, unknown> };
      w.__demoHooks = { ...w.__demoHooks, [cmd as string]: { delay: typeof delay === "object" ? (a: { ratingKey: string }) => (delay as Record<string, number>)[a.ratingKey] ?? 0 : delay } };
    },
    [cmd, delay] as const,
  );

/** Les saisons de la première série de la démo, et la page de sa première saison ouverte. */
async function saisons(page: Page) {
  const shows = (await invoke<{ ratingKey: string; kind: string }[]>(page, "plex_search", { query: "" })).filter((m) => m.kind === "show");
  const list = await invoke<{ ratingKey: string; title: string }[]>(page, "plex_children", { ratingKey: shows[0].ratingKey });
  expect(list.length).toBeGreaterThanOrEqual(2);
  await page.evaluate(async (key) => {
    const nav = await import("/src/lib/nav.svelte.ts");
    nav.go({ name: "item", ratingKey: key });
  }, list[0].ratingKey);
  await expect(page.locator(".season-tabs [role=tab][aria-selected=true]")).toHaveText(list[0].title);
  return list;
}

const onglet = (page: Page, titre: string) => page.locator(".season-tabs [role=tab]", { hasText: new RegExp(`^${titre}$`) });
const saisonAffichee = (page: Page) => page.locator(".season-tabs [role=tab][aria-selected=true]");

test("TV : après une lecture, la page reste sur la saison ouverte par son onglet", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "onglets des saisons : téléviseur");
  await ouvrir(page, info, "vide");
  const list = await saisons(page);
  await onglet(page, list[1].title).click();
  await expect(saisonAffichee(page)).toHaveText(list[1].title);
  // Fin d'une lecture (mpv ou lecteur du système) : la fiche se recharge.
  await invoke(page, "plugin:event|emit", { event: "player-stopped", payload: "x" });
  await page.waitForTimeout(700);
  await expect(saisonAffichee(page)).toHaveText(list[1].title);
});

test("TV : deux onglets de saison pressés coup sur coup, c'est le dernier qui s'affiche", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "onglets des saisons : téléviseur");
  await ouvrir(page, info, "vide");
  const list = await saisons(page);
  // La deuxième saison répond lentement ; on revient à la première avant sa réponse.
  await lent(page, "plex_metadata", { [list[1].ratingKey]: 700 });
  await onglet(page, list[1].title).click();
  await onglet(page, list[0].title).click();
  await page.waitForTimeout(1200);
  // La réponse tardive de la deuxième saison ne remplace pas la première.
  await expect(saisonAffichee(page)).toHaveText(list[0].title);
});

test("musique : changer d'onglet très vite ne double pas la liste", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "onglets de la bibliothèque musicale à la souris");
  const erreurs: string[] = [];
  page.on("pageerror", (e) => erreurs.push(e.message));
  await ouvrir(page, info, "musique");
  const tabs = page.locator(".tabs [role=tab]");
  const cartes = page.locator(".grid > *");
  // La liste des artistes, chargée tranquillement : la référence.
  await tabs.nth(1).click();
  await expect(cartes.first()).toBeVisible();
  const attendu = await cartes.count();
  await tabs.nth(0).click();
  // Serveur lent : Artistes, Albums, Artistes, avant la première réponse.
  await lent(page, "plex_section_items", 500);
  await tabs.nth(1).click();
  await tabs.nth(2).click();
  await tabs.nth(1).click();
  await expect(cartes.first()).toBeVisible();
  await page.waitForTimeout(1500);
  expect(await cartes.count()).toBe(attendu);
  expect(erreurs, "erreur de la page (clés en double)").toEqual([]);
});

for (const serveur of ["Plex", "Jellyfin"]) {
  test(`hors ligne (${serveur}) : au retour du réseau, la progression est renvoyée au serveur`, async ({ page }, info) => {
    test.skip(appareil(info) !== "telephone", "téléchargements : téléphone Android");
    await ouvrir(page, info, serveur === "Jellyfin" ? "vide&plugin" : "vide");
    // Les téléchargements n'existent que sur Android : le navigateur des tests se présente comme tel.
    await page.evaluate(async () => {
      const { device } = await import("/src/lib/device.ts");
      device.os = "android";
      const w = window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown, o: unknown) => Promise<unknown> }; sync: number };
      const invoke = w.__TAURI_INTERNALS__.invoke;
      w.sync = 0;
      w.__TAURI_INTERNALS__.invoke = (c, a, o) => {
        if (c === "offline_sync") w.sync++;
        return invoke(c, a, o);
      };
    });
    await page.evaluate(() => window.dispatchEvent(new Event("online")));
    await expect.poll(() => page.evaluate(() => (window as unknown as { sync: number }).sync)).toBe(1);
  });
}
