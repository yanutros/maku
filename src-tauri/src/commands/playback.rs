//! Lecture : vidéo (mpv ou lecteur natif d'Android), musique et sa file, progression envoyée au serveur, touches multimédia.

use crate::*;

#[derive(Serialize)]
pub(crate) struct NowPlaying {
    session: String,
    title: String,
    subtitle: Option<String>,
    /// Vidéo incrustée dans la fenêtre de Maku (Windows) ; sinon, fenêtre mpv séparée.
    embedded: bool,
}

#[derive(Serialize)]
pub(crate) struct MusicQueue {
    session: String,
    tracks: Vec<QueueTrack>,
    index: usize,
}

/// Mélange de Fisher-Yates, sans dépendance supplémentaire : l'aléa vient d'UUID v4.
pub(crate) fn shuffle<T>(list: &mut [T]) {
    for i in (1..list.len()).rev() {
        let j = (uuid::Uuid::new_v4().as_u128() % (i as u128 + 1)) as usize;
        list.swap(i, j);
    }
}

/// Arrête la lecture précédente (une seule à la fois, vidéo ou musique).
pub(crate) fn stop_player(state: &AppState) {
    drop(state.player.lock().unwrap().take());
}

#[tauri::command]
pub async fn play(
    app: AppHandle,
    state: State<'_, AppState>,
    rating_key: String,
    audio_stream: Option<u64>,
    subtitle_stream: Option<u64>,
    from_start: Option<bool>,
    max_kbps: Option<u32>,
    start_ms: Option<u64>,
    media_index: Option<usize>,
) -> CmdResult<NowPlaying> {
    let server = state.server()?;
    let session = uuid::Uuid::new_v4().simple().to_string();
    let mut info = match max_kbps {
        // Qualité réduite : mpv lit le flux converti par le serveur, pistes choisies et sous-titres incrustés.
        Some(kbps) => {
            let s = server.web_stream(&rating_key, media_index, audio_stream, subtitle_stream, Some(kbps), &session, false).await?;
            plex::PlayInfo {
                url: s.url,
                title: s.title,
                subtitle: s.subtitle,
                offset_ms: s.offset_ms,
                duration_ms: s.duration_ms,
                aid: None,
                sid: "auto".into(),
                sub_files: Vec::new(),
                sub_tracks: Vec::new(),
            }
        }
        None => server.play_info(&rating_key, media_index, audio_stream, subtitle_stream).await?,
    };
    if from_start == Some(true) {
        info.offset_ms = 0;
    }
    if let Some(ms) = start_ms {
        info.offset_ms = ms;
    }
    let mpv = player::find_mpv(state.cfg().mpv_path.as_deref())?;
    stop_player(&state);

    #[cfg(windows)]
    let parent = Some(
        app.get_webview_window("main")
            .ok_or_else(|| anyhow!(crate::i18n::tr("app.mainWindowNotFound")))?
            .hwnd()?
            .0 as isize,
    );
    #[cfg(not(windows))]
    let parent = None;

    let now = NowPlaying {
        session: session.clone(),
        title: info.title.clone(),
        subtitle: info.subtitle.clone(),
        embedded: parent.is_some(),
    };
    let entries = vec![Entry { rating_key, duration_ms: info.duration_ms, url: String::new() }];
    let control = player::launch(app, server, mpv, session, entries, Source::Video { info, parent })?;
    *state.player.lock().unwrap() = Some(control);
    Ok(now)
}

/// Système d'exploitation (« windows », « linux », « macos », « android », « ios ») : sur mobile,
/// l'interface lit elle-même vidéo et musique au lieu de passer par mpv.
#[tauri::command]
pub fn platform() -> &'static str {
    std::env::consts::OS
}

/// Construit une file de lecture musicale (voir `play_music`) sans la lancer.
pub(crate) async fn build_queue(
    server: &media::MediaServer,
    rating_key: &str,
    keys: Option<Vec<String>>,
    start: Option<String>,
    shuffle: Option<bool>,
    radio: bool,
) -> anyhow::Result<(Vec<QueueTrack>, usize)> {
    let mut tracks: Vec<QueueTrack> = match keys {
        _ if radio => server.radio(rating_key).await?,
        Some(keys) => {
            // Plusieurs playlists d'un coup (« Écouter tout », « Mix of Three ») : un titre présent dans
            // deux d'entre elles ne passe qu'une fois.
            let mut seen = std::collections::HashSet::new();
            futures::future::join_all(keys.iter().map(|k| server.queue(k)))
                .await
                .into_iter()
                .filter_map(Result::ok)
                .flatten()
                .filter(|t| seen.insert(t.rating_key.clone()))
                .collect()
        }
        None => server.queue(rating_key).await?,
    };
    if tracks.is_empty() {
        return Err(anyhow!(crate::i18n::tr("app.nothingToPlay")));
    }
    let mut index = start
        .as_deref()
        .and_then(|key| tracks.iter().position(|t| t.rating_key == key))
        .unwrap_or(0);
    if shuffle == Some(true) {
        // Le titre choisi (s'il y en a un) reste en tête.
        let first = start.is_some().then(|| tracks.remove(index));
        self::shuffle(&mut tracks);
        if let Some(t) = first {
            tracks.insert(0, t);
        }
        index = 0;
    }
    Ok((tracks, index))
}

/// File de lecture pour le lecteur intégré (mobile) : les titres et l'adresse de leur fichier.
#[tauri::command]
pub async fn music_queue(
    state: State<'_, AppState>,
    rating_key: String,
    keys: Option<Vec<String>>,
    start: Option<String>,
    shuffle: Option<bool>,
    radio: Option<bool>,
) -> CmdResult<MusicQueue> {
    let server = state.server()?;
    let (tracks, index) = build_queue(&server, &rating_key, keys, start, shuffle, radio.unwrap_or(false)).await?;
    Ok(MusicQueue { session: uuid::Uuid::new_v4().to_string(), tracks, index })
}

/// Vidéo pour le lecteur intégré (mobile) : flux HLS du transcodeur Plex.
#[tauri::command]
pub async fn video_stream(
    state: State<'_, AppState>,
    rating_key: String,
    audio_stream: Option<u64>,
    subtitle_stream: Option<u64>,
    max_kbps: Option<u32>,
    media_index: Option<usize>,
    copy_video: Option<bool>,
) -> CmdResult<WebStream> {
    let session = uuid::Uuid::new_v4().simple().to_string();
    // `copy_video` : seul le son est illisible (lecteur Android) ; la vidéo d'origine est gardée.
    let copy = copy_video.unwrap_or(false) && max_kbps.is_none();
    Ok(state.server()?.web_stream(&rating_key, media_index, audio_stream, subtitle_stream, max_kbps, &session, copy).await?)
}

/// Lecture native sur Android (ExoPlayer) : le fichier d'origine, sans conversion, avec les
/// pistes à choisir (numérotées comme pour mpv, à partir de 1 dans chaque type).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeVideo {
    session: String,
    url: String,
    title: String,
    subtitle: Option<String>,
    offset_ms: u64,
    duration_ms: u64,
    aid: Option<usize>,
    /// Numéro de sous-titre, « no » ou « auto ».
    sid: String,
    sub_files: Vec<plex::SubFile>,
}

#[tauri::command]
pub async fn native_video(
    state: State<'_, AppState>,
    rating_key: String,
    audio_stream: Option<u64>,
    subtitle_stream: Option<u64>,
    from_start: Option<bool>,
    start_ms: Option<u64>,
    media_index: Option<usize>,
) -> CmdResult<NativeVideo> {
    let info = state.server()?.play_info(&rating_key, media_index, audio_stream, subtitle_stream).await?;
    Ok(NativeVideo {
        session: uuid::Uuid::new_v4().simple().to_string(),
        url: info.url,
        title: info.title,
        subtitle: info.subtitle,
        offset_ms: start_ms.unwrap_or(if from_start == Some(true) { 0 } else { info.offset_ms }),
        duration_ms: info.duration_ms,
        aid: info.aid,
        sid: info.sid,
        sub_files: info.sub_tracks,
    })
}

#[tauri::command]
pub async fn stop_transcode(state: State<'_, AppState>, session: String) -> CmdResult<()> {
    Ok(state.server()?.stop_transcode(&session).await?)
}

/// Progression envoyée par le lecteur intégré ; au-delà de 90 % à l'arrêt, l'élément est marqué vu.
#[tauri::command]
pub async fn report_timeline(
    state: State<'_, AppState>,
    rating_key: String,
    playback: String,
    time_ms: u64,
    duration_ms: u64,
    session: Option<String>,
) -> CmdResult<()> {
    let server = state.server()?;
    server.timeline(&rating_key, &playback, time_ms, duration_ms, session.as_deref().unwrap_or_default()).await?;
    if playback == "stopped" && duration_ms > 0 && time_ms as f64 / duration_ms as f64 >= 0.9 {
        server.set_watched(&rating_key, true).await?;
    }
    Ok(())
}

/// Joue la musique d'un artiste, d'un album, d'une playlist ou un titre seul ; ou, avec `keys`,
/// une sélection de titres dans cet ordre. `start` : titre par lequel commencer ; `shuffle` : ordre aléatoire.
#[tauri::command]
pub async fn play_music(
    app: AppHandle,
    state: State<'_, AppState>,
    rating_key: String,
    keys: Option<Vec<String>>,
    start: Option<String>,
    shuffle: Option<bool>,
    radio: Option<bool>,
) -> CmdResult<MusicQueue> {
    let server = state.server()?;
    let (tracks, index) = build_queue(&server, &rating_key, keys, start, shuffle, radio.unwrap_or(false)).await?;
    let mpv = player::find_mpv(state.cfg().mpv_path.as_deref())?;
    stop_player(&state);

    let session = uuid::Uuid::new_v4().to_string();
    let entries = tracks
        .iter()
        .map(|t| Entry { rating_key: t.rating_key.clone(), duration_ms: t.duration_ms, url: t.url.clone() })
        .collect();
    let urls = tracks.iter().map(|t| t.url.clone()).collect();
    let control = player::launch(app, server, mpv, session.clone(), entries, Source::Music { urls, start: index })?;
    *state.player.lock().unwrap() = Some(control);
    Ok(MusicQueue { session, tracks, index })
}

/// Titres d'un artiste, d'un album, d'une playlist ou titre seul, à ajouter à la file (mobile, diffusion).
#[tauri::command]
pub async fn music_tracks(state: State<'_, AppState>, rating_key: String) -> CmdResult<Vec<QueueTrack>> {
    Ok(state.server()?.queue(&rating_key).await?)
}

/// « Lire ensuite » / « Ajouter à la file d'attente » pendant la lecture par mpv : les titres de
/// `rating_key` entrent dans la file à la position `at`, sans interrompre le titre en cours.
#[tauri::command]
pub async fn player_queue_insert(state: State<'_, AppState>, rating_key: String, at: usize) -> CmdResult<Vec<QueueTrack>> {
    let tracks = state.server()?.queue(&rating_key).await?;
    let entries = tracks.iter().map(|t| Entry { rating_key: t.rating_key.clone(), duration_ms: t.duration_ms, url: t.url.clone() }).collect();
    let urls = tracks.iter().map(|t| t.url.clone()).collect();
    let player = state.player.lock().unwrap();
    player.as_ref().ok_or_else(|| anyhow!(crate::i18n::tr("app.noPlayback")))?.insert(entries, urls, at)?;
    Ok(tracks)
}

/// Déplace un titre de la file de mpv (glisser-déposer dans la file d'attente).
#[tauri::command]
pub fn player_queue_move(state: State<'_, AppState>, from: usize, to: usize) -> CmdResult<()> {
    let player = state.player.lock().unwrap();
    Ok(player.as_ref().ok_or_else(|| anyhow!(crate::i18n::tr("app.noPlayback")))?.move_entry(from, to)?)
}

/// Bouton Aléatoire du lecteur : la file de mpv prend le nouvel ordre (`order[k]` : position d'avant du titre en `k`).
#[tauri::command]
pub fn player_queue_order(state: State<'_, AppState>, order: Vec<usize>) -> CmdResult<()> {
    let player = state.player.lock().unwrap();
    Ok(player.as_ref().ok_or_else(|| anyhow!(crate::i18n::tr("app.noPlayback")))?.reorder(order)?)
}

/// Commande mpv envoyée par la barre de lecture, par ex. `["cycle", "pause"]`.
#[tauri::command]
pub fn player_command(state: State<'_, AppState>, args: Vec<Value>) -> CmdResult<()> {
    if !allowed_player_command(&args) {
        return Err(anyhow!("commande refusée").into());
    }
    let player = state.player.lock().unwrap();
    let control = player.as_ref().ok_or_else(|| anyhow!(crate::i18n::tr("app.noPlayback")))?;
    Ok(control.send(args)?)
}

/// Commandes que l'interface peut envoyer à mpv : les commandes de lecture, rien d'autre. mpv sait aussi
/// lancer des programmes (`run`, `subprocess`), ouvrir ou écrire des fichiers (`loadfile`, `write-watch-later-config`…) :
/// ces commandes-là ne viennent que du code Rust, jamais de la page.
pub(crate) fn allowed_player_command(args: &[Value]) -> bool {
    const PROPERTIES: [&str; 7] = ["volume", "mute", "loop-playlist", "loop-file", "playlist-pos", "aid", "sid"];
    let arg = |i: usize| args.get(i).and_then(Value::as_str);
    let scalar = |i: usize| args.get(i).is_some_and(|v| v.is_number() || v.is_string() || v.is_boolean());
    match (arg(0), args.len()) {
        (Some("cycle"), 2) => matches!(arg(1), Some("pause" | "mute")),
        (Some("seek"), 2) => args[1].is_number(),
        (Some("seek"), 3) => args[1].is_number() && matches!(arg(2), Some("relative" | "absolute")),
        (Some("add"), 3) => arg(1) == Some("volume") && args[2].is_number(),
        (Some("playlist-next" | "playlist-prev"), 1) => true,
        (Some("set_property"), 3) => arg(1).is_some_and(|p| PROPERTIES.contains(&p)) && scalar(2),
        _ => false,
    }
}

#[tauri::command]
pub fn player_stop(state: State<'_, AppState>) {
    stop_player(&state);
}

/// Linux : ce qui joue, pour les touches multimédia et les commandes du bureau (MPRIS) ; `None`
/// quand plus rien ne joue. Ailleurs, rien à faire : les touches arrivent directement à la page.
#[tauri::command]
pub fn media_update(app: AppHandle, json: Option<String>) -> CmdResult<()> {
    #[cfg(target_os = "linux")]
    if let Some(keys) = app.try_state::<MediaKeys>() {
        let update = match json.as_deref().map(serde_json::from_str::<mpris::State>) {
            Some(Ok(state)) => mpris::Update::State(state),
            _ => mpris::Update::Stop,
        };
        let _ = keys.0.send(update);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (app, json);
    Ok(())
}

/// Linux : le lecteur MPRIS, à qui l'interface envoie ce qui joue.
#[cfg(target_os = "linux")]
pub(crate) struct MediaKeys(pub(crate) tokio::sync::mpsc::UnboundedSender<mpris::Update>);
