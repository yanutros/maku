// La version de Maku s'écrit à quatre endroits (package.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml et
// server/Cargo.toml) : ils doivent dire la même chose, et correspondre à l'étiquette poussée (« v0.31.0 »). Sinon
// une version sortirait avec des paquets d'un autre numéro, ou l'image de Maku Server écraserait la précédente.
//   node scripts/check-version.mjs            les quatre fichiers concordent
//   node scripts/check-version.mjs v0.31.0    … et valent cette étiquette
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

/** Version d'un Cargo.toml : la première ligne « version = "…" », celle du paquet. */
export const cargoVersion = (text) => /^version\s*=\s*"([^"]+)"/m.exec(text)?.[1] ?? null;

/** Problèmes trouvés (aucun : tout concorde). `versions` : fichier → version ; `tag` : étiquette, ou rien. */
export function problems(versions, tag) {
  const out = [];
  const entries = Object.entries(versions);
  for (const [file, version] of entries) if (!version) out.push(`${file} : version introuvable`);
  const known = entries.filter(([, v]) => v);
  const distinct = [...new Set(known.map(([, v]) => v))];
  if (distinct.length > 1) out.push(`versions différentes : ${known.map(([f, v]) => `${f} ${v}`).join(", ")}`);
  // Seules les étiquettes de version (« v1.2.3 ») sont comparées : pas une branche, ni « jellyfin-v… ».
  if (tag && /^v\d/.test(tag) && distinct.length === 1 && tag !== `v${distinct[0]}`) {
    out.push(`l'étiquette ${tag} ne correspond pas à la version ${distinct[0]} des fichiers`);
  }
  return out;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
  const versions = {
    "package.json": JSON.parse(read("package.json")).version ?? null,
    "src-tauri/tauri.conf.json": JSON.parse(read("src-tauri/tauri.conf.json")).version ?? null,
    "src-tauri/Cargo.toml": cargoVersion(read("src-tauri/Cargo.toml")),
    "server/Cargo.toml": cargoVersion(read("server/Cargo.toml")),
  };
  const found = problems(versions, process.argv[2]);
  if (found.length) {
    for (const p of found) console.error(`Version : ${p}`);
    process.exit(1);
  }
  console.log(`Version ${Object.values(versions)[0]} partout${process.argv[2] ? ` (étiquette ${process.argv[2]})` : ""}.`);
}
