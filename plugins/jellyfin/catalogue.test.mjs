// Tests du catalogue du plugin (catalogue.mjs) : `node --test plugins/jellyfin/catalogue.test.mjs`.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { previous, withVersion } from "./catalogue.mjs";

const meta = { guid: "g-maku", name: "Maku", description: "d", overview: "o", owner: "yanutros", category: "General", changelog: "c", targetAbi: "12.0.0.0" };
const release = (version) => ({ version, name: `maku-jellyfin_${version}.zip`, checksum: "abc", timestamp: "2026-01-01T00:00:00.000Z" });
const versions = (catalog) => catalog.find((p) => p.guid === meta.guid).versions.map((v) => v.version);

test("premier catalogue : la seule version publiée", () => {
  const catalog = withVersion(previous(null), meta, release("1.0.0.0"));
  assert.deepEqual(versions(catalog), ["1.0.0.0"]);
  assert.equal(catalog[0].versions[0].sourceUrl, "https://github.com/yanutros/maku/releases/download/jellyfin-plugin/maku-jellyfin_1.0.0.0.zip");
});

test("les versions précédentes sont gardées, la nouvelle en tête", () => {
  const old = JSON.stringify(withVersion(withVersion([], meta, release("1.0.0.0")), meta, release("1.1.0.0")));
  assert.deepEqual(versions(withVersion(previous(old), meta, release("1.2.0.0"))), ["1.2.0.0", "1.1.0.0", "1.0.0.0"]);
});

test("une version republiée remplace la sienne, sans doublon", () => {
  const old = JSON.stringify(withVersion(withVersion([], meta, release("1.0.0.0")), meta, release("1.1.0.0")));
  const again = withVersion(previous(old), meta, { ...release("1.1.0.0"), checksum: "nouvelle" });
  assert.deepEqual(versions(again), ["1.1.0.0", "1.0.0.0"]);
  assert.equal(again[0].versions[0].checksum, "nouvelle");
});

test("un autre plugin du catalogue n'est pas touché", () => {
  const other = { guid: "autre", name: "Autre", versions: [{ version: "9.0.0.0" }] };
  const catalog = withVersion(previous(JSON.stringify([other])), meta, release("1.0.0.0"));
  assert.deepEqual(catalog[0], other);
  assert.deepEqual(versions(catalog), ["1.0.0.0"]);
});

test("un ancien catalogue illisible arrête la publication", () => {
  // Téléchargement coupé, page d'erreur à la place du fichier, fichier vide.
  for (const text of ['[{"guid":"g-maku","versions":[', "<html>503</html>", "", '{"message":"Not Found"}']) {
    assert.throws(() => previous(text), /publication arrêtée/, JSON.stringify(text));
  }
});

test("en ligne de commande : rien n'est écrit si l'ancien catalogue est illisible", () => {
  const dir = mkdtempSync(join(tmpdir(), "maku-catalogue-"));
  try {
    const path = (name) => join(dir, name);
    writeFileSync(path("maku-jellyfin_1.2.0.0.zip"), "archive");
    writeFileSync(path("meta.json"), JSON.stringify(meta));
    const run = (old) =>
      execFileSync(process.execPath, [fileURLToPath(new URL("./catalogue.mjs", import.meta.url)), path("maku-jellyfin_1.2.0.0.zip"), "jellyfin-v1.2.0", path("meta.json"), old, path("manifest.json")], { stdio: "pipe" });
    writeFileSync(path("old.json"), JSON.stringify(withVersion([], meta, release("1.1.0.0"))));
    run(path("old.json"));
    const written = JSON.parse(readFileSync(path("manifest.json"), "utf8"));
    assert.deepEqual(versions(written), ["1.2.0.0", "1.1.0.0"]);
    // Somme MD5 de l'archive, comme Jellyfin l'attend.
    assert.equal(written[0].versions[0].checksum, createHash("md5").update("archive").digest("hex"));
    rmSync(path("manifest.json"));
    writeFileSync(path("coupe.json"), '[{"guid":"g-maku"');
    assert.throws(() => run(path("coupe.json")));
    assert.throws(() => run(path("absent.json")));
    assert.throws(() => readFileSync(path("manifest.json")));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
