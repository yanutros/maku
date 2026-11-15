//! Chromecast : la musique envoyée sur une enceinte ou un téléviseur Google Cast du réseau local.
//!
//! Les appareils se trouvent par mDNS (`_googlecast._tcp`). On leur parle ensuite le protocole Cast v2 :
//! une connexion TLS sur le port 8009 (certificat de l'appareil, non vérifiable), des messages protobuf
//! `CastMessage` précédés de leur longueur, dont la charge est du JSON. Maku lance le lecteur standard de
//! Google (« Default Media Receiver »), lui confie toute la file (`QUEUE_LOAD`) avec l'adresse des fichiers
//! du serveur, puis le pilote ; l'appareil lit lui-même les fichiers, Maku peut être fermé.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

const MEDIA_RECEIVER: &str = "CC1AD845";
const NS_CONNECTION: &str = "urn:x-cast:com.google.cast.tp.connection";
const NS_HEARTBEAT: &str = "urn:x-cast:com.google.cast.tp.heartbeat";
const NS_RECEIVER: &str = "urn:x-cast:com.google.cast.receiver";
const NS_MEDIA: &str = "urn:x-cast:com.google.cast.media";
const SENDER: &str = "sender-maku";
const PLATFORM: &str = "receiver-0";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
}

/// Un titre de la file, tel que l'appareil le lira.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub url: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub cover: Option<String>,
}

/// État de la lecture sur l'appareil, envoyé à la page (`cast-status`).
#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// `PLAYING`, `PAUSED`, `BUFFERING` ou `IDLE`.
    pub state: String,
    pub position: f64,
    pub duration: f64,
    /// Titre en cours dans la file envoyée.
    pub index: Option<usize>,
    pub volume: Option<f64>,
    pub muted: Option<bool>,
    /// La lecture est finie ou l'appareil a été repris par un autre (`idleReason`).
    pub idle_reason: Option<String>,
}

// ---------- Découverte ----------

/// Appareils Cast du réseau, trouvés pendant `wait`.
pub async fn discover(wait: Duration) -> Result<Vec<Device>> {
    let daemon = mdns_sd::ServiceDaemon::new().context("mDNS")?;
    let rx = daemon.browse("_googlecast._tcp.local.").context("mDNS")?;
    let mut found: Vec<Device> = Vec::new();
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            break;
        }
        match tokio::time::timeout(left, rx.recv_async()).await {
            Ok(Ok(mdns_sd::ServiceEvent::ServiceResolved(info))) => {
                let Some(ip) = info.get_addresses_v4().into_iter().next() else { continue };
                let txt = |k: &str| info.txt_properties.get(k).map(|p| p.val_str().to_owned()).filter(|v| !v.is_empty());
                let id = txt("id").unwrap_or_else(|| info.get_fullname().to_owned());
                if found.iter().any(|d| d.id == id) {
                    continue;
                }
                let name = txt("fn").unwrap_or_else(|| info.get_hostname().trim_end_matches('.').to_owned());
                found.push(Device { id, name, host: ip.to_string(), port: info.get_port() });
            }
            Ok(Ok(_)) => {}
            _ => break,
        }
    }
    let _ = daemon.shutdown();
    found.sort_by_key(|d| d.name.to_lowercase());
    Ok(found)
}

// ---------- Messages Cast (protobuf minimal) ----------

fn varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

fn field_str(out: &mut Vec<u8>, field: u8, s: &str) {
    out.push((field << 3) | 2);
    varint(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

/// `CastMessage` : version 0, source, destination, espace de noms, charge texte.
fn encode(source: &str, dest: &str, namespace: &str, payload: &str) -> Vec<u8> {
    let mut m = vec![0x08, 0x00];
    field_str(&mut m, 2, source);
    field_str(&mut m, 3, dest);
    field_str(&mut m, 4, namespace);
    m.extend_from_slice(&[0x28, 0x00]);
    field_str(&mut m, 6, payload);
    let mut framed = (m.len() as u32).to_be_bytes().to_vec();
    framed.extend(m);
    framed
}

struct Incoming {
    source: String,
    namespace: String,
    payload: String,
}

fn decode(buf: &[u8]) -> Option<Incoming> {
    let (mut i, mut source, mut namespace, mut payload) = (0, String::new(), String::new(), String::new());
    let read_varint = |i: &mut usize| -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = *buf.get(*i)?;
            *i += 1;
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    };
    while i < buf.len() {
        let key = read_varint(&mut i)?;
        let (field, wire) = (key >> 3, key & 7);
        match wire {
            0 => {
                read_varint(&mut i)?;
            }
            2 => {
                let len = read_varint(&mut i)? as usize;
                let bytes = buf.get(i..i + len)?;
                i += len;
                let text = String::from_utf8_lossy(bytes).into_owned();
                match field {
                    2 => source = text,
                    4 => namespace = text,
                    6 => payload = text,
                    _ => {}
                }
            }
            _ => return None,
        }
    }
    Some(Incoming { source, namespace, payload })
}

// ---------- Session ----------

#[derive(Debug)]
pub enum Command {
    Toggle,
    Play,
    Pause,
    Seek(f64),
    Jump(usize),
    Next,
    Previous,
    Volume(f64),
    Mute(bool),
    Stop,
    /// Titres à insérer à la position `at` de la file (au-delà : à la fin).
    Insert(Vec<Track>, usize),
    /// Titre déplacé de la position `from` à `to`.
    Reorder(usize, usize),
    /// Nouvel ordre de la file : `order[k]` est la position d'avant du titre placé en `k`.
    Order(Vec<usize>),
    /// Répétition : « REPEAT_OFF », « REPEAT_ALL » (la file) ou « REPEAT_SINGLE » (le titre).
    Repeat(&'static str),
}

/// Session en cours avec un appareil : la tâche qui lui parle reçoit les commandes par ce canal.
pub struct Session {
    commands: mpsc::UnboundedSender<Command>,
}

impl Session {
    pub fn send(&self, c: Command) {
        let _ = self.commands.send(c);
    }
}

/// Le certificat d'un appareil Cast est signé par Google pour l'appareil lui-même : il n'est pas vérifiable
/// comme celui d'un site. La connexion reste chiffrée.
#[derive(Debug)]
struct AcceptAny;

impl tokio_rustls::rustls::client::danger::ServerCertVerifier for AcceptAny {
    fn verify_server_cert(
        &self,
        _: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _: &[tokio_rustls::rustls::pki_types::CertificateDer<'_>],
        _: &tokio_rustls::rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: tokio_rustls::rustls::pki_types::UnixTime,
    ) -> std::result::Result<tokio_rustls::rustls::client::danger::ServerCertVerified, tokio_rustls::rustls::Error> {
        Ok(tokio_rustls::rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> std::result::Result<tokio_rustls::rustls::client::danger::HandshakeSignatureValid, tokio_rustls::rustls::Error> {
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> std::result::Result<tokio_rustls::rustls::client::danger::HandshakeSignatureValid, tokio_rustls::rustls::Error> {
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<tokio_rustls::rustls::SignatureScheme> {
        tokio_rustls::rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
    }
}

/// Type du fichier, d'après son extension (le lecteur de Google en a besoin pour choisir son décodeur).
fn content_type(url: &str) -> &'static str {
    let path = url.split('?').next().unwrap_or("").to_ascii_lowercase();
    match path.rsplit('.').next().unwrap_or("") {
        "flac" => "audio/flac",
        "m4a" | "mp4" | "aac" | "alac" => "audio/mp4",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "webm" => "audio/webm",
        _ => "audio/mpeg",
    }
}

type Tls = tokio_rustls::client::TlsStream<tokio::net::TcpStream>;

struct Link {
    write: tokio::io::WriteHalf<Tls>,
    request: u64,
}

impl Link {
    async fn send(&mut self, dest: &str, ns: &str, mut payload: Value) -> Result<u64> {
        self.request += 1;
        if payload.get("requestId").is_none() && ns != NS_CONNECTION && ns != NS_HEARTBEAT {
            payload["requestId"] = json!(self.request);
        }
        self.write.write_all(&encode(SENDER, dest, ns, &payload.to_string())).await?;
        Ok(self.request)
    }
}

async fn read_message<R: tokio::io::AsyncRead + Unpin>(read: &mut R) -> Result<Incoming> {
    let mut len = [0u8; 4];
    read.read_exact(&mut len).await?;
    let len = u32::from_be_bytes(len) as usize;
    if len > 1 << 20 {
        bail!("message Cast trop long");
    }
    let mut buf = vec![0u8; len];
    read.read_exact(&mut buf).await?;
    decode(&buf).ok_or_else(|| anyhow!("message Cast illisible"))
}

/// Lit les messages de l'appareil dans sa propre tâche, et les rend un par un. Un message arrive souvent en plusieurs
/// morceaux : lu directement dans la boucle de la session, il était abandonné à moitié dès qu'une autre branche
/// (le relevé de la position, chaque seconde) passait devant, et la suite du flux devenait illisible. La session se
/// terminait alors en « diffusion arrêtée ».
fn spawn_reader<R: tokio::io::AsyncRead + Unpin + Send + 'static>(mut read: R) -> mpsc::UnboundedReceiver<Result<Incoming>> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let message = read_message(&mut read).await;
            let failed = message.is_err();
            if tx.send(message).is_err() || failed {
                break;
            }
        }
    });
    rx
}

/// Taille maximale d'un message envoyé à l'appareil. Le protocole Cast refuse au-delà de 64 Ko, et ferme alors la
/// connexion : une file d'une centaine de titres (deux adresses et une pochette chacun) les dépasse largement.
const MESSAGE_BUDGET: usize = 40_000;

/// Découpe `items` en lots dont le texte tient chacun dans `budget` octets (un titre au moins par lot).
fn chunks(items: &[Value], budget: usize) -> Vec<Vec<Value>> {
    let mut out: Vec<Vec<Value>> = Vec::new();
    let mut size = 0;
    for item in items {
        let len = item.to_string().len() + 1;
        if out.is_empty() || size + len > budget {
            out.push(Vec::new());
            size = 0;
        }
        size += len;
        out.last_mut().unwrap().push(item.clone());
    }
    out
}

/// Une file trop longue pour un seul message : ce que l'appareil charge d'abord (le titre demandé et les suivants),
/// puis les lots à ajouter après, et ceux à insérer avant, dans l'ordre.
struct QueuePlan {
    first: Vec<Value>,
    after: Vec<Vec<Value>>,
    before: Vec<Vec<Value>>,
}

fn plan_queue(items: &[Value], start: usize, budget: usize) -> QueuePlan {
    let start = start.min(items.len().saturating_sub(1));
    let mut after = chunks(&items[start..], budget);
    let first = if after.is_empty() { Vec::new() } else { after.remove(0) };
    QueuePlan { first, after, before: chunks(&items[..start], budget) }
}

/// Place d'un titre dans la file d'origine, d'après sa place `pos` dans la file de l'appareil : celle-ci commence
/// au titre `start` (`anchor_pos` : sa place sur l'appareil, 0 tant que les titres d'avant ne sont pas insérés).
fn original_index(pos: usize, anchor_pos: usize, start: usize) -> Option<usize> {
    (pos + start).checked_sub(anchor_pos)
}

/// L'inverse : place sur l'appareil du titre `index` de la file d'origine, s'il y est déjà.
fn device_index(index: usize, anchor_pos: usize, start: usize) -> Option<usize> {
    (index + anchor_pos).checked_sub(start)
}

/// Un titre de la file, au format du lecteur de Google.
fn queue_item(t: &Track) -> Value {
    json!({
        "autoplay": true,
        "preloadTime": 10,
        "media": {
            "contentId": t.url,
            "contentUrl": t.url,
            "contentType": content_type(&t.url),
            "streamType": "BUFFERED",
            "metadata": {
                "metadataType": 3,
                "title": t.title,
                "artist": t.artist,
                "albumName": t.album,
                "images": t.cover.iter().map(|c| json!({ "url": c })).collect::<Vec<_>>(),
            },
        },
    })
}

/// Se connecte à l'appareil, lance le lecteur de Google et lui confie la file à partir de `start`
/// (`position` secondes dans ce titre). L'état de la lecture arrive ensuite par l'événement `cast-status`,
/// la fin de la session par `cast-ended`.
pub async fn start(app: AppHandle, device: Device, tracks: Vec<Track>, start: usize, position: f64) -> Result<Session> {
    let tcp = tokio::time::timeout(Duration::from_secs(8), tokio::net::TcpStream::connect((device.host.as_str(), device.port)))
        .await
        .map_err(|_| anyhow!(crate::i18n::tr("cast.unreachable")))??;
    let config = tokio_rustls::rustls::ClientConfig::builder_with_provider(Arc::new(tokio_rustls::rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAny))
        .with_no_client_auth();
    let name = tokio_rustls::rustls::pki_types::ServerName::try_from(device.host.clone()).map_err(|_| anyhow!("adresse"))?;
    let tls = tokio_rustls::TlsConnector::from(Arc::new(config)).connect(name, tcp).await?;
    let (mut read, write) = tokio::io::split(tls);
    let mut link = Link { write, request: 0 };

    link.send(PLATFORM, NS_CONNECTION, json!({ "type": "CONNECT" })).await?;
    link.send(PLATFORM, NS_RECEIVER, json!({ "type": "LAUNCH", "appId": MEDIA_RECEIVER })).await?;

    // Le lecteur lancé : son identifiant de transport, à qui parler ensuite, et sa session (pour l'arrêter).
    let (transport, app_session) = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let m = read_message(&mut read).await?;
            if m.namespace == NS_HEARTBEAT && m.payload.contains("PING") {
                link.send(&m.source, NS_HEARTBEAT, json!({ "type": "PONG" })).await?;
                continue;
            }
            if m.namespace != NS_RECEIVER {
                continue;
            }
            let v: Value = serde_json::from_str(&m.payload).unwrap_or_default();
            if v["type"] == "LAUNCH_ERROR" {
                bail!(crate::i18n::tr("cast.launchFailed"));
            }
            let app = v["status"]["applications"].as_array().and_then(|a| a.iter().find(|x| x["appId"] == MEDIA_RECEIVER));
            if let Some(t) = app.and_then(|a| a["transportId"].as_str()) {
                let session = app.and_then(|a| a["sessionId"].as_str()).unwrap_or_default().to_owned();
                return anyhow::Ok((t.to_owned(), session));
            }
        }
    })
    .await
    .map_err(|_| anyhow!(crate::i18n::tr("cast.launchFailed")))??;

    link.send(&transport, NS_CONNECTION, json!({ "type": "CONNECT" })).await?;
    let items: Vec<Value> = tracks.iter().map(queue_item).collect();
    // Le titre demandé et ses suivants d'abord ; le reste de la file suit par lots, une fois la lecture lancée.
    let start = start.min(items.len().saturating_sub(1));
    let QueuePlan { first, after, before } = plan_queue(&items, start, MESSAGE_BUDGET);
    link.send(
        &transport,
        NS_MEDIA,
        json!({ "type": "QUEUE_LOAD", "items": first, "startIndex": 0, "currentTime": position, "repeatMode": "REPEAT_OFF" }),
    )
    .await?;
    let mut pending = Some((after, before));
    let mut incoming = spawn_reader(read);

    let (commands, mut rx) = mpsc::unbounded_channel();
    let session = Session { commands };
    tauri::async_runtime::spawn(async move {
        let mut media_session: Option<u64> = None;
        let mut item_ids: Vec<u64> = Vec::new();
        // Liste complète reçue de l'appareil (`QUEUE_ITEM_IDS`), plutôt que les seuls titres de l'état.
        let mut ids_known = false;
        let mut status = Status::default();
        // Identifiant, sur l'appareil, du titre par lequel la lecture a commencé (le premier chargé).
        let mut anchor: Option<u64> = None;
        let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
        let mut poll = tokio::time::interval(Duration::from_secs(1));
        let mut reason = None;
        loop {
            tokio::select! {
                m = incoming.recv() => {
                    let Some(Ok(m)) = m else { reason = Some("disconnected"); break };
                    if m.namespace == NS_HEARTBEAT {
                        if m.payload.contains("PING") {
                            let _ = link.send(&m.source, NS_HEARTBEAT, json!({ "type": "PONG" })).await;
                        }
                        continue;
                    }
                    let v: Value = serde_json::from_str(&m.payload).unwrap_or_default();
                    if m.namespace == NS_CONNECTION && v["type"] == "CLOSE" {
                        reason = Some("closed");
                        break;
                    }
                    if m.namespace == NS_RECEIVER {
                        // Le lecteur de Google a quitté l'appareil (autre application lancée).
                        let apps = v["status"]["applications"].as_array().cloned().unwrap_or_default();
                        if v["type"] == "RECEIVER_STATUS" && !apps.iter().any(|a| a["transportId"] == transport.as_str()) {
                            reason = Some("closed");
                            break;
                        }
                        let vol = &v["status"]["volume"];
                        if let Some(l) = vol["level"].as_f64() { status.volume = Some(l); }
                        if let Some(m) = vol["muted"].as_bool() { status.muted = Some(m); }
                        let _ = app.emit("cast-status", &status);
                        continue;
                    }
                    // Identifiants des titres, dans l'ordre de la file : ils servent à savoir lequel est en cours.
                    if m.namespace == NS_MEDIA && v["type"] == "QUEUE_ITEM_IDS" {
                        if let Some(ids) = v["itemIds"].as_array() {
                            item_ids = ids.iter().filter_map(Value::as_u64).collect();
                            ids_known = true;
                        }
                        continue;
                    }
                    // File modifiée (titres ajoutés) : ses identifiants sont redemandés.
                    if m.namespace == NS_MEDIA && v["type"] == "QUEUE_CHANGE" {
                        if let Some(ms) = media_session {
                            let _ = link.send(&transport, NS_MEDIA, json!({ "type": "QUEUE_GET_ITEM_IDS", "mediaSessionId": ms })).await;
                        }
                        continue;
                    }
                    if m.namespace != NS_MEDIA || v["type"] != "MEDIA_STATUS" {
                        if m.namespace == NS_MEDIA && (v["type"] == "LOAD_FAILED" || v["type"] == "INVALID_REQUEST") {
                            let _ = app.emit("cast-error", crate::i18n::tr("cast.loadFailed"));
                        }
                        continue;
                    }
                    let Some(s) = v["status"].as_array().and_then(|a| a.first()) else {
                        // File terminée : plus de session de lecture sur l'appareil.
                        if media_session.is_some() {
                            reason = Some("finished");
                            break;
                        }
                        continue;
                    };
                    let first_status = media_session.is_none();
                    media_session = s["mediaSessionId"].as_u64().or(media_session);
                    if let (true, Some(ms)) = (first_status, media_session) {
                        let _ = link.send(&transport, NS_MEDIA, json!({ "type": "QUEUE_GET_ITEM_IDS", "mediaSessionId": ms })).await;
                    }
                    // En attendant la liste complète, les titres que l'état mentionne.
                    if let (false, Some(items)) = (ids_known, s["items"].as_array()) {
                        let ids: Vec<u64> = items.iter().filter_map(|i| i["itemId"].as_u64()).collect();
                        if !ids.is_empty() { item_ids = ids; }
                    }
                    if anchor.is_none() {
                        anchor = item_ids.first().copied().or_else(|| s["currentItemId"].as_u64());
                    }
                    // La lecture est lancée : le reste de la file part, lot par lot (après le dernier titre, puis
                    // avant le premier chargé).
                    if let (Some(ms), Some(first_id), true) = (media_session, anchor, pending.is_some()) {
                        let (after, before) = pending.take().unwrap_or_default();
                        let mut sent = true;
                        for lot in after {
                            sent &= link.send(&transport, NS_MEDIA, json!({ "type": "QUEUE_INSERT", "mediaSessionId": ms, "items": lot })).await.is_ok();
                        }
                        for lot in before {
                            sent &= link.send(&transport, NS_MEDIA, json!({ "type": "QUEUE_INSERT", "mediaSessionId": ms, "items": lot, "insertBefore": first_id })).await.is_ok();
                        }
                        if !sent { reason = Some("disconnected"); break; }
                    }
                    if let Some(st) = s["playerState"].as_str() { status.state = st.to_owned(); }
                    if let Some(t) = s["currentTime"].as_f64() { status.position = t; }
                    if let Some(d) = s["media"]["duration"].as_f64() { status.duration = d; }
                    if let Some(id) = s["currentItemId"].as_u64() {
                        // Les identifiants suivent l'ordre de la file : le premier connu donne le décalage. La file de
                        // l'appareil commence au titre demandé, tant que ceux d'avant n'y sont pas encore insérés.
                        let pos = item_ids.iter().position(|&x| x == id).or_else(|| item_ids.first().map(|&f| id.saturating_sub(f) as usize));
                        let anchor_pos = anchor.and_then(|a| item_ids.iter().position(|&x| x == a)).unwrap_or(0);
                        status.index = pos.and_then(|p| original_index(p, anchor_pos, start));
                    }
                    status.idle_reason = s["idleReason"].as_str().map(str::to_owned);
                    let _ = app.emit("cast-status", &status);
                    if status.state == "IDLE" && matches!(status.idle_reason.as_deref(), Some("FINISHED") | Some("CANCELLED")) && s["currentItemId"].is_null() {
                        reason = Some("finished");
                        break;
                    }
                }
                c = rx.recv() => {
                    let Some(c) = c else { break };
                    let ms = media_session.unwrap_or(0);
                    // Place, sur l'appareil, d'un titre de la file d'origine.
                    let anchor_pos = anchor.and_then(|a| item_ids.iter().position(|&x| x == a)).unwrap_or(0);
                    let on_device = |i: usize| device_index(i, anchor_pos, start);
                    let r = match c {
                        Command::Stop => {
                            let _ = link.send(PLATFORM, NS_RECEIVER, json!({ "type": "STOP", "sessionId": app_session })).await;
                            let _ = link.send(&transport, NS_MEDIA, json!({ "type": "STOP", "mediaSessionId": ms })).await;
                            reason = Some("stopped");
                            break;
                        }
                        Command::Toggle => {
                            let t = if status.state == "PLAYING" || status.state == "BUFFERING" { "PAUSE" } else { "PLAY" };
                            link.send(&transport, NS_MEDIA, json!({ "type": t, "mediaSessionId": ms })).await
                        }
                        Command::Play => link.send(&transport, NS_MEDIA, json!({ "type": "PLAY", "mediaSessionId": ms })).await,
                        Command::Pause => link.send(&transport, NS_MEDIA, json!({ "type": "PAUSE", "mediaSessionId": ms })).await,
                        Command::Seek(t) => link.send(&transport, NS_MEDIA, json!({ "type": "SEEK", "mediaSessionId": ms, "currentTime": t })).await,
                        Command::Next => link.send(&transport, NS_MEDIA, json!({ "type": "QUEUE_UPDATE", "mediaSessionId": ms, "jump": 1 })).await,
                        Command::Previous => link.send(&transport, NS_MEDIA, json!({ "type": "QUEUE_UPDATE", "mediaSessionId": ms, "jump": -1 })).await,
                        Command::Jump(i) => match on_device(i).and_then(|i| item_ids.get(i)) {
                            Some(&id) => link.send(&transport, NS_MEDIA, json!({ "type": "QUEUE_UPDATE", "mediaSessionId": ms, "currentItemId": id })).await,
                            None => Ok(0),
                        },
                        Command::Volume(l) => link.send(PLATFORM, NS_RECEIVER, json!({ "type": "SET_VOLUME", "volume": { "level": l.clamp(0.0, 1.0) } })).await,
                        Command::Mute(m) => link.send(PLATFORM, NS_RECEIVER, json!({ "type": "SET_VOLUME", "volume": { "muted": m } })).await,
                        Command::Insert(tracks, at) => {
                            let items: Vec<Value> = tracks.iter().map(queue_item).collect();
                            // Sans `insertBefore`, les titres vont à la fin. Par lots, comme la file elle-même : chacun
                            // se place devant le même titre, donc à la suite du précédent.
                            let before = on_device(at).and_then(|i| item_ids.get(i)).copied();
                            let mut sent = Ok(0);
                            for lot in chunks(&items, MESSAGE_BUDGET) {
                                let mut msg = json!({ "type": "QUEUE_INSERT", "mediaSessionId": ms, "items": lot });
                                if let Some(before) = before {
                                    msg["insertBefore"] = json!(before);
                                }
                                sent = link.send(&transport, NS_MEDIA, msg).await;
                                if sent.is_err() {
                                    break;
                                }
                            }
                            match sent {
                                Ok(_) => link.send(&transport, NS_MEDIA, json!({ "type": "QUEUE_GET_ITEM_IDS", "mediaSessionId": ms })).await,
                                e => e,
                            }
                        }
                        Command::Reorder(from, to) if from < item_ids.len() && to < item_ids.len() && from != to => {
                            let id = item_ids.remove(from);
                            item_ids.insert(to, id);
                            link.send(&transport, NS_MEDIA, json!({ "type": "QUEUE_REORDER", "mediaSessionId": ms, "itemIds": item_ids })).await
                        }
                        Command::Reorder(..) => Ok(0),
                        Command::Order(order) if order.len() == item_ids.len() && order.iter().all(|&i| i < item_ids.len()) => {
                            let ids: Vec<_> = order.iter().map(|&i| item_ids[i].clone()).collect();
                            item_ids = ids;
                            link.send(&transport, NS_MEDIA, json!({ "type": "QUEUE_REORDER", "mediaSessionId": ms, "itemIds": item_ids })).await
                        }
                        Command::Order(_) => Ok(0),
                        Command::Repeat(mode) => {
                            link.send(&transport, NS_MEDIA, json!({ "type": "QUEUE_UPDATE", "mediaSessionId": ms, "repeatMode": mode })).await
                        }
                    };
                    if r.is_err() { reason = Some("disconnected"); break; }
                }
                _ = heartbeat.tick() => {
                    if link.send(PLATFORM, NS_HEARTBEAT, json!({ "type": "PING" })).await.is_err() { reason = Some("disconnected"); break; }
                }
                _ = poll.tick() => {
                    // Position tenue à jour : l'appareil ne l'envoie qu'aux changements d'état.
                    if let Some(ms) = media_session {
                        if link.send(&transport, NS_MEDIA, json!({ "type": "GET_STATUS", "mediaSessionId": ms })).await.is_err() { reason = Some("disconnected"); break; }
                    }
                }
            }
        }
        let _ = link.write.shutdown().await;
        let _ = app.emit("cast-ended", reason.unwrap_or("stopped"));
    });
    Ok(session)
}

#[cfg(test)]
mod tests {
    /// Recherche réelle sur le réseau local (lancée à la main : `cargo test cherche -- --ignored --nocapture`).
    #[tokio::test(flavor = "current_thread")]
    #[ignore]
    async fn cherche_les_appareils() {
        let found = super::discover(std::time::Duration::from_secs(4)).await.unwrap();
        println!("{} appareil(s)", found.len());
        for d in found {
            println!("- port {}", d.port);
        }
    }

    #[test]
    fn un_message_se_relit() {
        let framed = super::encode("a", "b", "urn:x", "{\"type\":\"PING\"}");
        let m = super::decode(&framed[4..]).unwrap();
        assert_eq!(m.source, "a");
        assert_eq!(m.namespace, "urn:x");
        assert_eq!(m.payload, "{\"type\":\"PING\"}");
        assert_eq!(super::content_type("http://x/file.flac?X-Plex-Token=1"), "audio/flac");
    }

    /// Une file de `n` titres comme en envoie Maku : deux adresses avec jeton et une pochette par titre.
    fn queue(n: usize) -> Vec<serde_json::Value> {
        (0..n)
            .map(|i| {
                let url = format!("http://10.0.0.2:32400/library/parts/{}/1700000000/file.flac?X-Plex-Token=abcdefghijklmnopqrst", 1000 + i);
                let cover = format!("http://10.0.0.2:32400/photo/:/transcode?url=%2Flibrary%2Fmetadata%2F{}%2Fthumb%2F1700000000&width=600&height=600&minSize=1&upscale=1&X-Plex-Token=abcdefghijklmnopqrst", 1000 + i);
                super::queue_item(&super::Track { url, title: format!("Titre {i}"), artist: Some("Artiste".into()), album: Some("Album".into()), cover: Some(cover) })
            })
            .collect()
    }

    #[test]
    fn longue_file_envoyee_par_lots_sous_la_limite_du_protocole() {
        let items = queue(111);
        // D'un seul message, cette file dépasse la limite du protocole Cast (64 Ko) : l'appareil coupe la connexion.
        let whole = serde_json::json!({ "type": "QUEUE_LOAD", "items": items }).to_string().len();
        assert!(whole > 64 * 1024, "{whole}");
        for start in [0, 1, 57, 110] {
            let plan = super::plan_queue(&items, start, super::MESSAGE_BUDGET);
            // Le titre demandé est le premier chargé, et chaque message tient sous la limite.
            assert_eq!(plan.first[0], items[start]);
            for lot in std::iter::once(&plan.first).chain(&plan.after).chain(&plan.before) {
                assert!(!lot.is_empty());
                let size = serde_json::json!({ "type": "QUEUE_INSERT", "mediaSessionId": 1, "items": lot, "insertBefore": 1 }).to_string().len();
                assert!(size < 48 * 1024, "{size}");
            }
            // Tous les titres y sont, une fois, dans l'ordre : ceux d'avant, puis le premier lot et les suivants.
            let rebuilt: Vec<_> = plan.before.iter().flatten().chain(&plan.first).chain(plan.after.iter().flatten()).cloned().collect();
            assert_eq!(rebuilt, items, "début au titre {start}");
        }
        // Une courte file part d'un seul message, comme avant.
        let short = super::plan_queue(&queue(8), 3, super::MESSAGE_BUDGET);
        assert_eq!((short.first.len(), short.after.len(), short.before.len()), (5, 0, 1));
        // Un titre plus gros que la limite à lui seul part quand même, seul.
        assert_eq!(super::chunks(&queue(3), 10).len(), 3);
    }

    #[test]
    fn place_d_un_titre_entre_la_file_et_l_appareil() {
        // Lecture commencée au 57e titre : l'appareil n'a d'abord que les suivants.
        assert_eq!(super::original_index(0, 0, 57), Some(57));
        assert_eq!(super::original_index(3, 0, 57), Some(60));
        assert_eq!(super::device_index(60, 0, 57), Some(3));
        // Les titres d'avant ne sont pas encore sur l'appareil.
        assert_eq!(super::device_index(10, 0, 57), None);
        // Une fois insérés (le premier chargé est alors 57e sur l'appareil aussi), les places concordent.
        assert_eq!(super::original_index(10, 57, 57), Some(10));
        assert_eq!(super::device_index(10, 57, 57), Some(10));
        assert_eq!(super::original_index(57, 57, 57), Some(57));
    }

    /// Un message qui arrive en morceaux, pendant que d'autres branches de la boucle passent devant : il doit
    /// arriver entier, et le suivant aussi.
    #[tokio::test]
    async fn message_en_morceaux_lu_en_entier() {
        use tokio::io::AsyncWriteExt;
        let (mut device, maku) = tokio::io::duplex(64);
        let big = "x".repeat(3000);
        let frames = [super::encode("r", "*", "urn:x", &format!("{{\"type\":\"MEDIA_STATUS\",\"pad\":\"{big}\"}}")), super::encode("r", "*", "urn:x", "{\"type\":\"PING\"}")];
        tokio::spawn(async move {
            for frame in frames {
                // Par petits morceaux espacés, comme sur le réseau.
                for part in frame.chunks(700) {
                    device.write_all(part).await.unwrap();
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
            }
            // La connexion reste ouverte.
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        });
        let mut incoming = super::spawn_reader(maku);
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(10));
        let mut got = Vec::new();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
        while got.len() < 2 && tokio::time::Instant::now() < deadline {
            tokio::select! {
                m = incoming.recv() => got.push(m.expect("flux fermé").expect("message illisible").payload),
                _ = tick.tick() => {}
            }
        }
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], format!("{{\"type\":\"MEDIA_STATUS\",\"pad\":\"{}\"}}", "x".repeat(3000)));
        assert_eq!(got[1], "{\"type\":\"PING\"}");
    }

    /// Essai réel, lancé à la main : `MAKU_CAST_HOST=<adresse de l'appareil> cargo test file_reelle -- --ignored --nocapture`.
    /// Envoie à l'appareil une file de 111 titres (adresses fictives : rien ne joue), d'un seul message puis par
    /// lots, et dit ce qu'il répond. D'un seul message, il ferme la connexion ; par lots, il répond à chacun.
    #[tokio::test]
    #[ignore]
    async fn file_reelle() {
        use super::*;
        let host = std::env::var("MAKU_CAST_HOST").expect("MAKU_CAST_HOST");
        for batched in [false, true] {
            let tcp = tokio::net::TcpStream::connect((host.as_str(), 8009)).await.unwrap();
            let config = tokio_rustls::rustls::ClientConfig::builder_with_provider(Arc::new(tokio_rustls::rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(AcceptAny))
                .with_no_client_auth();
            let name = tokio_rustls::rustls::pki_types::ServerName::try_from(host.clone()).unwrap();
            let tls = tokio_rustls::TlsConnector::from(Arc::new(config)).connect(name, tcp).await.unwrap();
            let (read, write) = tokio::io::split(tls);
            let mut link = Link { write, request: 0 };
            let mut incoming = spawn_reader(read);
            link.send(PLATFORM, NS_CONNECTION, json!({ "type": "CONNECT" })).await.unwrap();
            link.send(PLATFORM, NS_RECEIVER, json!({ "type": "LAUNCH", "appId": MEDIA_RECEIVER })).await.unwrap();
            let mut transport = None;
            let mut session = String::new();
            let mut sent = false;
            let mut log: Vec<String> = Vec::new();
            let deadline = tokio::time::Instant::now() + Duration::from_secs(14);
            let outcome = loop {
                let m = match tokio::time::timeout_at(deadline, incoming.recv()).await {
                    Err(_) => break "connexion toujours ouverte",
                    Ok(None) | Ok(Some(Err(_))) => break "CONNEXION FERMÉE PAR L'APPAREIL",
                    Ok(Some(Ok(m))) => m,
                };
                if m.namespace == NS_HEARTBEAT {
                    if m.payload.contains("PING") {
                        let _ = link.send(&m.source, NS_HEARTBEAT, json!({ "type": "PONG" })).await;
                    }
                    continue;
                }
                let v: Value = serde_json::from_str(&m.payload).unwrap_or_default();
                if m.namespace == NS_MEDIA || v["type"] == "LAUNCH_ERROR" {
                    log.push(format!("{} ({} octets)", v["type"].as_str().unwrap_or("?"), m.payload.len()));
                }
                if transport.is_none() {
                    let app = v["status"]["applications"].as_array().and_then(|a| a.iter().find(|x| x["appId"] == MEDIA_RECEIVER)).cloned();
                    if let Some(t) = app.as_ref().and_then(|a| a["transportId"].as_str()) {
                        transport = Some(t.to_owned());
                        session = app.as_ref().and_then(|a| a["sessionId"].as_str()).unwrap_or_default().to_owned();
                    }
                }
                if let (Some(t), false) = (&transport, sent) {
                    sent = true;
                    link.send(t, NS_CONNECTION, json!({ "type": "CONNECT" })).await.unwrap();
                    let items = tests::queue(111);
                    let first = if batched { plan_queue(&items, 0, MESSAGE_BUDGET).first } else { items };
                    let load = json!({ "type": "QUEUE_LOAD", "items": first, "startIndex": 0, "currentTime": 0, "repeatMode": "REPEAT_OFF" });
                    println!("{} : QUEUE_LOAD de {} titres, {} octets", if batched { "par lots" } else { "d'un seul message" }, load["items"].as_array().unwrap().len(), load.to_string().len());
                    link.send(t, NS_MEDIA, load).await.unwrap();
                }
            };
            println!("  → {outcome} ; réponses du lecteur : {log:?}");
            let _ = link.send(PLATFORM, NS_RECEIVER, json!({ "type": "STOP", "sessionId": session })).await;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}
