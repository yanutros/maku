//! Statistiques personnelles depuis Tracearr.
//!
//! Tracearr ouvre ses sessions lui-même avec un code PIN Plex (son serveur
//! interroge plex.tv) : on lance la demande, l'utilisateur valide dans son
//! navigateur, puis la session tient dans des cookies valables 30 jours,
//! conservés dans la configuration.

use crate::plex::PlexServer;
use anyhow::{anyhow, bail, Result};
use reqwest::cookie::{CookieStore, Jar};
use reqwest::{Client, Method, RequestBuilder, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DAY_MS: u64 = 24 * 60 * 60 * 1000;

pub struct Tracearr {
    http: Client,
    cookies: Arc<Jar>,
    base: Url,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginStart {
    pub handle: String,
    pub auth_url: String,
}

/// État d'une connexion en attente de validation dans le navigateur.
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LoginState {
    Waiting,
    Done,
    Expired,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub watch_ms: u64,
    pub listen_ms: u64,
    pub plays: u64,
    pub titles: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    pub title: String,
    pub subtitle: Option<String>,
    pub poster: Option<String>,
    pub progress_ms: u64,
    pub duration_ms: u64,
    pub paused: bool,
    pub player: Option<String>,
    pub local: bool,
    pub decision: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopTitle {
    pub rank: u64,
    pub title: String,
    pub year: Option<u64>,
    pub kind: String,
    pub poster: Option<String>,
    /// Élément Plex correspondant, pour ouvrir sa fiche.
    pub rating_key: Option<String>,
    pub plays: u64,
    pub watch_ms: u64,
    pub episodes: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub username: Option<String>,
    pub now_playing: Vec<NowPlaying>,
    /// 30 derniers jours, et les 30 précédents pour comparer.
    pub month: Totals,
    pub previous: Totals,
    pub top_titles: Vec<TopTitle>,
    /// Statistiques de tous les temps (`/stats/me`), transmises telles quelles.
    pub all_time: Value,
    /// Images Plex du plus gros marathon et de la première lecture, pour illustrer leurs cartes.
    pub binge_art: Option<Art>,
    pub first_play_art: Option<Art>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Art {
    pub rating_key: String,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
}

/// Cherche un titre dans la bibliothèque Plex pour en récupérer les images.
async fn find_art(plex: Option<&PlexServer>, title: Option<&str>) -> Option<Art> {
    let (plex, title) = (plex?, title?);
    let found = plex.search(title).await.ok()?;
    let wanted = title.to_lowercase();
    let item = found
        .iter()
        .filter(|i| matches!(i.kind.as_str(), "movie" | "show"))
        .find(|i| i.title.to_lowercase() == wanted)
        .or_else(|| found.iter().find(|i| matches!(i.kind.as_str(), "movie" | "show")))?;
    Some(Art { rating_key: item.rating_key.clone(), poster: item.poster.clone(), backdrop: item.backdrop.clone() })
}

impl Tracearr {
    pub fn new(base: &str, saved_cookies: Option<&str>, relay: Option<String>) -> Result<Self> {
        let base = Url::parse(base.trim_end_matches('/')).map_err(|_| anyhow!(crate::i18n::tr_with("tracearr.invalidUrl", &[("url", base)])))?;
        let cookies = Arc::new(Jar::default());
        for pair in saved_cookies.unwrap_or_default().split("; ").filter(|p| p.contains('=')) {
            cookies.add_cookie_str(&format!("{pair}; Path=/"), &base);
        }
        let http = Client::builder()
            .cookie_provider(cookies.clone())
            .default_headers(crate::maku_server::relay_headers(relay))
            .timeout(Duration::from_secs(20))
            .redirect(crate::net::same_host())
            .build()?;
        Ok(Tracearr { http, cookies, base })
    }

    /// Cookies de session à conserver (« nom=valeur; … »).
    pub fn saved_cookies(&self) -> Option<String> {
        self.cookies.cookies(&self.base).and_then(|h| h.to_str().ok().map(str::to_owned))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base.as_str().trim_end_matches('/'), path)
    }

    fn builder(&self, method: Method, path: &str) -> RequestBuilder {
        // Tracearr vérifie l'origine des requêtes qui ouvrent une session.
        let origin = self.base.origin().ascii_serialization();
        self.http
            .request(method, self.url(&format!("/api/v1{path}")))
            .header("Accept", "application/json")
            .header("Origin", origin)
    }

    pub async fn start_login(&self) -> Result<LoginStart> {
        let resp = self
            .builder(Method::POST, "/auth/plex/initiate")
            .json(&json!({}))
            .send()
            .await
            .map_err(|e| anyhow!(crate::i18n::tr_with("tracearr.unreachable", &[("error", &e.to_string())])))?;
        if !resp.status().is_success() {
            bail!(crate::i18n::tr_with("tracearr.loginRefused", &[("status", &resp.status().to_string())]));
        }
        Ok(resp.json().await?)
    }

    pub async fn check_login(&self, handle: &str) -> Result<LoginState> {
        let resp = self
            .builder(Method::POST, "/auth/plex/check-pin")
            .json(&json!({ "handle": handle }))
            .send()
            .await?;
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or_default();
        if !status.is_success() {
            let message = body["message"].as_str().map(str::to_owned).unwrap_or_else(|| crate::i18n::tr("tracearr.refused"));
            bail!(crate::i18n::tr_with("tracearr.error", &[("message", &message)]));
        }
        if body["authorized"].as_bool() == Some(true) {
            if body["needsServerSelection"].as_bool() == Some(true) {
                bail!(crate::i18n::tr("tracearr.notSetUp"));
            }
            return Ok(LoginState::Done);
        }
        Ok(if body["expired"].as_bool() == Some(true) { LoginState::Expired } else { LoginState::Waiting })
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let resp = self.builder(Method::GET, path).send().await?;
        match resp.status() {
            StatusCode::UNAUTHORIZED => bail!("SESSION_EXPIRED"),
            s if !s.is_success() => bail!(crate::i18n::tr_with("tracearr.replied", &[("status", &s.to_string()), ("path", path)])),
            _ => Ok(resp.json().await?),
        }
    }

    pub async fn logout(&self) {
        let _ = self.builder(Method::POST, "/auth/sign-out").json(&json!({})).send().await;
    }

    /// Tableau de bord personnel : lecture en cours, 30 derniers jours, top titres, tous les temps.
    pub async fn dashboard(&self, plex: Option<&PlexServer>, timezone: &str) -> Result<Dashboard> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
        let tz = urlencoding::encode(timezone);
        let window = |from: u64, to: u64| {
            format!("/sessions/history/aggregates?self=true&startDate={}&endDate={}", iso(from), iso(to))
        };
        let month_path = window(now - 30 * DAY_MS, now);
        let previous_path = window(now - 60 * DAY_MS, now - 30 * DAY_MS);
        let titles_path = format!("/stats/me/titles?period=month&timezone={tz}");
        let stats_path = format!("/stats/me?timezone={tz}");
        let (me, active, month, previous, titles, stats) = tokio::join!(
            self.get("/auth/me"),
            self.get("/sessions/active?self=true"),
            self.get(&month_path),
            self.get(&previous_path),
            self.get(&titles_path),
            self.get(&stats_path),
        );
        let me = me?;
        let image = |path: Option<&str>| -> Option<String> {
            let path = path?;
            // Tracearr renvoie ses propres adresses de proxy : on remonte au chemin Plex d'origine.
            let plex_path = match path.find("url=") {
                Some(i) => urlencoding::decode(path[i + 4..].split('&').next()?).ok()?.into_owned(),
                None => path.to_owned(),
            };
            plex.filter(|_| plex_path.starts_with('/')).map(|p| p.image_url(&plex_path, 300, 450))
        };
        let totals = |v: Result<Value>| -> Totals {
            let v = v.unwrap_or_default();
            Totals {
                watch_ms: v["totalWatchTimeMs"].as_u64().unwrap_or(0),
                listen_ms: v["listenTimeMs"].as_u64().unwrap_or(0),
                plays: v["playCount"].as_u64().unwrap_or(0),
                titles: v["uniqueContent"].as_u64().unwrap_or(0),
            }
        };

        let now_playing = active
            .unwrap_or_default()
            .get("data")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .map(|s| {
                        let show = s["grandparentTitle"].as_str();
                        let artist = s["artistName"].as_str();
                        let title = s["mediaTitle"].as_str().unwrap_or_default().to_owned();
                        let (title, subtitle) = match (show, artist) {
                            (Some(show), _) => (
                                show.to_owned(),
                                Some(crate::i18n::tr_with(
                                    "tracearr.episodeLine",
                                    &[
                                        ("season", &s["seasonNumber"].as_u64().unwrap_or(0).to_string()),
                                        ("episode", &s["episodeNumber"].as_u64().unwrap_or(0).to_string()),
                                        ("title", &title),
                                    ],
                                )),
                            ),
                            (None, Some(artist)) => (
                                title,
                                Some(match s["albumName"].as_str() {
                                    Some(album) => format!("{artist} — {album}"),
                                    None => artist.to_owned(),
                                }),
                            ),
                            _ => (title, s["year"].as_u64().map(|y| y.to_string())),
                        };
                        NowPlaying {
                            title,
                            subtitle,
                            poster: image(s["thumbPath"].as_str()),
                            progress_ms: s["progressMs"].as_u64().unwrap_or(0),
                            duration_ms: s["totalDurationMs"].as_u64().unwrap_or(0),
                            paused: s["state"] == "paused",
                            player: s["playerName"].as_str().or(s["product"].as_str()).map(str::to_owned),
                            local: s["isLocal"].as_bool().unwrap_or(false),
                            decision: s["videoDecision"].as_str().or(s["audioDecision"].as_str()).map(str::to_owned),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let top_titles = titles
            .unwrap_or_default()
            .get("data")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .map(|t| {
                        let poster_path = t["posterUrl"].as_str();
                        TopTitle {
                            rank: t["rank"].as_u64().unwrap_or(0),
                            title: t["title"].as_str().unwrap_or_default().to_owned(),
                            year: t["year"].as_u64(),
                            kind: t["mediaType"].as_str().unwrap_or_default().to_owned(),
                            poster: image(poster_path),
                            rating_key: poster_path.and_then(rating_key_of),
                            plays: t["plays"].as_u64().unwrap_or(0),
                            watch_ms: t["watchTimeMs"].as_u64().unwrap_or(0),
                            episodes: t["episodes"].as_u64(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let all_time = stats.unwrap_or_default();
        let first = &all_time["firstPlay"];
        let (binge_art, first_play_art) = tokio::join!(
            find_art(plex, all_time["biggestBinge"]["showTitle"].as_str()),
            find_art(plex, first["grandparentTitle"].as_str().or(first["mediaTitle"].as_str())),
        );

        Ok(Dashboard {
            username: me["user"]["username"].as_str().or(me["username"].as_str()).map(str::to_owned),
            now_playing,
            month: totals(month),
            previous: totals(previous),
            top_titles,
            all_time,
            binge_art,
            first_play_art,
        })
    }
}

/// « /library/metadata/27205/thumb/… » (éventuellement encodé dans une adresse de proxy) → « 27205 ».
fn rating_key_of(path: &str) -> Option<String> {
    let decoded = urlencoding::decode(path).ok()?;
    let rest = decoded.split("/library/metadata/").nth(1)?;
    let key: String = rest.chars().take_while(char::is_ascii_digit).collect();
    (!key.is_empty()).then_some(key)
}

/// Horodatage en millisecondes → ISO 8601 UTC (« 2026-10-31T01:02:03.000Z »).
fn iso(ms: u64) -> String {
    let secs = ms / 1000;
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Algorithme « civil from days » (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        ms % 1000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates() {
        assert_eq!(iso(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso(1_793_407_044_192), "2026-10-31T00:37:24.192Z");
    }

    #[test]
    fn rating_keys() {
        assert_eq!(
            rating_key_of("/api/v1/images/proxy?server=x&url=%2Flibrary%2Fmetadata%2F27205%2Fthumb%2F1&width=360").as_deref(),
            Some("27205")
        );
        assert_eq!(rating_key_of("/library/metadata/12/thumb").as_deref(), Some("12"));
        assert_eq!(rating_key_of("https://image.tmdb.org/x.jpg"), None);
    }
}
