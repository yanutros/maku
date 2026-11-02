// Type d'appareil, pour adapter l'interface : ordinateur, tablette tactile ou téléviseur.
import { invoke } from "@tauri-apps/api/core";

const TV_WIDTH = 1280;

export const device = { os: "", mobile: false, tv: false, touch: false, phone: false };

type AndroidBridge = { isTv(): boolean; insets?(): string };

/**
 * Largeur de page fixe sur téléviseur. `force` : après une vidéo, le WebView garde une échelle de 1
 * malgré la balise ; la faire passer par la largeur de l'écran puis revenir à 1280 px la recalcule.
 */
function setTvViewport(force = false) {
  const meta = document.querySelector('meta[name="viewport"]');
  if (!meta) return;
  if (!force) return meta.setAttribute("content", `width=${TV_WIDTH}`);
  meta.setAttribute("content", "width=device-width");
  setTimeout(() => meta.setAttribute("content", `width=${TV_WIDTH}`), 120);
}

/** Classes posées sur <html> : `mobile`, `tv`, `touch`, `phone`. */
export async function detectDevice() {
  device.os = (await invoke<string | null>("platform").catch(() => null)) ?? "";
  device.mobile = device.os === "android" || device.os === "ios";
  // Android indique lui-même s'il tourne sur un téléviseur (voir MainActivity.kt) ; à défaut,
  // un appareil Android sans écran tactile en est un.
  const android = (window as { PlexrDevice?: AndroidBridge }).PlexrDevice;
  const androidTv = android?.isTv();
  device.touch = !androidTv && (navigator.maxTouchPoints > 0 || matchMedia("(pointer: coarse)").matches);
  // `?device=tv` impose l'interface du téléviseur (essais dans un navigateur).
  const params = new URLSearchParams(location.search);
  device.tv = params.get("device") === "tv" || (device.os === "android" && (androidTv ?? !device.touch));
  // Un téléviseur Full HD n'offre que 960 × 540 px à la page (densité ×2) : on l'affiche
  // comme un écran de 1280 px de large, que le WebView réduit pour tenir dans l'image.
  if (device.tv) {
    setTvViewport();
    // À la fin d'une vidéo (4K HDR notamment), le WebView remet l'échelle à 1 et la page apparaît
    // agrandie : dès que la zone visible ne couvre plus les 1280 px de la page, on rétablit l'échelle.
    let pending: ReturnType<typeof setTimeout> | undefined;
    const check = () => {
      clearTimeout(pending);
      // Laisse le WebView finir son propre changement avant de corriger.
      pending = setTimeout(() => {
        if (Math.abs((window.visualViewport?.width ?? window.innerWidth) - TV_WIDTH) > 1) setTvViewport(true);
      }, 400);
    };
    window.visualViewport?.addEventListener("resize", check);
    window.addEventListener("resize", check);
    document.addEventListener("visibilitychange", () => document.visibilityState === "visible" && check());
  }
  // Téléphone : petit écran tactile (le plus petit côté sous 600 px), avec une interface à part
  // (onglets en bas, rangées d'affiches). `?device=phone` l'impose (essais dans un navigateur).
  const forced = params.get("device") === "phone";
  device.phone = forced || (device.mobile && !device.tv && Math.min(screen.width, screen.height) < 600);
  if (forced) device.touch = true;
  const root = document.documentElement;
  // Barres d'état et de navigation d'Android, que l'interface ne doit pas recouvrir.
  const [top, bottom] = (android?.insets?.() ?? "0,0").split(",").map(Number);
  root.style.setProperty("--inset-top", `${top || 0}px`);
  root.style.setProperty("--inset-bottom", `${bottom || 0}px`);
  root.classList.toggle("phone", device.phone);
  root.classList.toggle("mobile", device.mobile);
  root.classList.toggle("tv", device.tv);
  root.classList.toggle("touch", device.touch);
}
