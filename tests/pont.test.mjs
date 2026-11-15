// Pont entre la page et Android (MainActivity.kt) : Android montre son objet à toute page affichée, dont la
// bande-annonce d'un autre site. Chaque méthode doit donc commencer par demander la clé de la page (BridgeGate) :
// une méthode ajoutée sans cette ligne serait ouverte à tous.
//
//   node --test tests/pont.test.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync(new URL("../src-tauri/gen/android/app/src/main/java/io/github/yanutros/plexr/MainActivity.kt", import.meta.url), "utf8").replace(/\r\n/g, "\n");

/** Méthodes du pont : leur nom, leurs paramètres et leur première ligne utile. */
function methods() {
  const start = source.indexOf("webView.addJavascriptInterface(object {");
  const end = source.indexOf('}, "MakuBridge")');
  assert.ok(start > 0 && end > start, "pont introuvable dans MainActivity.kt");
  const found = [];
  const re = /@JavascriptInterface\s+fun (\w+)\(([^)]*)\)[^\n]*\n([^\n]*)/g;
  for (const m of source.slice(start, end).matchAll(re)) found.push({ name: m[1], params: m[2], declaration: m[0].split("\n").at(-2), first: m[3].trim() });
  return found;
}

test("le pont a toutes ses méthodes, sous le nom que la page attend", () => {
  const names = methods().map((m) => m.name);
  assert.equal(new Set(names).size, names.length);
  assert.ok(names.length >= 18, `${names.length} méthodes trouvées`);
  assert.ok(names.includes("claim"));
  // L'ancien nom, que les cadres connaîtraient aussi, n'est plus donné à Android.
  assert.ok(!source.includes('"PlexrDevice")'));
  // Autant d'annotations que de méthodes lues : aucune n'a échappé à la lecture.
  const start = source.indexOf("webView.addJavascriptInterface(object {");
  const end = source.indexOf('}, "MakuBridge")');
  assert.equal(source.slice(start, end).split("@JavascriptInterface").length - 1, names.length);
});

test("chaque méthode du pont demande la clé avant d'agir", () => {
  for (const m of methods()) {
    assert.match(m.params, /^key: String(,|$)/, `${m.name} : la clé est son premier paramètre`);
    if (m.name === "claim") {
      assert.match(m.declaration, /= gate\.claim\(key\)$/, "claim ne fait que présenter la clé");
      continue;
    }
    // En une ligne (« = gate.allows(key) && … », « = if (gate.allows(key)) … else "" »), ou en première ligne du corps.
    const oneLine = /= (gate\.allows\(key\) && |if \(gate\.allows\(key\)\) .+ else "")/.test(m.declaration);
    const firstLine = /^if \(!gate\.allows\(key\)\) return\b/.test(m.first) || /^if \(gate\.allows\(key\)\) \S/.test(m.first);
    assert.ok(oneLine || (m.declaration.endsWith("{") && firstLine), `${m.name} agit sans avoir demandé la clé : ${m.declaration.trim()} / ${m.first}`);
  }
});
