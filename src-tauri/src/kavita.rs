//! Kavita : bibliothèques de livres, BD et mangas, à côté de celles du serveur vidéo (pas au téléviseur).
//!
//! Maku parle à Kavita avec une clé d'API de l'utilisateur (Kavita › Paramètres › Clés d'authentification),
//! envoyée dans l'en-tête `x-api-key`. La progression de lecture est donc celle de son compte Kavita. Les images
//! (couvertures, pages) sont lues directement par la page : la clé y voyage en paramètre `apiKey`, comme le fait
//! Kavita lui-même pour ses images et ses liens OPDS.

use anyhow::{anyhow, bail, Result};
use reqwest::{Client, Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Comparaison « contient » et champ « bibliothèques » des filtres de séries (SeriesFilterV2Dto de Kavita).
const FILTER_CONTAINS: u8 = 5;
const FIELD_LIBRARIES: u8 = 19;
/// Tri par nom de tri.
const SORT_NAME: u8 = 1;
/// Volume des chapitres rangés hors volume, et celui des hors-séries (numéros réservés de Kavita).
const LOOSE_LEAF_VOLUME: f64 = -100_000.0;
const SPECIAL_VOLUME: f64 = 100_000.0;

#[derive(Clone)]
pub struct Kavita {
    http: Client,
    base: String,
    key: String,
}

/// Bibliothèque Kavita : `kind` vaut « manga », « comic » ou « book » (romans, light novels).
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BookLibrary {
    pub id: i64,
    pub name: String,
    pub kind: &'static str,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BookSeries {
    pub id: i64,
    pub name: String,
    pub library_id: i64,
    pub cover: String,
    pub pages: i64,
    pub pages_read: i64,
    /// « images » (archive ou dossier d'images), « epub », « pdf » ou « unknown ».
    pub format: &'static str,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SeriesPage {
    pub items: Vec<BookSeries>,
    pub total: i64,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BookChapter {
    pub id: i64,
    pub volume_id: i64,
    /// Numéro affiché (« 12 », « 12.5 », « 1-3 ») ; vide pour un livre sans numéro.
    pub number: String,
    pub title: String,
    pub pages: i64,
    pub pages_read: i64,
    pub cover: String,
    pub special: bool,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BookVolume {
    pub id: i64,
    pub name: String,
    pub cover: String,
    pub pages: i64,
    pub pages_read: i64,
    pub chapters: Vec<BookChapter>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SeriesDetail {
    pub series: BookSeries,
    pub summary: Option<String>,
    /// Type de la bibliothèque (« manga » : lecture de droite à gauche proposée).
    pub library_kind: &'static str,
    pub volumes: Vec<BookVolume>,
    /// Chapitres rangés hors volume.
    pub chapters: Vec<BookChapter>,
    pub specials: Vec<BookChapter>,
    /// Chapitre où reprendre la lecture (Kavita le choisit d'après la progression).
    pub continue_chapter: Option<i64>,
}

/// Ce qu'il faut au lecteur pour un chapitre.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChapterInfo {
    pub chapter_id: i64,
    pub volume_id: i64,
    pub series_id: i64,
    pub library_id: i64,
    pub library_kind: &'static str,
    pub series_name: String,
    pub title: String,
    pub pages: i64,
    pub format: &'static str,
    /// Adresse d'une page, avec `{page}` à remplacer par son numéro (à partir de 0).
    pub page_url: String,
    /// Page où reprendre (0 : le début ; la dernière page lue sinon).
    pub resume_page: i64,
    /// Chapitres voisins dans l'ordre de lecture de la série (-1 : aucun).
    pub next_chapter: i64,
    pub prev_chapter: i64,
    /// Pages en largeur (une double page déjà assemblée) : affichées seules en lecture double page.
    pub wide: Vec<bool>,
}

/// Nom du chapitre sous celui de la série : son titre s'il en a un à lui, sinon « Volume 2 Chapitre 5 ». Kavita
/// reprend souvent le nom de la série (une BD par série), et donne le nom du fichier pour un hors-série : ni l'un
/// ni l'autre n'apporte quelque chose sous le nom de la série.
fn chapter_label(info: &Value) -> String {
    let series = text(&info["seriesName"]).unwrap_or_default().to_lowercase();
    let mut parts: Vec<String> = Vec::new();
    if let Some(t) = text(&info["chapterTitle"]).filter(|t| t.to_lowercase() != series) {
        parts.push(t);
    }
    if !info["isSpecial"].as_bool().unwrap_or(false) {
        if let Some(s) = text(&info["subtitle"]).filter(|s| s.to_lowercase() != series && !parts.contains(s)) {
            parts.push(s);
        }
    }
    parts.join(" — ")
}

/// Pages en largeur, d'après les dimensions que Kavita relève en préparant le chapitre.
fn wide_pages(dimensions: &Value, pages: i64) -> Vec<bool> {
    let mut wide = vec![false; pages.max(0) as usize];
    for d in dimensions.as_array().into_iter().flatten() {
        if let Some(slot) = usize::try_from(int(&d["pageNumber"])).ok().and_then(|i| wide.get_mut(i)) {
            *slot = d["isWide"].as_bool().unwrap_or(false);
        }
    }
    wide
}

/// Entrée du sommaire d'un livre EPUB : la page (section du livre) où elle commence, et l'ancre dans cette page.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TocItem {
    pub title: String,
    pub page: i64,
    pub part: String,
    pub children: Vec<TocItem>,
}

fn toc_item(v: &Value) -> TocItem {
    TocItem {
        title: text(&v["title"]).unwrap_or_default(),
        page: int(&v["page"]),
        part: text(&v["part"]).unwrap_or_default(),
        children: v["children"].as_array().map(|c| c.iter().map(toc_item).collect()).unwrap_or_default(),
    }
}

/// Kavita écrit les adresses des images et des polices d'un livre sans schéma, avec l'hôte tel qu'il l'a reçu
/// (« //hôte/api/book/12/book-resources?… ») : derrière un relais, ce n'est pas forcément l'adresse que connaît
/// Maku, et la page les chercherait avec son propre schéma. Elles sont réécrites avec l'adresse de Kavita.
fn absolute_resources(html: &str, base: &str, chapter_id: i64) -> String {
    let needle = format!("/api/book/{chapter_id}/book-resources?");
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find(&needle) {
        let before = &rest[..at];
        // Début de l'adresse : après le dernier délimiteur (guillemet, parenthèse, espace ou virgule d'un srcset).
        let start = before.rfind(['"', '\'', '(', ' ', '\n', '\t', ',']).map(|i| i + 1).unwrap_or(0);
        let head = &before[start..];
        out.push_str(&before[..start]);
        if head.is_empty() || head.starts_with("//") || head.starts_with("http://") || head.starts_with("https://") {
            out.push_str(base);
        } else {
            out.push_str(head);
        }
        out.push_str(&needle);
        rest = &rest[at + needle.len()..];
    }
    out.push_str(rest);
    out
}

fn library_kind(t: &Value) -> &'static str {
    match t.as_i64() {
        Some(0) => "manga",
        Some(2) | Some(4) => "book",
        _ => "comic",
    }
}

fn format(f: &Value) -> &'static str {
    match f.as_i64() {
        Some(0) | Some(1) => "images",
        Some(3) => "epub",
        Some(4) => "pdf",
        _ => "unknown",
    }
}

fn int(v: &Value) -> i64 {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).unwrap_or(0)
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned)
}

/// Adresse de Kavita telle que saisie : schéma obligatoire, sans « / » final ni chemin de page (« …/home »).
pub fn clean_url(url: &str) -> Result<String> {
    let url = url.trim().trim_end_matches('/');
    let parsed = reqwest::Url::parse(url).map_err(|_| anyhow!(crate::i18n::tr("kavita.badUrl")))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        bail!(crate::i18n::tr("kavita.badUrl"));
    }
    // L'adresse d'une page de Kavita (« …/home », « …/library/1 ») : on garde la racine.
    let mut base = format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or_default());
    if let Some(port) = parsed.port() {
        base.push_str(&format!(":{port}"));
    }
    let path = parsed.path().trim_end_matches('/');
    let app_pages = ["/home", "/login", "/library", "/libraries", "/settings", "/want-to-read", "/bookmarks"];
    let kept = app_pages.iter().find_map(|p| path.find(p).map(|i| &path[..i])).unwrap_or(path);
    base.push_str(kept.trim_end_matches('/'));
    Ok(base)
}

impl Kavita {
    pub fn new(http: Client, url: &str, key: &str) -> Result<Self> {
        let key = key.trim();
        // Clé de Kavita : lettres, chiffres, tirets ou soulignés ; jamais d'espace ni de « & ».
        if key.is_empty() || key.len() > 256 || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            bail!(crate::i18n::tr("kavita.badKey"));
        }
        Ok(Kavita { http, base: clean_url(url)?, key: key.to_owned() })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> Result<reqwest::Response> {
        let mut req = self.http.request(method, format!("{}{path}", self.base)).header("x-api-key", &self.key).header("Accept", "application/json");
        if let Some(body) = body {
            req = req.json(&body);
        }
        let resp = req.send().await.map_err(|e| anyhow!(crate::i18n::tr_with("kavita.unreachable", &[("error", &e.without_url().to_string())])))?;
        match resp.status() {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => bail!(crate::i18n::tr("kavita.refused")),
            s if !s.is_success() => bail!(crate::i18n::tr_with("kavita.replied", &[("status", &s.as_u16().to_string())])),
            _ => Ok(resp),
        }
    }

    async fn json(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        let resp = self.call(method, path, body).await?;
        let bytes = resp.bytes().await?;
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Image de Kavita (`path` commence par « /api/image/ » ou « /api/Reader/ »), avec la clé en paramètre.
    fn image(&self, path_and_query: &str) -> String {
        let sep = if path_and_query.contains('?') { '&' } else { '?' };
        format!("{}{path_and_query}{sep}apiKey={}", self.base, self.key)
    }

    fn series(&self, s: &Value) -> Option<BookSeries> {
        let id = s["id"].as_i64()?;
        Some(BookSeries {
            id,
            name: text(&s["name"]).unwrap_or_default(),
            library_id: int(&s["libraryId"]),
            cover: self.image(&format!("/api/image/series-cover?seriesId={id}")),
            pages: int(&s["pages"]),
            pages_read: int(&s["pagesRead"]),
            format: format(&s["format"]),
        })
    }

    fn chapter(&self, c: &Value) -> Option<BookChapter> {
        let id = c["id"].as_i64()?;
        let number = text(&c["range"]).or_else(|| text(&c["number"])).unwrap_or_default();
        // Numéros réservés de Kavita pour « pas de numéro » : rien à afficher.
        let number = if number.starts_with("-100000") || number.starts_with("100000") { String::new() } else { number };
        Some(BookChapter {
            id,
            volume_id: int(&c["volumeId"]),
            number,
            title: text(&c["titleName"]).or_else(|| text(&c["title"])).unwrap_or_default(),
            pages: int(&c["pages"]),
            pages_read: int(&c["pagesRead"]),
            cover: self.image(&format!("/api/image/chapter-cover?chapterId={id}")),
            special: c["isSpecial"].as_bool().unwrap_or(false),
        })
    }

    /// Bibliothèques auxquelles le compte a accès.
    pub async fn libraries(&self) -> Result<Vec<BookLibrary>> {
        let v = self.json(Method::GET, "/api/Library/libraries", None).await?;
        Ok(v.as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| Some(BookLibrary { id: l["id"].as_i64()?, name: text(&l["name"])?, kind: library_kind(&l["type"]) }))
            .collect())
    }

    /// Séries d'une bibliothèque, par nom, page `page` (à partir de 1).
    pub async fn library_series(&self, library_id: i64, page: u32, size: u32) -> Result<SeriesPage> {
        let filter = json!({
            "statements": [{ "comparison": FILTER_CONTAINS, "field": FIELD_LIBRARIES, "value": library_id.to_string() }],
            "combination": 1,
            "sortOptions": { "sortField": SORT_NAME, "isAscending": true },
            "limitTo": 0,
        });
        let resp = self.call(Method::POST, &format!("/api/Series/all-v2?PageNumber={}&PageSize={}", page.max(1), size.clamp(1, 200)), Some(filter)).await?;
        // Le nombre total est dans l'en-tête « Pagination » (JSON).
        let total = resp
            .headers()
            .get("Pagination")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| serde_json::from_str::<Value>(h).ok())
            .map(|p| if p["totalItems"].is_null() { int(&p["TotalItems"]) } else { int(&p["totalItems"]) });
        let v: Value = serde_json::from_slice(&resp.bytes().await?)?;
        let items: Vec<BookSeries> = v.as_array().into_iter().flatten().filter_map(|s| self.series(s)).collect();
        let total = total.unwrap_or(items.len() as i64);
        Ok(SeriesPage { items, total })
    }

    /// Séries en cours de lecture (« Continuer la lecture » de Kavita), d'une bibliothèque ou de toutes (0).
    pub async fn on_deck(&self, library_id: i64) -> Result<Vec<BookSeries>> {
        let v = self.json(Method::POST, &format!("/api/Series/on-deck?PageNumber=1&PageSize=20&libraryId={library_id}"), None).await?;
        Ok(v.as_array().into_iter().flatten().filter_map(|s| self.series(s)).collect())
    }

    pub async fn series_detail(&self, series_id: i64) -> Result<SeriesDetail> {
        let paths = [
            format!("/api/Series/{series_id}"),
            format!("/api/Series/metadata?seriesId={series_id}"),
            format!("/api/Series/series-detail?seriesId={series_id}"),
            format!("/api/Reader/continue-point?seriesId={series_id}"),
        ];
        let (series, metadata, detail, resume) = tokio::join!(
            self.json(Method::GET, &paths[0], None),
            self.json(Method::GET, &paths[1], None),
            self.json(Method::GET, &paths[2], None),
            self.json(Method::GET, &paths[3], None),
        );
        let series = self.series(&series?).ok_or_else(|| anyhow!(crate::i18n::tr("kavita.notFound")))?;
        let detail = detail?;
        let list = |key: &str| -> Vec<BookChapter> { detail[key].as_array().into_iter().flatten().filter_map(|c| self.chapter(c)).collect() };
        let volumes = detail["volumes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|v| {
                let n = v["minNumber"].as_f64().unwrap_or(0.0);
                n != LOOSE_LEAF_VOLUME && n != SPECIAL_VOLUME
            })
            .filter_map(|v| {
                let id = v["id"].as_i64()?;
                Some(BookVolume {
                    id,
                    name: text(&v["name"]).unwrap_or_default(),
                    cover: self.image(&format!("/api/image/volume-cover?volumeId={id}")),
                    pages: int(&v["pages"]),
                    pages_read: int(&v["pagesRead"]),
                    chapters: v["chapters"].as_array().into_iter().flatten().filter_map(|c| self.chapter(c)).collect(),
                })
            })
            .collect();
        Ok(SeriesDetail {
            summary: metadata.ok().and_then(|m| text(&m["summary"])),
            library_kind: library_kind(&detail["libraryType"]),
            volumes,
            chapters: list("chapters"),
            specials: list("specials"),
            continue_chapter: resume.ok().and_then(|c| c["id"].as_i64()),
            series,
        })
    }

    pub async fn chapter_info(&self, chapter_id: i64) -> Result<ChapterInfo> {
        let (info_path, progress_path) = (
            format!("/api/Reader/chapter-info?chapterId={chapter_id}&extractPdf=true&includeDimensions=true"),
            format!("/api/Reader/get-progress?chapterId={chapter_id}"),
        );
        let (info, progress) = tokio::join!(self.json(Method::GET, &info_path, None), self.json(Method::GET, &progress_path, None));
        let info = info?;
        let (series_id, volume_id) = (int(&info["seriesId"]), int(&info["volumeId"]));
        let (next_path, prev_path) = (
            format!("/api/Reader/next-chapter?seriesId={series_id}&volumeId={volume_id}&currentChapterId={chapter_id}"),
            format!("/api/Reader/prev-chapter?seriesId={series_id}&volumeId={volume_id}&currentChapterId={chapter_id}"),
        );
        let (next, prev) = tokio::join!(self.json(Method::GET, &next_path, None), self.json(Method::GET, &prev_path, None));
        let pages = int(&info["pages"]);
        // Chapitre fini : on le relit depuis le début.
        let read = progress.ok().map(|p| int(&p["pageNum"])).unwrap_or(0);
        let resume_page = if read >= pages { 0 } else { read.clamp(0, (pages - 1).max(0)) };
        let format = format(&info["seriesFormat"]);
        let title = chapter_label(&info);
        Ok(ChapterInfo {
            chapter_id,
            volume_id,
            series_id,
            library_id: int(&info["libraryId"]),
            library_kind: library_kind(&info["libraryType"]),
            series_name: text(&info["seriesName"]).unwrap_or_default(),
            title,
            pages,
            format,
            page_url: self.image(&format!("/api/Reader/image?chapterId={chapter_id}&page={{page}}&extractPdf={}", format == "pdf")),
            resume_page,
            next_chapter: next.ok().and_then(|v| v.as_i64()).unwrap_or(-1),
            prev_chapter: prev.ok().and_then(|v| v.as_i64()).unwrap_or(-1),
            wide: wide_pages(&info["pageDimensions"], pages),
        })
    }

    /// Page `page` (à partir de 0) d'un livre EPUB : le HTML que Kavita prépare pour son propre lecteur (styles
    /// limités à `.book-content`, liens internes marqués `kavita-page` et `kavita-part`), adresses rendues absolues.
    pub async fn book_page(&self, chapter_id: i64, page: i64) -> Result<String> {
        let body = self.call(Method::GET, &format!("/api/Book/{chapter_id}/book-page?page={page}"), None).await?.text().await?;
        // Selon l'en-tête Accept, Kavita rend la page telle quelle ou en chaîne JSON.
        let html = if body.trim_start().starts_with('"') { serde_json::from_str::<String>(body.trim())? } else { body };
        Ok(absolute_resources(&html, &self.base, chapter_id))
    }

    /// Sommaire d'un livre EPUB.
    pub async fn book_toc(&self, chapter_id: i64) -> Result<Vec<TocItem>> {
        let v = self.json(Method::GET, &format!("/api/Book/{chapter_id}/chapters"), None).await?;
        Ok(v.as_array().map(|a| a.iter().map(toc_item).collect()).unwrap_or_default())
    }

    /// Progression : `page` pages lues de ce chapitre (le nombre de pages : chapitre terminé).
    pub async fn save_progress(&self, p: &Progress) -> Result<()> {
        let body = json!({
            "volumeId": p.volume_id,
            "chapterId": p.chapter_id,
            "pageNum": p.page.max(0),
            "seriesId": p.series_id,
            "libraryId": p.library_id,
        });
        self.call(Method::POST, "/api/Reader/progress", Some(body)).await?;
        Ok(())
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub chapter_id: i64,
    pub volume_id: i64,
    pub series_id: i64,
    pub library_id: i64,
    pub page: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresse_de_kavita_nettoyee() {
        assert_eq!(clean_url("http://kavita.lan:5000/").unwrap(), "http://kavita.lan:5000");
        // L'adresse de la page d'accueil, copiée depuis le navigateur.
        assert_eq!(clean_url(" http://kavita.lan:5000/home ").unwrap(), "http://kavita.lan:5000");
        assert_eq!(clean_url("https://exemple.fr/kavita/library/3").unwrap(), "https://exemple.fr/kavita");
        assert_eq!(clean_url("https://exemple.fr/kavita").unwrap(), "https://exemple.fr/kavita");
        assert!(clean_url("kavita.lan:5000").is_err());
        assert!(clean_url("ftp://kavita.lan").is_err());
    }

    #[test]
    fn cle_refusee_si_elle_pourrait_casser_une_adresse() {
        let http = Client::new();
        assert!(Kavita::new(http.clone(), "http://k", "abc-DEF_123").is_ok());
        assert!(Kavita::new(http.clone(), "http://k", "").is_err());
        assert!(Kavita::new(http.clone(), "http://k", "a&b").is_err());
        assert!(Kavita::new(http, "http://k", "a b").is_err());
    }

    /// Faux Kavita : répond selon le chemin, et note chaque demande (méthode, chemin et paramètres, clé, corps).
    type Journal = std::sync::Arc<std::sync::Mutex<Vec<(String, String, String, String)>>>;

    async fn faux_kavita() -> (String, Journal) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let log: Journal = Default::default();
        let seen = log.clone();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = listener.accept().await {
                let seen = seen.clone();
                tokio::spawn(async move {
                    let mut data = Vec::new();
                    let mut buf = [0u8; 8192];
                    loop {
                        let n = s.read(&mut buf).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        data.extend_from_slice(&buf[..n]);
                        let text = String::from_utf8_lossy(&data).to_string();
                        if let Some(end) = text.find("\r\n\r\n") {
                            let length = text
                                .lines()
                                .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                                .unwrap_or(0);
                            if data.len() >= end + 4 + length {
                                break;
                            }
                        }
                    }
                    let text = String::from_utf8_lossy(&data).to_string();
                    let mut first = text.split_whitespace();
                    let (method, target) = (first.next().unwrap_or_default().to_owned(), first.next().unwrap_or_default().to_owned());
                    let key = text.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("x-api-key:").map(|v| v.trim().to_owned())).unwrap_or_default();
                    let body = text.split_once("\r\n\r\n").map(|(_, b)| b.to_owned()).unwrap_or_default();
                    seen.lock().unwrap().push((method, target.clone(), key.clone(), body));
                    let path = target.split('?').next().unwrap_or_default();
                    let (status, extra, reply): (&str, &str, &str) = if key != "cle-ok" {
                        ("401 Unauthorized", "", "")
                    } else {
                        match path {
                            "/api/Library/libraries" => ("200 OK", "", r#"[{"id":1,"name":"Mangas","type":0},{"id":2,"name":"Romans","type":2},{"id":3,"name":"BD","type":1}]"#),
                            "/api/Series/all-v2" => (
                                "200 OK",
                                "Pagination: {\"currentPage\":1,\"itemsPerPage\":2,\"totalItems\":57,\"totalPages\":29}\r\n",
                                r#"[{"id":11,"name":"La Lune","libraryId":1,"pages":120,"pagesRead":45,"format":1},{"id":12,"name":"Roman","libraryId":1,"pages":300,"pagesRead":0,"format":3}]"#,
                            ),
                            "/api/Series/11" => ("200 OK", "", r#"{"id":11,"name":"La Lune","libraryId":1,"pages":120,"pagesRead":45,"format":1}"#),
                            "/api/Series/metadata" => ("200 OK", "", r#"{"summary":"  Une histoire.  "}"#),
                            "/api/Series/series-detail" => (
                                "200 OK",
                                "",
                                r#"{"libraryType":0,"volumes":[{"id":101,"name":"Volume 1","minNumber":1,"pages":40,"pagesRead":40,"chapters":[{"id":1001,"volumeId":101,"range":"1","pages":20,"pagesRead":20,"isSpecial":false}]},{"id":199,"name":"","minNumber":-100000,"chapters":[]}],"chapters":[{"id":1009,"volumeId":199,"range":"9","titleName":"La fin","pages":20,"pagesRead":0}],"specials":[{"id":1500,"volumeId":150,"range":"100000","title":"Bonus","pages":5,"isSpecial":true}]}"#,
                            ),
                            "/api/Reader/continue-point" => ("200 OK", "", r#"{"id":1009}"#),
                            "/api/Reader/chapter-info" => (
                                "200 OK",
                                "",
                                r#"{"seriesId":11,"volumeId":101,"libraryId":1,"libraryType":0,"seriesName":"La Lune","title":"La Lune - Le départ","chapterTitle":"Le départ","subtitle":"Volume 1 Chapitre 1","pages":20,"seriesFormat":4,"pageDimensions":[{"pageNumber":0,"width":800,"height":1200,"isWide":false},{"pageNumber":3,"width":1600,"height":1200,"isWide":true},{"pageNumber":40,"isWide":true}]}"#,
                            ),
                            "/api/Reader/get-progress" => ("200 OK", "", r#"{"pageNum":20}"#),
                            "/api/Reader/next-chapter" => ("200 OK", "", "1009"),
                            "/api/Reader/prev-chapter" => ("200 OK", "", "-1"),
                            "/api/Reader/progress" => ("200 OK", "", ""),
                            // Page d'un livre, en chaîne JSON, avec l'hôte interne vu par Kavita derrière un relais.
                            "/api/Book/1001/book-page" => (
                                "200 OK",
                                "",
                                r#""<div class=\"chap\"><style>.book-content p{background:url(//interne:5000/api/book/1001/book-resources?apiKey=img&file=f.png)}</style><img src=\"//interne:5000/api/book/1001/book-resources?apiKey=img&file=Images%2Fa.jpg\"><a kavita-page=\"3\" kavita-part=\"n1\" href=\"javascript:void(0)\">Note</a></div>""#,
                            ),
                            "/api/Book/1001/chapters" => (
                                "200 OK",
                                "",
                                r#"[{"title":"Partie 1","page":1,"part":"","children":[{"title":" Chapitre 1 ","page":2,"part":"c1","children":[]}]},{"title":"Fin","page":9,"part":null}]"#,
                            ),
                            _ => ("404 Not Found", "", ""),
                        }
                    };
                    let resp = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len());
                    let _ = s.write_all(resp.as_bytes()).await;
                });
            }
        });
        (base, log)
    }

    #[tokio::test]
    async fn bibliotheques_series_et_pagination() {
        let (base, log) = faux_kavita().await;
        let k = Kavita::new(Client::new(), &format!("{base}/home"), "cle-ok").unwrap();
        let libs = k.libraries().await.unwrap();
        assert_eq!(libs.iter().map(|l| (l.id, l.kind)).collect::<Vec<_>>(), [(1, "manga"), (2, "book"), (3, "comic")]);
        let page = k.library_series(1, 2, 60).await.unwrap();
        assert_eq!(page.total, 57, "le total vient de l'en-tête Pagination");
        assert_eq!(page.items.len(), 2);
        assert_eq!((page.items[0].format, page.items[1].format), ("images", "epub"));
        assert_eq!(page.items[0].cover, format!("{base}/api/image/series-cover?seriesId=11&apiKey=cle-ok"));
        // La demande : la bonne page, filtrée sur la bibliothèque, triée par nom, avec la clé en en-tête.
        let log = log.lock().unwrap();
        let (method, target, key, body) = log.iter().find(|r| r.1.starts_with("/api/Series/all-v2")).unwrap();
        assert_eq!((method.as_str(), key.as_str()), ("POST", "cle-ok"));
        assert!(target.contains("PageNumber=2") && target.contains("PageSize=60"), "{target}");
        let body: Value = serde_json::from_str(body).unwrap();
        assert_eq!(body["statements"][0], json!({ "comparison": 5, "field": 19, "value": "1" }));
        assert_eq!(body["sortOptions"]["sortField"], 1);
    }

    #[tokio::test]
    async fn cle_refusee() {
        let (base, _) = faux_kavita().await;
        let k = Kavita::new(Client::new(), &base, "mauvaise").unwrap();
        assert!(k.libraries().await.is_err());
    }

    #[tokio::test]
    async fn detail_d_une_serie() {
        let (base, _) = faux_kavita().await;
        let k = Kavita::new(Client::new(), &base, "cle-ok").unwrap();
        let d = k.series_detail(11).await.unwrap();
        assert_eq!(d.summary.as_deref(), Some("Une histoire."));
        assert_eq!(d.library_kind, "manga");
        // Le volume « hors volume » de Kavita n'est pas un vrai volume : ses chapitres sont listés à part.
        assert_eq!(d.volumes.len(), 1);
        assert_eq!(d.volumes[0].chapters[0].number, "1");
        assert_eq!((d.chapters[0].id, d.chapters[0].title.as_str()), (1009, "La fin"));
        // Numéro réservé des hors-séries : pas affiché.
        assert_eq!((d.specials[0].number.as_str(), d.specials[0].title.as_str(), d.specials[0].special), ("", "Bonus", true));
        assert_eq!(d.continue_chapter, Some(1009));
    }

    #[tokio::test]
    async fn chapitre_et_progression() {
        let (base, log) = faux_kavita().await;
        let k = Kavita::new(Client::new(), &base, "cle-ok").unwrap();
        let c = k.chapter_info(1001).await.unwrap();
        // Chapitre déjà fini (20 pages lues sur 20) : relu depuis le début.
        assert_eq!(c.resume_page, 0);
        assert_eq!((c.format, c.next_chapter, c.prev_chapter), ("pdf", 1009, -1));
        // Sous le nom de la série : le titre du chapitre, puis son volume et son numéro.
        assert_eq!(c.title, "Le départ — Volume 1 Chapitre 1");
        // Page 4 en largeur ; une page hors du chapitre est ignorée.
        assert_eq!(c.wide.len(), 20);
        assert_eq!(c.wide.iter().enumerate().filter(|(_, w)| **w).map(|(i, _)| i).collect::<Vec<_>>(), [3]);
        assert!(log.lock().unwrap().iter().any(|r| r.1.starts_with("/api/Reader/chapter-info") && r.1.contains("includeDimensions=true")));
        // PDF : Kavita en sert les pages en images.
        assert_eq!(c.page_url, format!("{base}/api/Reader/image?chapterId=1001&page={{page}}&extractPdf=true&apiKey=cle-ok"));
        k.save_progress(&Progress { chapter_id: 1001, volume_id: 101, series_id: 11, library_id: 1, page: 7 }).await.unwrap();
        let log = log.lock().unwrap();
        let (_, _, _, body) = log.iter().find(|r| r.1 == "/api/Reader/progress").unwrap();
        let body: Value = serde_json::from_str(body).unwrap();
        assert_eq!(body, json!({ "volumeId": 101, "chapterId": 1001, "pageNum": 7, "seriesId": 11, "libraryId": 1 }));
    }

    #[tokio::test]
    async fn page_et_sommaire_d_un_livre() {
        let (base, log) = faux_kavita().await;
        let k = Kavita::new(Client::new(), &base, "cle-ok").unwrap();
        let html = k.book_page(1001, 4).await.unwrap();
        // Les ressources pointent vers l'adresse de Kavita que connaît Maku, pas vers l'hôte interne.
        assert!(html.contains(&format!("src=\"{base}/api/book/1001/book-resources?apiKey=img&file=Images%2Fa.jpg\"")), "{html}");
        assert!(html.contains(&format!("url({base}/api/book/1001/book-resources?apiKey=img&file=f.png)")), "{html}");
        assert!(!html.contains("interne"), "{html}");
        assert!(html.contains("kavita-page=\"3\""));
        assert!(log.lock().unwrap().iter().any(|r| r.1 == "/api/Book/1001/book-page?page=4"));
        let toc = k.book_toc(1001).await.unwrap();
        assert_eq!(toc.len(), 2);
        assert_eq!((toc[0].title.as_str(), toc[0].page), ("Partie 1", 1));
        assert_eq!((toc[0].children[0].title.as_str(), toc[0].children[0].page, toc[0].children[0].part.as_str()), ("Chapitre 1", 2, "c1"));
        assert_eq!((toc[1].part.as_str(), toc[1].children.len()), ("", 0));
    }

    #[test]
    fn ressources_d_un_livre_en_adresses_absolues() {
        let base = "https://exemple.fr/kavita";
        let r = |s: &str| absolute_resources(s, base, 7);
        assert_eq!(r("<img src=\"//kavita:5000/api/book/7/book-resources?file=a.png\">"), format!("<img src=\"{base}/api/book/7/book-resources?file=a.png\">"));
        assert_eq!(r("url('http://autre/api/book/7/book-resources?file=b')"), format!("url('{base}/api/book/7/book-resources?file=b')"));
        assert_eq!(r("<img src=\"/api/book/7/book-resources?file=c\">"), format!("<img src=\"{base}/api/book/7/book-resources?file=c\">"));
        // srcset : chaque adresse.
        assert_eq!(
            r("srcset=\"//h/api/book/7/book-resources?file=1 1x, //h/api/book/7/book-resources?file=2 2x\""),
            format!("srcset=\"{base}/api/book/7/book-resources?file=1 1x, {base}/api/book/7/book-resources?file=2 2x\"")
        );
        // Un autre livre, ou du texte : inchangés.
        assert_eq!(r("//h/api/book/8/book-resources?file=a"), "//h/api/book/8/book-resources?file=a");
        assert_eq!(r("<p>Été</p>"), "<p>Été</p>");
    }

    #[test]
    fn nom_du_chapitre_sans_repeter_la_serie() {
        // Une BD par série : Kavita donne le nom de la série comme titre, et le nom du fichier d'un hors-série.
        let bd = json!({ "seriesName": "Tome 08 Les deux", "chapterTitle": "Tome 08 les deux", "isSpecial": true, "subtitle": "8_Les_deux" });
        assert_eq!(chapter_label(&bd), "");
        let manga = json!({ "seriesName": "La Lune", "chapterTitle": "", "isSpecial": false, "subtitle": "Volume 2 Chapitre 5" });
        assert_eq!(chapter_label(&manga), "Volume 2 Chapitre 5");
        let bonus = json!({ "seriesName": "La Lune", "chapterTitle": "Bonus", "isSpecial": true, "subtitle": "bonus_01" });
        assert_eq!(chapter_label(&bonus), "Bonus");
    }

    #[test]
    fn types_et_formats() {
        assert_eq!(library_kind(&json!(0)), "manga");
        assert_eq!(library_kind(&json!(1)), "comic");
        assert_eq!(library_kind(&json!(5)), "comic");
        assert_eq!(library_kind(&json!(2)), "book");
        assert_eq!(library_kind(&json!(4)), "book");
        assert_eq!(format(&json!(1)), "images");
        assert_eq!(format(&json!(3)), "epub");
        assert_eq!(format(&json!(4)), "pdf");
    }
}
