// Lecteur vidéo : bouton lecture / pause, boutons du haut sur TV, et commandes qui s'effacent comme dans Plex.
import { expect, test } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

test("le bouton lecture / pause a la taille des autres boutons", async ({ page }, info) => {
  await ouvrir(page, info, "lecteur");
  const main = await page.locator(".overlay .icon-btn.main").boundingBox();
  const autre = await page.locator(".overlay .transport .icon-btn:not(.main)").first().boundingBox();
  expect(main!.width).toBe(autre!.width);
  expect(main!.height).toBe(autre!.height);
});

test("TV : pas de boutons retour, volume ni plein écran en haut", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "téléviseur");
  await ouvrir(page, info, "lecteur");
  await expect(page.locator(".overlay .icon-btn.main")).toBeVisible();
  await expect(page.locator(".overlay .top")).toHaveCount(0);
});

test("ordinateur : en pause, les commandes partent quand la souris sort de la fenêtre", async ({ page }, info) => {
  test.skip(appareil(info) === "tv" || appareil(info) === "telephone", "souris");
  await ouvrir(page, info, "lecteur"); // la démo est en pause
  const overlay = page.locator(".overlay");
  await page.mouse.move(400, 300);
  await expect(overlay).not.toHaveClass(/hidden/);
  await overlay.dispatchEvent("pointerleave", { pointerType: "mouse" });
  await expect(overlay).toHaveClass(/hidden/);
  // La souris revient : les commandes aussi.
  await page.mouse.move(420, 320);
  await expect(overlay).not.toHaveClass(/hidden/);
});

test("TV : en pause, les commandes partent seules ; le premier OK ne fait que les réafficher", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "télécommande");
  await ouvrir(page, info, "lecteur"); // la démo est en pause
  const overlay = page.locator(".overlay");
  await expect(overlay).toHaveClass(/hidden/, { timeout: 12000 });
  const avant = await page.evaluate(() => document.activeElement?.className ?? "");
  // Chaque commande envoyée au lecteur (la démo n'a pas mpv) : le premier OK n'en envoie aucune.
  await page.evaluate(() => {
    const w = window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown, o: unknown) => Promise<unknown> }; sent: unknown[] };
    const invoke = w.__TAURI_INTERNALS__.invoke;
    w.sent = [];
    w.__TAURI_INTERNALS__.invoke = (c, a, o) => {
      if (c === "player_command") w.sent.push(a);
      return invoke(c, a, o);
    };
  });
  await page.keyboard.press("Enter");
  await expect(overlay).not.toHaveClass(/hidden/);
  expect(await page.evaluate(() => (window as unknown as { sent: unknown[] }).sent.length), "OK a mis en pause ou en lecture").toBe(0);
  expect(await page.evaluate(() => document.activeElement?.className ?? "")).toBe(avant);
  // Commandes affichées : OK agit de nouveau sur le bouton sélectionné.
  await page.keyboard.press("Enter");
  await expect.poll(() => page.evaluate(() => (window as unknown as { sent: unknown[] }).sent.length)).toBeGreaterThan(0);
});

test("barre minimaliste (Réglages → Interface) : une seule rangée, les pistes en icônes", async ({ page }, info) => {
  const device = { tv: "tv", telephone: "phone" }[appareil(info) as string];
  await page.addInitScript(() => {
    localStorage.clear();
    localStorage.setItem("maku.playerBar", "minimal");
  });
  await page.goto(`/?demo=lecteur${device ? `&device=${device}` : ""}`);
  await page.waitForLoadState("networkidle");
  await expect(page.locator(".overlay .mini-row")).toBeVisible();
  await expect(page.locator(".overlay .capsule")).toHaveCount(0);
  // Les pistes s'ouvrent depuis leur icône, comme dans la barre complète.
  await page.locator(".pill.compact").nth(1).click();
  await expect(page.locator(".overlay .menu")).toBeVisible();
});

test("TV : Retour ferme le menu des pistes ouvert, sans quitter le film", async ({ page }, info) => {
  test.skip(appareil(info) !== "tv", "télécommande");
  await ouvrir(page, info, "lecteur");
  await page.locator(".overlay .pill").nth(1).click();
  await expect(page.locator(".overlay .menu")).toBeVisible();
  await page.goBack(); // touche Retour de la télécommande
  await expect(page.locator(".overlay .menu")).toHaveCount(0);
  await expect(page.locator(".overlay")).toBeVisible();
  // Un second Retour quitte le film, comme avant.
  await page.goBack();
  await expect(page.locator(".overlay")).toHaveCount(0);
});

test("vidéo figée : un message propose de réessayer, d'avancer de 10 s, de décoder le son ici ou de faire convertir par le serveur", async ({ page }, info) => {
  await ouvrir(page, info, "lecteur-fige");
  const message = page.getByRole("dialog", { name: /La lecture est bloquée à 3:07/ });
  await expect(message).toBeVisible();
  await expect(message.getByRole("button")).toHaveText(["Réessayer", "Passer 10 s", "Son décodé par l'appareil", "Conversion par le serveur"]);
  // Les commandes restent affichées tant que la vidéo est figée.
  await page.waitForTimeout(5000);
  await expect(page.locator(".overlay")).not.toHaveClass(/hidden/);
  if (appareil(info) === "tv") {
    // Télécommande : la sélection est sur « Réessayer », et les flèches restent dans le message.
    await expect(message.getByRole("button", { name: "Réessayer" })).toBeFocused();
    await page.keyboard.press("ArrowRight");
    await expect(message.getByRole("button", { name: "Passer 10 s" })).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(message.locator(":focus")).toHaveCount(1);
  }
  await message.getByRole("button", { name: "Réessayer" }).click();
  await expect(message).toHaveCount(0);
});

test("vidéo figée : « Conversion par le serveur » ferme le message", async ({ page }, info) => {
  await ouvrir(page, info, "lecteur-fige");
  const message = page.getByRole("dialog", { name: /La lecture est bloquée/ });
  await message.getByRole("button", { name: "Conversion par le serveur" }).click();
  await expect(message).toHaveCount(0);
});

test("vidéo figée avec le son envoyé tel quel : Maku retient « Décodé ici » et relance sans message", async ({ page }, info) => {
  await ouvrir(page, info, "lecteur-fige-son");
  await expect(page.locator(".toast")).toHaveText(/le son est désormais décodé par l'appareil/);
  await expect(page.getByRole("dialog", { name: /La lecture est bloquée/ })).toHaveCount(0);
  expect(await page.evaluate(() => localStorage.getItem("maku.passthrough"))).toBe("off");
});

test("écran de fin : la jaquette de la saison, et le numéro de l'épisode sans accent (S2 · E4)", async ({ page }, info) => {
  await ouvrir(page, info, "fin-episode");
  const fin = page.getByRole("dialog", { name: "Épisode suivant" });
  await expect(fin).toBeVisible();
  await expect(fin.locator(".episode")).toHaveText("S2 · E4 — Aurore");
  // La jaquette de la saison (en hauteur, comme une affiche), et non une image de l'épisode.
  const jaquette = fin.locator(".thumb.poster img");
  await expect(jaquette).toBeVisible();
  const r = (await fin.locator(".thumb.poster").boundingBox())!;
  expect(Math.round((r.height / r.width) * 10)).toBe(15);
  // La jaquette de démo de la saison porte « Saison 2 » ; celle de la série, non.
  await expect(jaquette).toHaveAttribute("src", /SAISON%202/);
});
