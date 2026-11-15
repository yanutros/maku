// Contrat entre l'interface et le Rust : chaque commande appelée par l'interface (`invoke("…")`) existe bien côté
// Rust (`generate_handler!`, src-tauri/src/lib.rs), et le mode démo ne simule que des commandes qui existent.
// Le mode démo répond `null` à une commande inconnue : sans ce contrôle, une commande mal nommée ou retirée du Rust
// passerait tous les tests de l'interface et ne casserait que dans l'application.
//   node --test tests/contrat.test.mjs
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));

function files(dir, keep) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? files(path, keep) : keep(name) ? [path] : [];
  });
}

/** Commandes déclarées au Rust. */
function handlers() {
  const lib = readFileSync(join(root, "src-tauri/src/lib.rs"), "utf8");
  const list = /generate_handler!\[([\s\S]*?)\]/.exec(lib);
  assert.ok(list, "generate_handler! introuvable dans lib.rs");
  return new Set(
    list[1]
      .split(",")
      .map((s) => s.replace(/\/\/.*$/gm, "").trim().split("::").pop())
      .filter(Boolean),
  );
}

/** Commandes appelées par l'interface, avec le fichier où chacune l'est. */
function invoked() {
  const found = new Map();
  for (const path of files(join(root, "src"), (n) => /\.(ts|svelte)$/.test(n) && n !== "demo.ts")) {
    const text = readFileSync(path, "utf8");
    for (const m of text.matchAll(/\binvoke(?:<[^(]*?>)?\(\s*(["'`])([a-z0-9_:|-]+)\1/gi)) {
      if (!m[2].startsWith("plugin:")) found.set(m[2], path.slice(root.length));
    }
  }
  return found;
}

/** Commandes simulées par le mode démo. */
function simulated() {
  const demo = readFileSync(join(root, "src/lib/demo.ts"), "utf8");
  // Le corps de `handle` seulement : les autres `switch` du fichier (vues de la démo, états) ne sont pas des commandes.
  const start = demo.indexOf("function handle(cmd: string");
  const end = demo.indexOf("export async function installDemo");
  assert.ok(start > 0 && end > start, "fonction handle introuvable dans demo.ts");
  const names = [...demo.slice(start, end).matchAll(/^    case "([a-z0-9_]+)":/gm)].map((m) => m[1]);
  assert.ok(names.length > 50, `trop peu de commandes simulées lues (${names.length})`);
  return new Set(names);
}

test("chaque commande appelée par l'interface existe côté Rust", () => {
  const rust = handlers();
  const calls = invoked();
  assert.ok(rust.size > 100 && calls.size > 100, `trop peu de commandes lues (${rust.size} au Rust, ${calls.size} à l'interface)`);
  const missing = [...calls].filter(([cmd]) => !rust.has(cmd)).map(([cmd, file]) => `${cmd} (${file})`);
  assert.deepEqual(missing, [], "commandes appelées par l'interface, absentes du Rust");
});

test("le mode démo ne simule que des commandes qui existent", () => {
  const rust = handlers();
  const unknown = [...simulated()].filter((cmd) => !rust.has(cmd));
  assert.deepEqual(unknown, [], "commandes simulées par la démo, absentes du Rust (renommées ?)");
});

test("aucune commande du Rust n'est restée sans appel de l'interface", () => {
  const calls = invoked();
  const unused = [...handlers()].filter((cmd) => !calls.has(cmd));
  assert.deepEqual(unused, [], "commandes déclarées au Rust que l'interface n'appelle plus");
});
