// Windows : prépare le lecteur mpv embarqué dans l'installeur de Maku (src-tauri/mpv-win), pour
// que la lecture fonctionne sans rien installer d'autre. Version épinglée de mpv-winbuild-cmake
// (https://github.com/shinchiro/mpv-winbuild-cmake, GPL), vérifiée par son empreinte SHA-256.
// Lancé avant chaque compilation Windows (voir tauri.windows.conf.json) ; ne fait rien ailleurs.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const MPV = {
  release: "20261017",
  file: "mpv-x86_64-20261017-git-dde0671072.7z",
  sha256: "2821d9eef879215e99975fae9616e1777f5e3747168edd3a571c41e21b8abbbc",
};
// Fichiers gardés : le lecteur, sa bibliothèque pour les anciens Windows et la configuration des
// polices des sous-titres.
const FILES = ["mpv.exe", "d3dcompiler_43.dll", "mpv/fonts.conf"];

if (process.platform !== "win32") process.exit(0);

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "src-tauri", "mpv-win");
const stamp = join(out, "VERSION");

if (existsSync(stamp) && readFileSync(stamp, "utf8").trim() === MPV.release && FILES.every((f) => existsSync(join(out, f)))) {
  process.exit(0);
}

const url = `https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/${MPV.release}/${MPV.file}`;
console.log(`mpv ${MPV.release} : téléchargement…`);
// GitHub renvoie parfois une erreur passagère (504) sur ses téléchargements : jusqu'à 10 essais, espacés de 20 s.
async function download(link) {
  for (let attempt = 1; ; attempt++) {
    const r = await fetch(link).catch((e) => ({ ok: false, status: String(e) }));
    if (r.ok || attempt >= 10) return r;
    console.warn(`mpv : essai ${attempt} échoué (${r.status}), nouvel essai dans 20 s`);
    await new Promise((done) => setTimeout(done, 20000));
  }
}
const response = await download(url);
if (!response.ok) throw new Error(`mpv : téléchargement impossible (${response.status})`);
const archive = Buffer.from(await response.arrayBuffer());
const digest = createHash("sha256").update(archive).digest("hex");
if (digest !== MPV.sha256) throw new Error(`mpv : empreinte inattendue (${digest}), archive refusée`);

const work = mkdtempSync(join(tmpdir(), "plexr-mpv-"));
try {
  const file = join(work, MPV.file);
  writeFileSync(file, archive);
  const target = join(work, "x");
  mkdirSync(target);
  // 7-Zip si présent (machines de la CI), sinon le tar de Windows 11, qui lit aussi le 7z.
  try {
    execFileSync("7z", ["x", "-y", `-o${target}`, file], { stdio: "ignore" });
  } catch {
    execFileSync(join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe"), ["-xf", file, "-C", target]);
  }
  rmSync(out, { recursive: true, force: true });
  for (const f of FILES) {
    mkdirSync(dirname(join(out, f)), { recursive: true });
    copyFileSync(join(target, f), join(out, f));
  }
  writeFileSync(stamp, MPV.release + "\n");
  console.log(`mpv ${MPV.release} : prêt dans src-tauri/mpv-win`);
} finally {
  rmSync(work, { recursive: true, force: true });
}
