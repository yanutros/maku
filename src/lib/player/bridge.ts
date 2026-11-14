// Pont vers Android (MainActivity.kt) : lecteur vidéo natif, journal de diagnostic, son envoyé tel quel à l'ampli.

/** Lecteur vidéo natif d'Android (NativeVideo.kt), quand l'application le fournit. */
type VideoBridge = {
  videoOpen?(json: string): void;
  videoCommand?(name: string, value: string): void;
  videoClose?(): void;
  audioPassthrough?(): string;
  diagLog?(message: string): void;
  diagText?(): string;
  diagShare?(): void;
};
export const bridge = (globalThis as { PlexrDevice?: VideoBridge }).PlexrDevice;
export const nativeVideo = !!bridge?.videoOpen;

/** Journal de diagnostic d'Android (DiagLog.kt) : lignes de la page, à transmettre en cas de blocage. */
export const diagAvailable = !!bridge?.diagText;
export function diag(message: string) {
  try {
    bridge?.diagLog?.(message);
  } catch {
    // Journal indisponible : la lecture continue.
  }
}
export const diagText = () => bridge?.diagText?.() ?? "";
export const diagShare = () => bridge?.diagShare?.();
if (diagAvailable) {
  addEventListener("error", (e) => diag(`Erreur de la page : ${e.message} (${e.filename?.split("/").pop()}:${e.lineno})`));
  addEventListener("unhandledrejection", (e) => diag(`Erreur de la page : ${String(e.reason).slice(0, 300)}`));
}

/** Lecteur natif d'Android : le son peut être envoyé tel quel à l'ampli (réglage de l'appareil). */
export const passthroughAvailable = nativeVideo && !!bridge?.audioPassthrough;
const PASSTHROUGH_KEY = "maku.passthrough";
export function passthroughMode(): "auto" | "off" {
  try {
    return localStorage.getItem(PASSTHROUGH_KEY) === "off" ? "off" : "auto";
  } catch {
    return "auto";
  }
}
export function setPassthroughMode(mode: "auto" | "off") {
  try {
    localStorage.setItem(PASSTHROUGH_KEY, mode);
  } catch {
    // Stockage indisponible : le mode automatique reste.
  }
}
/** Formats que la sortie audio accepte tels quels, et son nombre de canaux. */
export function passthroughFormats(): { formats: string[]; channels: number } | null {
  try {
    return bridge?.audioPassthrough ? JSON.parse(bridge.audioPassthrough()) : null;
  } catch {
    return null;
  }
}
