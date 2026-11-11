//! TMDB metadata for the Movies page (optional, needs the user's own API key) and the built-in
//! curated shelf. Pure parsing lives here so it can be tested against inline fixtures; the
//! network helpers never put the key into an error, a log line or a toast.

use anyhow::{Result, bail};
use serde_json::Value;
use url::Url;

pub const IMAGE_BASE: &str = "https://image.tmdb.org/t/p";
const API: &str = "https://api.themoviedb.org/3";

/// One film or series, from the curated shelf or from TMDB.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Title {
    pub tmdb: u64,
    pub tv: bool,
    pub title: String,
    pub overview: String,
    pub year: String,
    pub rating: Option<f32>,
    pub runtime: Option<u32>,
    pub genres: Vec<String>,
    pub poster: String,
    pub backdrop: String,
    /// Where a curated TV entry starts (the CLI pool pins an episode).
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Season {
    pub number: u32,
    pub name: String,
    pub episodes: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Episode {
    pub number: u32,
    pub name: String,
    pub overview: String,
    pub runtime: Option<u32>,
}

/// The CLI's verified pool of public TMDB titles (`tools/cinecat`): label, tmdb id, season, episode.
pub const POOL: &[(&str, u64, Option<u32>, Option<u32>)] = &[
    ("Fight Club", 550, None, None),
    ("Inception", 27205, None, None),
    ("The Matrix", 603, None, None),
    ("Interstellar", 157336, None, None),
    ("Parasite", 496243, None, None),
    ("Avengers Endgame", 299534, None, None),
    ("Dune", 438631, None, None),
    ("Dune Part Two", 693134, None, None),
    ("Oppenheimer", 872585, None, None),
    ("Barbie", 346698, None, None),
    ("Spider-Man No Way Home", 634649, None, None),
    ("The Godfather", 238, None, None),
    ("Spirited Away", 129, None, None),
    ("Mad Max Fury Road", 76341, None, None),
    ("Everything Everywhere All At Once", 545611, None, None),
    ("Top Gun Maverick", 361743, None, None),
    ("Metropolis", 19, None, None),
    ("Nosferatu", 1091, None, None),
    ("The Terminator", 218, None, None),
    ("Jurassic Park", 329, None, None),
    ("Toy Story", 862, None, None),
    ("Home Alone", 771, None, None),
    ("Project Hail Mary", 687163, None, None),
    ("The Dark Knight", 155, None, None),
    ("Resident Evil", 1423191, None, None),
    ("Apocalypse Z", 1507696, None, None),
    ("Hope", 1058424, None, None),
    ("Spider-Man Brand New Day", 969681, None, None),
    ("The Odyssey", 1368337, None, None),
    ("Heart of the Beast", 1263337, None, None),
    ("A Story of Derry", 1813355, None, None),
    ("The Hotel New Hampshire", 11308, None, None),
    ("Coco", 354912, None, None),
    ("The Mongoose", 1294189, None, None),
    ("Verity", 1283515, None, None),
    ("Fall 2 Deadpoint", 1101412, None, None),
    ("La Bola Negra", 1422041, None, None),
    ("Breaking Bad S1E1", 1396, Some(1), Some(1)),
    ("Friends S1E1", 1668, Some(1), Some(1)),
];

pub fn curated() -> Vec<Title> {
    POOL.iter()
        .map(|(label, tmdb, season, episode)| Title {
            tmdb: *tmdb,
            tv: season.is_some(),
            title: label.to_string(),
            season: *season,
            episode: *episode,
            ..Default::default()
        })
        .collect()
}

pub fn image_url(path: &str, size: &str) -> String {
    if path.starts_with("https://") {
        // Wikimedia artwork arrives as a full URL.
        path.to_owned()
    } else if path.starts_with('/') {
        format!("{IMAGE_BASE}/{size}{path}")
    } else {
        String::new()
    }
}

fn str_of(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().trim().to_owned()
}

fn year_of(date: &str) -> String {
    date.split('-')
        .next()
        .filter(|y| y.len() == 4 && y.bytes().all(|b| b.is_ascii_digit()))
        .unwrap_or_default()
        .to_owned()
}

fn title_from(v: &Value, tv_hint: Option<bool>) -> Option<Title> {
    let tmdb = v["id"].as_u64()?;
    let tv = match v["media_type"].as_str() {
        Some("tv") => true,
        Some("movie") => false,
        Some(_) => return None,
        None => tv_hint.unwrap_or(v.get("name").is_some() && v.get("title").is_none()),
    };
    let name = if tv {
        str_of(v, "name")
    } else {
        str_of(v, "title")
    };
    if name.is_empty() {
        return None;
    }
    let date = if tv {
        str_of(v, "first_air_date")
    } else {
        str_of(v, "release_date")
    };
    let runtime = v["runtime"]
        .as_u64()
        .or_else(|| v["episode_run_time"].get(0).and_then(Value::as_u64))
        .filter(|m| *m > 0)
        .map(|m| m as u32);
    Some(Title {
        tmdb,
        tv,
        title: name,
        overview: str_of(v, "overview"),
        year: year_of(&date),
        rating: v["vote_average"]
            .as_f64()
            .filter(|r| *r > 0.0)
            .map(|r| r as f32),
        runtime,
        genres: v["genres"]
            .as_array()
            .map(|g| {
                g.iter()
                    .filter_map(|x| x["name"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        poster: str_of(v, "poster_path"),
        backdrop: str_of(v, "backdrop_path"),
        season: tv.then_some(1),
        episode: tv.then_some(1),
    })
}

/// A `results` list from trending, search or discover. People are skipped.
pub fn parse_list(v: &Value) -> Vec<Title> {
    v["results"]
        .as_array()
        .map(|r| r.iter().filter_map(|x| title_from(x, None)).collect())
        .unwrap_or_default()
}

pub fn parse_details(v: &Value, tv: bool) -> Option<Title> {
    title_from(v, Some(tv)).map(|mut t| {
        t.tv = tv;
        t
    })
}

pub fn parse_seasons(v: &Value) -> Vec<Season> {
    v["seasons"]
        .as_array()
        .map(|s| {
            s.iter()
                .filter_map(|x| {
                    let number = x["season_number"].as_u64()? as u32;
                    let episodes = x["episode_count"].as_u64().unwrap_or(0) as u32;
                    // Season 0 holds specials; the provider is keyed by regular episodes.
                    (number > 0 && episodes > 0).then(|| Season {
                        number,
                        name: str_of(x, "name"),
                        episodes,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_episodes(v: &Value) -> Vec<Episode> {
    v["episodes"]
        .as_array()
        .map(|e| {
            e.iter()
                .filter_map(|x| {
                    Some(Episode {
                        number: x["episode_number"].as_u64()? as u32,
                        name: str_of(x, "name"),
                        overview: str_of(x, "overview"),
                        runtime: x["runtime"].as_u64().filter(|m| *m > 0).map(|m| m as u32),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `1h 48m` / `42m`.
pub fn runtime_text(minutes: u32) -> String {
    if minutes >= 60 {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}

/// A one-line summary: `2010 · 2h 28m · 8.4 · Action, Science Fiction`.
pub fn meta_text(t: &Title) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(if t.tv { "Series".into() } else { "Film".into() });
    if !t.year.is_empty() {
        parts.push(t.year.clone());
    }
    if let Some(m) = t.runtime {
        parts.push(runtime_text(m));
    }
    if let Some(r) = t.rating {
        parts.push(format!("{r:.1} ★"));
    }
    if !t.genres.is_empty() {
        parts.push(
            t.genres
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    parts.join(" · ")
}

/// Which request a TMDB call is; the key is added by [`fetch`] and never leaves it.
#[derive(Clone, Debug)]
pub enum Query {
    Trending,
    Search(String),
    Details { tv: bool, id: u64 },
    Season { id: u64, season: u32 },
}

fn endpoint(q: &Query, key: &str) -> Result<Url> {
    let (path, extra): (String, Vec<(&str, String)>) = match q {
        Query::Trending => ("trending/all/week".into(), vec![]),
        Query::Search(text) => (
            "search/multi".into(),
            vec![("query", text.clone()), ("include_adult", "false".into())],
        ),
        Query::Details { tv, id } => (format!("{}/{id}", if *tv { "tv" } else { "movie" }), vec![]),
        Query::Season { id, season } => (format!("tv/{id}/season/{season}"), vec![]),
    };
    let mut params = vec![("api_key", key.to_owned())];
    params.extend(extra);
    Ok(Url::parse_with_params(&format!("{API}/{path}"), params)?)
}

/// Runs one TMDB request. Errors name the HTTP status only: never the URL, so never the key.
pub async fn fetch(client: &reqwest::Client, key: &str, q: &Query) -> Result<Value> {
    let url = endpoint(q, key.trim())?;
    let response = client.get(url).send().await.map_err(|e| {
        anyhow::anyhow!(
            "TMDB could not be reached ({})",
            if e.is_timeout() {
                "timed out"
            } else {
                "network error"
            }
        )
    })?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        bail!("TMDB rejected the API key. Check it in Settings → Alpha.");
    }
    if !status.is_success() {
        bail!("TMDB answered HTTP {}", status.as_u16());
    }
    response
        .json::<Value>()
        .await
        .map_err(|_| anyhow::anyhow!("TMDB sent an unreadable answer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn trending_keeps_movies_and_series_and_skips_people() {
        let v = json!({"results": [
            {"id": 1, "media_type": "movie", "title": "Heat", "release_date": "1995-12-15", "poster_path": "/p.jpg", "backdrop_path": "/b.jpg", "vote_average": 8.0, "overview": "Cops."},
            {"id": 2, "media_type": "tv", "name": "Severance", "first_air_date": "2022-02-18", "vote_average": 0.0},
            {"id": 3, "media_type": "person", "name": "Someone"},
            {"id": 4, "media_type": "movie", "title": ""}
        ]});
        let list = parse_list(&v);
        assert_eq!(list.len(), 2);
        assert_eq!(
            (list[0].tmdb, list[0].tv, list[0].year.as_str()),
            (1, false, "1995")
        );
        assert_eq!(list[0].poster, "/p.jpg");
        assert_eq!(list[0].rating, Some(8.0));
        assert!(list[1].tv && list[1].rating.is_none() && list[1].season == Some(1));
    }

    #[test]
    fn details_read_runtime_genres_and_series_runtime() {
        let m = parse_details(&json!({"id": 9, "title": "Up", "release_date": "2009-05-29", "runtime": 96, "genres": [{"name": "Animation"}, {"name": "Family"}]}), false).unwrap();
        assert_eq!(m.runtime, Some(96));
        assert_eq!(meta_text(&m), "Film · 2009 · 1h 36m · Animation, Family");
        let s = parse_details(
            &json!({"id": 5, "name": "Show", "first_air_date": "", "episode_run_time": [45]}),
            true,
        )
        .unwrap();
        assert!(s.tv && s.runtime == Some(45) && s.year.is_empty());
    }

    #[test]
    fn seasons_skip_specials_and_empty_seasons() {
        let v = json!({"seasons": [
            {"season_number": 0, "name": "Specials", "episode_count": 3},
            {"season_number": 1, "name": "Season 1", "episode_count": 7},
            {"season_number": 2, "name": "Season 2", "episode_count": 0}
        ]});
        assert_eq!(
            parse_seasons(&v),
            vec![Season {
                number: 1,
                name: "Season 1".into(),
                episodes: 7
            }]
        );
    }

    #[test]
    fn episodes_parse() {
        let v = json!({"episodes": [{"episode_number": 1, "name": "Pilot", "runtime": 58, "overview": "x"}, {"name": "broken"}]});
        let e = parse_episodes(&v);
        assert_eq!(e.len(), 1);
        assert_eq!(
            (e[0].number, e[0].name.as_str(), e[0].runtime),
            (1, "Pilot", Some(58))
        );
    }

    #[test]
    fn image_urls_need_a_path() {
        assert_eq!(
            image_url("/a.jpg", "w342"),
            "https://image.tmdb.org/t/p/w342/a.jpg"
        );
        assert_eq!(image_url("", "w342"), "");
    }

    #[test]
    fn endpoints_carry_the_key_and_escape_queries() {
        let u = endpoint(&Query::Search("a b&c".into()), "K").unwrap();
        assert!(
            u.as_str()
                .starts_with("https://api.themoviedb.org/3/search/multi?api_key=K&query=a+b%26c")
        );
        assert_eq!(
            endpoint(
                &Query::Season {
                    id: 1396,
                    season: 2
                },
                "K"
            )
            .unwrap()
            .path(),
            "/3/tv/1396/season/2"
        );
    }

    #[test]
    fn curated_shelf_matches_the_pool() {
        let c = curated();
        assert_eq!(c.len(), POOL.len());
        assert!(
            c.iter()
                .any(|t| t.tmdb == 1396 && t.tv && t.season == Some(1) && t.episode == Some(1))
        );
        assert!(c.iter().filter(|t| !t.tv).all(|t| t.season.is_none()));
    }
}
