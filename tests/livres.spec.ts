// Kavita (livres, BD, mangas) : connexion dans Services, bibliothèques avec les autres (pas au téléviseur), page
// d'une série, lecteur d'images (sens de lecture, progression, chapitre suivant) et lecteur de livres EPUB.
import { expect, test, type Locator, type Page, type TestInfo } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

const sauvegardes = (page: Page) =>
  page.evaluate(() => (window as unknown as { __kavitaSaved: Record<string, unknown>[] }).__kavitaSaved.map((p) => ({ chapterId: p.chapterId, page: p.page })));

async function ouvrirMangas(page: Page) {
  await page.evaluate(async () => {
    const nav = await import("/src/lib/nav.svelte.ts");
    nav.go({ name: "section", key: "kavita:1", title: "Mangas", kind: "book" });
  });
  await expect(page.getByRole("heading", { name: "Mangas", exact: true })).toBeVisible();
}

test("les bibliothèques Kavita rejoignent les autres, sauf au téléviseur", async ({ page }, info) => {
  await ouvrir(page, info, "accueil&kavita");
  const menu = appareil(info) === "telephone" ? null : page.getByRole("navigation");
  if (appareil(info) === "tv") {
    await expect(page.getByRole("button", { name: "Films" }).first()).toBeAttached();
    await expect(page.getByRole("button", { name: "Mangas" })).toHaveCount(0);
  } else if (menu) {
    await expect(menu.getByRole("button", { name: "Mangas" })).toBeVisible();
    await expect(menu.getByRole("button", { name: "Romans" })).toBeVisible();
  }
  // Pas de rangée Kavita sur l'accueil : elles ont leur propre page.
  await expect(page.getByRole("heading", { name: /Mangas/ })).toHaveCount(0);
  const rangees = await page.evaluate(async () =>
    (await import("/src/lib/homeRows.svelte.ts")).availableRows([
      { key: "1", kind: "movie" },
      { key: "kavita:1", kind: "book" },
    ]),
  );
  expect(rangees).toContain("lib:1");
  expect(rangees).not.toContain("lib:kavita:1");
});

/** Ouvre la série de mangas entamée à sa page de reprise (la 6e). */
async function ouvrirManga(page: Page, info: TestInfo) {
  await ouvrir(page, info, "accueil&kavita");
  await ouvrirMangas(page);
  await page.getByRole("button", { name: "Le Gardien des Marées" }).first().click();
  await page.getByRole("button", { name: "Continuer" }).click();
  const lecteur = page.getByRole("dialog");
  await expect(lecteur.getByRole("img", { name: "Page 6" })).toBeVisible();
  return lecteur;
}

/** Ouvre le panneau Affichage du lecteur et y choisit `choix` (sens, ajustement, pages). */
async function affichage(lecteur: Locator, choix: string) {
  const panneau = lecteur.getByRole("group", { name: "Affichage" });
  if (!(await panneau.isVisible())) await lecteur.getByRole("button", { name: "Affichage" }).click();
  await panneau.getByRole("radio", { name: choix, exact: true }).click();
}

const compte = (lecteur: Locator) => lecteur.getByText(/^\d+(-\d+)? \/ 20$/);
const transformation = (page: Page) => page.locator(".reader .view").evaluate((el) => getComputedStyle(el).transform);

test("série de manga : reprise à la bonne page, de droite à gauche, progression envoyée", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  await ouvrir(page, info, "accueil&kavita");
  await ouvrirMangas(page);
  // « Continuer la lecture » : la série entamée.
  await expect(page.getByRole("heading", { name: "Continuer la lecture" })).toBeVisible();
  await page.getByRole("button", { name: "Le Gardien des Marées" }).first().click();
  await expect(page.getByRole("heading", { name: "Le Gardien des Marées" })).toBeVisible();
  await expect(page.getByText("Volume 2")).toBeVisible();

  await page.getByRole("button", { name: "Continuer" }).click();
  const lecteur = page.getByRole("dialog");
  // Reprise à la page où on s'était arrêté (5 pages lues : la 6e).
  await expect(lecteur.getByRole("img", { name: "Page 6" })).toBeVisible();
  await lecteur.getByRole("button", { name: "Affichage" }).click();
  await expect(lecteur.getByRole("radio", { name: "Droite → gauche" })).toHaveAttribute("aria-checked", "true");
  // Une page à la fois pour la suite.
  await affichage(lecteur, "Simple");
  await page.keyboard.press("Escape");
  await expect(lecteur.getByRole("group", { name: "Affichage" })).toHaveCount(0);
  // Manga : la flèche de gauche avance.
  await page.keyboard.press("ArrowLeft");
  await expect(lecteur.getByRole("img", { name: "Page 7" })).toBeVisible();
  await expect.poll(() => sauvegardes(page)).toContainEqual({ chapterId: 1213, page: 6 });

  // Gauche à droite : retenu pour la série, la flèche de droite avance.
  await affichage(lecteur, "Gauche → droite");
  await page.keyboard.press("Escape");
  await page.keyboard.press("ArrowRight");
  await expect(lecteur.getByRole("img", { name: "Page 8" })).toBeVisible();
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem("maku.readerDirection") ?? "{}"))).toEqual({ "lib:1": "ltr" });

  // Dernière page, puis la suivante : chapitre fini, le chapitre suivant s'ouvre au début.
  await page.keyboard.press("End");
  await expect(lecteur.getByRole("img", { name: "Page 20" })).toBeVisible();
  await page.keyboard.press("ArrowRight");
  await expect(lecteur.getByText("Chapitre 4")).toBeVisible();
  await expect(lecteur.getByRole("img", { name: "Page 1" })).toBeVisible();
  await expect.poll(() => sauvegardes(page)).toContainEqual({ chapterId: 1213, page: 20 });

  // Fermer : la progression du chapitre en cours part tout de suite.
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("Escape");
  await expect(lecteur).toHaveCount(0);
  // Sans attendre le délai de 800 ms des changements de page.
  await expect.poll(() => sauvegardes(page), { timeout: 400 }).toContainEqual({ chapterId: 1214, page: 1 });
});

test("page entière dans l'écran ; deux pages côte à côte sur un écran large, la double page seule", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  const lecteur = await ouvrirManga(page, info);
  const ecran = page.viewportSize()!;
  // Les pages de la démo sont bien plus hautes que l'écran : elles tiennent quand même entières.
  for (const img of await lecteur.getByRole("img").all()) {
    const r = (await img.boundingBox())!;
    expect(r.y).toBeGreaterThanOrEqual(-1);
    expect(r.y + r.height).toBeLessThanOrEqual(ecran.height + 1);
    expect(r.x + r.width).toBeLessThanOrEqual(ecran.width + 1);
  }
  if (ecran.width <= ecran.height) {
    // Écran en hauteur : une page à la fois.
    await expect(compte(lecteur)).toHaveText("6 / 20");
    await expect(lecteur.getByRole("img")).toHaveCount(1);
    return;
  }
  // Écran large : la 6e et la 7e côte à côte ; en manga, la 6e à droite.
  await expect(compte(lecteur)).toHaveText("6-7 / 20");
  const [six, sept] = [(await lecteur.getByRole("img", { name: "Page 6" }).boundingBox())!, (await lecteur.getByRole("img", { name: "Page 7" }).boundingBox())!];
  expect(sept.x).toBeLessThan(six.x);
  // La 10e est une double page : seule ; les suivantes reprennent par deux.
  await page.keyboard.press("ArrowLeft");
  await expect(compte(lecteur)).toHaveText("8-9 / 20");
  await page.keyboard.press("ArrowLeft");
  await expect(compte(lecteur)).toHaveText("10 / 20");
  await page.keyboard.press("ArrowLeft");
  await expect(compte(lecteur)).toHaveText("11-12 / 20");
  // La progression part à chaque paire (sa première page, comme Kavita).
  await expect.poll(() => sauvegardes(page)).toContainEqual({ chapterId: 1213, page: 10 });
  // « Simple » : une page à la fois, retenu pour la série.
  await affichage(lecteur, "Simple");
  await expect(compte(lecteur)).toHaveText("11 / 20");
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem("maku.readerLayout") ?? "{}"))).toEqual({ "lib:1": { spread: "single", fit: "screen", brightness: 100 } });
  // Appariement comme Kavita : couverture seule, double page seule, puis par deux.
  expect(await page.evaluate(async () => (await import("/src/lib/books.svelte.ts")).spreads(7, [false, false, false, true]))).toEqual([[0], [1, 2], [3], [4, 5], [6]]);
});

test("zoom : double-clic, Ctrl + molette, glisser ; zoomé, les côtés ne tournent pas la page", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  const lecteur = await ouvrirManga(page, info);
  const ecran = page.viewportSize()!;
  const avant = await compte(lecteur).textContent();
  expect(await transformation(page)).toBe("none");
  // Double-clic : zoom sur l'endroit visé, sans tourner la page.
  await page.mouse.dblclick(ecran.width * 0.4, ecran.height * 0.4);
  await expect.poll(() => transformation(page)).toMatch(/^matrix\(2\.5, 0, 0, 2\.5,/);
  await page.waitForTimeout(400);
  await expect(compte(lecteur)).toHaveText(avant!);
  // Un clic sur un côté : la page ne tourne pas.
  await page.mouse.click(ecran.width * 0.95, ecran.height * 0.5);
  await page.waitForTimeout(400);
  await expect(lecteur.getByRole("img", { name: "Page 6" })).toBeVisible();
  // Glisser déplace la page zoomée.
  const zoome = await transformation(page);
  await page.mouse.move(ecran.width * 0.5, ecran.height * 0.5);
  await page.mouse.down();
  await page.mouse.move(ecran.width * 0.5 + 80, ecran.height * 0.5 + 60, { steps: 4 });
  await page.mouse.up();
  await expect.poll(() => transformation(page)).not.toBe(zoome);
  // Échap revient à la page entière, sans fermer le lecteur.
  await page.keyboard.press("Escape");
  await expect.poll(() => transformation(page)).toBe("none");
  await expect(lecteur).toBeVisible();
  // Ctrl + molette zoome aussi ; tourner la page revient à la page entière.
  await page.mouse.move(ecran.width * 0.5, ecran.height * 0.5);
  await page.keyboard.down("Control");
  await page.mouse.wheel(0, -100);
  await page.keyboard.up("Control");
  await expect.poll(() => transformation(page)).not.toBe("none");
  await page.keyboard.press("ArrowLeft");
  await expect.poll(() => transformation(page)).toBe("none");
  // (Le clic sur le côté a masqué les commandes : la page se reconnaît à son image.)
  await expect(lecteur.getByRole("img", { name: "Page 6" })).toHaveCount(0);
});

test("Largeur : la page défile, puis passe à la suivante", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  test.skip(appareil(info) === "telephone", "écran en hauteur : la page en largeur y tient déjà");
  const lecteur = await ouvrirManga(page, info);
  await affichage(lecteur, "Largeur");
  await page.keyboard.press("Escape");
  await expect(compte(lecteur)).toHaveText("6 / 20");
  const scene = page.locator(".reader .stage");
  const defile = () => scene.evaluate((el) => el.scrollTop);
  // La page dépasse l'écran : la flèche du bas la fait descendre, sans changer de page.
  await expect.poll(() => scene.evaluate((el) => el.scrollHeight > el.clientHeight * 1.2)).toBe(true);
  await page.keyboard.press("ArrowDown");
  await expect.poll(defile).toBeGreaterThan(50);
  await expect(compte(lecteur)).toHaveText("6 / 20");
  // Arrivé en bas : la page suivante, depuis son haut.
  await scene.evaluate((el) => (el.scrollTop = el.scrollHeight));
  await page.keyboard.press("ArrowDown");
  await expect(compte(lecteur)).toHaveText("7 / 20");
  await expect.poll(defile).toBe(0);
  // En remontant, la page précédente reprend par son bas.
  await page.keyboard.press("ArrowUp");
  await expect(compte(lecteur)).toHaveText("6 / 20");
  await expect.poll(defile).toBeGreaterThan(50);
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem("maku.readerLayout") ?? "{}"))).toEqual({ "lib:1": { spread: "auto", fit: "width", brightness: 100 } });
});

test("défilement vertical : toutes les pages à la suite", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  const lecteur = await ouvrirManga(page, info);
  await affichage(lecteur, "Vertical");
  await page.keyboard.press("Escape");
  await expect(lecteur.getByRole("img")).toHaveCount(20);
  await expect(lecteur.getByRole("button", { name: "Chapitre suivant" }).first()).toBeAttached();
});

async function ouvrirLivre(page: Page, info: TestInfo) {
  await ouvrir(page, info, "accueil&kavita");
  await page.evaluate(async () => {
    const nav = await import("/src/lib/nav.svelte.ts");
    nav.go({ name: "book", id: 21 });
  });
  await expect(page.getByRole("heading", { name: "Le Phare de l'Ouest" })).toBeVisible();
  // Pas de bouton « Retour » dans la page : celui de la barre de Maku suffit.
  await expect(page.locator(".page.detail").getByRole("button", { name: "Retour" })).toHaveCount(0);
  await page.getByRole("button", { name: "Lire", exact: true }).click();
  const lecteur = page.getByRole("dialog");
  await expect(lecteur.getByText(/^Section \d sur 4/)).toBeVisible();
  return lecteur;
}

/** Compteur du bas : section, puis écran dans la section (« Section 2 sur 4 · 1/5 »). */
const compteur = (lecteur: Locator) => lecteur.getByText(/^Section \d sur 4/);

test("livre EPUB : sections, pages tournées, sommaire, note et progression", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  await ouvrirLivre(page, info);
  const lecteur = page.getByRole("dialog");
  // Kavita reprend à la section 2 : son texte, au premier écran.
  await expect(compteur(lecteur)).toHaveText(/^Section 2 sur 4 · 1\/\d+$/);
  await expect(lecteur.getByText(/^Paragraphe 2\.1\./)).toBeInViewport();

  // Tourner la page : l'écran suivant de la même section, puis retour.
  await page.keyboard.press("ArrowRight");
  await expect(compteur(lecteur)).toHaveText(/^Section 2 sur 4 · 2\/\d+$/);
  await expect(lecteur.getByText(/^Paragraphe 2\.1\./)).not.toBeInViewport();
  await page.keyboard.press("ArrowLeft");
  await expect(compteur(lecteur)).toHaveText(/^Section 2 sur 4 · 1\/\d+$/);
  // Avant le premier écran : la fin de la section précédente.
  await page.keyboard.press("ArrowLeft");
  await expect(compteur(lecteur)).toHaveText(/^Section 1 sur 4/);
  const [ecran, total] = ((await compteur(lecteur).textContent()) ?? "").split("· ")[1].split("/");
  expect(ecran).toBe(total);

  // Sommaire : l'ouverture, nettoyée (ni script ni « onerror » : rien ne s'exécute).
  await lecteur.getByRole("button", { name: "Sommaire" }).click();
  await lecteur.getByRole("navigation", { name: "Sommaire" }).getByRole("button", { name: "Ouverture" }).click();
  await expect(lecteur.getByRole("heading", { name: "Le Phare de l'Ouest" })).toBeInViewport();
  const propre = await page.evaluate(() => {
    const root = document.querySelector(".reader .book")!.shadowRoot!;
    return {
      script: root.querySelectorAll("script").length,
      onerror: root.querySelectorAll("[onerror]").length,
      execute: (window as unknown as { __epubPwned?: number }).__epubPwned ?? null,
    };
  });
  expect(propre).toEqual({ script: 0, onerror: 0, execute: null });

  // Lien interne du livre : la note, dans la section 3, à l'écran qui la contient.
  await lecteur.getByText("Voir la note").click();
  await expect(compteur(lecteur)).toHaveText(/^Section 3 sur 4/);
  await expect(lecteur.getByText("La note du gardien.")).toBeInViewport();
  await expect.poll(() => sauvegardes(page)).toContainEqual({ chapterId: 2113, page: 2 });

  // Dernière section, courte : le livre est lu (4 sections sur 4), puis le suivant de la série s'ouvre.
  await lecteur.getByRole("button", { name: "Sommaire" }).click();
  await lecteur.getByRole("navigation", { name: "Sommaire" }).getByRole("button", { name: "Épilogue" }).click();
  await expect(compteur(lecteur)).toHaveText(/^Section 4 sur 4/);
  await expect.poll(() => sauvegardes(page)).toContainEqual({ chapterId: 2113, page: 4 });
  await page.keyboard.press("ArrowRight");
  await expect(lecteur.getByText("Le Phare de l'Ouest — Chapitre 4")).toBeVisible();
});

test("livre EPUB : taille, thème et défilement retenus, reprise au même écran", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  const lecteur = await ouvrirLivre(page, info);
  // Deux écrans plus loin, on ferme : la réouverture revient au même endroit.
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  await expect(compteur(lecteur)).toHaveText(/· 3\/\d+$/);
  await page.keyboard.press("Escape");
  await expect(lecteur).toHaveCount(0);
  await page.getByRole("button", { name: "Lire", exact: true }).click();
  await expect(compteur(lecteur)).toHaveText(/^Section 2 sur 4 · 3\/\d+$/);

  await lecteur.getByRole("button", { name: "Affichage" }).click();
  const taille = lecteur.getByRole("group", { name: "Affichage" });
  await taille.getByRole("button", { name: "Texte plus grand" }).click();
  await expect(taille.getByText("110 %")).toBeVisible();
  await taille.getByRole("radio", { name: "Sépia" }).click();
  await expect(lecteur).toHaveCSS("background-color", "rgb(244, 236, 216)");
  await taille.getByRole("radio", { name: "Défilement" }).click();
  // Défilement : plus d'écrans, la section se lit d'un seul tenant.
  await expect(compteur(lecteur)).toHaveText("Section 2 sur 4");
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem("maku.epubReader") ?? "{}"))).toEqual({
    "lib:2": { layout: "scroll", columns: 2, size: 110, theme: "sepia", font: "book", lineHeight: 1.6, margin: 5 },
    last: { layout: "scroll", columns: 2, size: 110, theme: "sepia", font: "book", lineHeight: 1.6, margin: 5 },
  });

  // Retenus pour la lecture suivante.
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect(lecteur).toHaveCount(0);
  await page.getByRole("button", { name: "Lire", exact: true }).click();
  await expect(compteur(lecteur)).toHaveText("Section 2 sur 4");
  await expect(lecteur).toHaveCSS("background-color", "rgb(244, 236, 216)");
});

test("Réglages › Services : connecter Kavita", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  await ouvrir(page, info, "reglages");
  await page.evaluate(async () => {
    const nav = await import("/src/lib/nav.svelte.ts");
    nav.go({ name: "settings", page: "services" });
  });
  await page.getByLabel("Adresse de Kavita").fill("http://kavita.lan:5000/home");
  const connecter = page.getByRole("button", { name: "Connecter Kavita" });
  await expect(connecter).toBeDisabled();
  await page.getByLabel("Clé d'API Kavita").fill("une-cle-de-test");
  await connecter.click();
  await expect(page.getByText("Kavita connecté")).toBeVisible();
  // L'adresse retenue : la racine de Kavita, sans la page d'accueil.
  await expect(page.getByLabel("Adresse de Kavita")).toHaveValue("http://kavita.lan:5000");
});

// --- Kavita par Maku Server : un compte par utilisateur, l'administrateur choisit qui lit quoi ---

const accords = (page: Page) => page.evaluate(() => (window as unknown as { __kavitaGrants: { id: string; libraries: number[] }[] }).__kavitaGrants.map((g) => ({ id: g.id, libraries: g.libraries })));

test("Maku Server : Administration › Livres, ouvrir puis retirer des bibliothèques à un utilisateur", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  await ouvrir(page, info, "accueil&kavitarelay&admin");
  // Les bibliothèques du compte arrivent sans rien régler.
  if (appareil(info) !== "telephone") await expect(page.getByRole("navigation").getByRole("button", { name: "Mangas" })).toBeVisible();
  // Admin › Livres : dans le menu sur ordinateur, sous les bibliothèques au téléphone.
  if (appareil(info) === "telephone") {
    await page.locator("nav.tabs button", { hasText: "Bibliothèques" }).click();
    await page.locator("button.library", { hasText: "Livres" }).click();
  } else {
    await page.locator("button.item", { hasText: "Livres" }).click();
  }
  await expect(page.getByRole("heading", { name: "Livres", level: 1 })).toBeVisible();
  // Comme Dashboard › Utilisateurs : la liste des comptes, puis la fiche de celui qu'on choisit.
  const liste = page.locator(".users .list");
  await expect(liste.getByRole("button", { name: /Hugo.*Aucun accès/ })).toBeVisible();
  // Un ancien utilisateur qui a encore un compte, pour pouvoir le lui retirer.
  await expect(liste.getByRole("button", { name: /Ancien ami.*n'est plus sur le serveur/ })).toBeVisible();
  await liste.getByRole("button", { name: /Hugo/ }).click();
  const hugo = page.getByRole("group", { name: "Bibliothèques Kavita de Hugo" });
  await expect(page.getByText("Pas encore de compte Kavita")).toBeVisible();
  // Un interrupteur par bibliothèque : le premier allumé crée son compte (au téléphone, la liste est cachée derrière la fiche).
  await hugo.getByRole("checkbox", { name: /Romans/ }).check();
  await expect(liste.getByRole("button", { name: /Hugo.*1 bibliothèque/, includeHidden: true })).toBeAttached();
  await expect.poll(() => accords(page)).toContainEqual({ id: "plex:102", libraries: [2] });
  await expect(page.getByText("Compte Kavita : Hugo.102")).toBeVisible();
  await hugo.getByRole("checkbox", { name: /Mangas/ }).check();
  await expect(liste.getByRole("button", { name: /Hugo.*2 bibliothèques/, includeHidden: true })).toBeAttached();
  await expect.poll(() => accords(page)).toContainEqual({ id: "plex:102", libraries: [1, 2] });
  // « Retirer l'accès » : plus aucune bibliothèque, le compte reste.
  if (appareil(info) === "telephone") await page.getByRole("button", { name: "Retour" }).first().click();
  await liste.getByRole("button", { name: /Ancien ami/ }).click();
  await page.getByRole("button", { name: "Retirer l'accès" }).click();
  await expect(liste.getByRole("button", { name: /Ancien ami.*Aucun accès/, includeHidden: true })).toBeAttached();
  await expect.poll(() => accords(page)).toContainEqual({ id: "plex:900", libraries: [] });
});

test("Maku Server : Kavita fourni, rien à saisir dans les Réglages ; pas d'onglet Livres sans lui", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  await ouvrir(page, info, "reglages&kavitarelay");
  await page.evaluate(async () => {
    const nav = await import("/src/lib/nav.svelte.ts");
    nav.go({ name: "settings", page: "services" });
  });
  await expect(page.getByText("Fourni par Maku Server")).toBeVisible();
  await expect(page.getByLabel("Clé d'API Kavita")).toHaveCount(0);

  // Kavita saisi à la main : pas d'administration des livres.
  await ouvrir(page, info, "accueil&kavita&admin");
  if (appareil(info) === "telephone") await page.locator("nav.tabs button", { hasText: "Bibliothèques" }).click();
  await expect(page.getByRole("button", { name: "Dashboard" }).first()).toBeVisible();
  await expect(page.getByRole("button", { name: "Livres", exact: true })).toHaveCount(0);
  await page.evaluate(async () => {
    const nav = await import("/src/lib/nav.svelte.ts");
    nav.go({ name: "admin", page: "books" });
  });
  await expect(page.locator(".page .muted").first()).toBeVisible();
  await expect(page.getByRole("checkbox")).toHaveCount(0);
});

test("Maku Server : son Kavita remplace celui saisi à la main", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  // Kavita saisi à la main à l'étape d'avant ; le relais du serveur en propose un : c'est lui qui sert.
  await ouvrir(page, info, "accueil&kavitamain&kavitarelay&admin");
  if (appareil(info) === "telephone") await page.locator("nav.tabs button", { hasText: "Bibliothèques" }).click();
  await expect(page.getByRole("button", { name: "Livres", exact: true })).toBeVisible();
  await page.evaluate(async () => {
    const nav = await import("/src/lib/nav.svelte.ts");
    nav.go({ name: "settings", page: "services" });
  });
  await expect(page.getByText("Fourni par Maku Server")).toBeVisible();
});

test("série : son fond couvre la fenêtre, sans bande sombre contre le menu en haut", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  await ouvrir(page, info, "accueil&kavita");
  await page.evaluate(async () => {
    const nav = await import("/src/lib/nav.svelte.ts");
    nav.go({ name: "book", id: 12 });
  });
  await expect(page.getByRole("heading", { name: "Le Gardien des Marées" })).toBeVisible();
  // Le dégradé du haut ne couvre que la page : sur un fond qui va jusqu'au bord, il s'arrêtait net contre le menu.
  expect(await page.locator("main > header").evaluate((h) => getComputedStyle(h).backgroundImage)).toBe("none");
  // Le fond lui-même passe derrière le menu (et non sous le fond de la fenêtre).
  expect(await page.locator(".backdrop").evaluate((b) => getComputedStyle(b).zIndex)).toBe("0");
});

/** Bord droit de chaque groupe de boutons d'un panneau Affichage (ils doivent tous finir au même endroit). */
const bordsDroits = (panneau: Locator) =>
  panneau.locator(".modes, .size, .control, .slider").evaluateAll((els) => els.map((e) => Math.round(e.getBoundingClientRect().right)));

test("panneaux Affichage : tous les boutons alignés à droite, chaque réglage sur sa ligne à l'ordinateur", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  const lecteur = await ouvrirManga(page, info);
  // Texte élargi, comme avec une police plus large (Linux) ou une langue plus longue : la largeur doit suivre.
  await page.addStyleTag({ content: ".reader * { letter-spacing: 2.5px !important; }" });
  await lecteur.getByRole("button", { name: "Affichage" }).click();
  const bd = lecteur.getByRole("group", { name: "Affichage" });
  await expect(bd.getByRole("radiogroup")).toHaveCount(3);
  const droites = await bordsDroits(bd);
  expect(new Set(droites).size, `bords droits : ${droites}`).toBe(1);
  if (appareil(info) !== "telephone") {
    // Le libellé et ses boutons sur la même ligne.
    for (const groupe of await bd.locator(".setting").all()) {
      const [libelle, boutons] = [(await groupe.locator("span, label").first().boundingBox())!, (await groupe.locator(".modes, .slider").first().boundingBox())!];
      expect(boutons.y).toBeLessThan(libelle.y + libelle.height);
    }
  }
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");

  // Le lecteur EPUB : même règle.
  const livre = await ouvrirLivre(page, info);
  await livre.getByRole("button", { name: "Affichage" }).click();
  const epub = livre.getByRole("group", { name: "Affichage" });
  const droitesEpub = await bordsDroits(epub);
  // Police, taille, interligne, marges, thème, présentation.
  expect(droitesEpub.length).toBe(6);
  expect(new Set(droitesEpub).size, `bords droits : ${droitesEpub}`).toBe(1);
});

test("réglages retenus par bibliothèque : une autre série de la même bibliothèque les reprend", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  const lecteur = await ouvrirManga(page, info);
  // Mangas : de droite à gauche par défaut ; on passe en gauche à droite, page simple.
  await affichage(lecteur, "Gauche → droite");
  await affichage(lecteur, "Simple");
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect(lecteur).toHaveCount(0);
  // Une autre série de la bibliothèque Mangas : mêmes réglages, sans les refaire.
  await page.evaluate(async () => {
    const nav = await import("/src/lib/nav.svelte.ts");
    nav.go({ name: "book", id: 11 });
  });
  await page.getByRole("button", { name: /^(Lire|Continuer)$/ }).click();
  await lecteur.getByRole("button", { name: "Affichage" }).click();
  await expect(lecteur.getByRole("radio", { name: "Gauche → droite" })).toHaveAttribute("aria-checked", "true");
  await expect(lecteur.getByRole("radio", { name: "Simple" })).toHaveAttribute("aria-checked", "true");
});

test("livre : la taille du texte change vraiment, même quand le livre impose la sienne", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  const lecteur = await ouvrirLivre(page, info);
  const taille = () => lecteur.getByText(/^Paragraphe 2\.1\./).evaluate((p) => parseFloat(getComputedStyle(p).fontSize));
  const avant = await taille();
  await lecteur.getByRole("button", { name: "Affichage" }).click();
  const panneau = lecteur.getByRole("group", { name: "Affichage" });
  for (let i = 0; i < 3; i++) await panneau.getByRole("button", { name: "Texte plus grand" }).click();
  await expect(panneau.getByText("130 %")).toBeVisible();
  await expect.poll(taille).toBeCloseTo(avant * 1.3, 0);
  for (let i = 0; i < 6; i++) await panneau.getByRole("button", { name: "Texte plus petit" }).click();
  await expect(panneau.getByText("70 %")).toBeVisible();
  await expect.poll(taille).toBeCloseTo(avant * 0.7, 0);
});

test("livre : le texte reste dans la page, avec une marge à droite comme à gauche", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  const lecteur = await ouvrirLivre(page, info);
  const ecran = page.viewportSize()!;
  // Ce que l'œil voit : le texte de l'écran, et rien dans la marge de droite (la colonne de l'écran suivant, déjà à
  // côté, y débordait, coupée par le bord de la fenêtre).
  const vu = () =>
    page.evaluate((largeur) => {
      const root = document.querySelector(".reader .book")!.shadowRoot!;
      const fragments = [...root.querySelectorAll("p")].flatMap((p) => [...p.getClientRects()]).filter((r) => r.width > 0);
      // Les morceaux de paragraphe que l'on peut toucher à l'écran : ceux qui ne sont pas cachés.
      const visibles = fragments.filter((r) => {
        const x = Math.min(largeur - 1, Math.max(0, r.left + 4));
        const y = r.top + Math.min(8, r.height / 2);
        return root.elementFromPoint(x, y)?.closest("p") !== null && r.left < largeur;
      });
      // La marge de droite : rien d'un paragraphe ne doit s'y trouver.
      const droite = Math.max(...visibles.map((r) => r.right));
      let dansLaMarge = 0;
      for (let y = 140; y < window.innerHeight - 140; y += 24) {
        for (let x = Math.ceil(droite) + 4; x < largeur - 2; x += 6) if (root.elementFromPoint(x, y)?.closest("p")) dansLaMarge++;
      }
      return { gauche: Math.min(...visibles.map((r) => r.left)), droite, dansLaMarge };
    }, ecran.width);
  for (const ecranSuivant of [false, true]) {
    if (ecranSuivant) await page.keyboard.press("ArrowRight");
    await page.waitForTimeout(200);
    const { gauche, droite, dansLaMarge } = await vu();
    expect(dansLaMarge, "texte visible dans la marge de droite").toBe(0);
    expect(droite, `texte jusqu'à ${droite} px sur ${ecran.width}`).toBeLessThanOrEqual(ecran.width - 16);
    // Marges égales à gauche et à droite, comme dans Kavita (le texte justifié remplit sa colonne).
    expect(Math.abs(ecran.width - droite - gauche), `marges ${gauche} et ${ecran.width - droite}`).toBeLessThanOrEqual(3);
  }
});

test("livre : police, interligne, marges et présentation, comme dans Kavita, retenus pour la bibliothèque", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  const lecteur = await ouvrirLivre(page, info);
  const paragraphe = lecteur.getByText(/^Paragraphe 2\.1\./);
  const style = () => paragraphe.evaluate((p) => ({ police: getComputedStyle(p).fontFamily, interligne: parseFloat(getComputedStyle(p).lineHeight) / parseFloat(getComputedStyle(p).fontSize), gauche: Math.round(p.getBoundingClientRect().left) }));
  const avant = await style();
  await lecteur.getByRole("button", { name: "Affichage" }).click();
  const panneau = lecteur.getByRole("group", { name: "Affichage" });
  // Police : par défaut celle du livre ; « Très lisible » l'emporte sur elle.
  await expect(panneau.getByLabel("Police")).toHaveValue("book");
  await panneau.getByLabel("Police").selectOption("legible");
  await expect.poll(async () => (await style()).police).toContain("Verdana");
  // Interligne et marges : des curseurs, appliqués aussitôt.
  await panneau.getByLabel("Interligne").fill("2.2");
  await expect(panneau.getByText(/^2[,.]2$/)).toBeVisible();
  await expect.poll(async () => (await style()).interligne).toBeCloseTo(2.2, 1);
  await panneau.getByLabel("Marges").fill("15");
  await expect(panneau.getByText("15 %")).toBeVisible();
  await expect.poll(async () => (await style()).gauche).toBeGreaterThan(avant.gauche + 30);
  // Présentation : « 1 colonne » sur ordinateur (le téléphone n'en a qu'une de toute façon).
  if (appareil(info) !== "telephone") {
    await panneau.getByRole("radio", { name: "1 colonne" }).click();
    await expect(panneau.getByRole("radio", { name: "1 colonne" })).toHaveAttribute("aria-checked", "true");
  } else {
    await expect(panneau.getByRole("radio", { name: "2 colonnes" })).toHaveCount(0);
  }
  await expect
    .poll(() => page.evaluate(() => JSON.parse(localStorage.getItem("maku.epubReader") ?? "{}")["lib:2"]))
    .toMatchObject({ font: "legible", lineHeight: 2.2, margin: 15, ...(appareil(info) !== "telephone" ? { columns: 1 } : {}) });
});

/** Plein écran demandé à la fenêtre (null : jamais demandé). */
const pleinEcran = (page: Page) => page.evaluate(() => (window as unknown as { __fullscreen?: boolean }).__fullscreen ?? null);

test("plein écran : dans le lecteur de livres et de BD, Échap en sort sans fermer, et la fermeture aussi", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  const livre = await ouvrirLivre(page, info);
  await livre.getByRole("button", { name: "Plein écran" }).click();
  await expect.poll(() => pleinEcran(page)).toBe(true);
  // Rien autour du livre : les commandes sont cachées.
  await expect(livre.getByRole("button", { name: "Affichage" })).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect.poll(() => pleinEcran(page)).toBe(false);
  await expect(livre).toBeVisible();
  // Les commandes reviennent avec la sortie du plein écran.
  await expect(livre.getByRole("button", { name: "Affichage" })).toBeVisible();
  await expect(livre.getByRole("slider").first()).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(livre).toHaveCount(0);

  const bd = await ouvrirManga(page, info);
  await bd.getByRole("button", { name: "Plein écran" }).click();
  await expect.poll(() => pleinEcran(page)).toBe(true);
  await expect(bd.getByRole("button", { name: "Affichage" })).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect.poll(() => pleinEcran(page)).toBe(false);
  await expect(bd.getByRole("button", { name: "Affichage" })).toBeVisible();
  await bd.getByRole("button", { name: "Plein écran" }).click();
  await expect.poll(() => pleinEcran(page)).toBe(true);
  // Fermer le lecteur en plein écran en sort aussi.
  await page.evaluate(async () => {
    (await import("/src/lib/books.svelte.ts")).books.reading = null;
  });
  await expect(bd).toHaveCount(0);
  await expect.poll(() => pleinEcran(page)).toBe(false);
});

test("BD : luminosité des pages, retenue pour la bibliothèque", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  const lecteur = await ouvrirManga(page, info);
  await lecteur.getByRole("button", { name: "Affichage" }).click();
  await lecteur.getByLabel("Luminosité").fill("60");
  await expect(lecteur.getByText("60 %")).toBeVisible();
  await expect.poll(() => lecteur.getByRole("img", { name: "Page 6" }).evaluate((i) => getComputedStyle(i).filter)).toBe("brightness(0.6)");
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem("maku.readerLayout") ?? "{}")["lib:1"].brightness)).toBe(60);
});

/** Ouvre le menu « ⋮ » d'une couverture ou d'une ligne : le bouton, ou l'appui long (clic droit) au téléphone. */
async function menuDe(page: Page, info: TestInfo, element: Locator) {
  if (appareil(info) === "telephone") await element.click({ button: "right" });
  else await element.getByRole("button", { name: "Plus d'actions" }).click();
  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  return menu;
}

const marques = (page: Page) => page.evaluate(() => (window as unknown as { __kavitaMarks: unknown[] }).__kavitaMarks);

test("série Kavita : menu « ⋮ » pour la marquer comme lue ou non lue, et ouvrir sa fiche", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  await ouvrir(page, info, "accueil&kavita");
  await ouvrirMangas(page);
  // La série entamée : dans « Continuer la lecture » et dans toutes les séries.
  await expect(page.locator('[data-series="12"]')).toHaveCount(2);
  const carte = page.locator('[data-series="12"]').last();
  let menu = await menuDe(page, info, carte);
  await expect(menu.getByRole("menuitem")).toHaveText(["Marquer comme lu", "Marquer comme non lu", "Ouvrir la fiche"]);
  await menu.getByRole("menuitem", { name: "Marquer comme lu" }).click();
  await expect(page.getByText("Marqué comme lu")).toBeVisible();
  expect(await marques(page)).toEqual([{ seriesId: 12, read: true }]);
  // Lue : elle quitte « Continuer la lecture », et son menu ne propose plus que « non lu ».
  await expect(page.locator('[data-series="12"]')).toHaveCount(1);
  menu = await menuDe(page, info, page.locator('[data-series="12"]'));
  await expect(menu.getByRole("menuitem")).toHaveText(["Marquer comme non lu", "Ouvrir la fiche"]);
  await menu.getByRole("menuitem", { name: "Ouvrir la fiche" }).click();
  await expect(page.getByRole("heading", { name: "Le Gardien des Marées" })).toBeVisible();
});

test("série Kavita : « ⋮ » de la série, des volumes et des chapitres, et leurs détails", async ({ page }, info) => {
  test.skip(appareil(info) === "tv", "pas de Kavita au téléviseur");
  await ouvrir(page, info, "accueil&kavita");
  await ouvrirMangas(page);
  await page.getByRole("button", { name: "Le Gardien des Marées" }).first().click();
  await expect(page.getByRole("heading", { name: "Le Gardien des Marées" })).toBeVisible();

  // La série : à côté de « Continuer ».
  await page.locator(".actions").getByRole("button", { name: "Plus d'actions" }).click();
  await expect(page.getByRole("menu").getByRole("menuitem")).toHaveText(["Marquer comme lu", "Marquer comme non lu"]);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toHaveCount(0);

  // Un volume entamé : lire, lu, non lu, détails ; marqué lu, il ne propose plus « lu ».
  const volume = page.getByRole("group").filter({ hasText: "Volume 2" });
  let menu = await menuDe(page, info, volume);
  await expect(menu.getByRole("menuitem")).toHaveText(["Lire", "Marquer comme lu", "Marquer comme non lu", "Détails"]);
  await menu.getByRole("menuitem", { name: "Marquer comme lu" }).click();
  await expect(page.getByText("Marqué comme lu")).toBeVisible();
  expect(await marques(page)).toEqual([{ seriesId: 12, volumeId: 1202, chapterId: null, read: true }]);
  menu = await menuDe(page, info, volume);
  await expect(menu.getByRole("menuitem")).toHaveText(["Lire", "Marquer comme non lu", "Détails"]);

  // Détails du volume, comme dans Kavita.
  await menu.getByRole("menuitem", { name: "Détails" }).click();
  const fiche = page.getByRole("dialog", { name: "Détails" });
  await expect(fiche.getByRole("heading", { name: "Volume 2" })).toBeVisible();
  await expect(fiche.getByText("54 pages")).toBeVisible();
  await expect(fiche.getByText("Moins d'une heure")).toBeVisible();
  await expect(fiche.getByText("101 Mo")).toBeVisible();
  await expect(fiche.getByText("9 décembre 2025")).toBeVisible();
  await expect(fiche.getByText("Volume 2.cbz")).toBeVisible();
  // Des images : pas de mots.
  await expect(fiche.getByText("Mots")).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(fiche).toHaveCount(0);

  // Un chapitre hors volume : son « ⋮ » au bout de la ligne, ses mots (un livre en aurait).
  const chapitre = page.getByRole("group").filter({ hasText: "Chapitre 5" });
  await chapitre.getByRole("button", { name: "Plus d'actions" }).click();
  menu = page.getByRole("menu");
  await expect(menu.getByRole("menuitem")).toHaveText(["Lire", "Marquer comme lu", "Détails"]);
  await menu.getByRole("menuitem", { name: "Marquer comme lu" }).click();
  await expect.poll(() => marques(page)).toContainEqual({ seriesId: 12, volumeId: null, chapterId: 1215, read: true });
  await expect(chapitre.locator(".check")).toBeVisible();
  await chapitre.click({ button: "right" });
  await page.getByRole("menu").getByRole("menuitem", { name: "Détails" }).click();
  await expect(page.getByRole("dialog", { name: "Détails" }).getByText(/^5\s400$/)).toBeVisible();
});

test("BD : la barre des lettres mène à la première série d'une lettre, même pas encore chargée", async ({ page }, info) => {
  test.skip(appareil(info) === "tv" || appareil(info) === "telephone", "barre des lettres sur ordinateur seulement");
  await ouvrir(page, info, "accueil&kavita");
  await page.evaluate(async () => {
    const nav = await import("/src/lib/nav.svelte.ts");
    nav.go({ name: "section", key: "kavita:3", title: "BD", kind: "book" });
  });
  await expect(page.getByRole("heading", { name: "BD", exact: true })).toBeVisible();
  const lettres = page.getByRole("navigation", { name: "Aller à la lettre" });
  await expect(lettres).toBeVisible();
  // Pas de série en Q, X ni Y : lettres éteintes.
  for (const l of ["Q", "X", "Y"]) await expect(lettres.getByRole("button", { name: l, exact: true })).toBeDisabled();
  // Une première page de 60 séries : « Zénith » est à la troisième.
  await expect(page.locator("[data-series]")).toHaveCount(60);
  await lettres.getByRole("button", { name: "Z", exact: true }).click();
  await expect(page.getByRole("button", { name: "Zénith 1" })).toBeInViewport();
  // E : « Escale », pas « Étoile filante » (rangée après Z, comme dans Kavita).
  await lettres.getByRole("button", { name: "E", exact: true }).click();
  await expect(page.getByRole("button", { name: "Escale 1" })).toBeInViewport();
  await lettres.getByRole("button", { name: "#", exact: true }).click();
  await expect(page.getByRole("button", { name: "2 Mondes" })).toBeInViewport();
});
