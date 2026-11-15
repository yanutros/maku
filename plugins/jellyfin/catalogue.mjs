// Catalogue du plugin Maku pour Jellyfin (manifest.json) : la nouvelle version en tête, les précédentes gardées.
// Lancé par .github/workflows/plugin-jellyfin.yml à la publication d'un tag « jellyfin-v1.2.3 » :
//   node plugins/jellyfin/catalogue.mjs <archive.zip> <tag> <meta.json> <ancien manifest.json ou ""> <sortie>
// L'ancien catalogue doit être lisible quand il existe : un catalogue illisible arrête la publication, plutôt que
// d'être remplacé par un catalogue qui n'aurait plus que la nouvelle version.

import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { basename } from "node:path";
import { pathToFileURL } from "node:url";

const IMAGE = "https://yanutros.github.io/maku/jellyfin/maku.png";
const DOWNLOADS = "https://github.com/yanutros/maku/releases/download/jellyfin-plugin";

/** Catalogue déjà publié (`text` : contenu de manifest.json), ou vide s'il n'y en a pas encore (`null`). */
export function previous(text) {
  if (text === null) return [];
  let catalog;
  try {
    catalog = JSON.parse(text);
  } catch (e) {
    throw new Error(`ancien catalogue illisible (${e.message}) : publication arrêtée, pour ne pas perdre ses versions`);
  }
  if (!Array.isArray(catalog)) throw new Error("ancien catalogue inattendu (ce n'est pas une liste) : publication arrêtée");
  return catalog;
}

/** Le catalogue avec la version `version` en tête de celles du plugin ; les autres versions sont gardées. */
export function withVersion(catalog, meta, { version, name, checksum, timestamp }) {
  const next = catalog.map((p) => ({ ...p }));
  let entry = next.find((p) => p.guid === meta.guid);
  if (!entry) {
    entry = { versions: [] };
    next.push(entry);
  }
  Object.assign(entry, {
    guid: meta.guid,
    name: meta.name,
    description: meta.description,
    overview: meta.overview,
    owner: meta.owner,
    category: meta.category,
    imageUrl: IMAGE,
  });
  entry.versions = [
    { version, changelog: meta.changelog, targetAbi: meta.targetAbi, sourceUrl: `${DOWNLOADS}/${name}`, checksum, timestamp },
    ...(entry.versions ?? []).filter((v) => v.version !== version),
  ];
  return next;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const [zip, tag, metaPath, oldPath, out] = process.argv.slice(2);
  const catalog = previous(oldPath ? readFileSync(oldPath, "utf8") : null);
  const next = withVersion(catalog, JSON.parse(readFileSync(metaPath, "utf8")), {
    version: tag.replace(/^jellyfin-v/, "") + ".0",
    name: basename(zip),
    // Somme MD5 : c'est le format du catalogue de Jellyfin.
    checksum: createHash("md5").update(readFileSync(zip)).digest("hex"),
    timestamp: new Date().toISOString(),
  });
  writeFileSync(out, JSON.stringify(next, null, 2));
  const versions = next.flatMap((p) => p.versions.map((v) => v.version));
  console.log(`Catalogue : ${versions.length} version(s) (${versions.join(", ")})`);
}
