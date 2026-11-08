//! Lecture hors ligne : films, épisodes et titres de musique téléchargés sur l'appareil (Android).
//!
//! Chaque téléchargement est noté dans `offline/index.json` (dans les données de l'application) et son
//! fichier rangé à côté, avec son affiche. Un seul téléchargement à la fois, dans l'ordre de la file ;
//! l'interface en suit l'avancement par l'événement `offline-progress`.
//!
//! - **Original** : le fichier tel qu'il est sur le serveur, avec toutes ses pistes. Une coupure reprend où
//!   elle s'était arrêtée (en-tête `Range`).
//! - **Converti** (1080p ou 720p) : la version que le serveur prépare pour une lecture en streaming, récupérée
//!   morceau par morceau (HLS en MPEG-TS) et mise bout à bout : un seul fichier `.ts`, lisible par le lecteur
//!   d'Android. Une coupure reprend au morceau suivant.
//!
//! Hors ligne, la progression de lecture est gardée ici, puis renvoyée au serveur dès qu'il répond
//! (`sync`).

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

use crate::media::MediaServer;

/// Débits des versions converties, en kb/s.
pub const KBPS_1080: u32 = 8000;
pub const KBPS_720: u32 = 4000;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Queued,
    Downloading,
    Paused,
    Done,
    Failed,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    /// Identifiant local (serveur + titre + qualité).
    pub id: String,
    /// Serveur d'origine (`SavedServer::key`) : la progression lui est renvoyée, à lui seul.
    pub server: String,
    pub rating_key: String,
    /// `movie`, `episode` ou `track`.
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    /// Série d'un épisode, artiste d'un titre.
    pub show_title: Option<String>,
    /// Regroupement dans la page des téléchargements : série, album ou playlist d'origine.
    pub group_key: Option<String>,
    pub group_title: Option<String>,
    pub index: Option<u32>,
    pub parent_index: Option<u32>,
    /// `original`, `1080` ou `720`.
    pub quality: String,
    /// Fichier du média et de son affiche, dans le dossier des téléchargements.
    pub file: String,
    pub poster: Option<String>,
    pub size: u64,
    pub done: u64,
    pub status: Status,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub position_ms: u64,
    pub watched: bool,
    /// Progression jouée hors ligne, pas encore renvoyée au serveur.
    pub pending: bool,
    /// Converti : morceaux déjà mis bout à bout (reprise).
    #[serde(default)]
    pub segments_done: usize,
    pub added: u64,
    /// Adresse de l'affiche sur le serveur (téléchargée avec le média).
    #[serde(default)]
    pub poster_url: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

/// Ce qu'on demande à télécharger.
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub rating_key: String,
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub show_title: Option<String>,
    pub group_key: Option<String>,
    pub group_title: Option<String>,
    pub index: Option<u32>,
    pub parent_index: Option<u32>,
    pub quality: String,
    pub poster: Option<String>,
    pub duration_ms: u64,
    /// Titre de musique : son fichier sur le serveur (celui de la file de lecture).
    pub url: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub id: String,
    pub done: u64,
    pub size: u64,
    pub status: Status,
    pub error: Option<String>,
}

pub struct Offline {
    dir: PathBuf,
    items: Mutex<Vec<Download>>,
    running: AtomicBool,
    /// Téléchargements à interrompre (pause ou suppression), vus par la boucle en cours.
    stop: Mutex<HashSet<String>>,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Nom de fichier sans caractère gênant.
fn safe(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect()
}

/// Extension du fichier d'origine, d'après son adresse (`…/file.mkv?…`), sinon `.mkv`.
fn extension(url: &str) -> String {
    let path = url.split('?').next().unwrap_or_default();
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("");
    if (2..=4).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        format!(".{}", ext.to_ascii_lowercase())
    } else {
        ".mkv".into()
    }
}

impl Offline {
    pub fn new(dir: PathBuf) -> Self {
        let items = std::fs::read_to_string(dir.join("index.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<Download>>(&s).ok())
            .unwrap_or_default()
            .into_iter()
            // Un téléchargement en cours à la fermeture de Maku reprend au prochain lancement.
            .map(|mut d| {
                if d.status == Status::Downloading {
                    d.status = Status::Queued;
                }
                d
            })
            .collect();
        Self { dir, items: Mutex::new(items), running: AtomicBool::new(false), stop: Mutex::new(HashSet::new()) }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn list(&self) -> Vec<Download> {
        self.items.lock().unwrap().clone()
    }

    fn save(&self) {
        let items = self.items.lock().unwrap().clone();
        let _ = std::fs::create_dir_all(&self.dir);
        if let Ok(json) = serde_json::to_string_pretty(&items) {
            // Écrit à côté puis renommé : un index jamais à moitié écrit.
            let tmp = self.dir.join("index.json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, self.dir.join("index.json"));
            }
        }
    }

    fn update(&self, id: &str, f: impl FnOnce(&mut Download)) -> Option<Download> {
        let mut items = self.items.lock().unwrap();
        let d = items.iter_mut().find(|d| d.id == id)?;
        f(d);
        Some(d.clone())
    }

    /// Ajoute à la file (sans doublon : même serveur, même titre, même qualité).
    pub fn add(&self, server: &str, r: Request) -> Download {
        let id = safe(&format!("{server}-{}-{}", r.rating_key, r.quality));
        {
            let mut items = self.items.lock().unwrap();
            if let Some(d) = items.iter_mut().find(|d| d.id == id) {
                if d.status == Status::Failed || d.status == Status::Paused {
                    d.status = Status::Queued;
                    d.error = None;
                }
                let d = d.clone();
                drop(items);
                self.save();
                return d;
            }
            items.push(Download {
                id: id.clone(),
                server: server.into(),
                rating_key: r.rating_key,
                kind: r.kind,
                title: r.title,
                subtitle: r.subtitle,
                show_title: r.show_title,
                group_key: r.group_key,
                group_title: r.group_title,
                index: r.index,
                parent_index: r.parent_index,
                quality: r.quality,
                file: String::new(),
                poster: None,
                size: 0,
                done: 0,
                status: Status::Queued,
                error: None,
                duration_ms: r.duration_ms,
                position_ms: 0,
                watched: false,
                pending: false,
                segments_done: 0,
                added: now(),
                poster_url: r.poster,
                url: r.url,
            });
        }
        self.save();
        self.list().into_iter().find(|d| d.id == id).unwrap()
    }

    pub fn pause(&self, id: &str) {
        self.stop.lock().unwrap().insert(id.into());
        self.update(id, |d| {
            if d.status == Status::Queued || d.status == Status::Downloading {
                d.status = Status::Paused;
            }
        });
        self.save();
    }

    pub fn resume(&self, id: &str) {
        self.stop.lock().unwrap().remove(id);
        self.update(id, |d| {
            if d.status != Status::Done {
                d.status = Status::Queued;
                d.error = None;
            }
        });
        self.save();
    }

    /// Supprime le téléchargement et ses fichiers.
    pub fn remove(&self, id: &str) {
        self.stop.lock().unwrap().insert(id.into());
        let removed = {
            let mut items = self.items.lock().unwrap();
            let i = items.iter().position(|d| d.id == id);
            i.map(|i| items.remove(i))
        };
        if let Some(d) = removed {
            if !d.file.is_empty() {
                let _ = std::fs::remove_file(self.dir.join(&d.file));
            }
            // L'affiche peut servir à d'autres titres (album, série) : gardée tant qu'un autre s'en sert.
            if let Some(p) = d.poster.filter(|p| !self.list().iter().any(|o| o.poster.as_deref() == Some(p))) {
                let _ = std::fs::remove_file(self.dir.join(p));
            }
        }
        self.save();
    }

    /// Progression jouée hors ligne, renvoyée au serveur plus tard.
    pub fn set_progress(&self, id: &str, position_ms: u64, watched: bool) {
        self.update(id, |d| {
            d.position_ms = position_ms;
            d.watched = d.watched || watched;
            d.pending = true;
        });
        self.save();
    }

    /// Renvoie au serveur en cours la progression jouée hors ligne sur ses titres.
    pub async fn sync(&self, server: &MediaServer, server_key: &str) -> usize {
        let pending: Vec<Download> = self.list().into_iter().filter(|d| d.pending && d.server == server_key).collect();
        let mut sent = 0;
        for d in pending {
            let ok = if d.watched {
                server.set_watched(&d.rating_key, true).await.is_ok()
            } else {
                let session = uuid::Uuid::new_v4().simple().to_string();
                server.timeline(&d.rating_key, "stopped", d.position_ms, d.duration_ms, &session).await.is_ok()
            };
            if ok {
                self.update(&d.id, |x| x.pending = false);
                sent += 1;
            }
        }
        if sent > 0 {
            self.save();
        }
        sent
    }

    /// Lance la file si elle ne tourne pas déjà : un téléchargement à la fois, jusqu'à ce qu'il n'en reste plus.
    pub fn kick(self: &std::sync::Arc<Self>, app: AppHandle, http: reqwest::Client, server: MediaServer, server_key: String) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                let next = me
                    .list()
                    .into_iter()
                    .find(|d| d.status == Status::Queued && d.server == server_key);
                let Some(d) = next else { break };
                me.stop.lock().unwrap().remove(&d.id);
                me.update(&d.id, |x| x.status = Status::Downloading);
                me.emit(&app, &d.id);
                let result = me.fetch(&app, &http, &server, &d).await;
                let stopped = me.stop.lock().unwrap().remove(&d.id);
                if me.list().iter().all(|x| x.id != d.id) {
                    continue; // supprimé pendant le téléchargement
                }
                match result {
                    Ok(()) if !stopped => {
                        me.update(&d.id, |x| {
                            x.status = Status::Done;
                            x.error = None;
                            x.done = x.size.max(x.done);
                            x.size = x.done;
                        });
                    }
                    Ok(()) => {}
                    Err(e) => {
                        let message = if e.to_string().contains("No space") || e.to_string().contains("ENOSPC") {
                            crate::i18n::tr("offline.noSpace")
                        } else {
                            e.to_string()
                        };
                        me.update(&d.id, |x| {
                            if x.status == Status::Downloading {
                                x.status = Status::Failed;
                                x.error = Some(message);
                            }
                        });
                    }
                }
                me.save();
                me.emit(&app, &d.id);
            }
            me.running.store(false, Ordering::SeqCst);
        });
    }

    fn emit(&self, app: &AppHandle, id: &str) {
        if let Some(d) = self.list().into_iter().find(|d| d.id == id) {
            let _ = app.emit("offline-progress", Progress { id: d.id, done: d.done, size: d.size, status: d.status, error: d.error });
        }
    }

    fn stopped(&self, id: &str) -> bool {
        self.stop.lock().unwrap().contains(id)
    }

    async fn fetch(&self, app: &AppHandle, http: &reqwest::Client, server: &MediaServer, d: &Download) -> Result<()> {
        tokio::fs::create_dir_all(&self.dir).await?;
        // Affiche, une fois pour toutes (partagée par les titres d'un même album).
        if d.poster.is_none() {
            if let Some(url) = &d.poster_url {
                let name = format!("{}.jpg", safe(&format!("art-{}", d.group_key.as_deref().unwrap_or(&d.rating_key))));
                let path = self.dir.join(&name);
                if !path.exists() {
                    if let Ok(resp) = http.get(url).send().await.and_then(|r| r.error_for_status()) {
                        if let Ok(bytes) = resp.bytes().await {
                            let _ = tokio::fs::write(&path, &bytes).await;
                        }
                    }
                }
                if path.exists() {
                    self.update(&d.id, |x| x.poster = Some(name));
                }
            }
        }
        if d.quality == "original" {
            self.fetch_original(app, http, server, d).await
        } else {
            self.fetch_converted(app, http, server, d).await
        }
    }

    /// Fichier d'origine, d'un trait, repris là où il s'était arrêté.
    async fn fetch_original(&self, app: &AppHandle, http: &reqwest::Client, server: &MediaServer, d: &Download) -> Result<()> {
        let url = if let Some(url) = d.url.clone() {
            url
        } else {
            server.play_info(&d.rating_key, None, None, None).await?.url
        };
        let file = if d.file.is_empty() { format!("{}{}", d.id, extension(&url)) } else { d.file.clone() };
        self.update(&d.id, |x| x.file = file.clone());
        let path = self.dir.join(&file);
        let have = tokio::fs::metadata(&path).await.map(|m| m.len()).unwrap_or(0);
        let mut req = http.get(&url);
        if have > 0 {
            req = req.header("Range", format!("bytes={have}-"));
        }
        let mut resp = req.send().await?.error_for_status()?;
        // Le serveur a repris là où on s'était arrêté (206), sinon tout recommence.
        let resumed = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        let start = if resumed { have } else { 0 };
        let total = resp.content_length().map(|l| l + start).unwrap_or(0);
        let mut out = tokio::fs::OpenOptions::new().create(true).write(true).append(resumed).truncate(!resumed).open(&path).await?;
        let mut done = start;
        self.update(&d.id, |x| {
            x.size = total;
            x.done = done;
        });
        let mut last = Instant::now();
        while let Some(chunk) = resp.chunk().await? {
            if self.stopped(&d.id) {
                out.flush().await?;
                return Ok(());
            }
            out.write_all(&chunk).await?;
            done += chunk.len() as u64;
            if last.elapsed() > Duration::from_millis(500) {
                last = Instant::now();
                self.update(&d.id, |x| x.done = done);
                self.emit(app, &d.id);
            }
        }
        out.flush().await?;
        if total > 0 && done < total {
            bail!(crate::i18n::tr("offline.interrupted"));
        }
        self.update(&d.id, |x| {
            x.done = done;
            x.size = done;
        });
        Ok(())
    }

    /// Version convertie : morceaux HLS (MPEG-TS) mis bout à bout dans un seul fichier.
    async fn fetch_converted(&self, app: &AppHandle, http: &reqwest::Client, server: &MediaServer, d: &Download) -> Result<()> {
        let kbps = if d.quality == "1080" { KBPS_1080 } else { KBPS_720 };
        let session = uuid::Uuid::new_v4().simple().to_string();
        let stream = server.web_stream(&d.rating_key, None, None, None, Some(kbps), &session, false).await?;
        let result = self.fetch_segments(app, http, &stream.url, d).await;
        // La conversion du serveur s'arrête, téléchargement fini ou non.
        let _ = server.stop_transcode(&session).await;
        result
    }

    async fn fetch_segments(&self, app: &AppHandle, http: &reqwest::Client, master: &str, d: &Download) -> Result<()> {
        let lines = |t: &str| t.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_owned).collect::<Vec<_>>();
        let master_url = reqwest::Url::parse(master)?;
        let master_text = http.get(master).send().await?.error_for_status()?.text().await?;
        let variant = master_url.join(lines(&master_text).first().ok_or_else(|| anyhow!(crate::i18n::tr("offline.noStream")))?)?;
        // La liste complète des morceaux (#EXT-X-ENDLIST) est donnée d'emblée par Plex et Jellyfin.
        let playlist = http.get(variant.clone()).send().await?.error_for_status()?.text().await?;
        let segments = lines(&playlist);
        if segments.is_empty() {
            bail!(crate::i18n::tr("offline.noStream"));
        }
        let file = if d.file.is_empty() { format!("{}.ts", d.id) } else { d.file.clone() };
        self.update(&d.id, |x| x.file = file.clone());
        let path = self.dir.join(&file);
        let resume = d.segments_done.min(segments.len());
        let mut out = tokio::fs::OpenOptions::new().create(true).write(true).append(resume > 0).truncate(resume == 0).open(&path).await?;
        let mut done = if resume > 0 { tokio::fs::metadata(&path).await.map(|m| m.len()).unwrap_or(0) } else { 0 };
        let mut last = Instant::now();
        for (i, seg) in segments.iter().enumerate().skip(resume) {
            if self.stopped(&d.id) {
                out.flush().await?;
                return Ok(());
            }
            let url = variant.join(seg)?;
            // Le serveur convertit au fil de l'eau : un morceau pas encore prêt est redemandé.
            let mut bytes = None;
            for attempt in 0..30 {
                match http.get(url.clone()).send().await.and_then(|r| r.error_for_status()) {
                    Ok(r) => {
                        bytes = Some(r.bytes().await?);
                        break;
                    }
                    Err(_) if attempt < 29 => tokio::time::sleep(Duration::from_secs(2)).await,
                    Err(e) => return Err(e.into()),
                }
            }
            let bytes = bytes.unwrap_or_default();
            out.write_all(&bytes).await?;
            done += bytes.len() as u64;
            // Taille finale estimée d'après la moyenne des morceaux déjà reçus.
            let estimate = done / (i as u64 + 1) * segments.len() as u64;
            self.update(&d.id, |x| {
                x.segments_done = i + 1;
                x.done = done;
                x.size = estimate.max(done);
            });
            if last.elapsed() > Duration::from_millis(500) {
                last = Instant::now();
                out.flush().await?;
                self.save();
                self.emit(app, &d.id);
            }
        }
        out.flush().await?;
        self.update(&d.id, |x| {
            x.done = done;
            x.size = done;
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn extension_du_fichier() {
        assert_eq!(super::extension("http://x/library/parts/1/2/file.mkv?X-Plex-Token=a"), ".mkv");
        assert_eq!(super::extension("http://x/Videos/abc/stream?static=true"), ".mkv");
        assert_eq!(super::extension("http://x/a/b.FLAC"), ".flac");
    }

    #[test]
    fn nom_sans_caractere_genant() {
        assert_eq!(super::safe("plex:abc/12 3"), "plex_abc_12_3");
    }
}
