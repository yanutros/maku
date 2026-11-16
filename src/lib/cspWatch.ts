// Refus de la CSP (src-tauri/tauri.conf.json) pendant l'utilisation réelle : chaque ressource ou requête que la page
// n'a pas le droit de charger est notée dans la console et, sur Android, dans le journal de diagnostic (DiagLog.kt).
// Les tests vérifient la démo ; ce relevé couvre ce que la démo ne fait pas (vrai serveur, appareils, lecteurs).
import { diag } from "./player/bridge";

type Violation = Pick<SecurityPolicyViolationEvent, "effectiveDirective" | "blockedURI" | "sourceFile" | "lineNumber">;

/**
 * Le refus en une ligne, sans donnée personnelle : la directive, et de l'adresse refusée son seul schéma et son port
 * (ni l'hôte, ni le chemin, ni les paramètres, qui peuvent porter un jeton). Le fichier de la page qui l'a demandée,
 * sans son dossier.
 */
export function describeViolation(v: Violation): string {
  let target = v.blockedURI || "?";
  try {
    const url = new URL(v.blockedURI);
    target = `${url.protocol}//…${url.port ? `:${url.port}` : ""}`;
  } catch {
    // « inline », « eval », « data »… : des mots de la CSP, sans adresse.
  }
  const file = v.sourceFile ? ` (${v.sourceFile.split(/[/?#]/).filter(Boolean).pop() ?? "?"}${v.lineNumber ? `:${v.lineNumber}` : ""})` : "";
  return `CSP : ${v.effectiveDirective} refusé pour ${target}${file}`;
}

const seen = new Set<string>();

if (typeof document !== "undefined") {
  document.addEventListener("securitypolicyviolation", (e) => {
    const line = describeViolation(e);
    // Une même ressource refusée en boucle (une image dans une liste) : notée une fois.
    if (seen.has(line)) return;
    seen.add(line);
    console.warn(line);
    diag(line);
  });
}
