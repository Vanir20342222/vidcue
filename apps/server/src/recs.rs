//! Private recommendations, built on the server from the user's own synced library and
//! public data only: uploads of followed channels, items related to finished plays, and
//! songs by artists similar to the user's (MusicBrainz + ListenBrainz). Opt-in per user,
//! switchable per server. The server never uses anyone's cookies or provider account.
//!
//! Upstream access goes through [`PublicMedia`] and [`MusicSimilarity`] so tests run
//! offline with fakes. Every upstream answer is cached in `recs_cache`, shared by all
//! users; keys and values hold public data only (except the per-user rotation row).

use crate::api::{ApiError, ApiResult, AppState, Device};
use crate::stats::DAY_MS;
use crate::throttle::{self, Rule};
use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{any, get, post};
use axum::{Json, Router};
use deadpool_postgres::Pool;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use uuid::Uuid;
use vidcue_core::playback_history::PlaybackHistory;
use vidcue_core::{Media, MediaKind};

const HOUR_MS: i64 = 3_600_000;
/// `GET /api/recs` rebuilds a feed older than this.
pub const FEED_MAX_AGE_MS: i64 = 6 * HOUR_MS;
/// At most one build per user starts in this window (manual or automatic).
pub const RECS_BUILD: Rule = Rule {
    limit: 1,
    window_ms: 30 * 60 * 1000,
};
pub const MAX_ITEMS: usize = 60;
pub const MAX_CHANNELS: usize = 8;
pub const UPLOADS_PER_CHANNEL: usize = 3;
pub const RELATED_SEEDS: i64 = 5;
pub const RELATED_PER_SEED: usize = 4;
/// A play counts as a related-items seed when finished or at least this long.
pub const RELATED_MIN_SECONDS: f64 = 60.0;
pub const ARTIST_SEEDS: usize = 3;
pub const SIMILAR_ARTISTS: usize = 5;
pub const ARTIST_WINDOW_MS: i64 = 30 * DAY_MS;
pub const MUSICBRAINZ_MIN_SCORE: i64 = 90;
pub const LISTENBRAINZ_ALGORITHM: &str =
    "session_based_days_7500_session_300_contribution_5_threshold_10_limit_100_filter_True_skip_30";
pub const DEFAULT_FETCH_PER_HOUR: u32 = 120;
pub const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(10);
pub const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
/// Minimum spacing of requests to one upstream service.
pub const UPSTREAM_SPACING: Duration = Duration::from_secs(1);
/// A build stores what it has after this long.
pub const BUILD_DEADLINE: Duration = Duration::from_secs(5 * 60);
pub const MAX_CACHE_ROWS: i64 = 50_000;
pub const MAX_NAME_CHARS: usize = 100;
const UPLOADS_TTL_MS: i64 = 6 * HOUR_MS;
const RELATED_TTL_MS: i64 = 24 * HOUR_MS;
const SONGS_TTL_MS: i64 = 24 * HOUR_MS;
const ARTIST_TTL_MS: i64 = 30 * DAY_MS;
const SIMILAR_TTL_MS: i64 = 7 * DAY_MS;
const ROTATION_TTL_MS: i64 = 90 * DAY_MS;

#[async_trait::async_trait]
pub trait PublicMedia: Send + Sync {
    async fn channel_uploads(&self, channel_id: &str) -> anyhow::Result<Vec<Media>>;
    async fn related(&self, video_id: &str, music: bool) -> anyhow::Result<Vec<Media>>;
    async fn search_songs(&self, query: &str) -> anyhow::Result<Vec<Media>>;
}

#[async_trait::async_trait]
pub trait MusicSimilarity: Send + Sync {
    async fn artist_mbid(&self, name: &str) -> anyhow::Result<Option<String>>;
    async fn similar_artists(&self, mbid: &str) -> anyhow::Result<Vec<(String, String)>>;
}

// ---- Production upstreams ----

/// Public YouTube and YouTube Music pages without any session.
pub struct YouTube {
    browser: vidcue_providers::browse::Browser,
}

impl YouTube {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            browser: vidcue_providers::browse::Browser::new(
                vidcue_providers::http_client(None)?,
                None,
                0,
            ),
        })
    }
    async fn items(
        &self,
        music: bool,
        request: vidcue_core::BrowseRequest,
    ) -> anyhow::Result<Vec<Media>> {
        let page = tokio::time::timeout(UPSTREAM_TIMEOUT, self.browser.fetch(music, &request))
            .await
            .map_err(|_| anyhow::anyhow!("YouTube request timed out"))??;
        Ok(page.items)
    }
}

#[async_trait::async_trait]
impl PublicMedia for YouTube {
    async fn channel_uploads(&self, channel_id: &str) -> anyhow::Result<Vec<Media>> {
        // A `UC…` id on the web client opens the channel's Videos tab, newest first.
        self.items(
            false,
            vidcue_core::BrowseRequest::Collection(channel_id.into()),
        )
        .await
    }
    async fn related(&self, video_id: &str, music: bool) -> anyhow::Result<Vec<Media>> {
        self.items(music, vidcue_core::BrowseRequest::Related(video_id.into()))
            .await
    }
    async fn search_songs(&self, query: &str) -> anyhow::Result<Vec<Media>> {
        self.items(
            true,
            vidcue_core::BrowseRequest::MusicSearch {
                query: query.into(),
                category: "songs".into(),
            },
        )
        .await
    }
}

/// MusicBrainz artist search and ListenBrainz Labs similar artists.
pub struct OpenData {
    client: reqwest::Client,
}

impl OpenData {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                // No public URL, username or contact data.
                .user_agent(concat!(
                    "vidcue-server/",
                    env!("CARGO_PKG_VERSION"),
                    " (self-hosted)"
                ))
                .timeout(UPSTREAM_TIMEOUT)
                .connect_timeout(UPSTREAM_TIMEOUT)
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }
    async fn get(&self, url: reqwest::Url) -> anyhow::Result<String> {
        let response = self
            .client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("upstream request failed: {}", e.without_url()))?;
        anyhow::ensure!(
            response.status().is_success(),
            "upstream answered {}",
            response.status().as_u16()
        );
        vidcue_providers::browse::limited_text(response, MAX_RESPONSE_BYTES).await
    }
}

/// Quotes a value for a Lucene phrase query.
pub fn lucene_phrase(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        if matches!(c, '"' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

#[async_trait::async_trait]
impl MusicSimilarity for OpenData {
    async fn artist_mbid(&self, name: &str) -> anyhow::Result<Option<String>> {
        let mut url = reqwest::Url::parse("https://musicbrainz.org/ws/2/artist")?;
        url.query_pairs_mut()
            .append_pair("query", &format!("artist:{}", lucene_phrase(name)))
            .append_pair("limit", "1")
            .append_pair("fmt", "json");
        Ok(parse_musicbrainz(&self.get(url).await?))
    }
    async fn similar_artists(&self, mbid: &str) -> anyhow::Result<Vec<(String, String)>> {
        let mut url =
            reqwest::Url::parse("https://labs.api.listenbrainz.org/similar-artists/json")?;
        url.query_pairs_mut()
            .append_pair("artist_mbids", mbid)
            .append_pair("algorithm", LISTENBRAINZ_ALGORITHM);
        Ok(parse_listenbrainz(&self.get(url).await?))
    }
}

fn mbid_ok(id: &str) -> bool {
    Uuid::parse_str(id).is_ok() && id.len() == 36
}

/// First artist with a score of at least [`MUSICBRAINZ_MIN_SCORE`].
pub fn parse_musicbrainz(body: &str) -> Option<String> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        tracing::debug!("MusicBrainz answer is not JSON");
        return None;
    };
    value
        .get("artists")?
        .as_array()?
        .iter()
        .find(|artist| {
            let score = artist.get("score").and_then(|s| {
                s.as_i64()
                    .or_else(|| s.as_str().and_then(|s| s.parse().ok()))
            });
            score.is_some_and(|s| s >= MUSICBRAINZ_MIN_SCORE)
        })
        .and_then(|artist| artist.get("id")?.as_str())
        .filter(|id| mbid_ok(id))
        .map(str::to_ascii_lowercase)
}

/// `(name, mbid)` of similar artists, best first. Accepts the flat array the API returns
/// and the older `[{"data": [...]}]` dataset wrapping; anything else is empty.
pub fn parse_listenbrainz(body: &str) -> Vec<(String, String)> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        tracing::debug!("ListenBrainz answer is not JSON");
        return Vec::new();
    };
    let rows: Vec<&Value> = match &value {
        Value::Array(items) => items
            .iter()
            .flat_map(|item| match item.get("data").and_then(Value::as_array) {
                Some(data) => data.iter().collect::<Vec<_>>(),
                None => vec![item],
            })
            .collect(),
        _ => {
            tracing::debug!("ListenBrainz answer has an unknown shape");
            Vec::new()
        }
    };
    let mut out: Vec<(f64, String, String)> = rows
        .into_iter()
        .filter_map(|row| {
            let mbid = row.get("artist_mbid")?.as_str()?;
            let name = row.get("name")?.as_str()?.trim();
            (mbid_ok(mbid) && !name.is_empty() && name.chars().count() <= 512).then(|| {
                let score = row.get("score").and_then(Value::as_f64).unwrap_or(0.0);
                (score, name.to_owned(), mbid.to_ascii_lowercase())
            })
        })
        .collect();
    // Stable sort keeps the upstream order among equal scores.
    out.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.into_iter().map(|(_, n, m)| (n, m)).collect()
}

// ---- Configuration and engine ----

#[derive(Clone, Debug)]
pub struct Config {
    /// `VIDCUE_RECS`.
    pub enabled: bool,
    /// `VIDCUE_RECS_MUSIC`.
    pub music: bool,
    /// `VIDCUE_RECS_FETCH_PER_HOUR`: upstream requests per hour over all users.
    pub fetch_per_hour: u32,
    pub spacing: Duration,
    pub deadline: Duration,
}

fn switch(name: &str) -> bool {
    !matches!(
        std::env::var(name).ok().as_deref().map(str::trim),
        Some("off" | "false" | "0" | "no")
    )
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            enabled: switch("VIDCUE_RECS"),
            music: switch("VIDCUE_RECS_MUSIC"),
            fetch_per_hour: std::env::var("VIDCUE_RECS_FETCH_PER_HOUR")
                .ok()
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(DEFAULT_FETCH_PER_HOUR),
            spacing: UPSTREAM_SPACING,
            deadline: BUILD_DEADLINE,
        }
    }
}

#[derive(Clone, Copy)]
enum Upstream {
    YouTube = 0,
    MusicBrainz = 1,
    ListenBrainz = 2,
}

pub struct Recs {
    pub config: Config,
    media: Arc<dyn PublicMedia>,
    music: Arc<dyn MusicSimilarity>,
    /// One build at a time, server-wide.
    gate: Semaphore,
    building: Mutex<HashSet<Uuid>>,
    /// Upstream requests in the current hour: (window start, used).
    budget: Mutex<(Instant, u32)>,
    /// Earliest time of the next request per upstream.
    pacers: [tokio::sync::Mutex<Option<Instant>>; 3],
}

impl Recs {
    pub fn new(
        config: Config,
        media: Arc<dyn PublicMedia>,
        music: Arc<dyn MusicSimilarity>,
    ) -> Self {
        Self {
            config,
            media,
            music,
            gate: Semaphore::new(1),
            building: Mutex::default(),
            budget: Mutex::new((Instant::now(), 0)),
            pacers: Default::default(),
        }
    }

    pub fn from_env() -> Self {
        let mut config = Config::from_env();
        let upstreams = YouTube::new().and_then(|yt| Ok((yt, OpenData::new()?)));
        match upstreams {
            Ok((yt, open)) => Self::new(config, Arc::new(yt), Arc::new(open)),
            Err(e) => {
                tracing::warn!("recommendations unavailable: {e:#}");
                config.enabled = false;
                Self::new(config, Arc::new(Unavailable), Arc::new(Unavailable))
            }
        }
    }

    pub fn is_building(&self, user: Uuid) -> bool {
        self.building.lock().expect("recs building").contains(&user)
    }

    fn take_budget(&self) -> bool {
        let mut budget = self.budget.lock().expect("recs budget");
        if budget.0.elapsed() >= Duration::from_secs(3600) {
            *budget = (Instant::now(), 0);
        }
        if budget.1 >= self.config.fetch_per_hour {
            return false;
        }
        budget.1 += 1;
        true
    }

    async fn pace(&self, upstream: Upstream) {
        let mut next = self.pacers[upstream as usize].lock().await;
        if let Some(at) = *next {
            tokio::time::sleep_until(at.into()).await;
        }
        *next = Some(Instant::now() + self.config.spacing);
    }
}

/// Stand-in when the HTTP client cannot be built; recommendations are then off.
struct Unavailable;

#[async_trait::async_trait]
impl PublicMedia for Unavailable {
    async fn channel_uploads(&self, _: &str) -> anyhow::Result<Vec<Media>> {
        anyhow::bail!("unavailable")
    }
    async fn related(&self, _: &str, _: bool) -> anyhow::Result<Vec<Media>> {
        anyhow::bail!("unavailable")
    }
    async fn search_songs(&self, _: &str) -> anyhow::Result<Vec<Media>> {
        anyhow::bail!("unavailable")
    }
}

#[async_trait::async_trait]
impl MusicSimilarity for Unavailable {
    async fn artist_mbid(&self, _: &str) -> anyhow::Result<Option<String>> {
        anyhow::bail!("unavailable")
    }
    async fn similar_artists(&self, _: &str) -> anyhow::Result<Vec<(String, String)>> {
        anyhow::bail!("unavailable")
    }
}

// ---- Media helpers ----

/// `youtube:<id>` key of a playable, online, non-live video or song; `None` otherwise.
pub fn playable_key(media: &Media) -> Option<String> {
    if media.live
        || !matches!(media.kind, MediaKind::Video | MediaKind::Song)
        || vidcue_core::http_url(&media.url).is_err()
    {
        return None;
    }
    let key = PlaybackHistory::key(media);
    key.starts_with("youtube:").then_some(key)
}

fn url_key(url: &str) -> String {
    PlaybackHistory::key(&Media {
        url: url.into(),
        ..Default::default()
    })
}

fn video_id(key: &str) -> Option<&str> {
    key.strip_prefix("youtube:").filter(|id| {
        id.len() == 11
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    })
}

/// The server's slim form of stored media (same as `sync::slimmed`).
fn slim(media: &Media) -> Value {
    let mut value = serde_json::to_value(media).unwrap_or_default();
    if let Some(object) = value.as_object_mut() {
        object.insert("description".into(), Value::String(String::new()));
        object.insert("chapters".into(), Value::Array(Vec::new()));
        object.remove("offline_id");
    }
    value
}

/// The user's content filters (§8.5, server copy).
#[derive(Clone, Debug, Default)]
pub struct Filters {
    pub channels: Vec<String>,
    pub keywords: Vec<String>,
    pub shorts: bool,
    pub live: bool,
    /// Keys whose resume position is finished, when a `watched` filter exists.
    pub watched: HashSet<String>,
}

impl Filters {
    pub fn hides(&self, media: &Media, key: &str) -> bool {
        let creator = media.creator.to_lowercase();
        let title = media.title.to_lowercase();
        self.channels
            .iter()
            .any(|c| *c == media.channel_id || c.to_lowercase() == creator)
            || self.keywords.iter().any(|k| title.contains(k.as_str()))
            || (self.shorts
                && vidcue_core::http_url(&media.url)
                    .is_ok_and(|u| u.path().starts_with("/shorts/")))
            || (self.live && media.live)
            || self.watched.contains(key)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecItem {
    pub key: String,
    pub media: Media,
    pub reason: &'static str,
    pub source: String,
}

impl RecItem {
    fn to_json(&self) -> Value {
        json!({"media": slim(&self.media), "reason": self.reason, "source": self.source})
    }
}

/// Round-robin over the sources, dropping duplicates, up to `cap` items.
pub fn interleave(sources: Vec<Vec<RecItem>>, cap: usize) -> Vec<RecItem> {
    let mut iters: Vec<_> = sources.into_iter().map(Vec::into_iter).collect();
    let (mut out, mut seen) = (Vec::new(), HashSet::new());
    loop {
        let mut progressed = false;
        for iter in &mut iters {
            if let Some(item) = iter.next() {
                progressed = true;
                if out.len() < cap && seen.insert(item.key.clone()) {
                    out.push(item);
                }
            }
        }
        if !progressed || out.len() >= cap {
            return out;
        }
    }
}

// ---- Build ----

/// What the build knows about the user, read once.
#[derive(Default)]
pub struct Library {
    pub subscriptions: Vec<(String, String)>,
    /// Plays: (url key, url, title, creator, kind, at, seconds, finished).
    pub plays: Vec<PlayRow>,
    pub excluded: HashSet<String>,
    pub filters: Filters,
}

#[derive(Clone, Debug, Default)]
pub struct PlayRow {
    pub key: String,
    pub title: String,
    pub creator: String,
    pub kind: String,
    pub at: i64,
    pub seconds: f64,
    pub finished: bool,
}

async fn load_library(db: &Pool, user: Uuid) -> anyhow::Result<Library> {
    let client = db.get().await?;
    let rows = client
        .query(
            "SELECT collection, key, value - 'thumbnail' FROM sync_records
             WHERE user_id = $1 AND value IS NOT NULL
               AND collection IN ('subscriptions', 'history', 'saved', 'filters', 'positions')",
            &[&user],
        )
        .await?;
    let mut library = Library::default();
    let mut watched = HashSet::new();
    let mut watched_filter = false;
    for row in rows {
        let (collection, key, value): (String, String, Value) =
            (row.get(0), row.get(1), row.get(2));
        let text = |field: &str| {
            value
                .get(field)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        match collection.as_str() {
            "subscriptions" => library.subscriptions.push((key, text("title"))),
            "history" => {
                let url = text("url");
                let key = url_key(&url);
                library.excluded.insert(key.clone());
                library.plays.push(PlayRow {
                    key,
                    title: text("title"),
                    creator: text("creator"),
                    kind: text("kind"),
                    at: value.get("at").and_then(Value::as_f64).unwrap_or(0.0) as i64,
                    seconds: value.get("seconds").and_then(Value::as_f64).unwrap_or(0.0),
                    finished: value.get("finished").and_then(Value::as_bool) == Some(true),
                });
            }
            "saved" => {
                library.excluded.insert(key);
            }
            "positions" => {
                if value.get("finished").and_then(Value::as_bool) == Some(true) {
                    watched.insert(key);
                }
            }
            "filters" => match text("kind").as_str() {
                "channel" => library
                    .filters
                    .channels
                    .push(text("value").trim().to_owned()),
                "keyword" => library
                    .filters
                    .keywords
                    .push(text("value").trim().to_lowercase()),
                "shorts" => library.filters.shorts = true,
                "live" => library.filters.live = true,
                "watched" => watched_filter = true,
                _ => {}
            },
            _ => {}
        }
    }
    if watched_filter {
        library.filters.watched = watched;
    }
    library.filters.channels.retain(|c| !c.is_empty());
    library.filters.keywords.retain(|k| !k.is_empty());
    library.subscriptions.sort();
    library.plays.sort_by_key(|p| std::cmp::Reverse(p.at));
    Ok(library)
}

struct Run<'a> {
    db: &'a Pool,
    recs: &'a Recs,
    now: i64,
    started: Instant,
    /// Set once the hourly budget or the deadline ran out; cache hits still work.
    out_of_budget: bool,
}

impl Run<'_> {
    async fn cache_get<T: DeserializeOwned>(&self, key: &str) -> anyhow::Result<Option<T>> {
        let row = self
            .db
            .get()
            .await?
            .query_opt(
                "SELECT value FROM recs_cache WHERE key = $1 AND expires_at > $2",
                &[&key, &self.now],
            )
            .await?;
        Ok(row.and_then(|r| serde_json::from_value(r.get(0)).ok()))
    }

    async fn cache_put(&self, key: &str, value: &Value, ttl_ms: i64) -> anyhow::Result<()> {
        self.db
            .get()
            .await?
            .execute(
                "INSERT INTO recs_cache (key, value, fetched_at, expires_at) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value,
                    fetched_at = EXCLUDED.fetched_at, expires_at = EXCLUDED.expires_at",
                &[&key, value, &self.now, &(self.now + ttl_ms)],
            )
            .await?;
        Ok(())
    }

    /// A cached upstream answer, or a fresh one within budget and pacing.
    async fn cached<T, F>(
        &mut self,
        key: &str,
        ttl_ms: i64,
        upstream: Upstream,
        fetch: F,
    ) -> anyhow::Result<Option<T>>
    where
        T: serde::Serialize + DeserializeOwned,
        F: std::future::Future<Output = anyhow::Result<T>>,
    {
        if let Some(hit) = self.cache_get(key).await? {
            return Ok(Some(hit));
        }
        if self.out_of_budget
            || self.started.elapsed() >= self.recs.config.deadline
            || !self.recs.take_budget()
        {
            self.out_of_budget = true;
            return Ok(None);
        }
        self.recs.pace(upstream).await;
        match fetch.await {
            Ok(value) => {
                self.cache_put(key, &serde_json::to_value(&value)?, ttl_ms)
                    .await?;
                Ok(Some(value))
            }
            Err(_) => {
                // The error may name what was looked up; keep user data out of logs.
                tracing::debug!("recommendation upstream {} failed", upstream as u8);
                Ok(None)
            }
        }
    }
}

fn pick(
    library: &Library,
    items: &[Value],
    n: usize,
    reason: &'static str,
    source: &str,
) -> Vec<RecItem> {
    let mut seen = HashSet::new();
    items
        .iter()
        .filter_map(|v| serde_json::from_value::<Media>(v.clone()).ok())
        .filter_map(|media| {
            let key = playable_key(&media)?;
            (!library.excluded.contains(&key)
                && !library.filters.hides(&media, &key)
                && seen.insert(key.clone()))
            .then(|| RecItem {
                key,
                media,
                reason,
                source: source.to_owned(),
            })
        })
        .take(n)
        .collect()
}

fn slim_all(items: Vec<Media>) -> Vec<Value> {
    items
        .iter()
        .filter(|m| playable_key(m).is_some())
        .map(slim)
        .collect()
}

async fn subscriptions(
    run: &mut Run<'_>,
    user: Uuid,
    library: &Library,
) -> anyhow::Result<Vec<RecItem>> {
    let rotation_key = format!("rotation:{user}");
    let mut used: HashMap<String, i64> = run.cache_get(&rotation_key).await?.unwrap_or_default();
    let mut channels: Vec<&(String, String)> = library
        .subscriptions
        .iter()
        .filter(|(id, _)| vidcue_core::is_channel_id(id))
        .collect();
    channels.sort_by_key(|(id, _)| (used.get(id).copied().unwrap_or(0), id.clone()));
    channels.truncate(MAX_CHANNELS);
    used.retain(|id, _| library.subscriptions.iter().any(|(s, _)| s == id));
    for (id, _) in &channels {
        used.insert(id.clone(), run.now);
    }
    run.cache_put(&rotation_key, &json!(used), ROTATION_TTL_MS)
        .await?;
    let mut out = Vec::new();
    for (id, title) in channels {
        let media = run.recs.media.clone();
        let Some(items) = run
            .cached(
                &format!("yt:uploads:{id}"),
                UPLOADS_TTL_MS,
                Upstream::YouTube,
                async { Ok(slim_all(media.channel_uploads(id).await?)) },
            )
            .await?
        else {
            continue;
        };
        let picked = pick(library, &items, UPLOADS_PER_CHANNEL, "subscription", "");
        out.extend(picked.into_iter().map(|mut item| {
            item.source = if title.trim().is_empty() {
                item.media.creator.clone()
            } else {
                title.clone()
            };
            item
        }));
    }
    Ok(out)
}

async fn related(run: &mut Run<'_>, library: &Library) -> anyhow::Result<Vec<RecItem>> {
    let mut seeds = Vec::new();
    let mut seen = HashSet::new();
    for play in &library.plays {
        if seeds.len() as i64 >= RELATED_SEEDS {
            break;
        }
        if (play.finished || play.seconds >= RELATED_MIN_SECONDS)
            && let Some(id) = video_id(&play.key)
            && seen.insert(id)
        {
            seeds.push((id, play));
        }
    }
    let mut out = Vec::new();
    for (id, play) in seeds {
        let music = play.kind == "song";
        let media = run.recs.media.clone();
        let Some(items) = run
            .cached(
                &format!("yt:related:{id}:{music}"),
                RELATED_TTL_MS,
                Upstream::YouTube,
                async { Ok(slim_all(media.related(id, music).await?)) },
            )
            .await?
        else {
            continue;
        };
        out.extend(pick(
            library,
            &items,
            RELATED_PER_SEED,
            "related",
            &play.title,
        ));
    }
    Ok(out)
}

fn name_key(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .chars()
        .take(MAX_NAME_CHARS)
        .collect()
}

async fn similar_artists(run: &mut Run<'_>, library: &Library) -> anyhow::Result<Vec<RecItem>> {
    let since = run.now - ARTIST_WINDOW_MS;
    let mut seconds: HashMap<String, (f64, String)> = HashMap::new();
    for play in library
        .plays
        .iter()
        .filter(|p| {
            p.kind == "song" && p.at >= since && p.seconds >= crate::stats::MIN_PLAY_SECONDS
        })
        .filter(|p| !p.creator.trim().is_empty())
    {
        let entry = seconds
            .entry(name_key(&play.creator))
            .or_insert((0.0, play.creator.trim().to_owned()));
        entry.0 += play.seconds.min(crate::stats::MAX_PLAY_SECONDS);
    }
    let mut seeds: Vec<(String, f64, String)> =
        seconds.into_iter().map(|(k, (s, n))| (k, s, n)).collect();
    seeds.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    seeds.truncate(ARTIST_SEEDS);
    let known: HashSet<String> = library.plays.iter().map(|p| name_key(&p.creator)).collect();
    let mut lists = Vec::new();
    for (key, _, name) in &seeds {
        let music = run.recs.music.clone();
        let Some(found) = run
            .cached(
                &format!("mb:artist:{key}"),
                ARTIST_TTL_MS,
                Upstream::MusicBrainz,
                async { Ok(json!({ "mbid": music.artist_mbid(name).await? })) },
            )
            .await?
        else {
            continue;
        };
        let Some(mbid) = found
            .get("mbid")
            .and_then(Value::as_str)
            .filter(|m| mbid_ok(m))
        else {
            continue;
        };
        let music = run.recs.music.clone();
        let Some(similar) = run
            .cached(
                &format!("lb:similar:{mbid}"),
                SIMILAR_TTL_MS,
                Upstream::ListenBrainz,
                async {
                    Ok(music
                        .similar_artists(mbid)
                        .await?
                        .into_iter()
                        .map(|(name, mbid)| json!({"name": name, "mbid": mbid}))
                        .collect::<Vec<_>>())
                },
            )
            .await?
        else {
            continue;
        };
        lists.push(
            similar
                .iter()
                .filter_map(|s| s.get("name")?.as_str().map(str::to_owned))
                .collect::<Vec<_>>(),
        );
    }
    // Interleave the seeds' lists so one artist does not take every slot.
    let mut names = Vec::new();
    let mut taken = HashSet::new();
    let longest = lists.iter().map(Vec::len).max().unwrap_or(0);
    'outer: for i in 0..longest {
        for list in &lists {
            if let Some(name) = list.get(i) {
                let key = name_key(name);
                if !key.is_empty() && !known.contains(&key) && taken.insert(key.clone()) {
                    names.push((key, name.trim().to_owned()));
                    if names.len() >= SIMILAR_ARTISTS {
                        break 'outer;
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    for (key, name) in names {
        let media = run.recs.media.clone();
        let query: String = name.chars().take(MAX_NAME_CHARS).collect();
        let Some(items) = run
            .cached(
                &format!("yt:songs:{key}"),
                SONGS_TTL_MS,
                Upstream::YouTube,
                async { Ok(slim_all(media.search_songs(&query).await?)) },
            )
            .await?
        else {
            continue;
        };
        out.extend(pick(library, &items, 1, "similar_artist", &name));
    }
    Ok(out)
}

/// Builds and stores the user's feed. Returns the number of items stored.
pub async fn build(db: &Pool, recs: &Recs, user: Uuid, now: i64) -> anyhow::Result<usize> {
    let settings = load_settings(db, user).await?;
    if !settings.0 {
        return Ok(0);
    }
    let library = load_library(db, user).await?;
    let mut run = Run {
        db,
        recs,
        now,
        started: Instant::now(),
        out_of_budget: false,
    };
    let subs = subscriptions(&mut run, user, &library).await?;
    let related = related(&mut run, &library).await?;
    let similar = if recs.config.music && settings.1 {
        similar_artists(&mut run, &library).await?
    } else {
        Vec::new()
    };
    let items = interleave(vec![subs, related, similar], MAX_ITEMS);
    let value = Value::Array(items.iter().map(RecItem::to_json).collect());
    // A user who switched recommendations off meanwhile gets nothing stored.
    db.get()
        .await?
        .execute(
            "INSERT INTO recs_feed (user_id, built_at, items)
             SELECT $1, $2, $3 WHERE EXISTS (SELECT 1 FROM recs_settings WHERE user_id = $1 AND enabled)
             ON CONFLICT (user_id) DO UPDATE SET built_at = EXCLUDED.built_at, items = EXCLUDED.items",
            &[&user, &now, &value],
        )
        .await?;
    Ok(items.len())
}

/// Removes the user from the building set however the task ends.
struct Building<'a>(&'a Recs, Uuid);
impl Drop for Building<'_> {
    fn drop(&mut self) {
        self.0
            .building
            .lock()
            .expect("recs building")
            .remove(&self.1);
    }
}

/// Starts a background build unless one is queued or running for this user.
pub fn spawn_build(state: AppState, recs: Arc<Recs>, user: Uuid) {
    if !recs.building.lock().expect("recs building").insert(user) {
        return;
    }
    tokio::spawn(async move {
        let _building = Building(&recs, user);
        let Ok(_permit) = recs.gate.acquire().await else {
            return;
        };
        if let Err(e) = locked_build(&state, &recs, user).await {
            tracing::warn!("recommendation build failed: {e:#}");
        }
    });
}

async fn locked_build(state: &AppState, recs: &Recs, user: Uuid) -> anyhow::Result<()> {
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    // Another server process may be building for the same user.
    let locked: bool = tx
        .query_one(
            "SELECT pg_try_advisory_xact_lock(hashtextextended('recs:' || $1::text, 0))",
            &[&user.to_string()],
        )
        .await?
        .get(0);
    if locked {
        let deadline = recs.config.deadline + UPSTREAM_TIMEOUT;
        tokio::time::timeout(deadline, build(&state.0.db, recs, user, state.now()))
            .await
            .map_err(|_| anyhow::anyhow!("build timed out"))??;
    }
    tx.commit().await?;
    Ok(())
}

// ---- HTTP ----

pub fn routes() -> Router<AppState> {
    routes_with(Arc::new(Recs::from_env()))
}

pub fn routes_with(recs: Arc<Recs>) -> Router<AppState> {
    let router = Router::new().route("/api/features", get(features));
    if !recs.config.enabled {
        // Answer before authentication: the feature does not exist on this server.
        let off = || async { available_off() };
        return router
            .route("/api/recs/settings", any(off))
            .route("/api/recs", any(off))
            .route("/api/recs/refresh", any(off))
            .layer(Extension(recs));
    }
    router
        .route("/api/recs/settings", get(get_settings).put(put_settings))
        .route("/api/recs", get(feed))
        .route("/api/recs/refresh", post(refresh))
        .layer(Extension(recs))
}

async fn features(Extension(recs): Extension<Arc<Recs>>) -> Json<Value> {
    Json(json!({
        "features": ["health", "stats", "recs", "admin_web"],
        "recs_available": recs.config.enabled,
    }))
}

fn available_off() -> ApiError {
    ApiError(
        StatusCode::NOT_FOUND,
        "Recommendations are turned off on this server".into(),
    )
}

/// (enabled, music) for a user; off by default.
async fn load_settings(db: &Pool, user: Uuid) -> anyhow::Result<(bool, bool)> {
    Ok(db
        .get()
        .await?
        .query_opt(
            "SELECT enabled, music FROM recs_settings WHERE user_id = $1",
            &[&user],
        )
        .await?
        .map_or((false, true), |r| (r.get(0), r.get(1))))
}

async fn get_settings(State(state): State<AppState>, device: Device) -> ApiResult {
    let (enabled, music) = load_settings(&state.0.db, device.user).await?;
    Ok(Json(json!({ "enabled": enabled, "music": music })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsBody {
    enabled: Option<bool>,
    music: Option<bool>,
}

async fn put_settings(
    State(state): State<AppState>,
    device: Device,
    body: Result<Json<SettingsBody>, JsonRejection>,
) -> ApiResult {
    let Json(body) =
        body.map_err(|_| ApiError::bad("Body must be {\"enabled\": bool, \"music\": bool}"))?;
    let (enabled, music) = load_settings(&state.0.db, device.user).await?;
    let (enabled, music) = (body.enabled.unwrap_or(enabled), body.music.unwrap_or(music));
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    tx.execute(
        "INSERT INTO recs_settings (user_id, enabled, music, updated_at) VALUES ($1, $2, $3, $4)
         ON CONFLICT (user_id) DO UPDATE SET enabled = EXCLUDED.enabled, music = EXCLUDED.music,
            updated_at = EXCLUDED.updated_at",
        &[&device.user, &enabled, &music, &state.now()],
    )
    .await?;
    if !enabled {
        tx.execute("DELETE FROM recs_feed WHERE user_id = $1", &[&device.user])
            .await?;
        tx.execute(
            "DELETE FROM recs_cache WHERE key = $1",
            &[&format!("rotation:{}", device.user)],
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({ "enabled": enabled, "music": music })))
}

async fn feed(
    State(state): State<AppState>,
    Extension(recs): Extension<Arc<Recs>>,
    device: Device,
) -> ApiResult {
    let db = &state.0.db;
    let (enabled, _) = load_settings(db, device.user).await?;
    if !enabled {
        return Ok(Json(json!({ "enabled": false, "items": [] })));
    }
    let now = state.now();
    let row = db
        .get()
        .await?
        .query_opt(
            "SELECT built_at, items FROM recs_feed WHERE user_id = $1",
            &[&device.user],
        )
        .await?;
    let (built_at, items) = row.map_or((None, json!([])), |r| (Some(r.get::<_, i64>(0)), r.get(1)));
    let mut building = recs.is_building(device.user);
    if !building && built_at.is_none_or(|at| at < now - FEED_MAX_AGE_MS) {
        let key = format!("recs-build:{}", device.user);
        if throttle::attempt(db, &[(RECS_BUILD, &key)], now)
            .await?
            .is_some()
        {
            spawn_build(state.clone(), recs.clone(), device.user);
            building = true;
        }
    }
    Ok(Json(json!({
        "enabled": true,
        "built_at": built_at,
        "building": building,
        "items": items,
    })))
}

async fn refresh(
    State(state): State<AppState>,
    Extension(recs): Extension<Arc<Recs>>,
    device: Device,
) -> ApiResult<axum::response::Response> {
    if !load_settings(&state.0.db, device.user).await?.0 {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "Turn on recommendations first".into(),
        ));
    }
    let key = format!("recs-build:{}", device.user);
    state
        .attempt(
            &[(RECS_BUILD, &key)],
            "Recommendations were refreshed recently, try again later",
        )
        .await?;
    spawn_build(state.clone(), recs, device.user);
    Ok((StatusCode::ACCEPTED, Json(json!({ "building": true }))).into_response())
}

pub async fn maintenance(db: &Pool, now: i64) -> anyhow::Result<()> {
    db.get()
        .await?
        .batch_execute(&format!(
            "DELETE FROM recs_cache WHERE expires_at < {now};
             DELETE FROM recs_cache WHERE key IN
                (SELECT key FROM recs_cache ORDER BY fetched_at DESC, key OFFSET {MAX_CACHE_ROWS});"
        ))
        .await?;
    Ok(())
}

/// Offline upstreams for tests: canned answers and a call log.
#[cfg(test)]
pub(crate) mod fakes {
    use super::*;

    pub fn media(id: &str, title: &str, creator: &str, kind: MediaKind) -> Media {
        Media {
            id: id.into(),
            title: title.into(),
            creator: creator.into(),
            url: format!("https://www.youtube.com/watch?v={id}"),
            description: "long description".into(),
            kind,
            ..Default::default()
        }
    }

    #[derive(Default)]
    pub struct FakeMedia {
        pub uploads: HashMap<String, Vec<Media>>,
        pub related: HashMap<String, Vec<Media>>,
        pub songs: HashMap<String, Vec<Media>>,
        pub calls: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl PublicMedia for FakeMedia {
        async fn channel_uploads(&self, channel_id: &str) -> anyhow::Result<Vec<Media>> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("uploads:{channel_id}"));
            self.uploads
                .get(channel_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no channel"))
        }
        async fn related(&self, video_id: &str, music: bool) -> anyhow::Result<Vec<Media>> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("related:{video_id}:{music}"));
            Ok(self.related.get(video_id).cloned().unwrap_or_default())
        }
        async fn search_songs(&self, query: &str) -> anyhow::Result<Vec<Media>> {
            self.calls.lock().unwrap().push(format!("songs:{query}"));
            Ok(self.songs.get(query).cloned().unwrap_or_default())
        }
    }

    #[derive(Default)]
    pub struct FakeMusic {
        pub mbids: HashMap<String, String>,
        pub similar: HashMap<String, Vec<(String, String)>>,
        pub calls: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl MusicSimilarity for FakeMusic {
        async fn artist_mbid(&self, name: &str) -> anyhow::Result<Option<String>> {
            self.calls.lock().unwrap().push(format!("mbid:{name}"));
            Ok(self.mbids.get(name).cloned())
        }
        async fn similar_artists(&self, mbid: &str) -> anyhow::Result<Vec<(String, String)>> {
            self.calls.lock().unwrap().push(format!("similar:{mbid}"));
            Ok(self.similar.get(mbid).cloned().unwrap_or_default())
        }
    }

    pub fn config(enabled: bool, fetch_per_hour: u32) -> Config {
        Config {
            enabled,
            music: true,
            fetch_per_hour,
            spacing: Duration::ZERO,
            deadline: Duration::from_secs(60),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fakes::media;
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/recs/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn musicbrainz_takes_the_first_confident_artist() {
        assert_eq!(
            parse_musicbrainz(&fixture("musicbrainz_artist.json")).as_deref(),
            Some("a74b1b7f-71a5-4011-9441-d0b5e4122711")
        );
        assert_eq!(
            parse_musicbrainz(
                r#"{"artists":[{"id":"a74b1b7f-71a5-4011-9441-d0b5e4122711","score":"95"}]}"#
            )
            .as_deref(),
            Some("a74b1b7f-71a5-4011-9441-d0b5e4122711")
        );
        assert_eq!(
            parse_musicbrainz(
                r#"{"artists":[{"id":"a74b1b7f-71a5-4011-9441-d0b5e4122711","score":89}]}"#
            ),
            None
        );
        assert_eq!(
            parse_musicbrainz(r#"{"artists":[{"id":"not-an-id","score":100}]}"#),
            None
        );
        assert_eq!(parse_musicbrainz("<html>"), None);
        assert_eq!(parse_musicbrainz(r#"{"error":"rate limited"}"#), None);
    }

    #[test]
    fn listenbrainz_parses_defensively() {
        let similar = parse_listenbrainz(&fixture("listenbrainz_similar.json"));
        assert_eq!(
            similar.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            ["Thom Yorke", "Portishead", "Massive Attack"]
        );
        assert!(similar.iter().all(|(_, m)| mbid_ok(m)));
        let wrapped = format!(
            r#"[{{"type":"dataset","data":{}}}]"#,
            fixture("listenbrainz_similar.json")
        );
        assert_eq!(parse_listenbrainz(&wrapped), similar);
        assert!(parse_listenbrainz(r#"{"unexpected":true}"#).is_empty());
        assert!(parse_listenbrainz("not json").is_empty());
        assert!(parse_listenbrainz(r#"[{"name":"No id"}]"#).is_empty());
    }

    #[test]
    fn lucene_phrases_are_escaped() {
        assert_eq!(
            lucene_phrase(r#"AC/DC "live" \o/"#),
            r#""AC/DC \"live\" \\o/""#
        );
    }

    #[test]
    fn only_playable_youtube_items_count() {
        let video = media("aaaaaaaaaaa", "A", "C", MediaKind::Video);
        assert_eq!(playable_key(&video).as_deref(), Some("youtube:aaaaaaaaaaa"));
        let mut live = video.clone();
        live.live = true;
        assert_eq!(playable_key(&live), None);
        let playlist = media("PLxyz", "P", "C", MediaKind::Playlist);
        assert_eq!(playable_key(&playlist), None);
        let mut other = video.clone();
        other.url = "https://radio.example/stream".into();
        assert_eq!(playable_key(&other), None);
        assert_eq!(video_id("youtube:aaaaaaaaaaa"), Some("aaaaaaaaaaa"));
        assert_eq!(video_id("youtube:short"), None);
        let slimmed = slim(&video);
        assert_eq!(slimmed["description"], "");
    }

    #[test]
    fn filters_follow_the_client_semantics() {
        let mut m = media(
            "aaaaaaaaaaa",
            "Big Live Show",
            "Some Channel",
            MediaKind::Video,
        );
        m.channel_id = "UCaaaaaaaaaaaaaaaaaaaaaa".into();
        let key = "youtube:aaaaaaaaaaa";
        let none = Filters::default();
        assert!(!none.hides(&m, key));
        let f = |filters: Filters| filters.hides(&m, key);
        assert!(f(Filters {
            channels: vec!["UCaaaaaaaaaaaaaaaaaaaaaa".into()],
            ..Default::default()
        }));
        assert!(f(Filters {
            channels: vec!["some channel".into()],
            ..Default::default()
        }));
        assert!(f(Filters {
            keywords: vec!["live show".into()],
            ..Default::default()
        }));
        assert!(!f(Filters {
            keywords: vec!["concert".into()],
            ..Default::default()
        }));
        assert!(!f(Filters {
            shorts: true,
            live: true,
            ..Default::default()
        }));
        assert!(f(Filters {
            watched: [key.to_owned()].into(),
            ..Default::default()
        }));
        let mut short = m.clone();
        short.url = "https://www.youtube.com/shorts/aaaaaaaaaaa".into();
        assert!(
            Filters {
                shorts: true,
                ..Default::default()
            }
            .hides(&short, key)
        );
    }

    #[test]
    fn interleave_round_robins_dedupes_and_caps() {
        let item = |id: &str, reason| RecItem {
            key: format!("youtube:{id}"),
            media: media(id, id, "c", MediaKind::Video),
            reason,
            source: String::new(),
        };
        let out = interleave(
            vec![
                vec![
                    item("a", "subscription"),
                    item("b", "subscription"),
                    item("c", "subscription"),
                ],
                vec![item("a", "related"), item("d", "related")],
                vec![item("e", "similar_artist")],
            ],
            4,
        );
        let ids: Vec<_> = out.iter().map(|i| i.media.id.as_str()).collect();
        assert_eq!(ids, ["a", "e", "b", "d"]);
        assert_eq!(out[0].reason, "subscription");
    }
}
