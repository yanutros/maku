// Petits défauts : un rechargement raté ne remplace pas la page affichée, la tête de lecture tenue ne saute pas, et
// ce que Maku garde par bibliothèque ou par fiche ne passe pas d'un serveur ou d'un profil à l'autre.
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

type Invoke = (cmd: string, args?: unknown) => Promise<unknown>;
const invoke = <T>(page: Page, cmd: string, args: unknown = {}) =>
  page.evaluate(([cmd, args]) => (window as unknown as { __TAURI_INTERNALS__: { invoke: Invoke } }).__TAURI_INTERNALS__.invoke(cmd as string, args), [cmd, args]) as Promise<T>;

/** Événement du Rust (fin d'une lecture, position de mpv…). */
const evenement = (page: Page, event: string, payload: unknown) => invoke(page, "plugin:event|emit", { event, payload });

/** Le serveur met `delay` ms à répondre à `cmd`, ou échoue avec `error`. */
const serveur = (page: Page, cmd: string, hook: { delay?: number; error?: string } | null) =>
  page.evaluate(
    ([cmd, hook]) => {
      const w = window as unknown as { __demoHooks?: Record<string, unknown> };
      const hooks = { ...w.__demoHooks };
      if (hook) hooks[cmd as string] = hook;
      else delete hooks[cmd as string];
      w.__demoHooks = hooks;
    },
    [cmd, hook] as const,
  );

const module = <T>(page: Page, path: string, code: string, arg: unknown = null) =>
  page.evaluate(
    async ([path, code, arg]) => {
      const m = await import(/* @vite-ignore */ path as string);
      return new Function("m", "arg", code as string)(m, arg);
    },
    [path, code, arg],
  ) as Promise<T>;

test("accueil : un rechargement raté après une lecture garde la page, avec un message", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même page sur tous les appareils");
  await ouvrir(page, info, "vide");
  const cartes = page.locator("main .card");
  await expect(cartes.first()).toBeVisible();
  const avant = await cartes.count();
  // Fin d'une lecture, alors que le serveur ne répond plus.
  await serveur(page, "plex_home", { error: "error sending request for url (http://serveur.example:32400/hubs)" });
  await evenement(page, "player-stopped", "x");
  await expect(page.locator(".toasts .toast")).toHaveCount(1);
  // L'accueil est toujours là, entier : pas de carte d'erreur à sa place.
  expect(await cartes.count()).toBe(avant);
  await expect(page.locator("main .notice.error, main .error-card")).toHaveCount(0);
  // Le serveur revenu, un nouveau rechargement passe sans rien laisser derrière.
  await serveur(page, "plex_home", null);
  await evenement(page, "player-stopped", "x");
  await page.waitForTimeout(400);
  expect(await cartes.count()).toBe(avant);
});

test("fiche : un rechargement raté après une lecture garde la fiche, avec un message", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même page sur tous les appareils");
  await ouvrir(page, info, "fiche");
  const fiche = page.locator(".page.detail");
  await expect(fiche).toBeVisible();
  const titre = await fiche.locator("h1").first().textContent();
  await serveur(page, "plex_metadata", { error: "error sending request for url (http://serveur.example:32400/library/metadata/1)" });
  await evenement(page, "player-stopped", "x");
  await expect(page.locator(".toasts .toast")).toHaveCount(1);
  await expect(fiche).toBeVisible();
  expect(await fiche.locator("h1").first().textContent()).toBe(titre);
});

test("lecteur : la tête de lecture tenue ne revient pas à la position de la lecture", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "barre de progression à la souris");
  await ouvrir(page, info, "lecteur");
  const position = () => module<number>(page, "/src/lib/player.svelte.ts", "return m.player.position;");
  // Sans rien tenir, la position suit la lecture.
  await evenement(page, "player-prop", { name: "time-pos", data: 100 });
  await expect.poll(position).toBe(100);
  // On tient la tête de lecture à 1 000 s : la lecture, qui continue, ne la ramène pas.
  const barre = page.locator(".overlay .bar, .overlay .seek, .overlay [role=slider]").first();
  await barre.dispatchEvent("pointerdown", { pointerId: 1, clientX: 400, clientY: 700, bubbles: true }).catch(() => {});
  await module(page, "/src/lib/player.svelte.ts", "m.player.scrubbing = true; m.player.position = 1000;");
  await evenement(page, "player-prop", { name: "time-pos", data: 101 });
  await evenement(page, "player-prop", { name: "time-pos", data: 102 });
  await page.waitForTimeout(200);
  expect(await position()).toBe(1000);
  // Relâchée : la position suit de nouveau la lecture.
  await module(page, "/src/lib/player.svelte.ts", "m.player.scrubbing = false;");
  await evenement(page, "player-prop", { name: "time-pos", data: 1001 });
  await expect.poll(position).toBe(1001);
});

test("bibliothèque : le filtre et le tri sont gardés par serveur, pas seulement par numéro de bibliothèque", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même règle sur tous les appareils");
  await ouvrir(page, info, "vide");
  const resultat = await module<Record<string, unknown>>(
    page,
    "/src/lib/libraryView.svelte.ts",
    `
    const nav = arg;
    const tri = { flag: null, value: null, sort: "year", desc: true };
    return (async () => {
      const { nav } = await import("/src/lib/nav.svelte.ts");
      const out = {};
      // Un choix gardé avant ce rangement, sous le seul numéro de la bibliothèque.
      localStorage.setItem("maku.libraryView", JSON.stringify({ "2": { flag: "unwatched", value: null, sort: null, desc: false } }));
      nav.serverId = "plex:a";
      m.saveView("1", tri);
      out.aSurA = m.loadView("1").sort;
      // Il revient au premier serveur qui ouvre la bibliothèque…
      out.ancienSurA = m.loadView("2").flag;
      nav.serverId = "plex:b";
      // … et à lui seul : l'autre serveur, qui a aussi une bibliothèque « 1 » et « 2 », part de zéro.
      out.aSurB = m.loadView("1").sort;
      out.ancienSurB = m.loadView("2").flag;
      m.saveView("1", { ...tri, sort: "title", desc: false });
      nav.serverId = "plex:a";
      out.aApresB = m.loadView("1").sort;
      out.cles = Object.keys(JSON.parse(localStorage.getItem("maku.libraryView"))).sort();
      nav.serverId = "plex:demo";
      return out;
    })();
    `,
  );
  expect(resultat).toEqual({
    aSurA: "year",
    ancienSurA: "unwatched",
    aSurB: null,
    ancienSurB: null,
    aApresB: "year",
    cles: ["plex:a/1", "plex:a/2", "plex:b/1"],
  });
});

test("fiche gardée en mémoire : oubliée quand le serveur ou le profil change", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même règle sur tous les appareils");
  await ouvrir(page, info, "fiche");
  const fiche = page.locator(".page.detail");
  await expect(fiche).toBeVisible();
  const cle = await module<string>(page, "/src/lib/nav.svelte.ts", "return m.nav.view.ratingKey;");
  const aller = (vue: object) => module(page, "/src/lib/nav.svelte.ts", "m.go(arg);", vue);
  // Le serveur met 1,5 s à redonner la fiche : seule la mémoire peut l'afficher tout de suite.
  await serveur(page, "plex_metadata", { delay: 1500 });
  await aller({ name: "home" });
  await aller({ name: "item", ratingKey: cle });
  await expect(fiche, "fiche gardée en mémoire au retour").toBeVisible({ timeout: 500 });
  // Changement de profil ou de serveur : la même fiche, redemandée, n'est plus affichée depuis la mémoire.
  await aller({ name: "home" });
  await module(page, "/src/lib/nav.svelte.ts", "m.scopeChanged();");
  await aller({ name: "item", ratingKey: cle });
  await page.waitForTimeout(400);
  await expect(fiche).toHaveCount(0);
  await expect(fiche).toBeVisible({ timeout: 4000 });
});
