// Linux : la vidéo dans la fenêtre de Maku (réglage expérimental). mpv garde sa fenêtre, en plein écran et sans ses
// commandes ; celle de Maku passe par-dessus dès que l'image s'affiche.
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

const module = <T>(page: Page, path: string, code: string, arg: unknown = null) =>
  page.evaluate(
    async ([path, code, arg]) => {
      const m = await import(/* @vite-ignore */ path as string);
      return new Function("m", "arg", code as string)(m, arg);
    },
    [path, code, arg],
  ) as Promise<T>;

/** Note ce que la page demande à sa fenêtre (plein écran, passage au premier plan). */
async function noterFenetre(page: Page) {
  await page.evaluate(() => {
    const w = window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown, o: unknown) => Promise<unknown> }; fenetre: string[] };
    const invoke = w.__TAURI_INTERNALS__.invoke;
    w.fenetre = [];
    w.__TAURI_INTERNALS__.invoke = (c, a, o) => {
      if (c === "plugin:window|set_fullscreen") w.fenetre.push(`plein écran ${(a as { value: boolean }).value}`);
      if (c === "plugin:window|set_focus") w.fenetre.push("premier plan");
      return invoke(c, a, o);
    };
  });
  return () => page.evaluate(() => (window as unknown as { fenetre: string[] }).fenetre);
}

/** Ouvre Réglages → Lecteur, Maku se croyant sur le système `os`. */
async function reglagesLecteur(page: Page, info: Parameters<typeof ouvrir>[1], os: string) {
  await ouvrir(page, info, "accueil");
  await module(page, "/src/lib/device.ts", "m.device.os = arg;", os);
  await module(page, "/src/lib/nav.svelte.ts", 'm.go({ name: "settings" });');
  await page.getByRole("tab", { name: "Lecteur" }).click();
  await expect(page.locator('[data-pref="audio"]')).toBeVisible();
}

test("Réglages → Lecteur : « vidéo dans Maku » n'est proposé que sous Linux, et désactivé au départ", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "réglage de l'ordinateur");
  await reglagesLecteur(page, info, "windows");
  await expect(page.locator('[data-pref="linux-embed"]')).toHaveCount(0);
  await reglagesLecteur(page, info, "linux");
  const interrupteur = page.locator('[data-pref="linux-embed"] input');
  await expect(interrupteur).not.toBeChecked();
  expect(await page.evaluate(() => localStorage.getItem("maku.linuxEmbed"))).toBeNull();
  // Activé : gardé pour le prochain démarrage, et Maku dit qu'il faut le relancer.
  await interrupteur.check();
  expect(await page.evaluate(() => localStorage.getItem("maku.linuxEmbed"))).toBe("1");
  await expect(page.locator(".toasts .toast")).toContainText("Relance Maku");
  // Désactivé : retiré, au lieu d'une valeur que le démarrage pourrait mal lire.
  await interrupteur.uncheck();
  expect(await page.evaluate(() => localStorage.getItem("maku.linuxEmbed"))).toBeNull();
});

test("lecteur, vidéo dans Maku : la fenêtre passe en plein écran et au premier plan quand l'image s'affiche", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "fenêtre de l'ordinateur");
  await ouvrir(page, info, "lecteur");
  const fenetre = await noterFenetre(page);
  const lecteur = (code: string) => module(page, "/src/lib/player.svelte.ts", code);
  // mpv vient d'être lancé sous la fenêtre de Maku : tant qu'il n'affiche rien, Maku ne bouge pas.
  await lecteur("m.player.embedded = true; m.player.under = true; m.player.videoReady = false;");
  await page.waitForTimeout(300);
  expect(await fenetre()).toEqual([]);
  // L'image est là : plein écran, puis premier plan ; et une seconde fois, mpv redemandant le plein écran de son côté.
  await lecteur("m.player.videoReady = true;");
  await expect.poll(fenetre).toEqual(["plein écran true", "premier plan"]);
  await expect.poll(fenetre, { timeout: 4000 }).toEqual(["plein écran true", "premier plan", "plein écran true", "premier plan"]);
  // Le film reste en plein écran : F et Échap ne laissent pas les commandes dans une petite fenêtre, loin de l'image.
  await page.keyboard.press("f");
  await page.waitForTimeout(300);
  // Rien n'est demandé à la fenêtre : ni sortie, ni nouvelle bascule.
  expect(await fenetre()).toHaveLength(4);
  // La page est transparente : le film, dessous, se voit.
  expect(await page.evaluate(() => document.documentElement.classList.contains("playing"))).toBe(true);
  // Fermer la vidéo rend la fenêtre comme elle était.
  await page.keyboard.press("Escape");
  await expect.poll(fenetre).toContain("plein écran false");
});

test("lecteur, fenêtre mpv séparée ou incrustée : Maku ne change pas de taille tout seul", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "fenêtre de l'ordinateur");
  await ouvrir(page, info, "lecteur");
  const fenetre = await noterFenetre(page);
  await module(page, "/src/lib/player.svelte.ts", "m.player.embedded = true; m.player.under = false; m.player.videoReady = true;");
  await page.waitForTimeout(1600);
  expect(await fenetre()).toEqual([]);
});
