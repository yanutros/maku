// Démarrage et rafraîchissements : les demandes indépendantes partent ensemble, les bibliothèques ne sont demandées
// qu'une fois, et les pages qui se rafraîchissent seules s'arrêtent quand personne ne les regarde.
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

/** Note l'heure de chaque appel des commandes `cmds`, dès le chargement de la page ; `lent` les retarde (ms). */
const noter = (page: Page, cmds: string[], lent: Record<string, number> = {}) =>
  page.addInitScript(
    ([cmds, lent]) => {
      const w = window as unknown as { appels: [string, number][]; __demoHooks: Record<string, unknown> };
      w.appels = [];
      w.__demoHooks = Object.fromEntries(
        (cmds as string[]).map((cmd) => [
          cmd,
          {
            delay: () => {
              w.appels.push([cmd, performance.now()]);
              return (lent as Record<string, number>)[cmd] ?? 0;
            },
          },
        ]),
      );
    },
    [cmds, lent] as const,
  );

const appels = (page: Page) => page.evaluate(() => (window as unknown as { appels: [string, number][] }).appels);
const nombre = async (page: Page, cmd: string) => (await appels(page)).filter(([c]) => c === cmd).length;

test("démarrage : les bibliothèques ne sont demandées qu'une fois, accueil compris", async ({ page }, info) => {
  await noter(page, ["plex_sections"]);
  await ouvrir(page, info, "vide");
  await expect(page.locator("main")).toBeVisible();
  await page.waitForTimeout(500);
  expect(await nombre(page, "plex_sections")).toBe(1);
});

test("démarrage : bibliothèques et droit d'administration sont demandés en même temps", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même démarrage sur tous les appareils");
  // Le serveur met 600 ms à donner ses bibliothèques.
  await noter(page, ["plex_sections", "plex_is_admin"], { plex_sections: 600 });
  await ouvrir(page, info, "vide");
  await expect.poll(() => nombre(page, "plex_is_admin")).toBeGreaterThan(0);
  const list = await appels(page);
  const debut = (cmd: string) => list.find(([c]) => c === cmd)![1];
  // L'un n'attend pas la réponse de l'autre.
  expect(Math.abs(debut("plex_is_admin") - debut("plex_sections"))).toBeLessThan(300);
});

/** Maku n'est plus à l'écran (application en arrière-plan, fenêtre réduite). */
const cacher = (page: Page, cache: boolean) =>
  page.evaluate((cache) => {
    Object.defineProperty(document, "hidden", { configurable: true, get: () => cache });
  }, cache);

test("statistiques : pas de rafraîchissement quand Maku n'est pas à l'écran, ni pendant une vidéo", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même page sur tous les appareils");
  await page.clock.install();
  await noter(page, ["tracearr_dashboard"]);
  await ouvrir(page, info, "stats");
  await expect(page.locator(".top")).toBeVisible();
  const depart = await nombre(page, "tracearr_dashboard");
  expect(depart).toBeGreaterThan(0);
  // À l'écran : un rafraîchissement toutes les 15 s.
  await page.clock.fastForward(16_000);
  await expect.poll(() => nombre(page, "tracearr_dashboard")).toBe(depart + 1);
  // Hors de l'écran : plus rien.
  await cacher(page, true);
  for (let i = 0; i < 3; i++) await page.clock.fastForward(16_000);
  expect(await nombre(page, "tracearr_dashboard")).toBe(depart + 1);
  // De retour, puis une vidéo par-dessus : toujours rien.
  await cacher(page, false);
  await page.evaluate(async () => {
    const p = await import("/src/lib/player.svelte.ts");
    Object.assign(p.player, { active: true, video: true });
  });
  for (let i = 0; i < 3; i++) await page.clock.fastForward(16_000);
  expect(await nombre(page, "tracearr_dashboard")).toBe(depart + 1);
  // La vidéo fermée : le rafraîchissement reprend.
  await page.evaluate(async () => {
    const p = await import("/src/lib/player.svelte.ts");
    Object.assign(p.player, { active: false, video: false });
  });
  await page.clock.fastForward(16_000);
  await expect.poll(() => nombre(page, "tracearr_dashboard")).toBe(depart + 2);
});

test("tableau de bord : une demande à la fois, et aucune quand Maku n'est pas à l'écran", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "tableau de bord de l'administrateur : ordinateur");
  await page.clock.install();
  // Le serveur met 10 s à répondre : trois rafraîchissements tombent pendant l'attente.
  await noter(page, ["plex_sessions"], { plex_sessions: 10_000 });
  await ouvrir(page, info, "vide&admin");
  await page.evaluate(async () => {
    const nav = await import("/src/lib/nav.svelte.ts");
    nav.go({ name: "admin", page: "dashboard" });
  });
  await expect.poll(() => nombre(page, "plex_sessions")).toBe(1);
  for (let i = 0; i < 3; i++) await page.clock.fastForward(3_100);
  expect(await nombre(page, "plex_sessions"), "demandes empilées pendant l'attente").toBe(1);
  // La réponse arrivée, le rafraîchissement reprend.
  await page.clock.fastForward(4_000);
  await expect.poll(() => nombre(page, "plex_sessions")).toBe(2);
  // Hors de l'écran : plus de demande, même une fois la réponse en cours arrivée.
  await cacher(page, true);
  const avant = await nombre(page, "plex_sessions");
  for (let i = 0; i < 6; i++) await page.clock.fastForward(3_100);
  expect(await nombre(page, "plex_sessions")).toBe(avant);
});
