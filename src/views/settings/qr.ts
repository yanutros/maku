import { tick } from "svelte";
import { reveal } from "../../lib/spatial";

/** QR code qui vient d'être déplié : la sélection descend dessus, et il vient à l'écran. */
export async function revealQr() {
  await tick();
  const el = document.querySelector<HTMLElement>(".settings .qr-open");
  el?.focus({ preventScroll: true });
  if (el) reveal(el);
}
