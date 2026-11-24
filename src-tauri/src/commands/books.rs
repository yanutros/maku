//! Kavita : livres, BD et mangas (voir kavita.rs). Réglé par serveur, comme Seerr et Tracearr.

use crate::kavita::{BookLibrary, BookSeries, ChapterInfo, Kavita, Progress, SeriesDetail, SeriesPage, TocItem};
use crate::*;

impl AppState {
    /// Kavita du serveur en cours, s'il est configuré.
    fn kavita(&self) -> anyhow::Result<Kavita> {
        let c = self.cfg();
        match (c.kavita_url, c.kavita_key) {
            (Some(url), Some(key)) => Kavita::new(self.http.clone(), &url, &key),
            _ => Err(anyhow!(crate::i18n::tr("kavita.notConfigured"))),
        }
    }
}

/// Connecte Kavita (adresse et clé d'API), après avoir vérifié que la clé ouvre bien ses bibliothèques ; une
/// adresse vide le déconnecte. Rend l'adresse retenue (nettoyée).
#[tauri::command]
pub async fn kavita_configure(state: State<'_, AppState>, url: String, key: String) -> CmdResult<Option<String>> {
    if url.trim().is_empty() {
        state.update_cfg(|c| {
            c.kavita_url = None;
            c.kavita_key = None;
        })?;
        return Ok(None);
    }
    let k = Kavita::new(state.http.clone(), &url, &key)?;
    k.libraries().await?;
    let base = k.base().to_owned();
    let key = key.trim().to_owned();
    state.update_cfg(|c| {
        c.kavita_url = Some(base.clone());
        c.kavita_key = Some(key);
    })?;
    Ok(Some(base))
}

#[tauri::command]
pub async fn kavita_libraries(state: State<'_, AppState>) -> CmdResult<Vec<BookLibrary>> {
    Ok(state.kavita()?.libraries().await?)
}

#[tauri::command]
pub async fn kavita_series(state: State<'_, AppState>, library_id: i64, page: u32, size: u32) -> CmdResult<SeriesPage> {
    Ok(state.kavita()?.library_series(library_id, page, size).await?)
}

#[tauri::command]
pub async fn kavita_on_deck(state: State<'_, AppState>, library_id: i64) -> CmdResult<Vec<BookSeries>> {
    Ok(state.kavita()?.on_deck(library_id).await?)
}

#[tauri::command]
pub async fn kavita_series_detail(state: State<'_, AppState>, series_id: i64) -> CmdResult<SeriesDetail> {
    Ok(state.kavita()?.series_detail(series_id).await?)
}

#[tauri::command]
pub async fn kavita_chapter(state: State<'_, AppState>, chapter_id: i64) -> CmdResult<ChapterInfo> {
    Ok(state.kavita()?.chapter_info(chapter_id).await?)
}

/// Livre EPUB : une page (section du livre), en HTML préparé par Kavita.
#[tauri::command]
pub async fn kavita_book_page(state: State<'_, AppState>, chapter_id: i64, page: i64) -> CmdResult<String> {
    Ok(state.kavita()?.book_page(chapter_id, page).await?)
}

#[tauri::command]
pub async fn kavita_book_toc(state: State<'_, AppState>, chapter_id: i64) -> CmdResult<Vec<TocItem>> {
    Ok(state.kavita()?.book_toc(chapter_id).await?)
}

#[tauri::command]
pub async fn kavita_progress(state: State<'_, AppState>, progress: Progress) -> CmdResult<()> {
    Ok(state.kavita()?.save_progress(&progress).await?)
}
