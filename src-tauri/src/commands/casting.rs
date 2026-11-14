//! Chromecast : appareils du réseau, lecture et file confiées au lecteur de Google.

use crate::*;

/// Appareils Chromecast du réseau local.
#[tauri::command]
pub async fn cast_devices() -> CmdResult<Vec<cast::Device>> {
    Ok(cast::discover(Duration::from_secs(3)).await?)
}

/// Envoie la file sur un appareil, à partir du titre `index` (`position` secondes dedans).
#[tauri::command]
pub async fn cast_start(app: AppHandle, state: State<'_, AppState>, device: cast::Device, tracks: Vec<cast::Track>, index: usize, position: f64) -> CmdResult<()> {
    let old = state.cast.lock().unwrap().take();
    if let Some(old) = old {
        old.send(cast::Command::Stop);
    }
    let session = cast::start(app, device, tracks, index, position).await?;
    *state.cast.lock().unwrap() = Some(session);
    Ok(())
}

/// Commande à l'appareil : `toggle`, `play`, `pause`, `seek` (secondes), `jump` (titre), `next`, `previous`,
/// `volume` (0 à 100), `mute` (1 ou 0).
#[tauri::command]
pub fn cast_command(state: State<'_, AppState>, action: String, value: Option<f64>) -> CmdResult<()> {
    let v = value.unwrap_or(0.0);
    let c = match action.as_str() {
        "toggle" => cast::Command::Toggle,
        "play" => cast::Command::Play,
        "pause" => cast::Command::Pause,
        "seek" => cast::Command::Seek(v),
        "jump" => cast::Command::Jump(v as usize),
        "next" => cast::Command::Next,
        "previous" => cast::Command::Previous,
        "volume" => cast::Command::Volume(v / 100.0),
        "mute" => cast::Command::Mute(v != 0.0),
        _ => return Ok(()),
    };
    if let Some(s) = state.cast.lock().unwrap().as_ref() {
        s.send(c);
    }
    Ok(())
}

/// Arrête la diffusion (l'appareil quitte le lecteur de Google).
#[tauri::command]
pub fn cast_stop(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(s) = state.cast.lock().unwrap().take() {
        s.send(cast::Command::Stop);
    }
    Ok(())
}

/// Même chose pendant une diffusion Chromecast.
#[tauri::command]
pub fn cast_queue_insert(state: State<'_, AppState>, tracks: Vec<cast::Track>, at: usize) -> CmdResult<()> {
    if let Some(s) = state.cast.lock().unwrap().as_ref() {
        s.send(cast::Command::Insert(tracks, at));
    }
    Ok(())
}

#[tauri::command]
pub fn cast_queue_move(state: State<'_, AppState>, from: usize, to: usize) -> CmdResult<()> {
    if let Some(s) = state.cast.lock().unwrap().as_ref() {
        s.send(cast::Command::Reorder(from, to));
    }
    Ok(())
}

/// Bouton Aléatoire du lecteur, pendant une diffusion : la file de l'appareil prend le nouvel ordre.
#[tauri::command]
pub fn cast_queue_order(state: State<'_, AppState>, order: Vec<usize>) -> CmdResult<()> {
    if let Some(s) = state.cast.lock().unwrap().as_ref() {
        s.send(cast::Command::Order(order));
    }
    Ok(())
}

/// Bouton Répéter, pendant une diffusion : l'appareil répète la file, le titre, ou rien.
#[tauri::command]
pub fn cast_repeat(state: State<'_, AppState>, mode: String) -> CmdResult<()> {
    let mode = match mode.as_str() {
        "REPEAT_ALL" => "REPEAT_ALL",
        "REPEAT_SINGLE" => "REPEAT_SINGLE",
        _ => "REPEAT_OFF",
    };
    if let Some(s) = state.cast.lock().unwrap().as_ref() {
        s.send(cast::Command::Repeat(mode));
    }
    Ok(())
}
