//! Lecture via mpv, piloté par son IPC JSON.
//!
//! Sous Windows, mpv dessine la vidéo dans la fenêtre de l'application (`--wid`) :
//! sa fenêtre est placée sous la webview, dont le fond devient transparent pendant
//! la lecture, si bien que la barre de contrôle HTML s'affiche par-dessus la vidéo.
//! On relaie les propriétés de mpv à l'interface et on remonte la progression à Plex.

use crate::media::MediaServer;
use crate::plex::PlayInfo;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

/// Au-delà de cette proportion regardée, l'élément est marqué comme vu.
const WATCHED_RATIO: f64 = 0.9;
const MPV_EXE: &str = if cfg!(windows) { "mpv.exe" } else { "mpv" };
/// Propriétés mpv relayées à l'interface (événement `player-prop`).
const OBSERVED: &[&str] = &[
    "time-pos",
    "duration",
    "pause",
    "volume",
    "mute",
    "track-list",
    "aid",
    "sid",
    "paused-for-cache",
    "demuxer-cache-time",
    "playlist-pos",
    "loop-playlist",
];

trait IpcStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> IpcStream for T {}

/// Lecture en cours. La lâcher (ou la remplacer) ferme mpv.
pub struct PlayerControl {
    commands: mpsc::UnboundedSender<Value>,
    _stop: oneshot::Sender<()>,
    /// Processus mpv, pour l'arrêter tout de suite à la fermeture de Maku (voir `kill_now`).
    pid: Option<u32>,
}

impl PlayerControl {
    /// Envoie une commande mpv, par ex. `["seek", 10, "relative"]`.
    pub fn send(&self, args: Vec<Value>) -> Result<()> {
        self.commands
            .send(json!({ "command": args }))
            .ok()
            .with_context(|| crate::i18n::tr("player.noPlayback"))
    }

    /// Fermeture de Maku : arrête mpv sur-le-champ (le suivi asynchrone n'en aurait plus le temps).
    pub fn kill_now(&self) {
        let Some(pid) = self.pid else { return };
        #[cfg(windows)]
        {
            use windows_sys::Win32::Foundation::CloseHandle;
            use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
            unsafe {
                let process = OpenProcess(PROCESS_TERMINATE, 0, pid);
                if !process.is_null() {
                    TerminateProcess(process, 0);
                    CloseHandle(process);
                }
            }
        }
        #[cfg(unix)]
        {
            let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
        }
    }
}

/// Windows : mpv est rattaché à un « objet tâche » de Maku, fermé par Windows quand Maku s'arrête
/// pour quelque raison que ce soit (fermeture, plantage, arrêt forcé) — et mpv avec lui. Sans cela,
/// la musique continuait après la fermeture de Maku, sans plus rien pour l'arrêter.
#[cfg(windows)]
fn tie_to_plexr(child: &Child) {
    use std::sync::OnceLock;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    // L'objet tâche n'est jamais fermé explicitement : il l'est à la fin du processus de Maku.
    static JOB: OnceLock<isize> = OnceLock::new();
    let job = *JOB.get_or_init(|| unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return 0;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        job as isize
    });
    if job != 0 {
        if let Some(process) = child.raw_handle() {
            unsafe {
                AssignProcessToJobObject(job as _, process as _);
            }
        }
    }
}

#[derive(Serialize, Clone)]
struct PropEvent<'a> {
    name: &'a str,
    data: &'a Value,
}

/// Cherche mpv : chemin des réglages, à côté de l'exécutable, puis dans le PATH.
pub fn find_mpv(configured: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = configured.filter(|p| !p.trim().is_empty()) {
        let path = PathBuf::from(p.trim());
        if path.is_file() {
            return Ok(path);
        }
        bail!(crate::i18n::tr_with("player.mpvNotAtPath", &[("path", &path.display().to_string())]));
    }
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) {
        candidates.push(dir.join("mpv").join(MPV_EXE));
        candidates.push(dir.join(MPV_EXE));
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|d| d.join(MPV_EXE)));
    }
    if cfg!(target_os = "macos") {
        // Une application lancée depuis le Finder n'hérite pas du PATH de Homebrew.
        candidates.push(PathBuf::from("/opt/homebrew/bin/mpv"));
        candidates.push(PathBuf::from("/usr/local/bin/mpv"));
        candidates.push(PathBuf::from("/Applications/mpv.app/Contents/MacOS/mpv"));
    }
    if cfg!(target_os = "linux") {
        candidates.push(PathBuf::from("/usr/bin/mpv"));
        candidates.push(PathBuf::from("/usr/local/bin/mpv"));
        // mpv installé en Flatpak : on passe par son lanceur exporté.
        candidates.push(PathBuf::from("/var/lib/flatpak/exports/bin/io.mpv.Mpv"));
    }
    if cfg!(windows) {
        candidates.push(PathBuf::from(r"C:\Program Files\mpv\mpv.exe"));
        // Emplacement de l'installeur winget « shinchiro.mpv ».
        candidates.push(PathBuf::from(r"C:\Program Files\MPV Player\mpv.exe"));
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            candidates.push(PathBuf::from(local).join("Programs").join("mpv").join(MPV_EXE));
        }
    }
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .with_context(|| crate::i18n::tr("player.mpvNotFound"))
}

fn ipc_path() -> String {
    let id = uuid::Uuid::new_v4().simple().to_string();
    if cfg!(windows) {
        format!(r"\\.\pipe\plexr-mpv-{id}")
    } else {
        std::env::temp_dir().join(format!("plexr-mpv-{id}.sock")).display().to_string()
    }
}

#[cfg(windows)]
async fn connect_ipc(path: &str) -> Result<Box<dyn IpcStream>> {
    use tokio::net::windows::named_pipe::ClientOptions;
    for _ in 0..50 {
        if let Ok(client) = ClientOptions::new().open(path) {
            return Ok(Box::new(client));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    bail!(crate::i18n::tr("player.ipcUnavailable"))
}

#[cfg(unix)]
async fn connect_ipc(path: &str) -> Result<Box<dyn IpcStream>> {
    for _ in 0..50 {
        if let Ok(stream) = tokio::net::UnixStream::connect(path).await {
            return Ok(Box::new(stream));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    bail!(crate::i18n::tr("player.ipcUnavailable"))
}

#[cfg(windows)]
mod embed {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, GetClassNameW, SetWindowPos, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };

    unsafe extern "system" fn find_mpv_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let mut name = [0u16; 32];
        let len = GetClassNameW(hwnd, name.as_mut_ptr(), name.len() as i32).max(0) as usize;
        if String::from_utf16_lossy(&name[..len]) == "mpv" {
            *(lparam as *mut isize) = hwnd as isize;
            return 0;
        }
        1
    }

    /// Place la fenêtre vidéo de mpv sous la webview. Renvoie `false` tant qu'elle n'existe pas.
    pub fn send_video_to_back(parent: isize) -> bool {
        let mut found: isize = 0;
        unsafe {
            EnumChildWindows(parent as HWND, Some(find_mpv_window), &mut found as *mut isize as LPARAM);
            if found == 0 {
                return false;
            }
            SetWindowPos(found as HWND, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
        true
    }
}

/// Élément de la file de lecture, pour remonter la progression à Plex.
pub struct Entry {
    pub rating_key: String,
    pub duration_ms: u64,
}

/// Ce que mpv doit lire.
pub enum Source {
    /// Une vidéo, intégrée dans la fenêtre `parent` (HWND) ; sans elle, mpv ouvre sa propre fenêtre.
    Video { info: PlayInfo, parent: Option<isize> },
    /// Une file de titres audio, sans fenêtre, en commençant par l'indice `start`.
    Music { urls: Vec<String>, start: usize },
}

/// Lance mpv puis suit la lecture en tâche de fond. À la fin, l'événement
/// `player-stopped` porte `session_id`, pour ignorer une lecture déjà remplacée.
pub fn launch(
    app: AppHandle,
    server: MediaServer,
    mpv: PathBuf,
    session_id: String,
    entries: Vec<Entry>,
    source: Source,
) -> Result<PlayerControl> {
    let pipe = ipc_path();
    let mut cmd = Command::new(&mpv);
    cmd.arg(format!("--input-ipc-server={pipe}"))
        .arg("--keep-open=no")
        .arg("--idle=no")
        // Pas de ligne d'état dans la console : les messages d'erreur restent affichés.
        .arg("--quiet")
        .kill_on_drop(true);
    let mut start = 0;
    let mut offset_ms = 0;
    let mut parent = None;
    let mut playlist_file = None;
    match &source {
        Source::Video { info, parent: wid } => {
            cmd.arg(&info.url)
                .arg(format!("--title={}", info.title))
                .arg("--force-window=yes")
                .arg("--hwdec=auto-safe")
                .arg(format!("--sid={}", info.sid));
            match wid {
                // Vidéo incrustée : l'interface de Maku fournit ses propres contrôles et raccourcis.
                Some(hwnd) => {
                    cmd.arg(format!("--wid={hwnd}"))
                        .arg("--osc=no")
                        .arg("--input-default-bindings=no")
                        .arg("--osd-level=0")
                        .arg("--input-vo-keyboard=no")
                        .arg("--input-cursor=no");
                }
                // Fenêtre mpv séparée (Linux, macOS) : on garde ses contrôles, Maku la pilote aussi.
                None => {
                    cmd.arg("--fullscreen");
                }
            }
            if info.offset_ms > 0 {
                cmd.arg(format!("--start={}", info.offset_ms / 1000));
            }
            if let Some(aid) = info.aid {
                cmd.arg(format!("--aid={aid}"));
            }
            for file in &info.sub_files {
                cmd.arg(format!("--sub-file={file}"));
            }
            offset_ms = info.offset_ms;
            parent = *wid;
        }
        Source::Music { urls, start: first } => {
            // Une longue file dépasserait la limite de la ligne de commande : on passe par une playlist.
            let path = std::env::temp_dir().join(format!("plexr-queue-{}.m3u", uuid::Uuid::new_v4().simple()));
            std::fs::write(&path, format!("#EXTM3U\n{}\n", urls.join("\n")))
                .with_context(|| crate::i18n::tr("player.queueWriteFailed"))?;
            cmd.arg(format!("--playlist={}", path.display()))
                .arg(format!("--playlist-start={first}"))
                .arg("--vid=no")
                .arg("--input-default-bindings=no")
                .arg("--force-window=no")
                .arg("--audio-display=no")
                .arg("--gapless-audio=weak");
            start = *first;
            playlist_file = Some(path);
        }
    }
    let child = cmd.spawn().with_context(|| crate::i18n::tr_with("player.launchFailed", &[("path", &mpv.display().to_string())]));
    let child = match child {
        Ok(c) => c,
        Err(e) => {
            if let Some(path) = &playlist_file {
                let _ = std::fs::remove_file(path);
            }
            return Err(e);
        }
    };

    #[cfg(windows)]
    tie_to_plexr(&child);
    let pid = child.id();
    let (commands, command_rx) = mpsc::unbounded_channel();
    let (stop, stop_rx) = oneshot::channel();
    tauri::async_runtime::spawn(async move {
        let session = Session { app: &app, server: &server, entries: &entries, parent, id: &session_id };
        if let Err(e) = session.follow(child, stop_rx, command_rx, &pipe, start, offset_ms).await {
            eprintln!("Suivi de lecture interrompu : {e:#}");
        }
        if let Some(path) = playlist_file {
            let _ = std::fs::remove_file(path);
        }
        let _ = app.emit("player-stopped", &session_id);
    });
    Ok(PlayerControl { commands, _stop: stop, pid })
}

struct Session<'a> {
    app: &'a AppHandle,
    server: &'a MediaServer,
    entries: &'a [Entry],
    parent: Option<isize>,
    /// Identifiant de la lecture, envoyé avec la progression (voir `PlexServer::timeline`).
    id: &'a str,
}

impl Session<'_> {
    async fn follow(
        &self,
        mut child: Child,
        mut stop: oneshot::Receiver<()>,
        mut commands: mpsc::UnboundedReceiver<Value>,
        pipe: &str,
        start: usize,
        offset_ms: u64,
    ) -> Result<()> {
        let mut current = start.min(self.entries.len().saturating_sub(1));
        let mut position = offset_ms as f64 / 1000.0;
        let mut duration = self.duration_of(current);

        match connect_ipc(pipe).await {
            Ok(stream) => {
                #[cfg(windows)]
                if let Some(parent) = self.parent {
                    tauri::async_runtime::spawn_blocking(move || {
                        for _ in 0..100 {
                            if embed::send_video_to_back(parent) {
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(50));
                        }
                    });
                }
                #[cfg(not(windows))]
                let _ = self.parent;

                let (reader, mut writer) = tokio::io::split(stream);
                let mut observe = String::new();
                for (i, name) in OBSERVED.iter().enumerate() {
                    observe.push_str(&json!({ "command": ["observe_property", i + 1, name] }).to_string());
                    observe.push('\n');
                }
                writer.write_all(observe.as_bytes()).await?;

                let mut lines = BufReader::new(reader).lines();
                let mut paused = false;
                let mut last_time_event = Instant::now() - Duration::from_secs(1);
                let mut tick = tokio::time::interval(Duration::from_secs(10));
                loop {
                    tokio::select! {
                        line = lines.next_line() => {
                            let Ok(Some(line)) = line else { break };
                            let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
                            if msg["event"] != "property-change" {
                                continue;
                            }
                            let Some(name) = msg["name"].as_str() else { continue };
                            let data = &msg["data"];
                            match name {
                                "time-pos" => {
                                    position = data.as_f64().unwrap_or(position);
                                    // La position change à chaque image : on limite à 4 mises à jour par seconde.
                                    if last_time_event.elapsed() < Duration::from_millis(250) {
                                        continue;
                                    }
                                    last_time_event = Instant::now();
                                }
                                "duration" => duration = data.as_f64().unwrap_or(duration),
                                "pause" => {
                                    paused = data.as_bool().unwrap_or(false);
                                    self.report(current, if paused { "paused" } else { "playing" }, position, duration).await;
                                }
                                // Passage au titre suivant de la file : on clôt le précédent.
                                "playlist-pos" => {
                                    if let Some(next) = data.as_u64().map(|n| n as usize) {
                                        if next != current && next < self.entries.len() {
                                            self.finish(current, position, duration).await;
                                            current = next;
                                            position = 0.0;
                                            duration = self.duration_of(current);
                                        }
                                    }
                                }
                                _ => {}
                            }
                            let _ = self.app.emit("player-prop", PropEvent { name, data });
                        }
                        Some(command) = commands.recv() => {
                            writer.write_all(format!("{command}\n").as_bytes()).await?;
                        }
                        _ = tick.tick() => {
                            self.report(current, if paused { "paused" } else { "playing" }, position, duration).await;
                        }
                        _ = &mut stop => {
                            let _ = child.kill().await;
                            break;
                        }
                    }
                }
            }
            Err(_) => {
                // Sans IPC on ne suit pas la progression, mais on attend la fin du lecteur.
                tokio::select! {
                    _ = child.wait() => {}
                    _ = &mut stop => { let _ = child.kill().await; }
                }
            }
        }
        let _ = child.wait().await;
        self.finish(current, position, duration).await;
        Ok(())
    }

    fn duration_of(&self, index: usize) -> f64 {
        self.entries.get(index).map_or(0.0, |e| e.duration_ms as f64 / 1000.0)
    }

    /// Fin d'un élément : dernière position, puis « vu » / « écouté » au-delà de 90 %.
    async fn finish(&self, index: usize, position: f64, duration: f64) {
        self.report(index, "stopped", position, duration).await;
        let Some(entry) = self.entries.get(index) else { return };
        if duration > 0.0 && position / duration >= WATCHED_RATIO {
            if let Err(e) = self.server.set_watched(&entry.rating_key, true).await {
                eprintln!("Impossible de marquer comme vu : {e:#}");
            }
        }
    }

    async fn report(&self, index: usize, state: &str, position: f64, duration: f64) {
        let Some(entry) = self.entries.get(index) else { return };
        let to_ms = |s: f64| (s.max(0.0) * 1000.0) as u64;
        if let Err(e) = self.server.timeline(&entry.rating_key, state, to_ms(position), to_ms(duration), self.id).await {
            eprintln!("Timeline Plex non envoyée : {e:#}");
        }
    }
}
