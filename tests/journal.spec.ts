// Journal de diagnostic (Android) : ce que la page y écrit d'un élément ne contient ni titre ni nom de profil.
import { expect, test } from "@playwright/test";
import { ouvrir } from "./outils";

test("journal de diagnostic : un élément est décrit par sa zone, sa balise et sa classe, jamais par son texte", async ({ page }, info) => {
  await ouvrir(page, info, "films");
  // Toutes les jaquettes et tous les boutons affichés : leur libellé est un titre de la bibliothèque.
  const decrits = await page.evaluate(async () => {
    const nav = await import("/src/lib/nav.svelte.ts");
    return [...document.querySelectorAll("main button, main a, aside button, aside a")].map((el) => ({
      texte: [el.getAttribute("aria-label"), el.getAttribute("title"), el.textContent?.trim()].filter((t): t is string => !!t && t.length > 3),
      decrit: nav.describeElement(el),
    }));
  });
  expect(decrits.length).toBeGreaterThan(10);
  expect(decrits.some((d) => d.texte.length > 0), "des éléments ont un libellé").toBe(true);
  for (const { texte, decrit } of decrits) {
    expect(decrit).toMatch(/^(menu|page|autre) : [a-z0-9]+(\.[A-Za-z0-9_-]+){0,3}$/);
    for (const t of texte) expect(decrit, `libellé « ${t} » dans le journal`).not.toContain(t);
  }
});
