// Sécurité de la page : ce qu'elle a le droit de charger (la CSP de src-tauri/tauri.conf.json), et la clé qu'elle
// présente à Android pour être la seule à se servir de son pont.
import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import { appareil, ouvrir } from "./outils";

type Csp = Record<string, string>;
const securite = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")).app.security as { csp: Csp; devCsp: Csp };

const entete = (csp: Csp) =>
  Object.entries(csp)
    .map(([directive, sources]) => `${directive} ${sources}`)
    .join("; ");

/** La page est servie avec cette CSP, et note chaque refus dans `window.__refus`. */
async function avecCsp(page: Page, csp: Csp) {
  await page.route("**/*", async (route) => {
    if (route.request().resourceType() !== "document" || new URL(route.request().url()).hostname !== "localhost") return route.fallback();
    const response = await route.fetch();
    await route.fulfill({ response, headers: { ...response.headers(), "content-security-policy": entete(csp) } });
  });
  // Le lecteur de bandes-annonces n'est pas appelé pour de bon : une page vide à sa place.
  await page.route("https://www.youtube-nocookie.com/**", (route) => route.fulfill({ contentType: "text/html", body: "<html><body>bande-annonce</body></html>" }));
  await page.addInitScript(() => {
    const w = window as unknown as { __refus: string[] };
    w.__refus = [];
    document.addEventListener("securitypolicyviolation", (e) => w.__refus.push(`${e.effectiveDirective} ${e.blockedURI}`));
  });
}

const refus = (page: Page) => page.evaluate(() => (window as unknown as { __refus: string[] }).__refus);

test("CSP : celle du développement n'ajoute que la connexion de Vite", () => {
  const { csp, devCsp } = securite;
  expect(Object.keys(devCsp)).toEqual(Object.keys(csp));
  for (const directive of Object.keys(csp)) {
    if (directive === "connect-src") expect(devCsp[directive].startsWith(csp[directive] + " ")).toBe(true);
    else expect(devCsp[directive], directive).toBe(csp[directive]);
  }
});

for (const vue of ["accueil", "fiche", "series", "lecteur", "musique", "stats", "telechargements", "demandes"]) {
  test(`CSP : rien de Maku n'est refusé (${vue})`, async ({ page }, info) => {
    test.skip(appareil(info) === "safari", "même page ; un seul moteur suffit avec l'ordinateur, la télé et le téléphone");
    await avecCsp(page, securite.devCsp);
    await ouvrir(page, info, vue);
    await page.waitForTimeout(300);
    expect(await refus(page)).toEqual([]);
    // La CSP est bien en place : sans elle, ce test ne prouverait rien.
    const active = await page.evaluate(() => {
      const script = document.createElement("script");
      script.textContent = "window.__ecrit = 1;";
      document.body.append(script);
      return (window as unknown as { __ecrit?: number }).__ecrit === undefined ? "refusé" : "exécuté";
    });
    expect(active).toBe("refusé");
  });
}

test("CSP : la bande-annonce s'affiche dans son cadre", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même règle sur tous les appareils");
  await avecCsp(page, securite.devCsp);
  await ouvrir(page, info, "fiche");
  await page.evaluate(async () => {
    const { nav } = await import(/* @vite-ignore */ "/src/lib/nav.svelte.ts" as string);
    nav.trailer = "abcdefghijk";
  });
  const cadre = page.frameLocator(".trailer iframe");
  await expect(cadre.locator("body")).toHaveText("bande-annonce");
  expect(await refus(page)).toEqual([]);
});

test("CSP : un script venu d'ailleurs ne s'exécute pas, un autre site ne s'affiche pas", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même règle sur tous les appareils");
  await avecCsp(page, securite.devCsp);
  await page.route("https://ailleurs.example/**", (route) =>
    route.fulfill({ contentType: route.request().url().endsWith(".js") ? "text/javascript" : "text/html", body: route.request().url().endsWith(".js") ? "window.__execute = (window.__execute || 0) + 1;" : "<html><body>autre site</body></html>" }),
  );
  await ouvrir(page, info, "fiche");
  // Ce qu'un texte venu du serveur (titre, résumé, notes de version) pourrait glisser dans la page.
  await page.evaluate(() => {
    const zone = document.createElement("div");
    zone.id = "glisse";
    zone.innerHTML = `<img src="x" onerror="window.__execute = (window.__execute || 0) + 1"><iframe src="https://ailleurs.example/page"></iframe><form action="https://ailleurs.example/vol"><button id="envoi">ok</button></form>`;
    document.body.append(zone);
    for (const src of [null, "https://ailleurs.example/vol.js"]) {
      const script = document.createElement("script");
      if (src) script.src = src;
      else script.textContent = "window.__execute = (window.__execute || 0) + 1;";
      document.body.append(script);
    }
    const base = document.createElement("base");
    base.href = "https://ailleurs.example/";
    document.head.append(base);
  });
  await page.waitForTimeout(500);
  expect(await page.evaluate(() => (window as unknown as { __execute?: number }).__execute ?? 0)).toBe(0);
  const vus = (await refus(page)).map((r) => r.split(" ")[0]);
  for (const directive of ["script-src-attr", "script-src-elem", "frame-src", "base-uri"]) expect(vus, directive).toContain(directive);
  // L'adresse de base de la page n'a pas bougé : ses liens et ses images ne partent pas vers l'autre site.
  expect(await page.evaluate(() => document.baseURI.startsWith("http://localhost"))).toBe(true);
});

test("CSP : les images et les médias du serveur passent, d'où qu'il réponde ; rien d'autre ne part vers lui", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même règle sur tous les appareils");
  await avecCsp(page, securite.devCsp);
  const png = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==", "base64");
  // Le serveur de l'utilisateur : une adresse quelconque, en clair sur son réseau ou en https ailleurs.
  for (const origine of ["http://serveur.example:32400", "https://serveur.example"]) {
    await page.route(origine + "/**", (route) => route.fulfill({ contentType: route.request().url().includes("affiche") ? "image/png" : "audio/mpeg", body: png }));
  }
  await ouvrir(page, info, "accueil");
  const charge = await page.evaluate(async () => {
    const image = (src: string) =>
      new Promise<string>((fini) => {
        const img = new Image();
        img.onload = () => fini("chargée");
        img.onerror = () => fini("refusée");
        img.src = src;
      });
    const requete = (url: string) => fetch(url).then(() => "partie", () => "refusée");
    // Le média : demandé au serveur (ce faux fichier ne se lit pas, mais la demande doit partir).
    for (const src of ["http://serveur.example:32400/titre.mp3", "https://serveur.example/film.mp4"]) {
      const audio = new Audio(src);
      audio.preload = "auto";
      audio.load();
    }
    return {
      clair: await image("http://serveur.example:32400/affiche.png"),
      https: await image("https://serveur.example/affiche.png"),
      // Hors images et médias, la page ne parle à aucun autre hôte : tout le reste passe par le Rust.
      requete: await requete("https://serveur.example/library"),
    };
  });
  await page.waitForTimeout(300);
  expect(charge).toEqual({ clair: "chargée", https: "chargée", requete: "refusée" });
  expect(await refus(page)).toEqual(["connect-src https://serveur.example/library"]);
});

// ---------- Pont Android

/** Faux pont d'Android, tel que MainActivity.kt le donne à la page : il note ce qu'il reçoit. */
async function fauxPont(page: Page) {
  await page.addInitScript(() => {
    const w = window as unknown as { __appels: unknown[][]; __cle: string | null; MakuBridge: Record<string, unknown> };
    w.__appels = [];
    // Android garde la clé d'un rechargement de la page à l'autre : ici, dans une valeur que le test remet lui-même.
    w.__cle = (window.name || null) as string | null;
    const note =
      (nom: string, rendu: unknown = undefined) =>
      (...args: unknown[]) => {
        w.__appels.push([nom, ...args]);
        return args[0] === w.__cle ? rendu : nom === "isTv" ? false : "";
      };
    w.MakuBridge = {
      claim: (cle: string) => {
        w.__appels.push(["claim", cle]);
        if (w.__cle === null) {
          w.__cle = cle;
          window.name = cle;
        }
        return w.__cle === cle;
      },
      isTv: note("isTv", true),
      insets: note("insets", "24,48"),
      diagLog: note("diagLog"),
      diagText: note("diagText", "journal"),
      setIconTheme: note("setIconTheme"),
      audioPassthrough: note("audioPassthrough", "[]"),
      mediaStop: note("mediaStop"),
      mediaUpdate: note("mediaUpdate"),
      setVideoMode: note("setVideoMode"),
      leave: note("leave"),
    };
  });
}

const appels = (page: Page) => page.evaluate(() => (window as unknown as { __appels: unknown[][] }).__appels);

test("pont Android : la page présente sa clé en premier, et la joint à chaque appel", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même module sur tous les appareils");
  await fauxPont(page);
  await ouvrir(page, info, "accueil");
  const vus = await appels(page);
  // Avant toute autre demande : la clé, 128 bits tirés au hasard.
  expect(vus[0][0]).toBe("claim");
  const cle = vus[0][1] as string;
  expect(cle).toMatch(/^[0-9a-f]{32}$/);
  expect(vus.filter((a) => a[0] === "claim")).toHaveLength(1);
  // L'interface s'est servie du pont (téléviseur ou non, marges, icône…), toujours avec la clé.
  expect(vus.length).toBeGreaterThan(1);
  for (const appel of vus.slice(1)) expect(appel[1], String(appel[0])).toBe(cle);
  // Les réponses d'Android arrivent bien à l'interface, et ses paramètres suivent la clé.
  const reponse = await page.evaluate(() => {
    const pont = (window as unknown as { PlexrDevice: Record<string, (...a: unknown[]) => unknown> }).PlexrDevice;
    pont.setVideoMode(true);
    return { journal: pont.diagText(), tv: pont.isTv(), claim: typeof pont.claim, inconnue: typeof pont.nimporte };
  });
  expect(reponse).toEqual({ journal: "journal", tv: true, claim: "undefined", inconnue: "undefined" });
  expect((await appels(page)).at(-3)).toEqual(["setVideoMode", cle, true]);
  // La clé n'est rangée nulle part où un autre script la trouverait en regardant la page.
  const exposee = await page.evaluate(
    (cle) =>
      Object.getOwnPropertyNames(window).some((nom) => {
        // (« name » et « __cle » : la mémoire du faux pont de ce test.)
        if (nom === "name" || nom === "__cle") return false;
        try {
          return (window as unknown as Record<string, unknown>)[nom] === cle;
        } catch {
          return false;
        }
      }),
    cle,
  );
  expect(exposee).toBe(false);
});

test("pont Android : la page rechargée présente la même clé", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même module sur tous les appareils");
  await fauxPont(page);
  await ouvrir(page, info, "accueil");
  const cle = (await appels(page))[0][1];
  await page.reload();
  await page.locator("main").waitFor({ state: "attached" });
  const vus = await appels(page);
  // Android a gardé la clé du premier chargement : une autre clé fermerait le pont à la page elle-même.
  expect(vus[0]).toEqual(["claim", cle]);
  expect(await page.evaluate(() => (window as unknown as { PlexrDevice: { isTv(): boolean } }).PlexrDevice.isTv())).toBe(true);
});

test("pont Android : un cadre d'un autre site n'a pas la clé", async ({ page }, info) => {
  test.skip(appareil(info) !== "pc", "même module sur tous les appareils");
  await fauxPont(page);
  await page.route("https://ailleurs.example/**", (route) => route.fulfill({ contentType: "text/html", body: "<html><body>cadre</body></html>" }));
  await ouvrir(page, info, "accueil");
  const cle = (await appels(page))[0][1] as string;
  await page.evaluate(() => {
    const cadre = document.createElement("iframe");
    cadre.src = "https://ailleurs.example/cadre";
    document.body.append(cadre);
  });
  const trouve = () => page.frames().find((f) => f.url().startsWith("https://ailleurs.example"));
  await expect.poll(() => !!trouve()).toBe(true);
  const cadre = trouve()!;
  await cadre.waitForLoadState();
  // Ce que le cadre peut atteindre : ni la page de Maku, ni son stockage, où la clé est rangée.
  const vu = await cadre.evaluate(() => {
    const essais: Record<string, string> = {};
    for (const [nom, lire] of Object.entries({
      parent: () => String((window.parent as unknown as { PlexrDevice?: unknown }).PlexrDevice),
      stockage: () => String(window.parent.sessionStorage.getItem("maku.bridgeKey")),
      sien: () => String(sessionStorage.getItem("maku.bridgeKey")),
    })) {
      try {
        essais[nom] = lire();
      } catch {
        essais[nom] = "interdit";
      }
    }
    return essais;
  });
  expect(vu).toEqual({ parent: "interdit", stockage: "interdit", sien: "null" });
  expect(Object.values(vu)).not.toContain(cle);
});
