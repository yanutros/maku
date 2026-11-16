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
/// Temps laissé au serveur pour recevoir la progression pendant la lecture. Au-delà, l'envoi est abandonné : le
/// suivant, dix secondes plus tard, donnera une position plus récente.
const REPORT_TIMEOUT: Duration = Duration::from_secs(8);

/// Envoi de la progression en cours, mené à côté du suivi de mpv.
type Report<'a> = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>>;

/// Progression d'un titre envoyée au serveur, sans jamais dépasser `REPORT_TIMEOUT`.
fn progress<'a>(server: &'a MediaServer, id: &'a str, rating_key: String, state: &'static str, position: f64, duration: f64) -> Report<'a> {
    let to_ms = |s: f64| (s.max(0.0) * 1000.0) as u64;
    Box::pin(async move {
        match tokio::time::timeout(REPORT_TIMEOUT, server.timeline(&rating_key, state, to_ms(position), to_ms(duration), id)).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => eprintln!("Timeline Plex non envoyée : {}", crate::net::redact(&format!("{e:#}"))),
            Err(_) => eprintln!("Timeline Plex non envoyée : le serveur ne répond pas"),
        }
    })
}
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
    "loop-file",
    // Vrai dès que mpv affiche l'image : la page ne devient transparente qu'à ce moment-là.
    "vo-configured",
];

trait IpcStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> IpcStream for T {}

/// Ordre transmis au suivi de la lecture : une commande mpv, ou une modification de la file musicale
/// (mpv et la liste des titres suivis changent ensemble, pour que la progression aille au bon titre).
enum Control {
    Mpv(Value),
    /// Titres à insérer à la position `at` de la file.
    Insert { entries: Vec<Entry>, urls: Vec<String>, at: usize },
    /// Titre déplacé de `from` à `to` (sa position finale).
    Move { from: usize, to: usize },
    /// Nouvel ordre des titres à venir : `order[k]` est la position d'avant du titre placé en `k` (bouton
    /// Aléatoire). Le titre en cours et ceux d'avant gardent leur place.
    Order(Vec<usize>),
}

/// Lecture en cours. La lâcher (ou la remplacer) ferme mpv.
pub struct PlayerControl {
    commands: mpsc::UnboundedSender<Control>,
    _stop: oneshot::Sender<()>,
    /// Processus mpv, pour l'arrêter tout de suite à la fermeture de Maku (voir `kill_now`).
    pid: Option<u32>,
}

impl PlayerControl {
    /// Envoie une commande mpv, par ex. `["seek", 10, "relative"]`.
    pub fn send(&self, args: Vec<Value>) -> Result<()> {
        self.control(Control::Mpv(json!({ "command": args })))
    }

    /// Insère des titres dans la file musicale, à la position `at` (au-delà : à la fin).
    pub fn insert(&self, entries: Vec<Entry>, urls: Vec<String>, at: usize) -> Result<()> {
        self.control(Control::Insert { entries, urls, at })
    }

    /// Déplace un titre de la file musicale de `from` à `to`.
    pub fn move_entry(&self, from: usize, to: usize) -> Result<()> {
        self.control(Control::Move { from, to })
    }

    /// Remet la file musicale dans un nouvel ordre (voir `Control::Order`).
    pub fn reorder(&self, order: Vec<usize>) -> Result<()> {
        self.control(Control::Order(order))
    }

    fn control(&self, c: Control) -> Result<()> {
        self.commands.send(c).ok().with_context(|| crate::i18n::tr("player.noPlayback"))
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
/// Le chemin indiqué dans les Réglages désigne-t-il mpv ? D'après le nom du fichier : Maku lance ce programme à
/// chaque lecture, et ne doit pas pouvoir être amené à en lancer un autre.
pub fn is_mpv_name(path: &std::path::Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_ascii_lowercase();
    // « io.mpv.Mpv » : le lanceur de mpv installé en Flatpak.
    matches!(name.as_str(), "mpv" | "mpv.exe" | "mpv.com" | "io.mpv.mpv")
}

pub fn find_mpv(configured: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = configured.filter(|p| !p.trim().is_empty()) {
        let path = PathBuf::from(p.trim());
        if !is_mpv_name(&path) {
            bail!(crate::i18n::tr_with("player.notMpv", &[("path", &path.display().to_string())]));
        }
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
    /// Fichier lu par mpv (musique), pour remettre la file dans un autre ordre.
    pub url: String,
}

/// Ce que mpv doit lire.
pub enum Source {
    /// Une vidéo, intégrée dans la fenêtre `parent` (HWND) ; sans elle, mpv ouvre sa propre fenêtre. `under` : cette
    /// fenêtre est placée sous celle de Maku, qui garde les commandes (Linux, voir `overlay`).
    Video { info: PlayInfo, parent: Option<isize>, under: bool },
    /// Une file de titres audio, sans fenêtre, en commençant par l'indice `start`.
    Music { urls: Vec<String>, start: usize },
}

/// Fichier temporaire lisible par son seul propriétaire (Linux, macOS) : il contient des adresses avec leur jeton.
fn private_temp(name: &str, text: &str) -> Result<PathBuf> {
    use std::io::Write;
    let path = std::env::temp_dir().join(format!("plexr-{name}-{}", uuid::Uuid::new_v4().simple()));
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(&path)?.write_all(text.as_bytes())?;
    Ok(path)
}

/// Sous-titres externes, dans un fichier de réglages pour mpv (`--include`). La forme « %longueur%valeur » prend
/// l'adresse telle quelle, quels que soient ses caractères.
fn sub_files_conf(files: &[String]) -> String {
    files.iter().map(|f| format!("sub-files-append=%{}%{f}\n", f.len())).collect()
}

/// La commande mpv prête à être lancée, et ce qu'il faut pour suivre la lecture.
struct Prepared {
    cmd: Command,
    start: usize,
    offset_ms: u64,
    parent: Option<isize>,
    /// Fichiers temporaires à supprimer quand mpv s'arrête.
    temp: Vec<PathBuf>,
}

/// Prépare la commande mpv. Les adresses des fichiers portent le jeton du serveur : elles passent par des fichiers
/// temporaires privés, jamais par la ligne de commande, que les autres comptes de la machine peuvent lire.
fn prepare(mpv: &std::path::Path, pipe: &str, source: &Source) -> Result<Prepared> {
    let mut cmd = Command::new(mpv);
    cmd.arg(format!("--input-ipc-server={pipe}"))
        .arg("--keep-open=no")
        .arg("--idle=no")
        // Pas de ligne d'état dans la console : les messages d'erreur restent affichés.
        .arg("--quiet")
        .kill_on_drop(true);
    let mut start = 0;
    let mut offset_ms = 0;
    let mut parent = None;
    let mut temp = Vec::new();
    let queue_failed = || crate::i18n::tr("player.queueWriteFailed");
    match source {
        Source::Video { info, parent: wid, under } => {
            let playlist = private_temp("video.m3u", &format!("#EXTM3U\n{}\n", info.url)).with_context(queue_failed)?;
            cmd.arg(format!("--playlist={}", playlist.display()))
                .arg(format!("--title={}", info.title))
                .arg("--force-window=yes")
                .arg("--hwdec=auto-safe")
                .arg(format!("--sid={}", info.sid));
            if let Some(hwnd) = wid {
                cmd.arg(format!("--wid={hwnd}"));
            } else {
                cmd.arg("--fullscreen");
            }
            // Vidéo incrustée, ou sous la fenêtre de Maku : l'interface de Maku fournit ses propres contrôles et
            // raccourcis. Fenêtre mpv séparée (Linux, macOS) : on garde ceux de mpv, Maku la pilote aussi.
            if wid.is_some() || *under {
                cmd.arg("--osc=no").arg("--input-default-bindings=no").arg("--osd-level=0").arg("--input-vo-keyboard=no").arg("--input-cursor=no");
            }
            if info.offset_ms > 0 {
                cmd.arg(format!("--start={}", info.offset_ms / 1000));
            }
            if let Some(aid) = info.aid {
                cmd.arg(format!("--aid={aid}"));
            }
            temp.push(playlist);
            if !info.sub_files.is_empty() {
                let conf = private_temp("subs.conf", &sub_files_conf(&info.sub_files)).with_context(queue_failed);
                match conf {
                    Ok(conf) => {
                        cmd.arg(format!("--include={}", conf.display()));
                        temp.push(conf);
                    }
                    Err(e) => {
                        remove_all(&temp);
                        return Err(e);
                    }
                }
            }
            offset_ms = info.offset_ms;
            parent = *wid;
        }
        Source::Music { urls, start: first } => {
            // Une longue file dépasserait la limite de la ligne de commande : on passe par une playlist.
            let path = private_temp("queue.m3u", &format!("#EXTM3U\n{}\n", urls.join("\n"))).with_context(queue_failed)?;
            cmd.arg(format!("--playlist={}", path.display()))
                .arg(format!("--playlist-start={first}"))
                .arg("--vid=no")
                .arg("--input-default-bindings=no")
                .arg("--force-window=no")
                .arg("--audio-display=no")
                // Sans blanc entre deux titres : le suivant est ouvert à l'avance et enchaîné sans coupure.
                .arg("--gapless-audio=weak")
                .arg("--prefetch-playlist=yes");
            start = *first;
            temp.push(path);
        }
    }
    Ok(Prepared { cmd, start, offset_ms, parent, temp })
}

fn remove_all(files: &[PathBuf]) {
    for path in files {
        let _ = std::fs::remove_file(path);
    }
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
    let Prepared { mut cmd, start, offset_ms, parent, temp } = prepare(&mpv, &pipe, &source)?;
    let own_window = matches!(source, Source::Video { parent: None, .. });
    let child = cmd.spawn().with_context(|| crate::i18n::tr_with("player.launchFailed", &[("path", &mpv.display().to_string())]));
    let child = match child {
        Ok(c) => c,
        Err(e) => {
            remove_all(&temp);
            return Err(e);
        }
    };

    #[cfg(windows)]
    tie_to_plexr(&child);
    let pid = child.id();
    let (commands, command_rx) = mpsc::unbounded_channel();
    let (stop, stop_rx) = oneshot::channel();
    tauri::async_runtime::spawn(async move {
        let mut session = Session { app: &app, server: &server, entries, parent, own_window, id: &session_id };
        if let Err(e) = session.follow(child, stop_rx, command_rx, &pipe, start, offset_ms).await {
            eprintln!("Suivi de lecture interrompu : {e:#}");
        }
        remove_all(&temp);
        let _ = app.emit("player-stopped", &session_id);
    });
    Ok(PlayerControl { commands, _stop: stop, pid })
}

/// Vidéo dans la fenêtre de mpv (Linux, macOS) : faut-il lui redemander le plein écran, maintenant que `name` vient
/// de changer ?
///
/// mpv est lancé avec « --fullscreen », mais sa fenêtre s'ouvre avant que le film soit chargé, et certains bureaux
/// (mpv 0.40 sous X11, constaté sur Fedora) la laissent alors en fenêtre, à côté de Maku. La demande est donc refaite
/// une fois, dès que mpv annonce la durée du film : il est chargé, la fenêtre a sa taille.
fn fullscreen_again(own_window: bool, done: bool, name: &str, data: &Value) -> bool {
    own_window && !done && name == "duration" && data.as_f64().is_some_and(|d| d > 0.0)
}

struct Session<'a> {
    app: &'a AppHandle,
    server: &'a MediaServer,
    /// Titres de la file, dans l'ordre de mpv (la file peut changer pendant la lecture).
    entries: Vec<Entry>,
    parent: Option<isize>,
    /// Une vidéo dans la fenêtre de mpv, et non dans celle de Maku (voir `fullscreen_again`).
    own_window: bool,
    /// Identifiant de la lecture, envoyé avec la progression (voir `PlexServer::timeline`).
    id: &'a str,
}

impl<'a> Session<'a> {
    /// Progression du titre `index`, à envoyer sans bloquer le suivi de mpv (voir `follow`).
    fn progress(&self, index: usize, state: &'static str, position: f64, duration: f64) -> Option<Report<'a>> {
        let entry = self.entries.get(index)?;
        Some(progress(self.server, self.id, entry.rating_key.clone(), state, position, duration))
    }

    async fn follow(
        &mut self,
        mut child: Child,
        mut stop: oneshot::Receiver<()>,
        mut commands: mpsc::UnboundedReceiver<Control>,
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
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                // La progression part à côté : un serveur lent ou injoignable ne doit geler ni la barre de lecture
                // ni les commandes (pause, déplacement), qui passent par cette même boucle. Un nouvel envoi
                // remplace celui qui n'a pas abouti.
                let mut reporting: Option<Report<'a>> = None;
                let mut fullscreen_asked = false;
                loop {
                    tokio::select! {
                        _ = async { reporting.as_mut().unwrap().await }, if reporting.is_some() => {
                            reporting = None;
                        }
                        line = lines.next_line() => {
                            let Ok(Some(line)) = line else { break };
                            let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
                            if msg["event"] != "property-change" {
                                continue;
                            }
                            let Some(name) = msg["name"].as_str() else { continue };
                            let data = &msg["data"];
                            if fullscreen_again(self.own_window, fullscreen_asked, name, data) {
                                fullscreen_asked = true;
                                let again = json!({ "command": ["set_property", "fullscreen", true] });
                                writer.write_all(format!("{again}\n").as_bytes()).await?;
                            }
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
                                    reporting = self.progress(current, if paused { "paused" } else { "playing" }, position, duration);
                                }
                                // Passage au titre suivant de la file : on clôt le précédent.
                                "playlist-pos" => {
                                    if let Some(next) = data.as_u64().map(|n| n as usize) {
                                        if next != current && next < self.entries.len() {
                                            // Borné lui aussi : le titre suivant joue déjà.
                                            let _ = tokio::time::timeout(REPORT_TIMEOUT, self.finish(current, position, duration)).await;
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
                            let lines = match command {
                                Control::Mpv(c) => vec![c],
                                Control::Insert { entries, urls, at } => {
                                    let at = at.min(self.entries.len());
                                    let lines = insert_commands(self.entries.len(), at, &urls);
                                    if at <= current && !self.entries.is_empty() {
                                        current += entries.len();
                                    }
                                    self.entries.splice(at..at, entries);
                                    lines
                                }
                                Control::Move { from, to } => {
                                    let len = self.entries.len();
                                    if from >= len || to >= len || from == to {
                                        continue;
                                    }
                                    let e = self.entries.remove(from);
                                    self.entries.insert(to, e);
                                    current = moved_index(current, from, to);
                                    vec![move_command(from, to)]
                                }
                                Control::Order(order) => {
                                    let Some(lines) = reorder_commands(&order, current, &self.entries) else {
                                        continue;
                                    };
                                    let mut old: Vec<Option<Entry>> = self.entries.drain(..).map(Some).collect();
                                    self.entries = order.iter().map(|&i| old[i].take().unwrap()).collect();
                                    lines
                                }
                            };
                            for line in lines {
                                writer.write_all(format!("{line}\n").as_bytes()).await?;
                            }
                        }
                        _ = tick.tick() => {
                            reporting = self.progress(current, if paused { "paused" } else { "playing" }, position, duration);
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

/// Commande mpv qui déplace un titre de `from` à sa position finale `to` : mpv le place devant
/// l'élément visé, qui vers le bas est celui d'après.
fn move_command(from: usize, to: usize) -> Value {
    json!({ "command": ["playlist-move", from, if from < to { to + 1 } else { to }] })
}

/// Titres ajoutés à la file de mpv (longue de `len`) à la position `at` : chacun est ajouté à la fin,
/// puis remonté à sa place. Seul Rust envoie `loadfile`, jamais la page (voir `allowed_player_command`).
fn insert_commands(len: usize, at: usize, urls: &[String]) -> Vec<Value> {
    let mut out = Vec::new();
    for (i, url) in urls.iter().enumerate() {
        out.push(json!({ "command": ["loadfile", url, "append"] }));
        let (last, target) = (len + i, at + i);
        if target < last {
            out.push(json!({ "command": ["playlist-move", last, target] }));
        }
    }
    out
}

/// Commandes mpv qui mettent les titres à venir dans l'ordre `order` : ils sont retirés puis remis à la fin,
/// dans le nouvel ordre. Le titre en cours (`current`) et ceux d'avant ne bougent pas : mpv ne change jamais de
/// titre en cours de route (le déplacer titre par titre figeait une longue file et faisait sauter la barre).
/// `None` si l'ordre n'est pas une permutation de la file qui garde ces premiers titres à leur place.
fn reorder_commands(order: &[usize], current: usize, entries: &[Entry]) -> Option<Vec<Value>> {
    let len = entries.len();
    if order.len() != len || current >= len {
        return None;
    }
    let mut seen = vec![false; len];
    for (k, &i) in order.iter().enumerate() {
        if i >= len || std::mem::replace(&mut seen[i], true) || (k <= current && i != k) {
            return None;
        }
    }
    let head = current + 1;
    let mut lines = Vec::with_capacity(2 * (len - head));
    lines.extend((head..len).map(|_| json!({ "command": ["playlist-remove", head] })));
    lines.extend(order[head..].iter().map(|&i| json!({ "command": ["loadfile", entries[i].url, "append"] })));
    Some(lines)
}

/// Position du titre en cours après le déplacement d'un titre de `from` à `to`.
fn moved_index(current: usize, from: usize, to: usize) -> usize {
    if current == from {
        to
    } else if from < current && to >= current {
        current - 1
    } else if from > current && to <= current {
        current + 1
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plein_ecran_redemande_une_fois_le_film_charge() {
        let (duration, nothing) = (json!(5400.5), Value::Null);
        // Fenêtre de mpv à part : dès que la durée du film est connue, et une seule fois.
        assert!(fullscreen_again(true, false, "duration", &duration));
        assert!(!fullscreen_again(true, true, "duration", &duration));
        // Pas avant que le film soit chargé (durée encore inconnue), ni sur une autre annonce.
        assert!(!fullscreen_again(true, false, "duration", &nothing));
        assert!(!fullscreen_again(true, false, "duration", &json!(0.0)));
        assert!(!fullscreen_again(true, false, "time-pos", &duration));
        // Vidéo incrustée dans la fenêtre de Maku (Windows), ou musique : jamais.
        assert!(!fullscreen_again(false, false, "duration", &duration));
    }

    #[test]
    fn seul_mpv_est_accepte_comme_lecteur() {
        for ok in ["mpv", "/usr/bin/mpv", "/var/lib/flatpak/exports/bin/io.mpv.Mpv", "/Applications/mpv.app/Contents/MacOS/mpv"] {
            assert!(is_mpv_name(std::path::Path::new(ok)), "{ok}");
        }
        assert!(is_mpv_name(&PathBuf::from("dossier").join("MPV.EXE")));
        assert!(is_mpv_name(&PathBuf::from("dossier").join("mpv.com")));
        for refused in ["", "/bin/sh", "cmd.exe", "powershell.exe", "/tmp/mpv.sh", "mpv.exe.bat", "/usr/bin/mpv/../sh", "notmpv"] {
            assert!(!is_mpv_name(std::path::Path::new(refused)), "{refused}");
        }
        // Un chemin enregistré qui ne désigne pas mpv n'est jamais lancé.
        assert!(find_mpv(Some(std::env::current_exe().unwrap().to_str().unwrap())).is_err());
    }

    /// Arguments de la commande, et contenu des fichiers temporaires (supprimés ensuite).
    fn prepared(source: &Source) -> (Vec<String>, Vec<String>) {
        let p = prepare(std::path::Path::new("mpv"), "pipe", source).unwrap();
        let args = p.cmd.as_std().get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        let files = p.temp.iter().map(|f| std::fs::read_to_string(f).unwrap()).collect();
        #[cfg(unix)]
        for f in &p.temp {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(f).unwrap().permissions().mode() & 0o777, 0o600, "{}", f.display());
        }
        remove_all(&p.temp);
        (args, files)
    }

    #[test]
    fn aucun_jeton_sur_la_ligne_de_commande_de_mpv() {
        let info = PlayInfo {
            url: "http://10.0.0.2:32400/library/parts/1/file.mkv?X-Plex-Token=SECRET".into(),
            title: "Un film".into(),
            subtitle: None,
            offset_ms: 90_000,
            duration_ms: 3_600_000,
            aid: Some(2),
            sid: "no".into(),
            sub_files: vec!["http://10.0.0.2:32400/library/streams/7?X-Plex-Token=SECRET".into(), "http://10.0.0.2:32400/library/streams/8?X-Plex-Token=SECRET&a=b c".into()],
            sub_tracks: Vec::new(),
        };
        let (args, files) = prepared(&Source::Video { info: info.clone(), parent: None, under: false });
        // Sous la fenêtre de Maku (Linux) : plein écran, sans les commandes ni les raccourcis de mpv, et sans « --wid ».
        let (under, under_files) = prepared(&Source::Video { info: info.clone(), parent: None, under: true });
        for f in under_files {
            let _ = std::fs::remove_file(f);
        }
        assert!(under.contains(&"--fullscreen".to_string()) && under.contains(&"--osc=no".to_string()) && under.contains(&"--input-default-bindings=no".to_string()), "{under:?}");
        assert!(!under.iter().any(|a| a.starts_with("--wid")), "{under:?}");
        // Incrustée (Windows) : dans la fenêtre donnée, sans demander le plein écran à mpv.
        let (embedded, embedded_files) = prepared(&Source::Video { info, parent: Some(42), under: false });
        for f in embedded_files {
            let _ = std::fs::remove_file(f);
        }
        assert!(embedded.contains(&"--wid=42".to_string()) && embedded.contains(&"--osc=no".to_string()) && !embedded.contains(&"--fullscreen".to_string()), "{embedded:?}");
        // Fenêtre mpv séparée : ses commandes sont gardées.
        assert!(args.contains(&"--fullscreen".to_string()) && !args.contains(&"--osc=no".to_string()), "{args:?}");
        assert!(args.iter().all(|a| !a.contains("SECRET") && !a.contains("10.0.0.2")), "{args:?}");
        assert!(args.iter().any(|a| a.starts_with("--playlist=")) && args.iter().any(|a| a.starts_with("--include=")), "{args:?}");
        assert!(args.contains(&"--start=90".to_string()) && args.contains(&"--aid=2".to_string()) && args.contains(&"--sid=no".to_string()), "{args:?}");
        // Le fichier et ses sous-titres sont dans les fichiers privés, la longueur de chaque adresse devant elle.
        assert_eq!(files[0], "#EXTM3U\nhttp://10.0.0.2:32400/library/parts/1/file.mkv?X-Plex-Token=SECRET\n");
        assert_eq!(
            files[1],
            "sub-files-append=%59%http://10.0.0.2:32400/library/streams/7?X-Plex-Token=SECRET\nsub-files-append=%65%http://10.0.0.2:32400/library/streams/8?X-Plex-Token=SECRET&a=b c\n"
        );

        let urls = vec!["http://10.0.0.2:32400/a.flac?X-Plex-Token=SECRET".to_string(), "http://10.0.0.2:32400/b.flac?X-Plex-Token=SECRET".to_string()];
        let (args, files) = prepared(&Source::Music { urls, start: 1 });
        assert!(args.iter().all(|a| !a.contains("SECRET")), "{args:?}");
        assert!(args.contains(&"--playlist-start=1".to_string()), "{args:?}");
        assert_eq!(files[0].lines().count(), 3);
    }

    /// Serveur qui accepte la connexion et ne répond jamais.
    async fn silent_server() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((conn, _)) = listener.accept().await {
                held.push(conn);
            }
        });
        base
    }

    #[tokio::test(start_paused = true)]
    async fn progression_abandonnee_si_le_serveur_ne_repond_pas() {
        // Même client que l'application : sans la limite propre à la progression, l'envoi attendrait 30 s.
        let http = reqwest::Client::builder().timeout(Duration::from_secs(30)).build().unwrap();
        let server = MediaServer::Plex(crate::plex::PlexServer { http, base: silent_server().await, token: "t".into(), client_id: "c".into(), tracks: Default::default() });
        let started = tokio::time::Instant::now();
        progress(&server, "session", "42".into(), "playing", 12.0, 3600.0).await;
        assert!(started.elapsed() <= REPORT_TIMEOUT, "{:?}", started.elapsed());
    }

    /// Simule la file de mpv pour vérifier les commandes envoyées.
    fn apply(list: &mut Vec<String>, commands: &[Value]) {
        for c in commands {
            let c = &c["command"];
            match c[0].as_str().unwrap() {
                "loadfile" => list.push(c[1].as_str().unwrap().to_owned()),
                "playlist-remove" => {
                    list.remove(c[1].as_u64().unwrap() as usize);
                }
                "playlist-move" => {
                    let (a, b) = (c[1].as_u64().unwrap() as usize, c[2].as_u64().unwrap() as usize);
                    let e = list.remove(a);
                    list.insert(if a < b { b - 1 } else { b }, e);
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn deplacer_dans_la_file() {
        for (from, to) in [(0, 3), (3, 0), (1, 2), (2, 1), (0, 4), (4, 0)] {
            let mut list: Vec<String> = (0..5).map(|i| i.to_string()).collect();
            let mut expected = list.clone();
            let e = expected.remove(from);
            expected.insert(to, e);
            apply(&mut list, &[move_command(from, to)]);
            assert_eq!(list, expected, "{from} → {to}");
        }
        assert_eq!(moved_index(2, 2, 0), 0);
        assert_eq!(moved_index(2, 0, 4), 1);
        assert_eq!(moved_index(2, 4, 0), 3);
        assert_eq!(moved_index(2, 3, 4), 2);
    }

    #[test]
    fn inserer_dans_la_file() {
        let mut list: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        let urls = ["x", "y"].map(String::from);
        apply(&mut list, &insert_commands(3, 1, &urls));
        assert_eq!(list, ["a", "x", "y", "b", "c"]);
        let mut list: Vec<String> = ["a", "b"].map(String::from).to_vec();
        apply(&mut list, &insert_commands(2, 2, &urls));
        assert_eq!(list, ["a", "b", "x", "y"]);
    }

    #[test]
    fn remettre_la_suite_dans_un_ordre() {
        let entries: Vec<Entry> = (0..6).map(|i| Entry { rating_key: i.to_string(), duration_ms: 0, url: i.to_string() }).collect();
        let order = [0, 1, 4, 2, 5, 3];
        let mut list: Vec<String> = (0..6).map(|i| i.to_string()).collect();
        apply(&mut list, &reorder_commands(&order, 1, &entries).unwrap());
        assert_eq!(list, order.map(|i| i.to_string()));
        // Le titre en cours (ou un d'avant) déplacé, ou pas une permutation de la file : rien.
        assert!(reorder_commands(&[1, 0, 2, 3, 4, 5], 1, &entries).is_none());
        assert!(reorder_commands(&[0, 1, 2, 2, 4, 5], 1, &entries).is_none());
        assert!(reorder_commands(&[0, 1], 0, &entries).is_none());
    }
}
