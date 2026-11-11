//! Movies (alpha): streams films and series through the Cinecat backend
//! (`vidcue_providers::cinecat`) and plays them in the shared mpv session.
//!
//! The catalog is the built-in curated shelf (the verified pool from `tools/cinecat`), with
//! optional TMDB metadata when the user has entered their own API key. Nothing here claims that
//! a title will play: availability depends on third-party providers, so a failed check keeps
//! every provider's outcome on screen and offers a retry.

use std::sync::Arc;

use rune::TextEdit;
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use url::Url;
use vidcue_core::{Media, MediaKind};
use vidcue_providers::cinecat::{self, Backend, Stream, StreamKind, Subtitle, WebSources};

use crate::app::{App, ToastKind};
use crate::player::mpv::Load;

#[path = "movies_catalog.rs"]
mod catalog;
#[path = "movies_tmdb.rs"]
pub mod tmdb;
#[path = "movies_wiki.rs"]
pub mod wiki;
pub use tmdb::{Episode, Query, Season, Title};

/// Defaults shared with `tools/cinecat`.
const API: &str = "https://edge.cinecat.eu";
const ZCDN: &str = "https://zcdn2.2wwlgkfo-31a.workers.dev";
const ZCDN1: &str = "https://zcdn1.2wwlgkfo-31a.workers.dev";
const ZCDN3: &str = "https://zcdn3.2wwlgkfo-31a.workers.dev";
const APHRODITE: &str = "https://cdn.hls.lol";
const GRANITE: &str = "https://sub.vdrk.site";
/// Provider order the backend tries first, for the progress list.
pub const PROVIDERS: [&str; 4] = ["zcdn", "zcdn1", "zcdn3", "aphrodite"];
const MAX_CAPTIONS: usize = 24;

/// What one provider did during a check.
#[derive(Clone, Debug, PartialEq)]
pub struct Attempt {
    pub provider: String,
    pub ok: bool,
    pub ms: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Idle,
    Checking,
    Failed {
        message: String,
        attempts: Vec<Attempt>,
    },
}

/// Everything needed to resolve (and re-resolve) one title.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub tmdb: u64,
    pub tv: bool,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub title: String,
    pub overview: String,
    pub poster: String,
}

/// A loadable list: trending from TMDB.
#[derive(Default)]
pub struct Shelf {
    pub items: Vec<Title>,
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<String>,
}

pub struct Movies {
    backend: Option<Arc<Backend>>,
    pub shelf: Vec<Title>,
    pub trending: Shelf,
    pub results: Vec<Title>,
    pub searching: bool,
    pub searched: Option<String>,
    pub search_error: Option<String>,
    pub search: TextEdit,
    pub id_tmdb: TextEdit,
    pub id_season: TextEdit,
    pub id_episode: TextEdit,
    pub id_tv: bool,
    pub selected: Option<Title>,
    pub seasons: Vec<Season>,
    pub episodes: Vec<Episode>,
    catalog_episodes: std::collections::HashMap<u32, Vec<Episode>>,
    pub season: u32,
    pub episode: u32,
    pub featured: usize,
    /// The shelf the open sheet came from, so only that card carries the shared-element art.
    pub row: String,
    pub phase: Phase,
    pub id_error: Option<String>,
    token: Option<CancellationToken>,
    generation: u64,
    last: Option<Target>,
}

impl Default for Movies {
    fn default() -> Self {
        Self {
            backend: None,
            shelf: tmdb::curated(),
            trending: Shelf::default(),
            results: Vec::new(),
            searching: false,
            searched: None,
            search_error: None,
            search: TextEdit::new(""),
            id_tmdb: TextEdit::new(""),
            id_season: TextEdit::new("1"),
            id_episode: TextEdit::new("1"),
            id_tv: false,
            selected: None,
            seasons: Vec::new(),
            episodes: Vec::new(),
            catalog_episodes: Default::default(),
            season: 1,
            episode: 1,
            featured: 0,
            row: String::new(),
            phase: Phase::Idle,
            id_error: None,
            token: None,
            generation: 0,
            last: None,
        }
    }
}

impl Movies {
    pub fn new() -> Self {
        Self::default()
    }

    /// The title the hero shows: first trending entry with a backdrop, else a curated one.
    pub fn featured_title(&self) -> Option<&Title> {
        let rich: Vec<&Title> = self
            .trending
            .items
            .iter()
            .filter(|t| !t.backdrop.is_empty())
            .collect();
        if !rich.is_empty() {
            return rich.get(self.featured % rich.len()).copied();
        }
        self.shelf.get(self.featured % self.shelf.len().max(1))
    }

    /// Lazily built; the error text is shown on the page instead of panicking.
    fn backend(&mut self) -> Result<Arc<Backend>, String> {
        if let Some(b) = &self.backend {
            return Ok(b.clone());
        }
        let build = || -> anyhow::Result<Backend> {
            let u = |s: &str| Url::parse(s);
            let primary = u(ZCDN)?;
            let mut sources = WebSources::new(primary.clone(), u(APHRODITE)?)?;
            for (name, origin) in [("zcdn1", ZCDN1), ("zcdn3", ZCDN3)] {
                let origin = u(origin)?;
                if origin != primary {
                    sources = sources.with_zcdn_backup(name, origin)?;
                }
            }
            Ok(Backend::new(u(API)?, 2)?.with_web_sources(sources))
        };
        let backend =
            Arc::new(build().map_err(|e| vidcue_providers::diagnostic(&format!("{e:#}")))?);
        self.backend = Some(backend.clone());
        Ok(backend)
    }

    fn cancel(&mut self) {
        if let Some(t) = self.token.take() {
            t.cancel();
        }
        self.generation += 1;
    }
}

// ---- pure helpers ----

/// `cinecat:550` or `cinecat:1396:1:2`.
pub fn media_id(tmdb: u64, season: Option<u32>, episode: Option<u32>) -> String {
    match (season, episode) {
        (Some(s), Some(e)) => format!("cinecat:{tmdb}:{s}:{e}"),
        _ => format!("cinecat:{tmdb}"),
    }
}

pub fn media_url(tmdb: u64, season: Option<u32>, episode: Option<u32>) -> String {
    match (season, episode) {
        (Some(s), Some(e)) => format!("cinecat://tmdb/{tmdb}/{s}/{e}"),
        _ => format!("cinecat://tmdb/{tmdb}"),
    }
}

/// `Breaking Bad · S1E2`.
pub fn display_title(title: &str, season: Option<u32>, episode: Option<u32>) -> String {
    match (season, episode) {
        (Some(s), Some(e)) => format!("{title} · S{s}E{e}"),
        _ => title.to_owned(),
    }
}

/// HLS first, then a progressive file. DASH is never chosen: mpv playback of it is not supported.
pub fn choose_stream(streams: &[Stream]) -> Option<&Stream> {
    streams
        .iter()
        .find(|s| s.kind == StreamKind::Hls)
        .or_else(|| streams.iter().find(|s| s.kind == StreamKind::File))
}

/// `(label, url)` as the player's caption list stores them.
pub fn caption_of(s: &Subtitle) -> (String, String) {
    let label = if !s.display.trim().is_empty() {
        s.display.trim()
    } else {
        s.language.as_str()
    };
    (label.to_owned(), s.url.to_string())
}

/// Recovers per-provider outcomes from the backend's "No provider served…: a: err; b: err"
/// message, which is all a failed `resolve_auto` returns.
pub fn parse_failure(message: &str) -> Vec<Attempt> {
    let Some((_, detail)) = message.split_once("failed: ") else {
        return Vec::new();
    };
    let mut out: Vec<Attempt> = Vec::new();
    for piece in detail.split("; ") {
        let named = piece.split_once(": ").filter(|(name, _)| {
            !name.is_empty()
                && name.len() <= 40
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        });
        match named {
            Some((name, error)) => out.push(Attempt {
                provider: name.to_owned(),
                ok: false,
                ms: 0,
                error: Some(error.trim().to_owned()),
            }),
            None => {
                if let Some(Attempt { error: Some(e), .. }) = out.last_mut() {
                    e.push_str("; ");
                    e.push_str(piece);
                }
            }
        }
    }
    out
}

fn start_of(time: f64) -> Option<f64> {
    Some(time).filter(|t| *t > 1.0)
}

// ---- navigation hooks ----

/// The page opened: start trending when a key is set.
/// Fills the curated shelf with posters, intros and years from Wikimedia (no key needed), once.
fn enrich_shelf(app: &mut App) {
    if app.offline() || app.scratch.contains_key("movies-enriched") {
        return;
    }
    app.scratch.insert("movies-enriched".into(), String::new());
    let ids: Vec<(u64, bool)> = app.movies.shelf.iter().map(|t| (t.tmdb, t.tv)).collect();
    let Ok(client) = vidcue_providers::http_client(app.config.proxy.as_deref()) else {
        return;
    };
    app.be.task(
        async move { wiki::enrich(&client, &ids).await },
        |app, r, _| match r {
            Ok(meta) => {
                for t in app.movies.shelf.iter_mut() {
                    if let Some(m) = meta.get(&(t.tmdb, t.tv)) {
                        if t.poster.is_empty() {
                            t.poster = m.poster.clone();
                        }
                        if t.overview.is_empty() {
                            t.overview = m.overview.clone();
                        }
                        if t.year.is_empty() {
                            t.year = m.year.clone();
                        }
                    }
                }
            }
            Err(_) => {
                // Artwork is optional; a later visit tries again.
                app.scratch.remove("movies-enriched");
            }
        },
    );
}

pub fn enter(app: &mut App) {
    enrich_shelf(app);
    if app.prefs.tmdb_key.trim().is_empty()
        || app.offline()
        || app.movies.trending.loaded
        || app.movies.trending.loading
    {
        return;
    }
    load_trending(app);
}

pub fn load_trending(app: &mut App) {
    app.movies.trending.loading = true;
    app.movies.trending.error = None;
    tmdb_call(app, Query::Trending, |app, r| {
        let s = &mut app.movies.trending;
        s.loading = false;
        match r {
            Ok(v) => {
                s.items = tmdb::parse_list(&v);
                s.loaded = true;
            }
            Err(e) => s.error = Some(e),
        }
    });
}

/// Re-resolves the last title from scratch and resumes where the player was.
pub fn retry(app: &mut App) {
    let Some(target) = app.movies.last.clone() else {
        return;
    };
    let start = start_of(app.player.snap.time);
    play_target(app, target, start);
}

/// Runs one TMDB request with the user's key; the answer lands on the UI thread.
fn tmdb_call(
    app: &mut App,
    q: Query,
    then: impl FnOnce(&mut App, Result<Value, String>) + Send + 'static,
) {
    let key = app.prefs.tmdb_key.trim().to_owned();
    if key.is_empty() || app.offline() {
        app.be.land(Box::new(move |app, _| {
            then(app, Err("TMDB is unavailable".into()))
        }));
        return;
    }
    let proxy = app.config.proxy.clone();
    app.be.task(
        async move {
            let client = vidcue_providers::http_client(proxy.as_deref())?;
            tmdb::fetch(&client, &key, &q).await
        },
        move |app, r, _| then(app, r.map_err(|e| e.to_string())),
    );
}

// ---- catalog actions ----

pub fn accept_notice(app: &mut App) {
    app.edit_prefs(|p| p.alpha_notice_accepted = true);
}

pub fn next_featured(app: &mut App) {
    app.movies.featured = app.movies.featured.wrapping_add(1);
}

/// Searches TMDB for the text in the search field.
pub fn search(app: &mut App) {
    let q = app.movies.search.text.trim().to_owned();
    if q.is_empty() {
        app.movies.results.clear();
        app.movies.searched = None;
        app.movies.search_error = None;
        return;
    }
    app.movies.searching = true;
    app.movies.search_error = None;
    let asked = q.clone();
    if app.prefs.tmdb_key.trim().is_empty() {
        let query = q.clone();
        let proxy = app.config.proxy.clone();
        app.be.task(
            async move { catalog::search(&query, proxy.as_deref()).await },
            move |app, r, _| {
                let m = &mut app.movies;
                if m.search.text.trim() != asked {
                    return;
                }
                m.searching = false;
                match r {
                    Ok((list, warning)) => {
                        m.results = list;
                        m.searched = Some(asked);
                        if let Some(warning) = warning {
                            app.toast(ToastKind::Info, warning);
                        }
                    }
                    Err(e) => {
                        m.search_error = Some(vidcue_providers::diagnostic(&format!("{e:#}")))
                    }
                }
            },
        );
        return;
    }
    tmdb_call(app, Query::Search(q), move |app, r| {
        let m = &mut app.movies;
        // A newer search (or a cleared field) wins.
        if m.search.text.trim() != asked {
            return;
        }
        m.searching = false;
        match r {
            Ok(v) => {
                m.results = tmdb::parse_list(&v);
                m.searched = Some(asked);
            }
            Err(e) => m.search_error = Some(e),
        }
    });
}

/// Opens the detail sheet for a title and, with a TMDB key, fills in its metadata.
pub fn open_title(app: &mut App, title: Title) {
    let (tmdb_id, tv) = (title.tmdb, title.tv);
    let m = &mut app.movies;
    m.season = title.season.unwrap_or(1);
    m.episode = title.episode.unwrap_or(1);
    m.seasons.clear();
    m.episodes.clear();
    m.catalog_episodes.clear();
    let imdb = if title.imdb.is_empty() {
        match (tmdb_id, tv) {
            (1396, true) => "tt0903747".to_owned(),
            (1668, true) => "tt0108778".to_owned(),
            _ => String::new(),
        }
    } else {
        title.imdb.clone()
    };
    m.selected = Some(title);
    if !matches!(m.phase, Phase::Checking) {
        m.phase = Phase::Idle;
    }
    if app.prefs.tmdb_key.trim().is_empty() {
        if imdb.is_empty() {
            return;
        }
        let proxy = app.config.proxy.clone();
        app.be.task(
            async move { catalog::details(tv, &imdb, proxy.as_deref()).await },
            move |app, result, _| {
                let m = &mut app.movies;
                if !m
                    .selected
                    .as_ref()
                    .is_some_and(|t| t.tmdb == tmdb_id && t.tv == tv)
                {
                    return;
                }
                match result {
                    Ok((full, episodes)) => {
                        m.selected = Some(full);
                        m.seasons = episodes
                            .iter()
                            .map(|(&number, entries)| Season {
                                number,
                                name: if number == 0 {
                                    "Specials".into()
                                } else {
                                    format!("Season {number}")
                                },
                                episodes: entries.len() as u32,
                            })
                            .collect();
                        m.seasons.sort_by_key(|s| (s.number == 0, s.number));
                        m.catalog_episodes = episodes;
                        if !m.catalog_episodes.contains_key(&m.season) {
                            m.season = m.seasons.first().map_or(1, |s| s.number);
                        }
                        m.episodes = m
                            .catalog_episodes
                            .get(&m.season)
                            .cloned()
                            .unwrap_or_default();
                        if !m.episodes.iter().any(|e| e.number == m.episode) {
                            m.episode = m.episodes.first().map_or(1, |e| e.number);
                        }
                    }
                    Err(e) => {
                        app.toast(ToastKind::Error, format!("Title details unavailable: {e}"));
                    }
                }
            },
        );
        return;
    }
    tmdb_call(app, Query::Details { tv, id: tmdb_id }, move |app, r| {
        let Ok(v) = r else { return };
        let m = &mut app.movies;
        let Some(sel) = m
            .selected
            .as_mut()
            .filter(|s| s.tmdb == tmdb_id && s.tv == tv)
        else {
            return;
        };
        if let Some(mut full) = tmdb::parse_details(&v, tv) {
            // Keep the curated start point; TMDB has no opinion on it.
            full.season = sel.season;
            full.episode = sel.episode;
            if full.poster.is_empty() {
                full.poster = sel.poster.clone();
            }
            *sel = full;
        }
        if tv {
            m.seasons = tmdb::parse_seasons(&v);
            if let Some(first) = m.seasons.first().map(|s| s.number)
                && !m.seasons.iter().any(|s| s.number == m.season)
            {
                m.season = first;
                m.episode = 1;
            }
            let season = m.season;
            load_episodes(app, tmdb_id, season);
        }
    });
}

fn load_episodes(app: &mut App, id: u64, season: u32) {
    tmdb_call(app, Query::Season { id, season }, move |app, r| {
        let m = &mut app.movies;
        if m.selected.as_ref().is_some_and(|s| s.tmdb == id)
            && m.season == season
            && let Ok(v) = r
        {
            m.episodes = tmdb::parse_episodes(&v);
        }
    });
}

pub fn pick_season(app: &mut App, season: u32) {
    let Some(id) = app.movies.selected.as_ref().map(|s| s.tmdb) else {
        return;
    };
    app.movies.season = season;
    app.movies.episode = 1;
    app.movies.episodes = app
        .movies
        .catalog_episodes
        .get(&season)
        .cloned()
        .unwrap_or_default();
    if let Some(first) = app.movies.episodes.first() {
        app.movies.episode = first.number;
    }
    if !app.prefs.tmdb_key.trim().is_empty() {
        load_episodes(app, id, season);
    }
}

pub fn pick_episode(app: &mut App, episode: u32) {
    app.movies.episode = episode.max(1);
}

/// Steps the season or episode number when TMDB lists are unavailable.
pub fn step(app: &mut App, season: bool, delta: i32) {
    let v = if season {
        app.movies.season
    } else {
        app.movies.episode
    } as i64
        + delta as i64;
    let v = v.clamp(1, 999) as u32;
    if season {
        pick_season(app, v)
    } else {
        pick_episode(app, v)
    }
}

/// Stops a provider check without closing the sheet.
pub fn cancel_check(app: &mut App) {
    app.movies.cancel();
    app.movies.phase = Phase::Idle;
}

pub fn close_detail(app: &mut App) {
    app.movies.cancel();
    app.movies.phase = Phase::Idle;
    app.movies.selected = None;
}

/// "Open by TMDB id": builds a title from the id fields and opens its sheet.
pub fn open_by_id(app: &mut App) {
    let m = &mut app.movies;
    let Ok(id) = m
        .id_tmdb
        .text
        .trim()
        .parse::<u64>()
        .map_err(|_| ())
        .and_then(|v| if v > 0 { Ok(v) } else { Err(()) })
    else {
        m.id_error = Some("Enter a numeric TMDB id, for example 27205.".into());
        return;
    };
    let tv = m.id_tv;
    let (season, episode) = if tv {
        match (
            m.id_season.text.trim().parse::<u32>(),
            m.id_episode.text.trim().parse::<u32>(),
        ) {
            (Ok(s), Ok(e)) if s > 0 && e > 0 => (Some(s), Some(e)),
            _ => {
                m.id_error = Some("Season and episode must be numbers, starting at 1.".into());
                return;
            }
        }
    } else {
        (None, None)
    };
    m.id_error = None;
    let known = m
        .shelf
        .iter()
        .find(|t| t.tmdb == id && t.tv == tv)
        .map(|t| t.title.clone());
    let title = Title {
        tmdb: id,
        tv,
        title: known.unwrap_or_else(|| format!("TMDB {id}")),
        season,
        episode,
        ..Default::default()
    };
    open_title(app, title);
}

pub fn selected_target(app: &App) -> Option<Target> {
    let t = app.movies.selected.as_ref()?;
    let (season, episode) = if t.tv {
        (Some(app.movies.season), Some(app.movies.episode))
    } else {
        (None, None)
    };
    Some(Target {
        tmdb: t.tmdb,
        tv: t.tv,
        season,
        episode,
        title: t.title.clone(),
        overview: t.overview.clone(),
        poster: tmdb::image_url(&t.poster, "w342").clone(),
    })
}

/// Plays the title in the detail sheet (or the featured one when no sheet is open).
pub fn play_selected(app: &mut App) {
    if let Some(target) = selected_target(app) {
        play_target(app, target, None);
    }
}

pub fn play_title(app: &mut App, t: &Title) {
    let target = Target {
        tmdb: t.tmdb,
        tv: t.tv,
        season: t.tv.then(|| t.season.unwrap_or(1)),
        episode: t.tv.then(|| t.episode.unwrap_or(1)),
        title: t.title.clone(),
        overview: t.overview.clone(),
        poster: tmdb::image_url(&t.poster, "w342"),
    };
    // Show the sheet so progress and failures have somewhere to live.
    open_title(app, t.clone());
    play_target(app, target, None);
}

// ---- resolve and play ----

struct Ready {
    provider: String,
    load: Load,
    attempts: Vec<Attempt>,
    captions: Vec<(String, String)>,
}

struct Failure {
    message: String,
    attempts: Vec<Attempt>,
}

fn attempts_of(list: &[cinecat::FailoverAttempt]) -> Vec<Attempt> {
    list.iter()
        .map(|a| Attempt {
            provider: a.provider.clone(),
            ok: a.succeeded,
            ms: a.elapsed_ms,
            error: a.error.clone(),
        })
        .collect()
}

async fn resolve(
    backend: Arc<Backend>,
    media: cinecat::Media,
    cancel: CancellationToken,
    title: String,
    start: Option<f64>,
) -> Result<Ready, Failure> {
    let auto = backend
        .resolve_auto(&media, None, true, &cancel)
        .await
        .map_err(|e| {
            let message = vidcue_providers::diagnostic(&format!("{e:#}"));
            Failure {
                attempts: parse_failure(&message),
                message: if cancel.is_cancelled() {
                    "Cancelled".into()
                } else {
                    "No provider could serve this title right now.".into()
                },
            }
        })?;
    let attempts = attempts_of(&auto.attempts);
    let Some(stream) = choose_stream(&auto.resolution.streams) else {
        return Err(Failure {
            message: format!(
                "{} only offered a DASH stream, which Vidcue cannot play yet.",
                auto.resolution.provider
            ),
            attempts,
        });
    };
    let mut subs: Vec<Subtitle> = stream.subtitles.clone();
    if subs.is_empty()
        && let Ok(base) = Url::parse(GRANITE)
    {
        // Optional: a missing or slow subtitle service never blocks playback.
        let found = tokio::time::timeout(
            std::time::Duration::from_secs(8),
            backend.search_granite_subtitles(&base, &media, &cancel),
        )
        .await;
        if let Ok(Ok(list)) = found {
            subs = list;
        }
    }
    subs.truncate(MAX_CAPTIONS);
    let captions: Vec<(String, String)> = subs.iter().map(caption_of).collect();
    let load = Load {
        url: stream.url.to_string(),
        headers: stream
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        subtitles: captions
            .iter()
            .map(|(label, url)| (url.clone(), label.clone()))
            .collect(),
        start,
        title: Some(title),
        ..Default::default()
    };
    Ok(Ready {
        provider: auto.resolution.provider.clone(),
        load,
        attempts,
        captions,
    })
}

/// Cancels any check in flight, then resolves `target` and starts playback on success.
pub fn play_target(app: &mut App, target: Target, start: Option<f64>) {
    if app.offline() {
        app.movies.phase = Phase::Failed {
            message: "Streaming is unavailable in an offline session.".into(),
            attempts: Vec::new(),
        };
        return;
    }
    let backend = match app.movies.backend() {
        Ok(b) => b,
        Err(e) => {
            app.movies.phase = Phase::Failed {
                message: format!("The Cinecat backend could not start: {e}"),
                attempts: Vec::new(),
            };
            return;
        }
    };
    app.movies.cancel();
    let cancel = CancellationToken::new();
    app.movies.token = Some(cancel.clone());
    app.movies.phase = Phase::Checking;
    app.movies.last = Some(target.clone());
    let generation = app.movies.generation;
    let media = cinecat::Media {
        tmdb: target.tmdb,
        season: target.season,
        episode: target.episode,
    };
    let title = display_title(&target.title, target.season, target.episode);
    app.be.task(
        resolve(backend, media, cancel, title, start),
        move |app, out, _| landed(app, generation, target, out),
    );
}

fn landed(app: &mut App, generation: u64, target: Target, out: Result<Ready, Failure>) {
    if app.movies.generation != generation {
        return;
    }
    app.movies.token = None;
    match out {
        Ok(ready) => {
            app.movies.phase = Phase::Idle;
            let media = Media {
                id: media_id(target.tmdb, target.season, target.episode),
                url: media_url(target.tmdb, target.season, target.episode),
                title: display_title(&target.title, target.season, target.episode),
                description: target.overview.clone(),
                thumbnail: target.poster.clone(),
                creator: "Movies (alpha)".into(),
                kind: MediaKind::Video,
                ..Default::default()
            };
            let skipped = ready.attempts.iter().filter(|a| !a.ok).count();
            let mut lines = vec![format!(
                "Streaming from {} · checked 3 segments",
                ready.provider
            )];
            if skipped > 0 {
                lines.push(format!(
                    "{skipped} provider{} did not pass the check first",
                    if skipped == 1 { "" } else { "s" }
                ));
            }
            lines.push("Alpha: availability depends on Cinecat providers".into());
            crate::player::session::play_resolved(app, media, ready.load, lines, ready.captions);
        }
        Err(f) => {
            app.toast(
                ToastKind::Error,
                format!("Could not start “{}”: {}", target.title, f.message),
            );
            app.movies.phase = Phase::Failed {
                message: f.message,
                attempts: f.attempts,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn stream(kind: StreamKind, url: &str) -> Stream {
        Stream {
            kind,
            url: Url::parse(url).unwrap(),
            headers: BTreeMap::new(),
            subtitles: Vec::new(),
        }
    }

    #[test]
    fn ids_and_urls_include_the_episode_only_for_series() {
        assert_eq!(media_id(550, None, None), "cinecat:550");
        assert_eq!(media_id(1396, Some(1), Some(2)), "cinecat:1396:1:2");
        assert_eq!(media_url(550, None, None), "cinecat://tmdb/550");
        assert_eq!(media_url(1396, Some(2), Some(5)), "cinecat://tmdb/1396/2/5");
        assert_eq!(display_title("Friends", Some(1), Some(1)), "Friends · S1E1");
        assert_eq!(display_title("Heat", None, None), "Heat");
    }

    #[test]
    fn stream_choice_prefers_hls_then_file_and_skips_dash() {
        let dash = stream(StreamKind::Dash, "https://a.test/m.mpd");
        let file = stream(StreamKind::File, "https://a.test/m.mp4");
        let hls = stream(StreamKind::Hls, "https://a.test/m.m3u8");
        assert_eq!(
            choose_stream(&[dash.clone(), file.clone(), hls.clone()])
                .unwrap()
                .url,
            hls.url
        );
        assert_eq!(
            choose_stream(&[dash.clone(), file.clone()]).unwrap().url,
            file.url
        );
        assert!(choose_stream(&[dash]).is_none());
        assert!(choose_stream(&[]).is_none());
    }

    #[test]
    fn captions_use_the_display_name_or_the_language() {
        let mut s = Subtitle {
            url: Url::parse("https://s.test/en.vtt").unwrap(),
            language: "en".into(),
            display: "English".into(),
            format: "vtt".into(),
        };
        assert_eq!(
            caption_of(&s),
            ("English".into(), "https://s.test/en.vtt".into())
        );
        s.display = " ".into();
        assert_eq!(caption_of(&s).0, "en");
    }

    #[test]
    fn failure_message_is_split_per_provider() {
        let msg = "No provider served the requested media; 3 candidate(s) failed: zcdn: HTTP 404; zcdn1: timed out; segment probe; aphrodite: blocked";
        let a = parse_failure(msg);
        assert_eq!(
            a.iter().map(|a| a.provider.as_str()).collect::<Vec<_>>(),
            ["zcdn", "zcdn1", "aphrodite"]
        );
        assert_eq!(a[1].error.as_deref(), Some("timed out; segment probe"));
        assert!(a.iter().all(|a| !a.ok));
        assert!(parse_failure("Invalid media").is_empty());
    }

    #[test]
    fn resume_points_ignore_the_first_second() {
        assert_eq!(start_of(0.4), None);
        assert_eq!(start_of(93.0), Some(93.0));
    }

    #[test]
    fn featured_falls_back_to_the_curated_shelf() {
        let mut m = Movies::new();
        assert_eq!(m.featured_title().unwrap().tmdb, 550);
        m.featured = 1;
        assert_eq!(m.featured_title().unwrap().tmdb, 27205);
        m.trending.items = vec![Title {
            tmdb: 7,
            title: "T".into(),
            backdrop: "/b.jpg".into(),
            ..Default::default()
        }];
        assert_eq!(m.featured_title().unwrap().tmdb, 7);
    }
}
