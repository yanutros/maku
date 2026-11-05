use anyhow::{anyhow, bail, Result};
use reqwest::cookie::{CookieStore, Jar};
use reqwest::{Client, Method, RequestBuilder, Response, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

const TMDB_IMAGES: &str = "https://image.tmdb.org/t/p";

/// Statut Seerr « demandé » (2) ; les autres : 3 en cours, 4 partiel, 5 disponible.
const STATUS_PENDING: u8 = 2;

/// Client Seerr. La session est ouverte avec le compte de l'utilisateur (jeton Plex, ou
/// identifiant Jellyfin dont seuls les cookies de session sont gardés) : les demandes sont
/// faites à son nom, avec ses quotas et ses droits.
pub struct Seerr {
    http: Client,
    cookies: Arc<Jar>,
    base: String,
    /// Pour rouvrir la session Plex quand elle expire ; sans lui (Jellyfin), il faut se reconnecter.
    plex_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawResult {
    id: u64,
    media_type: String,
    title: Option<String>,
    name: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    overview: Option<String>,
    vote_average: Option<f32>,
    media_info: Option<RawMediaInfo>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawMediaInfo {
    status: Option<u8>,
    seasons: Vec<RawSeasonStatus>,
    requests: Vec<RawRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSeasonStatus {
    season_number: u32,
    status: u8,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawRequest {
    /// 1 = en attente d'approbation, 2 = approuvée, 3 = refusée.
    status: u8,
    seasons: Vec<RawRequestSeason>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRequestSeason {
    season_number: u32,
}

#[derive(Deserialize)]
struct RawPage {
    results: Vec<RawResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTv {
    #[serde(default)]
    seasons: Vec<RawSeason>,
    media_info: Option<RawMediaInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSeason {
    season_number: u32,
    name: Option<String>,
    episode_count: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeerrMedia {
    pub tmdb_id: u64,
    pub media_type: String,
    pub title: String,
    pub year: Option<String>,
    pub overview: Option<String>,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    pub rating: Option<f32>,
    /// 0 = inconnu de Seerr, 2 demandé, 3 en cours, 4 partiel, 5 disponible.
    pub status: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeerrSeason {
    pub season_number: u32,
    pub name: String,
    pub episode_count: u32,
    pub status: u8,
}

impl Seerr {
    fn new(base: &str, plex_token: Option<String>, saved_cookies: Option<&str>) -> Result<Self> {
        let base = base.trim_end_matches('/').to_string();
        let cookies = Arc::new(Jar::default());
        if let Ok(url) = Url::parse(&base) {
            for pair in saved_cookies.unwrap_or_default().split("; ").filter(|p| p.contains('=')) {
                cookies.add_cookie_str(&format!("{pair}; Path=/"), &url);
            }
        }
        let http = Client::builder()
            .cookie_provider(cookies.clone())
            .timeout(Duration::from_secs(20))
            .redirect(crate::net::same_host())
            .build()?;
        Ok(Seerr { http, cookies, base, plex_token })
    }

    /// Session enregistrée (compte Jellyfin) si elle existe, sinon connexion avec le jeton Plex.
    pub async fn connect(base: &str, plex_token: Option<String>, saved_cookies: Option<&str>) -> Result<Self> {
        if saved_cookies.is_some() {
            return Self::new(base, plex_token, saved_cookies);
        }
        if plex_token.is_none() {
            bail!(crate::i18n::tr("seerr.plexRequired"));
        }
        let seerr = Self::new(base, plex_token, None)?;
        seerr.login().await?;
        Ok(seerr)
    }

    /// Connexion sans Plex (compte Jellyfin, ou compte local de Seerr) : le mot de passe sert une fois, la session est gardée
    /// (`saved_cookies`).
    pub async fn connect_jellyfin(base: &str, username: &str, password: &str) -> Result<Self> {
        let seerr = Self::new(base, None, None)?;
        let public = seerr.prepare().await?;
        // Seerr ne gère qu'un type de serveur : 2 Jellyfin, 3 Emby. Un Seerr prévu pour Plex
        // accepte encore ses comptes locaux (adresse e-mail et mot de passe).
        let (path, body) = match public["mediaServerType"].as_u64() {
            Some(2 | 3) => ("/auth/jellyfin", json!({ "username": username, "password": password })),
            _ => ("/auth/local", json!({ "email": username, "password": password })),
        };
        let resp = seerr.builder(Method::POST, path).json(&body).send().await?;
        if !resp.status().is_success() {
            bail!(crate::i18n::tr_with("seerr.loginRefused", &[("status", &resp.status().to_string())]));
        }
        Ok(seerr)
    }

    /// Cookies de session à conserver (« nom=valeur; … »).
    pub fn saved_cookies(&self) -> Option<String> {
        let url = Url::parse(&self.base).ok()?;
        self.cookies.cookies(&url).and_then(|h| h.to_str().ok().map(str::to_owned))
    }

    /// Avec la protection CSRF de Seerr, une première requête dépose le jeton à renvoyer.
    /// Renvoie les réglages publics de Seerr.
    async fn prepare(&self) -> Result<Value> {
        let resp = self
            .builder(Method::GET, "/settings/public")
            .send()
            .await
            .map_err(|e| anyhow!(crate::i18n::tr_with("seerr.unreachable", &[("error", &e.to_string())])))?;
        Ok(resp.json().await.unwrap_or_default())
    }

    async fn login(&self) -> Result<()> {
        let Some(token) = &self.plex_token else { bail!(crate::i18n::tr("seerr.sessionExpired")) };
        self.prepare().await?;
        let resp = self
            .builder(Method::POST, "/auth/plex")
            .json(&json!({ "authToken": token }))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!(crate::i18n::tr_with("seerr.loginRefused", &[("status", &resp.status().to_string())]));
        }
        Ok(())
    }

    /// Valeur du cookie XSRF-TOKEN, à renvoyer en en-tête sur les requêtes qui modifient.
    fn csrf_token(&self) -> Option<String> {
        let url = Url::parse(&self.base).ok()?;
        let header = self.cookies.cookies(&url)?;
        header
            .to_str()
            .ok()?
            .split("; ")
            .find_map(|c| c.strip_prefix("XSRF-TOKEN="))
            .map(str::to_owned)
    }

    fn builder(&self, method: Method, path: &str) -> RequestBuilder {
        let write = method != Method::GET;
        let rb = self
            .http
            .request(method, format!("{}/api/v1{}", self.base, path))
            .header("Accept", "application/json");
        match self.csrf_token().filter(|_| write) {
            Some(token) => rb.header("X-XSRF-TOKEN", urlencoding::decode(&token).map(|t| t.into_owned()).unwrap_or(token)),
            None => rb,
        }
    }

    /// Envoie une requête et rouvre la session une fois si elle a expiré.
    async fn request(&self, method: Method, path: &str, body: Option<&Value>) -> Result<Response> {
        let mut relogged = false;
        loop {
            let mut rb = self.builder(method.clone(), path);
            if let Some(b) = body {
                rb = rb.json(b);
            }
            let resp = rb.send().await?;
            let status = resp.status();
            if (status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN) && !relogged {
                relogged = true;
                self.login().await?;
                continue;
            }
            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                let message = serde_json::from_str::<Value>(&text)
                    .ok()
                    .and_then(|v| v["message"].as_str().map(str::to_owned))
                    .unwrap_or(text);
                bail!(crate::i18n::tr_with("seerr.replied", &[("status", &status.to_string()), ("message", &message)]));
            }
            return Ok(resp);
        }
    }

    fn to_media(r: RawResult) -> SeerrMedia {
        let date = r.release_date.or(r.first_air_date).filter(|d| d.len() >= 4);
        SeerrMedia {
            tmdb_id: r.id,
            title: r.title.or(r.name).unwrap_or_default(),
            year: date.map(|d| d[..4].to_string()),
            overview: r.overview.filter(|o| !o.is_empty()),
            poster: r.poster_path.map(|p| format!("{TMDB_IMAGES}/w342{p}")),
            backdrop: r.backdrop_path.map(|p| format!("{TMDB_IMAGES}/w1280{p}")),
            rating: r.vote_average,
            status: r.media_info.and_then(|i| i.status).unwrap_or(0),
            media_type: r.media_type,
        }
    }

    async fn media_list(&self, path: &str) -> Result<Vec<SeerrMedia>> {
        let page: RawPage = self.request(Method::GET, path, None).await?.json().await?;
        Ok(page
            .results
            .into_iter()
            .filter(|r| r.media_type == "movie" || r.media_type == "tv")
            .map(Self::to_media)
            .collect())
    }

    pub async fn search(&self, query: &str, page: u32) -> Result<Vec<SeerrMedia>> {
        // Seerr exige des espaces encodés en %20 (et non « + »).
        self.media_list(&format!(
            "/search?query={}&page={page}&language={}",
            urlencoding::encode(query),
            crate::i18n::language()
        ))
        .await
    }

    pub async fn trending(&self, page: u32) -> Result<Vec<SeerrMedia>> {
        self.media_list(&format!("/discover/trending?page={page}&language={}", crate::i18n::language())).await
    }

    pub async fn tv_seasons(&self, tmdb_id: u64) -> Result<Vec<SeerrSeason>> {
        let tv: RawTv = self
            .request(Method::GET, &format!("/tv/{tmdb_id}?language={}", crate::i18n::language()), None)
            .await?
            .json()
            .await?;
        let info = tv.media_info.unwrap_or_default();
        let season_status = |n: u32| -> u8 {
            if let Some(s) = info.seasons.iter().find(|s| s.season_number == n) {
                return s.status;
            }
            let requested = info
                .requests
                .iter()
                .filter(|r| r.status == 1 || r.status == 2)
                .any(|r| r.seasons.iter().any(|s| s.season_number == n));
            if requested {
                STATUS_PENDING
            } else {
                0
            }
        };
        Ok(tv
            .seasons
            .into_iter()
            .map(|s| SeerrSeason {
                status: season_status(s.season_number),
                name: s.name.unwrap_or_else(|| crate::i18n::tr_with("seerr.season", &[("number", &s.season_number.to_string())])),
                episode_count: s.episode_count.unwrap_or(0),
                season_number: s.season_number,
            })
            .collect())
    }

    pub async fn create_request(&self, media_type: &str, tmdb_id: u64, seasons: Option<Vec<u32>>) -> Result<()> {
        let mut body = json!({ "mediaType": media_type, "mediaId": tmdb_id });
        if media_type == "tv" {
            let seasons = seasons.filter(|s| !s.is_empty()).ok_or_else(|| anyhow!(crate::i18n::tr("seerr.chooseSeason")))?;
            body["seasons"] = json!(seasons);
        }
        self.request(Method::POST, "/request", Some(&body)).await?;
        Ok(())
    }
}

// ---------- Fiche détaillée et rangées de découverte ----------

#[derive(Serialize)]
pub struct Person {
    pub name: String,
    /// Personnage (casting) ou fonction (équipe).
    pub role: Option<String>,
    pub photo: Option<String>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Ratings {
    pub rt_critics: Option<u64>,
    pub rt_audience: Option<u64>,
    pub rt_url: Option<String>,
    pub imdb: Option<f64>,
    pub imdb_url: Option<String>,
    pub tmdb: Option<f64>,
}

#[derive(Serialize)]
pub struct Provider {
    pub name: String,
    pub logo: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeerrDetail {
    #[serde(flatten)]
    pub media: SeerrMedia,
    pub tagline: Option<String>,
    pub runtime: Option<u64>,
    pub certification: Option<String>,
    pub genres: Vec<String>,
    pub release_date: Option<String>,
    pub original_title: Option<String>,
    /// Statut TMDB (« Released », « Ended »…), traduit côté interface.
    pub production_status: Option<String>,
    pub original_language: Option<String>,
    pub countries: Vec<String>,
    /// Studios (films) ou chaînes (séries).
    pub studios: Vec<String>,
    pub budget: Option<u64>,
    pub revenue: Option<u64>,
    pub seasons: Option<u64>,
    pub episodes: Option<u64>,
    pub crew: Vec<Person>,
    pub cast: Vec<Person>,
    pub trailer: Option<String>,
    pub ratings: Ratings,
    pub collection: Option<String>,
    pub providers: Vec<Provider>,
    pub keywords: Vec<String>,
    /// Élément Plex quand le titre est disponible, pour le lire directement.
    pub plex_rating_key: Option<String>,
    pub tmdb_url: String,
}

/// Rangées de la page Découvrir, dans l'ordre de Seerr.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum Slider {
    Trending,
    PopularMovies,
    UpcomingMovies,
    PopularTv,
    UpcomingTv,
}

fn image(path: Option<&str>, size: &str) -> Option<String> {
    path.filter(|p| !p.is_empty()).map(|p| format!("{TMDB_IMAGES}/{size}{p}"))
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x[key].as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

/// Certification française, sinon américaine (films : `releases`, séries : `contentRatings`).
fn certification(d: &Value) -> Option<String> {
    let pick = |country: &str| -> Option<String> {
        if let Some(list) = d["releases"]["results"].as_array() {
            let r = list.iter().find(|r| r["iso_3166_1"] == country)?;
            return r["release_dates"]
                .as_array()?
                .iter()
                .filter_map(|x| x["certification"].as_str())
                .find(|c| !c.is_empty())
                .map(str::to_owned);
        }
        d["contentRatings"]["results"]
            .as_array()?
            .iter()
            .find(|r| r["iso_3166_1"] == country)?["rating"]
            .as_str()
            .filter(|c| !c.is_empty())
            .map(str::to_owned)
    };
    pick("FR").or_else(|| pick("US"))
}

/// Ajoute le type aux résultats qui ne le précisent pas (listes de films ou de séries).
fn with_kind(results: &Value, kind: Option<&str>) -> Vec<RawResult> {
    results
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let mut r = r.clone();
            if r["mediaType"].is_null() {
                r["mediaType"] = json!(kind?);
            }
            serde_json::from_value::<RawResult>(r).ok()
        })
        .filter(|r| r.media_type == "movie" || r.media_type == "tv")
        .collect()
}

impl Seerr {
    async fn json(&self, path: &str) -> Result<Value> {
        Ok(self.request(Method::GET, path, None).await?.json().await?)
    }

    /// Fiche complète d'un film ou d'une série, avec les notes Rotten Tomatoes / IMDb.
    pub async fn details(&self, media_type: &str, tmdb_id: u64) -> Result<SeerrDetail> {
        let kind = if media_type == "tv" { "tv" } else { "movie" };
        let detail_path = format!("/{kind}/{tmdb_id}?language={}", crate::i18n::language());
        let ratings_path = if kind == "tv" {
            format!("/tv/{tmdb_id}/ratings")
        } else {
            format!("/movie/{tmdb_id}/ratingscombined")
        };
        let (d, ratings) = tokio::join!(self.json(&detail_path), self.json(&ratings_path));
        let d = d?;
        let ratings = ratings.unwrap_or_default();
        // Notes : les films renvoient { rt, imdb }, les séries directement les données RT.
        let rt = if ratings["rt"].is_object() { &ratings["rt"] } else { &ratings };

        let movie = kind == "movie";
        let crew: Vec<Person> = if movie {
            const JOBS: &[(&str, &str)] = &[
                ("Director", "seerr.jobs.director"),
                ("Screenplay", "seerr.jobs.screenplay"),
                ("Writer", "seerr.jobs.screenplay"),
                ("Novel", "seerr.jobs.novel"),
                ("Producer", "seerr.jobs.producer"),
            ];
            let all = d["credits"]["crew"].as_array().cloned().unwrap_or_default();
            JOBS.iter()
                .flat_map(|(job, label)| {
                    all.iter().filter(move |c| c["job"] == *job).map(move |c| Person {
                        name: c["name"].as_str().unwrap_or_default().to_owned(),
                        role: Some(crate::i18n::tr(label)),
                        photo: None,
                    })
                })
                .take(6)
                .collect()
        } else {
            d["createdBy"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| Person {
                    name: c["name"].as_str().unwrap_or_default().to_owned(),
                    role: Some(crate::i18n::tr("seerr.creator")),
                    photo: None,
                })
                .collect()
        };
        let cast = d["credits"]["cast"]
            .as_array()
            .into_iter()
            .flatten()
            .take(20)
            .map(|c| Person {
                name: c["name"].as_str().unwrap_or_default().to_owned(),
                role: c["character"].as_str().filter(|s| !s.is_empty()).map(str::to_owned),
                photo: image(c["profilePath"].as_str(), "w185"),
            })
            .collect();
        let trailer = d["relatedVideos"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|v| v["type"] == "Trailer" && v["site"] == "YouTube")
            .and_then(|v| v["url"].as_str().map(str::to_owned));
        let providers = d["watchProviders"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|p| p["iso_3166_1"] == "FR")
            .and_then(|p| p["flatrate"].as_array())
            .map(|list| {
                list.iter()
                    .filter_map(|p| {
                        Some(Provider {
                            name: p["name"].as_str()?.to_owned(),
                            logo: image(p["logoPath"].as_str(), "w92")?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let date = d["releaseDate"].as_str().or(d["firstAirDate"].as_str()).filter(|s| !s.is_empty());
        let media = SeerrMedia {
            tmdb_id,
            media_type: kind.to_owned(),
            title: d["title"].as_str().or(d["name"].as_str()).unwrap_or_default().to_owned(),
            year: date.filter(|s| s.len() >= 4).map(|s| s[..4].to_owned()),
            overview: d["overview"].as_str().filter(|s| !s.is_empty()).map(str::to_owned),
            poster: image(d["posterPath"].as_str(), "w500"),
            backdrop: image(d["backdropPath"].as_str(), "w1280"),
            rating: d["voteAverage"].as_f64().map(|v| v as f32),
            status: d["mediaInfo"]["status"].as_u64().unwrap_or(0) as u8,
        };
        let rating_key = &d["mediaInfo"]["ratingKey"];
        Ok(SeerrDetail {
            tagline: d["tagline"].as_str().filter(|s| !s.is_empty()).map(str::to_owned),
            runtime: d["runtime"].as_u64().or_else(|| d["episodeRunTime"][0].as_u64()).filter(|r| *r > 0),
            certification: certification(&d),
            genres: strings(&d["genres"], "name"),
            release_date: date.map(str::to_owned),
            original_title: d["originalTitle"].as_str().or(d["originalName"].as_str()).map(str::to_owned),
            production_status: d["status"].as_str().map(str::to_owned),
            original_language: d["originalLanguage"].as_str().map(str::to_owned),
            countries: strings(&d["productionCountries"], "name"),
            studios: if movie { strings(&d["productionCompanies"], "name") } else { strings(&d["networks"], "name") },
            budget: d["budget"].as_u64().filter(|b| *b > 0),
            revenue: d["revenue"].as_u64().filter(|b| *b > 0),
            seasons: d["numberOfSeasons"].as_u64(),
            episodes: d["numberOfEpisodes"].as_u64(),
            crew,
            cast,
            trailer,
            ratings: Ratings {
                rt_critics: rt["criticsScore"].as_u64(),
                rt_audience: rt["audienceScore"].as_u64(),
                rt_url: rt["url"].as_str().map(str::to_owned),
                imdb: ratings["imdb"]["criticsScore"].as_f64(),
                imdb_url: ratings["imdb"]["url"].as_str().map(str::to_owned),
                tmdb: d["voteAverage"].as_f64().filter(|v| *v > 0.0),
            },
            collection: d["collection"]["name"].as_str().map(str::to_owned),
            providers,
            keywords: strings(&d["keywords"], "name").into_iter().take(16).collect(),
            plex_rating_key: rating_key.as_str().map(str::to_owned).or_else(|| rating_key.as_u64().map(|k| k.to_string())),
            tmdb_url: format!("https://www.themoviedb.org/{kind}/{tmdb_id}"),
            media,
        })
    }

    /// `relation` : « recommendations » ou « similar ».
    pub async fn related(&self, media_type: &str, tmdb_id: u64, relation: &str) -> Result<Vec<SeerrMedia>> {
        let kind = if media_type == "tv" { "tv" } else { "movie" };
        let relation = if relation == "similar" { "similar" } else { "recommendations" };
        let raw = self.json(&format!("/{kind}/{tmdb_id}/{relation}?page=1&language={}", crate::i18n::language())).await?;
        Ok(with_kind(&raw["results"], Some(kind)).into_iter().map(Self::to_media).collect())
    }

    pub async fn slider(&self, slider: Slider, page: u32) -> Result<Vec<SeerrMedia>> {
        let (path, kind) = match slider {
            Slider::Trending => ("/discover/trending", None),
            Slider::PopularMovies => ("/discover/movies", Some("movie")),
            Slider::UpcomingMovies => ("/discover/movies/upcoming", Some("movie")),
            Slider::PopularTv => ("/discover/tv", Some("tv")),
            Slider::UpcomingTv => ("/discover/tv/upcoming", Some("tv")),
        };
        let raw = self.json(&format!("{path}?page={page}&language={}", crate::i18n::language())).await?;
        Ok(with_kind(&raw["results"], kind).into_iter().map(Self::to_media).collect())
    }

    /// Carte d'un titre connu seulement par son identifiant TMDB.
    async fn card(&self, kind: &str, tmdb_id: u64) -> Option<SeerrMedia> {
        let mut d = self.json(&format!("/{kind}/{tmdb_id}?language={}", crate::i18n::language())).await.ok()?;
        d["mediaType"] = json!(kind);
        serde_json::from_value::<RawResult>(d).ok().map(Self::to_media)
    }

    async fn cards(&self, ids: Vec<(String, u64)>) -> Vec<SeerrMedia> {
        futures::future::join_all(ids.iter().map(|(k, id)| self.card(k, *id)))
            .await
            .into_iter()
            .flatten()
            .collect()
    }

    /// Dernières demandes, comme sur l'accueil de Seerr.
    pub async fn recent_requests(&self) -> Result<Vec<SeerrMedia>> {
        let raw = self.json("/request?take=12&skip=0&sort=added&filter=all").await?;
        let mut seen = std::collections::HashSet::new();
        let ids = raw["results"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| {
                let kind = r["media"]["mediaType"].as_str()?.to_owned();
                let id = r["media"]["tmdbId"].as_u64()?;
                seen.insert((kind.clone(), id)).then_some((kind, id))
            })
            .collect();
        Ok(self.cards(ids).await)
    }

    /// Liste de suivi Plex de l'utilisateur, telle que Seerr la voit.
    pub async fn watchlist(&self) -> Result<Vec<SeerrMedia>> {
        let raw = self.json("/discover/watchlist?page=1").await?;
        let ids = raw["results"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| Some((r["mediaType"].as_str()?.to_owned(), r["tmdbId"].as_u64()?)))
            .take(20)
            .collect();
        Ok(self.cards(ids).await)
    }
}
