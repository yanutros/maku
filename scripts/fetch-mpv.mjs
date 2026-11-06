// Prépare le lecteur mpv embarqué dans le paquet de Maku, pour que la lecture fonctionne sans rien
// installer d'autre, et lise directement presque tous les fichiers (sans conversion par le serveur) :
// - Windows (src-tauri/mpv-win) : version épinglée de mpv-winbuild-cmake
//   (https://github.com/shinchiro/mpv-winbuild-cmake, GPL) ;
// - macOS (src-tauri/mpv-mac) : versions officielles de mpv (https://github.com/mpv-player/mpv, GPL),
//   Apple Silicon et Intel, gardées en archives : Maku les déploie au premier lancement (player.rs).
// Chaque archive est vérifiée par son empreinte SHA-256. Lancé avant chaque compilation Windows et
// macOS (voir tauri.windows.conf.json et tauri.macos.conf.json) ; ne fait rien ailleurs.
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

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// mpv pour macOS 15 et plus (versions officielles) : le .app complet, bibliothèques comprises.
const MAC = {
  release: "v0.42.0",
  builds: [
    { arch: "arm64", file: "mpv-v0.42.0-macos-15-arm.zip", sha256: "be1b8d2445e6880860869c080b8710dee189721f1fa4689840db6a2c10d7be69" },
    { arch: "x86_64", file: "mpv-v0.42.0-macos-15-intel.zip", sha256: "2d2f34350ff844b6014c35f21ca142937275e19c0c673150a8564fcd2f3bfa14" },
  ],
};

async function download(url, sha256) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`mpv : téléchargement impossible (${response.status})`);
  const data = Buffer.from(await response.arrayBuffer());
  const digest = createHash("sha256").update(data).digest("hex");
  if (digest !== sha256) throw new Error(`mpv : empreinte inattendue (${digest}), archive refusée`);
  return data;
}

if (process.platform === "darwin") {
  const out = join(root, "src-tauri", "mpv-mac");
  const stamp = join(out, "VERSION");
  const ready = () => MAC.builds.every((b) => existsSync(join(out, `mpv-${b.arch}.tar.gz`)));
  if (existsSync(stamp) && readFileSync(stamp, "utf8").trim() === MAC.release && ready()) process.exit(0);
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  const work = mkdtempSync(join(tmpdir(), "plexr-mpv-"));
  try {
    for (const b of MAC.builds) {
      console.log(`mpv ${MAC.release} (${b.arch}) : téléchargement…`);
      const zip = join(work, b.file);
      writeFileSync(zip, await download(`https://github.com/mpv-player/mpv/releases/download/${MAC.release}/${b.file}`, b.sha256));
      // L'archive contient mpv.tar.gz, qui garde les signatures de mpv.app : on le garde tel quel.
      writeFileSync(join(out, `mpv-${b.arch}.tar.gz`), execFileSync("unzip", ["-p", zip, "mpv.tar.gz"], { maxBuffer: 512 * 1024 * 1024 }));
    }
    writeFileSync(stamp, MAC.release + "\n");
    console.log(`mpv ${MAC.release} : prêt dans src-tauri/mpv-mac`);
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
  process.exit(0);
}

if (process.platform !== "win32") process.exit(0);
const out = join(root, "src-tauri", "mpv-win");
const stamp = join(out, "VERSION");

if (existsSync(stamp) && readFileSync(stamp, "utf8").trim() === MPV.release && FILES.every((f) => existsSync(join(out, f)))) {
  process.exit(0);
}

const url = `https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/${MPV.release}/${MPV.file}`;
console.log(`mpv ${MPV.release} : téléchargement…`);
const archive = await download(url, MPV.sha256);

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
