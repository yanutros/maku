// Même fonction d'un appareil à l'autre : rien d'invisible mais actif au tactile, Retour et Échap ferment ce qui est
// ouvert, le menu d'un titre propose les mêmes actions à la TV et ailleurs, les touches de lecture des télécommandes
// agissent, et le téléphone règle ses bibliothèques.
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir, retour } from "./outils";

type Invoke = (cmd: string, args?: unknown) => Promise<unknown>;
const invoke = <T>(page: Page, cmd: string, args: unknown = {}) =>
  page.evaluate(([cmd, args]) => (window as unknown as { __TAURI_INTERNALS__: { invoke: Invoke } }).__TAURI_INTERNALS__.invoke(cmd as string, args), [cmd, args]) as Promise<T>;

/** Le module de navigation, tel que l'application le charge ; `code` le reçoit sous le nom `nav`, avec `arg`. */
const nav = <T>(page: Page, code: string, arg: unknown = null) =>
  page.evaluate(
    async ([code, arg]) => {
      const nav = await import("/src/lib/nav.svelte.ts");
      return new Function("nav", "arg", code as string)(nav, arg);
    },
    [code, arg],
  ) as Promise<T>;

type Titre = { ratingKey: string; title: string; kind: string };
const film = async (page: Page) => (await invoke<Titre[]>(page, "plex_search", { query: "" })).filter((m) => m.kind === "movie")[0];

/** Ouvre le menu d'un titre (clic droit sur ordinateur, appui long sur TV), sur un serveur `kind`. */
const menu = (page: Page, tv: boolean, kind: "plex" | "jellyfin", item: object) =>
  nav(page, `nav.nav.serverKind = arg.kind; if (arg.tv) nav.nav.tvMenu = { item: arg.item }; else nav.nav.menu = { item: arg.item, x: 200, y: 200 };`, { tv, kind, item });

test("tactile : le centre d'une affiche ouvre la fiche, sans bouton de lecture invisible", async ({ page }, info) => {
  test.skip(appareil(info) !== "telephone", "écran tactile");
  await ouvrir(page, info, "films");
  // Une jaquette lisible directement (film) : c'est elle qui porte le bouton de lecture.
  const carte = page.locator(".card:has(.img > .play)").first();
  await expect(carte).toBeVisible();
  // Le bouton de lecture ne se montrait qu'au survol, que le tactile n'a pas : il n'est plus là.
  await expect(carte.locator(".img > .play")).toBeHidden();
  // Un appui au milieu de l'affiche, là où se trouvait ce bouton.
  await carte.locator(".img").tap();
  await expect.poll(() => nav<string>(page, `return nav.nav.view.name;`)).toBe("item");
  expect(await page.evaluate(async () => (await import("/src/lib/player.svelte.ts")).player.active), "la lecture a démarré").toBe(false);
});

test("Retour et Échap ferment la fenêtre « Modifier la demande », sans quitter la page", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de gestion des demandes au téléviseur");
  await ouvrir(page, info, "demandes-admin&admin");
  const modifier = page.getByRole("button", { name: "Modifier" }).first();
  const fenetre = page.getByRole("dialog", { name: "Modifier la demande" });
  await modifier.click();
  await expect(fenetre).toBeVisible();
  await retour(page);
  await expect(fenetre).toHaveCount(0);
  // Toujours sur la page des demandes.
  await expect(modifier).toBeVisible();
  await modifier.click();
  await expect(fenetre).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(fenetre).toHaveCount(0);
  await expect(modifier).toBeVisible();
});

test("Échap ferme la fenêtre de demande d'un titre", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "clavier");
  await ouvrir(page, info, "decouvrir");
  const media = (await invoke<object[]>(page, "seerr_trending", { page: 1 }))[0];
  await nav(page, `nav.nav.requesting = arg;`, media);
  const fenetre = page.locator(".dialog[role=dialog]");
  await expect(fenetre).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(fenetre).toHaveCount(0);
});

test("TV : Retour ferme le QR code de la connexion à Tracearr", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "QR code : téléviseur");
  await ouvrir(page, info, "stats");
  await nav(page, `nav.nav.authUrl = "https://tracearr.exemple.invalid/auth";`);
  const qr = page.locator(".auth-qr");
  await expect(qr).toBeVisible();
  await retour(page);
  await expect(qr).toHaveCount(0);
  // La page des statistiques est toujours là : Retour n'a fermé que le QR code.
  expect(await nav<string>(page, `return nav.nav.view.name;`)).toBe("stats");
});

test("menu d'un titre : la liste de visionnage Plex, à la TV comme ailleurs", async ({ page }, info) => {
  test.skip(appareil(info) === "telephone", "le téléphone ouvre le même menu que l'ordinateur");
  const tv = appareil(info) === "tv";
  await ouvrir(page, info, "films");
  const item = { ...(await film(page)), logoGuid: "plex://movie/demo" };
  const entree = (nom: string) => page.getByRole("menuitem", { name: nom });
  await menu(page, tv, "plex", item);
  await expect(entree("Ajouter à la liste de visionnage")).toBeVisible();
  await entree("Ajouter à la liste de visionnage").click();
  await expect(page.getByRole("menu")).toHaveCount(0);
  await menu(page, tv, "plex", item);
  await expect(entree("Retirer de la liste de visionnage")).toBeVisible();
  await page.keyboard.press("Escape");
  if (tv) await nav(page, `nav.nav.tvMenu = null;`);
  // Jellyfin et Emby n'ont pas de liste de visionnage.
  await menu(page, tv, "jellyfin", item);
  await expect(page.getByRole("menu")).toBeVisible();
  await expect(entree("Ajouter à la liste de visionnage")).toHaveCount(0);
  await expect(entree("Retirer de la liste de visionnage")).toHaveCount(0);
});

test("TV : le menu d'un titre ne propose la note en étoiles qu'avec Plex, et ouvre les informations", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "menu du téléviseur");
  await ouvrir(page, info, "films");
  const item = await film(page);
  await menu(page, true, "plex", item);
  await expect(page.getByRole("menuitem", { name: "Évaluer" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Voir les informations" })).toBeVisible();
  await nav(page, `nav.nav.tvMenu = null;`);
  // Jellyfin et Emby n'ont qu'un « J'aime » : pas d'étoiles, qui y enregistraient autre chose.
  await menu(page, true, "jellyfin", item);
  await expect(page.getByRole("menu")).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Évaluer" })).toHaveCount(0);
  await page.getByRole("menuitem", { name: "Voir les informations" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
});

test("TV : le résumé d'une fiche a plus de trois lignes, faute de pouvoir le déplier", async ({ page }, info) => {
  await ouvrir(page, info, "fiche");
  const lignes = await page.locator(".summary").first().evaluate((el) => getComputedStyle(el).webkitLineClamp);
  expect(lignes).toBe(appareil(info) === "tv" ? "8" : "3");
});

test("lecteur : les touches lecture, pause, avance, retour et arrêt d'une télécommande agissent", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv" && appareil(info) !== "pc", "télécommande ou clavier multimédia");
  await ouvrir(page, info, "lecteur"); // la démo est en pause
  await page.evaluate(() => {
    const w = window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown, o: unknown) => Promise<unknown> }; sent: unknown[] };
    const invoke = w.__TAURI_INTERNALS__.invoke;
    w.sent = [];
    w.__TAURI_INTERNALS__.invoke = (c, a, o) => {
      if (c === "player_command") w.sent.push((a as { args: unknown[] }).args);
      return invoke(c, a, o);
    };
  });
  const touche = (key: string) => page.evaluate((key) => window.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true })), key);
  const envoyees = () => page.evaluate(() => (window as unknown as { sent: unknown[] }).sent);
  // En pause : « pause » ne fait rien, « lecture » relance.
  await touche("MediaPause");
  expect(await envoyees()).toEqual([]);
  await touche("MediaPlay");
  await touche("MediaFastForward");
  await touche("MediaRewind");
  expect(await envoyees()).toEqual([
    ["cycle", "pause"],
    ["seek", 30, "relative"],
    ["seek", -10, "relative"],
  ]);
  await touche("MediaStop");
  await expect(page.locator(".overlay")).toHaveCount(0);
});

test("téléphone : un onglet n'empile pas les pages visitées avant lui", async ({ page }, info) => {
  test.skip(appareil(info) !== "telephone", "barre d'onglets du téléphone");
  await ouvrir(page, info, "vide");
  const onglets = page.locator("nav.tabs button");
  for (const i of [1, 2, 3, 1]) {
    await onglets.nth(i).click();
    await expect(onglets.nth(i)).toHaveAttribute("aria-current", "page");
  }
  expect(await nav<number>(page, `return nav.nav.history.length;`)).toBe(0);
});

test("téléphone : les Réglages rangent et masquent les bibliothèques, sans les réglages du menu latéral", async ({ page }, info) => {
  test.skip(appareil(info) !== "telephone", "réglages du téléphone");
  await ouvrir(page, info, "reglages-interface");
  await expect(page.locator(".libs .lib").first()).toBeVisible();
  expect(await page.locator(".libs .lib").count()).toBeGreaterThan(1);
  // Pas de menu latéral sur téléphone : rien à replier.
  await expect(page.getByText("Menu replié")).toHaveCount(0);
  // Masquer une bibliothèque la retire de l'onglet Bibliothèques.
  const nom = (await page.locator(".libs .lib .lib-name").first().textContent())!.trim();
  await page.locator(".libs .lib").first().getByRole("button").last().click();
  await page.locator("nav.tabs button").nth(1).click();
  await expect(page.locator(".page .list")).toBeVisible();
  await expect(page.locator(".page .list").getByText(nom, { exact: true })).toHaveCount(0);
});
