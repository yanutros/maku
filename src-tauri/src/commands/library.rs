//! Parcourir les bibliothèques : accueil, rangées, fiches, recherche, acteurs, notes et paroles.

use crate::*;

#[tauri::command]
pub async fn plex_sections(state: State<'_, AppState>) -> CmdResult<Vec<Section>> {
    Ok(state.server()?.sections().await?)
}

#[tauri::command]
pub async fn plex_section_items(
    state: State<'_, AppState>,
    key: String,
    start: u32,
    size: u32,
    kind: Option<u32>,
    genre: Option<String>,
    sort: Option<String>,
    filters: Option<Vec<(String, String)>>,
) -> CmdResult<Page> {
    Ok(state
        .server()?
        .section_items(&key, start, size, kind, genre.as_deref(), sort.as_deref(), &filters.unwrap_or_default())
        .await?)
}

/// Filtres et tris proposés pour une bibliothèque de films ou de séries.
#[tauri::command]
pub async fn plex_section_filters(state: State<'_, AppState>, key: String, kind: String) -> CmdResult<plex::SectionFilters> {
    Ok(state.server()?.section_filters(&key, &kind).await?)
}

/// Valeurs possibles d'un filtre (genres, années, acteurs…) dans une bibliothèque.
#[tauri::command]
pub async fn plex_filter_values(state: State<'_, AppState>, key: String, kind: String, filter: String) -> CmdResult<Vec<plex::FilterValue>> {
    Ok(state.server()?.filter_values(&key, &kind, &filter).await?)
}

#[tauri::command]
pub async fn plex_genres(state: State<'_, AppState>, key: String) -> CmdResult<Vec<Genre>> {
    Ok(state.server()?.genres(&key).await?)
}

#[tauri::command]
pub async fn plex_section_hubs(state: State<'_, AppState>, key: String) -> CmdResult<Vec<Hub>> {
    Ok(state.server()?.section_hubs(&key).await?)
}

#[tauri::command]
pub async fn plex_playlists(state: State<'_, AppState>, key: String) -> CmdResult<Vec<Item>> {
    Ok(state.server()?.playlists(&key).await?)
}

#[tauri::command]
pub async fn plex_tracks(state: State<'_, AppState>, rating_key: String) -> CmdResult<Vec<Item>> {
    Ok(state.server()?.tracks(&rating_key).await?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Home {
    on_deck: Vec<Item>,
    recently_added: Vec<Item>,
}

#[tauri::command]
pub async fn plex_home(state: State<'_, AppState>) -> CmdResult<Home> {
    let server = state.server()?;
    let (on_deck, recently_added) = tokio::try_join!(server.on_deck(), server.recently_added())?;
    Ok(Home { on_deck, recently_added })
}

#[tauri::command]
pub async fn plex_metadata(state: State<'_, AppState>, rating_key: String) -> CmdResult<ItemDetail> {
    Ok(state.server()?.metadata(&rating_key).await?)
}

/// Logo d'un titre absent du serveur, cherché sur le service en ligne de Plex (une fois par titre).
#[tauri::command]
pub async fn plex_remote_logo(state: State<'_, AppState>, guid: String, fallback: Option<bool>) -> CmdResult<Option<String>> {
    let fallback = fallback.unwrap_or(true);
    let cache_key = format!("{guid}|{fallback}");
    if let Some(known) = state.remote_logos.lock().unwrap().get(&cache_key) {
        return Ok(known.clone());
    }
    let c = state.cfg();
    let Some(token) = c.plex_token else { return Ok(None) };
    // Peu de requêtes à la fois : le service de Plex refuse les rafales (l'accueil en demande des dizaines).
    let _slot = state.logo_slots.acquire().await;
    if let Some(known) = state.remote_logos.lock().unwrap().get(&cache_key) {
        return Ok(known.clone());
    }
    // Un échec (réseau, limite de requêtes) n'est pas retenu : le logo sera redemandé plus tard.
    let Ok(logo) = plex::remote_logo(&state.http, &c.client_id, &token, &guid, fallback, c.tmdb_key.as_deref()).await else {
        return Ok(None);
    };
    state.remote_logos.lock().unwrap().insert(cache_key, logo.clone());
    Ok(logo)
}

/// Films et séries où joue une personne (identifiant Plex de l'acteur), toutes bibliothèques confondues.
#[tauri::command]
pub async fn plex_person_items(state: State<'_, AppState>, id: String) -> CmdResult<Vec<plex::Item>> {
    Ok(state.server()?.person_items(&id).await?)
}

#[tauri::command]
pub async fn plex_collections(state: State<'_, AppState>, key: String) -> CmdResult<Vec<Item>> {
    Ok(state.server()?.collections(&key).await?)
}

#[tauri::command]
pub async fn plex_collection_items(state: State<'_, AppState>, rating_key: String) -> CmdResult<Vec<Item>> {
    Ok(state.server()?.collection_items(&rating_key).await?)
}

#[tauri::command]
pub async fn plex_related(state: State<'_, AppState>, rating_key: String) -> CmdResult<Vec<Item>> {
    Ok(state.server()?.related(&rating_key).await?)
}

#[tauri::command]
pub async fn plex_children(state: State<'_, AppState>, rating_key: String) -> CmdResult<Vec<Item>> {
    Ok(state.server()?.children(&rating_key).await?)
}

#[tauri::command]
pub async fn plex_search(state: State<'_, AppState>, query: String) -> CmdResult<Vec<Item>> {
    Ok(state.server()?.search(&query).await?)
}

#[tauri::command]
pub async fn plex_adjacent(state: State<'_, AppState>, rating_key: String) -> CmdResult<Adjacent> {
    Ok(state.server()?.adjacent(&rating_key).await?)
}

/// Repères d'un épisode (intro, générique), pour le bouton « Passer ».
#[tauri::command]
pub async fn plex_markers(state: State<'_, AppState>, rating_key: String) -> CmdResult<Vec<plex::Marker>> {
    Ok(state.server()?.markers(&rating_key).await?)
}

/// « Voir les informations » d'un titre : format et fichier.
#[tauri::command]
pub async fn media_info(state: State<'_, AppState>, rating_key: String) -> CmdResult<plex::MediaInfo> {
    Ok(state.server()?.media_info(&rating_key).await?)
}

/// Miniatures d'aperçu de la vidéo, pour la barre de progression (aucune si le serveur n'en a pas).
#[tauri::command]
pub async fn video_previews(
    state: State<'_, AppState>,
    rating_key: String,
    media_index: Option<usize>,
) -> CmdResult<Option<plex::Previews>> {
    Ok(state.server()?.previews(&rating_key, media_index).await?)
}

/// Bande-annonce, dans la langue de l'interface autant que possible : une bande-annonce du serveur
/// dont le titre dit cette langue, sinon celle de TMDB dans cette langue (avec la clé de
/// l'utilisateur), sinon une bande-annonce du serveur, sinon le lien YouTube de la fiche (Jellyfin),
/// sinon TMDB en anglais. `None` : rien trouvé.
#[tauri::command]
pub async fn plex_trailer(state: State<'_, AppState>, rating_key: String) -> CmdResult<Option<plex::Trailer>> {
    let source = state.server()?.trailer(&rating_key).await?;
    let lang = crate::i18n::language();
    // Comment les titres de bandes-annonces nomment la langue (« Verity (French Teaser Trailer) »).
    let names: &[&str] = match lang {
        "fr" => &["french", "français", "francais", "vf"],
        "es" => &["spanish", "español", "espanol", "castellano"],
        "de" => &["german", "deutsch"],
        "nl" => &["dutch", "nederlands"],
        _ => &["english"],
    };
    let in_lang = |title: &str| {
        let t = title.to_lowercase();
        names.iter().any(|n| t.contains(n))
    };
    let local = |key: &str| plex::Trailer { rating_key: Some(key.to_string()), youtube: None };
    let youtube = |id: &str| plex::Trailer { rating_key: None, youtube: Some(id.to_string()) };
    if let Some((key, _)) = source.local.iter().find(|(_, title)| in_lang(title)) {
        return Ok(Some(local(key)));
    }
    let c = state.cfg();
    let online = match (c.tmdb_key.as_deref(), source.tmdb_kind, source.tmdb_id.as_deref()) {
        (Some(key), Some(kind), Some(id)) => plex::tmdb_trailers(&state.http, key, kind, id, lang).await,
        _ => Vec::new(),
    };
    if let Some((id, _)) = online.iter().find(|(_, same)| *same) {
        return Ok(Some(youtube(id)));
    }
    if let Some((key, _)) = source.local.first() {
        return Ok(Some(local(key)));
    }
    if let Some(id) = &source.youtube {
        return Ok(Some(youtube(id)));
    }
    Ok(online.first().map(|(id, _)| youtube(id)))
}

/// Notes Allociné d'un film ou d'une série (voir allocine.rs), désactivables dans les Réglages.
#[tauri::command]
pub async fn allocine_ratings(state: State<'_, AppState>, imdb_id: String, show: bool) -> CmdResult<Vec<plex::SiteRating>> {
    Ok(allocine::ratings(&state.http, &imdb_id, show).await?)
}

/// Paroles d'un morceau (lrclib.net), synchronisées si possible.
#[tauri::command]
pub async fn track_lyrics(state: State<'_, AppState>, artist: String, title: String, album: Option<String>, duration: f64) -> CmdResult<lyrics::Lyrics> {
    Ok(lyrics::fetch(&state.http, &artist, &title, album.as_deref(), duration).await?)
}
