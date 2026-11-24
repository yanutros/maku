import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  // Le backend Rust est surveillé par Tauri, et Maku Server (server/) se compile à part : Vite ignore les deux (sinon
  // EBUSY sur les binaires en cours d'écriture, qui arrêtait le serveur de développement).
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**", "**/server/**"] } },
  build: { target: "es2021" },
});
