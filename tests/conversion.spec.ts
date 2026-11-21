// Android : un fichier que l'appareil ne lit pas tel quel (format refusé, mémoire de l'appareil qui s'effondre) passe
// à la conversion du serveur, et la fois suivante y part directement. « Originale » le relit tel quel.
// Le lecteur natif est simulé : le pont d'Android (`MakuBridge`) note chaque fichier ouvert.
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

type W = { __opened: { url: string; offsetMs: number }[]; __plexrVideo: (e: unknown) => void };

async function android(page: Page) {
  await page.addInitScript(() => {
    const w = window as unknown as W & { MakuBridge: unknown };
    w.__opened = [];
    // Les méthodes du pont dont le test ne dépend pas (thème de l'icône, barres système…) ne font rien.
    const known: Record<string, (...a: string[]) => unknown> = {
      claim: () => true,
      videoOpen: (_key, json) => w.__opened.push(JSON.parse(json)),
      isTv: () => false,
      insets: () => "0,0",
    };
    w.MakuBridge = new Proxy(known, { get: (t, name: string) => t[name] ?? (() => undefined) });
  });
}

const opened = (page: Page) => page.evaluate(() => (window as unknown as W).__opened.map((o) => o.url));
const remembered = (page: Page) => page.evaluate(() => JSON.parse(localStorage.getItem("maku.converted") ?? "[]"));
const signal = (page: Page, e: unknown) => page.evaluate((e) => (window as unknown as W).__plexrVideo(e), e);

/** La même vidéo, relancée comme on la relancerait depuis sa fiche. */
async function relancer(page: Page) {
  await page.evaluate(async () => {
    const m = await import("/src/lib/player.svelte.ts");
    const key = m.player.ratingKey;
    m.stop();
    await m.play(key);
  });
}

test("format refusé : conversion, retenue pour la lecture suivante, et « Originale » relit le fichier", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "le lecteur d'Android est le même partout ; une fois suffit");
  await android(page);
  await ouvrir(page, info, "lecteur&android");
  await expect.poll(() => opened(page)).toEqual(["demo://original"]);

  // Seul le son est refusé : le serveur ne convertit que lui, puis tout quand ça ne suffit pas.
  await signal(page, { unsupported: "audio" });
  await expect.poll(() => opened(page)).toEqual(["demo://original", "demo://conversion?copy=1"]);
  expect(await remembered(page)).toEqual([[expect.stringMatching(/^plex:demo\|.+\|0$/), "audio"]]);
  await signal(page, { error: "décodeur" });
  await expect.poll(() => opened(page)).toHaveLength(3);
  expect((await opened(page))[2]).toBe("demo://conversion?copy=0");
  expect((await remembered(page))[0][1], "la conversion complète remplace celle du son seul").toBe("full");

  // Lecture suivante : directement la conversion complète, sans essayer le fichier d'origine.
  await relancer(page);
  await expect.poll(() => opened(page)).toHaveLength(4);
  expect((await opened(page))[3]).toBe("demo://conversion?copy=0");

  // Le menu de qualité le dit, et « Lire le fichier d'origine » oublie le fichier.
  await page.getByRole("button", { name: "Qualité" }).click();
  await expect(page.getByText("Cet appareil ne lit pas ce fichier tel quel")).toBeVisible();
  await page.getByRole("button", { name: "Lire le fichier d'origine" }).click();
  await expect.poll(() => opened(page)).toHaveLength(5);
  expect((await opened(page))[4]).toBe("demo://original");
  expect(await remembered(page)).toEqual([]);
});

test("mémoire de l'appareil qui s'effondre : conversion sans rien demander, puis arrêt si elle s'effondre encore", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "le lecteur d'Android est le même partout ; une fois suffit");
  await android(page);
  await ouvrir(page, info, "lecteur&android");
  await expect.poll(() => opened(page)).toEqual(["demo://original"]);

  await signal(page, { memory: { free: 164, dropped: 462 } });
  await expect.poll(() => opened(page)).toEqual(["demo://original", "demo://conversion?copy=0"]);
  await expect(page.getByText("La mémoire de l'appareil s'épuisait")).toBeVisible();
  expect((await remembered(page))[0][1]).toBe("full");
  const active = () => page.evaluate(async () => (await import("/src/lib/player.svelte.ts")).player.active);
  expect(await active()).toBe(true);

  // Déjà entièrement convertie : plus d'autre décodeur à essayer, la lecture s'arrête avant que l'appareil ne fige.
  await signal(page, { memory: { free: 150, dropped: 300 } });
  await expect.poll(active).toBe(false);
  await expect(page.getByText("La lecture est arrêtée pour que l'appareil ne se fige pas.")).toBeVisible();
});

test("qualité réduite choisie : la conversion n'est pas retenue", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "le lecteur d'Android est le même partout ; une fois suffit");
  await android(page);
  await ouvrir(page, info, "lecteur&android");
  await page.evaluate(async () => (await import("/src/lib/player.svelte.ts")).setQuality(4000));
  await expect.poll(() => opened(page)).toEqual(["demo://original", "demo://conversion?copy=0"]);
  // Image figée, conversion demandée : avec une qualité choisie, ce n'est pas le fichier qui est en cause.
  await page.evaluate(async () => (await import("/src/lib/player.svelte.ts")).unstall("transcode"));
  await expect.poll(() => opened(page)).toHaveLength(3);
  expect(await remembered(page)).toEqual([]);
});
