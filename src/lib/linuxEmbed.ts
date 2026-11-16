// Linux : la vidéo dans la fenêtre de Maku (réglage expérimental). Le Rust le lit au démarrage, avant de créer la
// fenêtre (src-tauri/src/overlay.rs) : le changer demande de relancer Maku.
const KEY = "maku.linuxEmbed";

export function linuxEmbed(): boolean {
  try {
    return localStorage.getItem(KEY) === "1";
  } catch {
    return false;
  }
}

export function setLinuxEmbed(on: boolean) {
  try {
    if (on) localStorage.setItem(KEY, "1");
    else localStorage.removeItem(KEY);
  } catch {
    // Stockage indisponible : le réglage ne peut pas être gardé.
  }
}
