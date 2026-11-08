// Tests de l'interface (Playwright) : Maku en mode démo (bibliothèque fictive, sans serveur ni compte), sur un
// ordinateur, un téléviseur (flèches, OK, Retour), un téléphone et le moteur de Safari (iPad, Mac).
//
//   npm test                    tous les tests
//   npm test -- --project=tv    un seul appareil
//   npm run test:update         met à jour les images de référence (après un changement voulu)
import { defineConfig, devices } from "@playwright/test";

const PORT = 5199;

export default defineConfig({
  testDir: "tests",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  // Images de référence par appareil, rangées à côté des tests.
  snapshotPathTemplate: "{testDir}/images/{projectName}/{arg}{ext}",
  // Sur GitHub (Linux), les polices ne sont pas celles de Windows : les images de référence, prises sous Windows,
  // n'y sont pas comparées. Les règles (fonds, cadres, navigation…) le sont, elles, partout.
  ignoreSnapshots: !!process.env.CI,
  expect: {
    toHaveScreenshot: { maxDiffPixelRatio: 0.01, animations: "disabled" },
  },
  use: {
    baseURL: `http://localhost:${PORT}`,
    locale: "fr-FR",
    timezoneId: "Europe/Paris",
    colorScheme: "dark",
    trace: "retain-on-failure",
  },
  projects: [
    { name: "pc", use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } } },
    { name: "tv", use: { ...devices["Desktop Chrome"], viewport: { width: 1920, height: 1080 } } },
    { name: "telephone", use: { ...devices["Pixel 7"] } },
    // Moteur de Safari, celui de l'iPad et du Mac : il a ses propres défauts (coins des calques flous…).
    { name: "safari", use: { ...devices["Desktop Safari"], viewport: { width: 1180, height: 820 } } },
  ],
  webServer: {
    command: `npx vite --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
