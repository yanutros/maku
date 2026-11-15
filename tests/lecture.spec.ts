// Lecture : une lecture arrêtée ou remplacée pendant l'attente du serveur ne démarre pas « en fantôme », le plein
// écran se quitte quand la vidéo se ferme, et une session Tracearr expirée ne recharge pas l'application en boucle.
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

type Hooks = Record<string, { delay?: number | Record<string, number>; error?: string | Record<string, string> }>;

/**
 * Commandes retardées ou mises en échec (voir `__demoHooks`, demo.ts) : `delay` et `error` valent pour toute la
 * commande, ou seulement pour un titre (`{ [ratingKey]: … }`).
 */
const serveur = (page: Page, hooks: Hooks) =>
  page.evaluate((hooks) => {
    const out: Record<string, unknown> = {};
    for (const [cmd, h] of Object.entries(hooks)) {
      const { delay, error } = h;
      out[cmd] = {
        delay: typeof delay === "object" ? (a: { ratingKey: string }) => delay[a.ratingKey] ?? 0 : delay,
        error: typeof error === "object" ? (a: { ratingKey: string }) => error[a.ratingKey] ?? null : error,
      };
    }
    (window as unknown as { __demoHooks: unknown }).__demoHooks = out;
  }, hooks);

/** Le lecteur, tel que l'application le charge (même instance) ; `code` reçoit le module sous le nom `p`. */
const lecteur = <T>(page: Page, code: string) =>
  page.evaluate(async (code) => {
    const p = await import("/src/lib/player.svelte.ts");
    return new Function("p", code)(p);
  }, code) as Promise<T>;

/** État du lecteur. */
const etat = (page: Page) =>
  lecteur<{ active: boolean; video: boolean; session: string; title: string; ratingKey: string; premier: string | null }>(
    page,
    `const { active, video, session, title, ratingKey, queue } = p.player; return { active, video, session, title, ratingKey, premier: queue[0]?.ratingKey ?? null };`,
  );

/** Deux films de la démo. */
async function films(page: Page) {
  const list = await page.evaluate(
    () => (window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown) => Promise<{ ratingKey: string; title: string; kind: string }[]> } }).__TAURI_INTERNALS__.invoke("plex_search", { query: "" }),
  );
  const [a, b] = list.filter((m) => m.kind === "movie");
  expect(a.title).not.toBe(b.title);
  return { a, b };
}

test("lecture : deux films lancés coup sur coup, c'est le second qui joue, même si le premier répond en dernier", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "lecteur de l'ordinateur");
  await ouvrir(page, info, "vide");
  const { a, b } = await films(page);
  await serveur(page, { play: { delay: { [a.ratingKey]: 700, [b.ratingKey]: 50 } } });
  await lecteur(page, `p.play("${a.ratingKey}").catch(() => {}); p.play("${b.ratingKey}").catch(() => {});`);
  await expect.poll(async () => (await etat(page)).title).toBe(b.title);
  // La réponse du premier arrive ensuite : elle ne remplace rien.
  await page.waitForTimeout(1000);
  expect(await etat(page)).toMatchObject({ active: true, video: true, title: b.title, ratingKey: b.ratingKey });
  await expect(page.locator(".overlay")).toContainText(b.title);
});

test("lecture : arrêtée pendant l'attente du serveur, elle ne démarre pas ensuite", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "lecteur de l'ordinateur");
  await ouvrir(page, info, "vide");
  const { a } = await films(page);
  await serveur(page, { play: { delay: 700 } });
  await lecteur(page, `p.play("${a.ratingKey}").catch(() => {});`);
  await expect(page.locator(".overlay")).toBeVisible();
  await lecteur(page, `p.stop();`);
  await page.waitForTimeout(1000);
  // La réponse tardive n'a rien laissé : ni lecture, ni session, ni titre.
  expect(await etat(page)).toMatchObject({ active: false, session: "", title: "" });
  await expect(page.locator(".overlay")).toHaveCount(0);
});

test("lecture : l'échec d'un film remplacé ne referme pas le lecteur du suivant", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "lecteur de l'ordinateur");
  await ouvrir(page, info, "vide");
  const { a, b } = await films(page);
  await serveur(page, { play: { delay: { [a.ratingKey]: 700, [b.ratingKey]: 50 }, error: { [a.ratingKey]: "Serveur injoignable" } } });
  await lecteur(page, `window.echecs = []; p.play("${a.ratingKey}").catch((e) => window.echecs.push(String(e))); p.play("${b.ratingKey}").catch((e) => window.echecs.push(String(e)));`);
  await page.waitForTimeout(1200);
  expect(await etat(page)).toMatchObject({ active: true, video: true, title: b.title });
  await expect(page.locator(".overlay")).toBeVisible();
  // Et personne n'affiche l'erreur d'un film que l'on ne regarde plus.
  expect(await page.evaluate(() => (window as unknown as { echecs: string[] }).echecs)).toEqual([]);
});

test("lecture : un film lancé puis une musique, la réponse tardive du film ne touche pas à la musique", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "lecteur de l'ordinateur");
  await ouvrir(page, info, "ecoute");
  const musique = await etat(page);
  expect(musique).toMatchObject({ active: true, video: false, session: "demo-music" });
  const { a } = await films(page);
  await serveur(page, { play: { delay: 700 } });
  await lecteur(page, `p.play("${a.ratingKey}").catch(() => {}); p.playMusic("${musique.ratingKey || musique.premier}").catch(() => {});`);
  await page.waitForTimeout(1200);
  expect(await etat(page)).toMatchObject({ active: true, video: false, session: "demo-music", title: "" });
});

test("lecture : une musique arrêtée pendant l'attente du serveur ne remplit pas la file ensuite", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "lecteur de l'ordinateur");
  await ouvrir(page, info, "ecoute");
  const album = (await etat(page)).premier!;
  await lecteur(page, `p.stop();`);
  await serveur(page, { play_music: { delay: 700 } });
  await lecteur(page, `p.playMusic("${album}").catch(() => {});`);
  await page.waitForTimeout(100);
  await lecteur(page, `p.stop();`);
  await page.waitForTimeout(1000);
  expect(await etat(page)).toMatchObject({ active: false, session: "", premier: null });
});

test("lecteur : la vidéo fermée autrement que par son bouton quitte le plein écran", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "plein écran de l'ordinateur");
  await ouvrir(page, info, "lecteur");
  // Chaque changement de plein écran demandé à la fenêtre (la démo n'en a pas).
  await page.evaluate(() => {
    const w = window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown, o: unknown) => Promise<unknown> }; plein: boolean[] };
    const invoke = w.__TAURI_INTERNALS__.invoke;
    w.plein = [];
    w.__TAURI_INTERNALS__.invoke = (c, a, o) => {
      if (c === "plugin:window|set_fullscreen") w.plein.push((a as { value: boolean }).value);
      return invoke(c, a, o);
    };
  });
  const plein = () => page.evaluate(() => (window as unknown as { plein: boolean[] }).plein);
  await page.keyboard.press("f");
  await expect.poll(plein).toEqual([true]);
  // Fin du film, bouton précédent de la souris, touche Stop : la lecture s'arrête sans passer par « Fermer ».
  await lecteur(page, `p.stop();`);
  await expect(page.locator(".overlay")).toHaveCount(0);
  await expect.poll(plein).toEqual([true, false]);
});

test("statistiques : une session Tracearr expirée ne recharge l'application qu'une fois, et le rafraîchissement s'arrête", async ({ page }, info) => {
  await page.clock.install();
  await ouvrir(page, info, "stats");
  await expect(page.locator(".top")).toBeVisible();
  // Appels comptés à partir d'ici : la session relue par l'application, et le tableau de Tracearr.
  await page.evaluate(() => {
    const w = window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown, o: unknown) => Promise<unknown> }; appels: Record<string, number> };
    const invoke = w.__TAURI_INTERNALS__.invoke;
    w.appels = { get_session: 0, tracearr_dashboard: 0 };
    w.__TAURI_INTERNALS__.invoke = (c, a, o) => {
      if (c in w.appels) w.appels[c]++;
      return invoke(c, a, o);
    };
  });
  const appels = () => page.evaluate(() => (window as unknown as { appels: Record<string, number> }).appels);
  await serveur(page, { tracearr_dashboard: { error: "SESSION_EXPIRED" } });
  // Premier rafraîchissement (15 s) : la session a expiré, l'application relit la sienne (le menu aussi, quand il y en a un).
  await page.clock.fastForward(16_000);
  await expect.poll(async () => (await appels()).get_session).toBeGreaterThan(0);
  await page.waitForTimeout(300);
  const apres = await appels();
  expect(apres.tracearr_dashboard).toBe(1);
  expect(apres.get_session).toBeLessThanOrEqual(2);
  // Une minute de plus : plus aucun appel, ni à Tracearr ni pour relire la session.
  for (let i = 0; i < 4; i++) await page.clock.fastForward(16_000);
  await page.waitForTimeout(300);
  expect(await appels()).toEqual(apres);
});
