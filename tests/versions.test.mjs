// Contrôle des numéros de version avant une publication (scripts/check-version.mjs).
//   node --test tests/versions.test.mjs
import assert from "node:assert/strict";
import { test } from "node:test";
import { cargoVersion, problems } from "../scripts/check-version.mjs";

const same = { "package.json": "0.31.0", "src-tauri/tauri.conf.json": "0.31.0", "src-tauri/Cargo.toml": "0.31.0", "server/Cargo.toml": "0.31.0" };

test("les quatre fichiers concordent, avec ou sans étiquette", () => {
  assert.deepEqual(problems(same), []);
  assert.deepEqual(problems(same, "v0.31.0"), []);
  // Hors publication d'une version : une branche, ou l'étiquette du plugin Jellyfin.
  assert.deepEqual(problems(same, "main"), []);
  assert.deepEqual(problems(same, "jellyfin-v1.4.0"), []);
});

test("un fichier oublié est signalé", () => {
  const found = problems({ ...same, "server/Cargo.toml": "0.30.0" });
  assert.equal(found.length, 1);
  assert.match(found[0], /versions différentes/);
  assert.match(found[0], /server\/Cargo\.toml 0\.30\.0/);
});

test("une étiquette qui ne correspond pas aux fichiers est refusée", () => {
  assert.match(problems(same, "v0.32.0")[0], /v0\.32\.0 ne correspond pas à la version 0\.31\.0/);
  assert.match(problems(same, "v0.31")[0], /ne correspond pas/);
});

test("une version illisible est signalée", () => {
  assert.match(problems({ ...same, "src-tauri/Cargo.toml": null })[0], /src-tauri\/Cargo\.toml : version introuvable/);
});

test("la version d'un Cargo.toml est celle du paquet", () => {
  assert.equal(cargoVersion('[package]\nname = "plexr"\nversion = "0.31.0"\n\n[dependencies]\ntauri = { version = "2" }\n'), "0.31.0");
  assert.equal(cargoVersion("[package]\nname = \"x\"\n"), null);
});
