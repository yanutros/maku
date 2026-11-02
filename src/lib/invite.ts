// Invitations : l'administrateur partage ses adresses Seerr et Tracearr dans un code (et son QR
// code) ; l'utilisateur l'applique et Maku se connecte aux deux avec son compte Plex.
// Le code ne contient que des adresses, aucun identifiant.
import jsQR from "jsqr";
import { api, errorText, tracearrLogin } from "./api";
import { t } from "./i18n";

const PREFIX = "MAKU-INVITE:";
/** Les invitations créées par Plexr (captures encore en circulation) restent valables. */
const LEGACY_PREFIX = "PLEXR-INVITE:";

export interface Invite {
  seerr?: string;
  tracearr?: string;
}

/** Avancement, tel qu'affiché par la page du téléphone (voir relay.html). */
export interface InviteStatus {
  step: "running" | "done" | "error";
  seerr?: "run" | "ok" | "err";
  seerrError?: string;
  tracearr?: "run" | "ok" | "err";
  tracearrError?: string;
  authUrl?: string;
  error?: string;
}

export function encodeInvite(invite: Invite): string {
  const json = JSON.stringify({ s: invite.seerr || undefined, t: invite.tracearr || undefined });
  const bytes = new TextEncoder().encode(json);
  const b64 = btoa(String.fromCharCode(...bytes)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  return PREFIX + b64;
}

export function decodeInvite(text: string): Invite | null {
  const code = text.trim();
  const prefix = [PREFIX, LEGACY_PREFIX].find((p) => code.startsWith(p));
  if (!prefix) return null;
  try {
    const b64 = code.slice(prefix.length).replace(/-/g, "+").replace(/_/g, "/");
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    const raw = JSON.parse(new TextDecoder().decode(bytes));
    const url = (v: unknown) => (typeof v === "string" && /^https?:\/\//.test(v) ? v : undefined);
    const invite = { seerr: url(raw.s), tracearr: url(raw.t) };
    return invite.seerr || invite.tracearr ? invite : null;
  } catch {
    return null;
  }
}

/** Texte du QR code contenu dans une image (photo ou capture d'écran). */
export async function readQrImage(file: Blob): Promise<string | null> {
  const img = await createImageBitmap(file);
  const scale = Math.min(1, 1200 / Math.max(img.width, img.height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(img.width * scale);
  canvas.height = Math.round(img.height * scale);
  const ctx = canvas.getContext("2d")!;
  ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
  const data = ctx.getImageData(0, 0, canvas.width, canvas.height);
  return jsQR(data.data, data.width, data.height, { inversionAttempts: "attemptBoth" })?.data ?? null;
}

/**
 * Enregistre les adresses de l'invitation et s'y connecte. `report` reçoit chaque étape. Avec
 * `phone`, le lien d'autorisation Tracearr y figure (`authUrl`) pour être ouvert sur le téléphone ;
 * sinon il s'ouvre comme d'habitude (navigateur, ou QR code sur téléviseur).
 */
export async function applyInvite(
  invite: Invite,
  report: (status: InviteStatus) => void,
  phone = false,
): Promise<InviteStatus> {
  const status: InviteStatus = { step: "running" };
  const send = () => report({ ...status });
  if (invite.seerr) {
    status.seerr = "run";
    send();
    try {
      await api.seerrConfigure(invite.seerr);
      status.seerr = "ok";
    } catch (e) {
      status.seerr = "err";
      status.seerrError = errorText(e);
    }
  }
  if (invite.tracearr) {
    status.tracearr = "run";
    send();
    try {
      await api.tracearrConfigure(invite.tracearr);
      await tracearrLogin(
        phone
          ? (url) => {
              status.authUrl = url;
              send();
            }
          : undefined,
      );
      status.tracearr = "ok";
    } catch (e) {
      status.tracearr = "err";
      status.tracearrError = errorText(e);
    }
    status.authUrl = undefined;
  }
  const failed = status.seerr === "err" || status.tracearr === "err";
  status.step = failed ? "error" : "done";
  if (failed) status.error = t("invite.partialFailure");
  send();
  return status;
}
