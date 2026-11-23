//! Local stores behind the synced collections (PROTOCOL.md §8.3–§8.5): resume entries,
//! play events, playlists, followed channels and filters; the state migration from the
//! URL-keyed `positions`; the change hooks of existing ops; and the `LocalStores`
//! implementation that applies pulled changes.
use super::*;
use std::collections::HashSet;
use uuid::Uuid;
use vidcue_core::playback_history::{PlaybackHistory, Position};
use vidcue_sync::{
    Change,
    filters::Filter,
    fractional,
    protocol::{
        HistoryEvent, PORTABLE_DOWNLOAD_FIELDS, PORTABLE_SETTINGS, SharedChange, SharedPlaylist,
    },
};

pub const MAX_PLAYS: usize = 5000;
pub const MAX_RESUME: usize = 2000;
pub const MAX_PLAYLISTS: usize = 500;
pub const MAX_PLAYLIST_ENTRIES: usize = 5000;
pub const MAX_FOLLOWS: usize = 2000;
pub const MAX_FILTERS: usize = 500;
pub const MAX_SAVED: usize = 2000;
const MAX_TITLE_CHARS: usize = 200;
/// A position this close to the end counts as finished.
const FINISH_MARGIN_MS: u64 = 5_000;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResumeEntry {
    /// Slim media (no description, chapters or offline id).
    pub media: Media,
    pub ms: u64,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// Unix ms of the last local or remote change; 0 for migrated entries.
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub finished: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayEvent {
    pub id: Uuid,
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub creator: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub thumbnail: String,
    pub at: i64,
    #[serde(default)]
    pub seconds: f64,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub finished: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalPlaylist {
    pub id: Uuid,
    pub title: String,
    #[serde(default)]
    pub created_at: i64,
    /// Set while the playlist is a server-side shared playlist (same id).
    #[serde(default)]
    pub shared_id: Option<Uuid>,
    /// `owner` or `member` for shared playlists.
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub entries: Vec<LocalEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalEntry {
    pub id: Uuid,
    pub position: String,
    pub media: Media,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Follow {
    pub channel_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub thumbnail: String,
    #[serde(default)]
    pub at: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncMeta {
    /// Clock of the last change of each local register, keyed `collection\u{1f}key`.
    #[serde(default)]
    pub clocks: HashMap<String, i64>,
    #[serde(default)]
    pub last_clock: i64,
    /// Pulled entries whose playlist header has not arrived yet (kept hidden).
    #[serde(default)]
    pub hidden_entries: HashMap<Uuid, Vec<LocalEntry>>,
}

/// One local register write for the outbox.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LocalOp {
    Personal {
        collection: &'static str,
        key: String,
        value: Option<Value>,
        clock: i64,
    },
    Shared {
        playlist: Uuid,
        entry: Uuid,
        value: Option<Value>,
        clock: i64,
    },
}

fn register(collection: &str, key: &str) -> String {
    format!("{collection}\u{1f}{key}")
}

/// `max(now, last + 1)`, so local edits order strictly.
pub(crate) fn next_clock(meta: &mut SyncMeta) -> i64 {
    meta.last_clock = vidcue_sync::now_ms().max(meta.last_clock + 1);
    meta.last_clock
}

/// Records a local change of a register and returns its clock.
fn stamp(meta: &mut SyncMeta, collection: &str, key: &str) -> i64 {
    let clock = next_clock(meta);
    meta.clocks.insert(register(collection, key), clock);
    clock
}

fn personal(
    meta: &mut SyncMeta,
    collection: &'static str,
    key: &str,
    value: Option<Value>,
) -> LocalOp {
    let clock = stamp(meta, collection, key);
    if value.is_none() {
        meta.clocks.remove(&register(collection, key));
    }
    LocalOp::Personal {
        collection,
        key: key.into(),
        value,
        clock,
    }
}

fn clock_of(meta: &SyncMeta, collection: &str, key: &str) -> i64 {
    meta.clocks
        .get(&register(collection, key))
        .copied()
        .unwrap_or(0)
}

/// Long descriptions and chapters are refetched; synced copies stay small.
pub(crate) fn slim(media: &Media) -> Media {
    Media {
        description: String::new(),
        chapters: Vec::new(),
        offline_id: None,
        ..media.clone()
    }
}

pub(crate) fn online(media: &Media) -> bool {
    vidcue_core::http_url(&media.url).is_ok()
}

pub(crate) fn media_key(media: &Media) -> String {
    PlaybackHistory::key(media)
}

/// The YouTube video id in a shorts/live/embed path (watch URLs carry it in `v`).
fn path_video_id(url: &str) -> String {
    let Ok(parsed) = url::Url::parse(url) else {
        return String::new();
    };
    if !vidcue_core::youtube_host(parsed.host_str().unwrap_or_default()) {
        return String::new();
    }
    let mut parts = parsed.path_segments().into_iter().flatten();
    match (parts.next(), parts.next()) {
        (Some("shorts" | "live" | "embed"), Some(id)) if id.len() == 11 => id.into(),
        _ => String::new(),
    }
}

/// The best known media for a URL: from the library, else a minimal one.
pub(crate) fn known_media(state: &MobileState, url: &str) -> Media {
    state
        .recent
        .iter()
        .rev()
        .chain(&state.saved)
        .chain(&state.queue)
        .chain(&state.playback.items)
        .find(|m| m.url == url)
        .cloned()
        .unwrap_or_else(|| Media {
            id: path_video_id(url),
            url: url.into(),
            ..Default::default()
        })
}

/// Converts the URL-keyed `positions` of older versions into `resume` entries.
pub(crate) fn migrate(state: &mut MobileState) -> bool {
    if state.positions.is_empty() {
        return false;
    }
    for (url, ms) in std::mem::take(&mut state.positions) {
        let media = slim(&known_media(state, &url));
        let key = media_key(&media);
        state.resume.entry(key).or_insert(ResumeEntry {
            duration_ms: media.duration.map(|d| d.saturating_mul(1000)),
            media,
            ms,
            updated_at: 0,
            finished: false,
        });
    }
    true
}

fn kind_name(media: &Media) -> String {
    serde_json::to_value(media.kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "video".into())
}

fn history_value(play: &PlayEvent) -> Value {
    serde_json::to_value(HistoryEvent {
        url: play.url.clone(),
        title: play.title.chars().take(512).collect(),
        creator: play.creator.chars().take(512).collect(),
        kind: play.kind.clone(),
        thumbnail: play.thumbnail.chars().take(512).collect(),
        at: play.at,
        seconds: play.seconds,
        duration: play.duration,
        finished: play.finished,
    })
    .unwrap_or_default()
}

fn resume_pushable(entry: &ResumeEntry) -> bool {
    online(&entry.media) && !(entry.updated_at == 0 && entry.media.title.is_empty())
}

fn position_value(entry: &ResumeEntry) -> Value {
    serde_json::to_value(Position {
        media: entry.media.clone(),
        seconds: entry.ms as f64 / 1000.0,
        duration: entry.duration_ms.map(|d| d as f64 / 1000.0),
        updated_at: entry.updated_at.max(0) as u64,
        finished: entry.finished,
    })
    .unwrap_or_default()
}

fn entry_value(entry: &LocalEntry) -> Value {
    json!({"position": entry.position, "media": slim(&entry.media)})
}

fn follow_value(follow: &Follow) -> Value {
    let mut value = json!({"channel_id": follow.channel_id, "title": follow.title});
    if vidcue_core::http_url(&follow.thumbnail).is_ok() && follow.thumbnail.len() <= 2048 {
        value["thumbnail"] = follow.thumbnail.clone().into();
    }
    value
}

fn title_ok(title: &str) -> Result<String> {
    let title = title.trim();
    ensure!(
        !title.is_empty()
            && title.chars().count() <= MAX_TITLE_CHARS
            && !title.chars().any(char::is_control),
        "Use a title of 1 to 200 characters"
    );
    Ok(title.into())
}

fn sort_entries(entries: &mut [LocalEntry]) {
    entries.sort_by(|a, b| a.position.cmp(&b.position).then(a.id.cmp(&b.id)));
}

/// Portable value of one setting field from a config.
fn setting_value(config: &Value, field: &str) -> Value {
    if field == "download" {
        PORTABLE_DOWNLOAD_FIELDS
            .iter()
            .map(|f| ((*f).to_owned(), config["download"][*f].clone()))
            .collect::<serde_json::Map<_, _>>()
            .into()
    } else {
        config[field].clone()
    }
}

/// Settings and EQ preset writes between two configs.
pub(crate) fn config_ops(meta: &mut SyncMeta, old: &Config, new: &Config) -> Vec<LocalOp> {
    let (Ok(old_value), Ok(new_value)) = (serde_json::to_value(old), serde_json::to_value(new))
    else {
        return Vec::new();
    };
    let mut ops = Vec::new();
    for field in PORTABLE_SETTINGS {
        let value = setting_value(&new_value, field);
        if setting_value(&old_value, field) != value {
            ops.push(personal(meta, "settings", field, Some(value)));
        }
    }
    for (name, gains) in &new.equalizer_custom {
        if old.equalizer_custom.get(name) != Some(gains) {
            ops.push(personal(
                meta,
                "eq_presets",
                name,
                Some(json!({"gains": gains})),
            ));
        }
    }
    for name in old.equalizer_custom.keys() {
        if !new.equalizer_custom.contains_key(name) {
            ops.push(personal(meta, "eq_presets", name, None));
        }
    }
    ops
}

/// Offline playlist definition writes between two libraries.
pub(crate) fn offline_ops(
    meta: &mut SyncMeta,
    old: &[vidcue_core::offline::OfflinePlaylist],
    new: &[vidcue_core::offline::OfflinePlaylist],
) -> Vec<LocalOp> {
    let mut ops = Vec::new();
    for playlist in new {
        let value = serde_json::to_value(playlist).ok();
        let before = old
            .iter()
            .find(|p| p.id == playlist.id)
            .and_then(|p| serde_json::to_value(p).ok());
        if before != value {
            ops.push(personal(meta, "offline_playlists", &playlist.id, value));
        }
    }
    for playlist in old {
        if !new.iter().any(|p| p.id == playlist.id) {
            ops.push(personal(meta, "offline_playlists", &playlist.id, None));
        }
    }
    ops
}

/// `saved` hook: added or removed (by media key).
pub(crate) fn saved_op(meta: &mut SyncMeta, media: &Media, saved: bool) -> Vec<LocalOp> {
    if !online(media) {
        return Vec::new();
    }
    let key = media_key(media);
    let value = saved.then(|| serde_json::to_value(slim(media)).unwrap_or_default());
    vec![personal(meta, "saved", &key, value)]
}

/// `history_record` hook: a new play event, returned with its op.
pub(crate) fn record_play(state: &mut MobileState, media: &Media) -> (Uuid, Vec<LocalOp>) {
    let play = PlayEvent {
        id: Uuid::new_v4(),
        url: media.url.clone(),
        title: media.title.clone(),
        creator: media.creator.clone(),
        kind: kind_name(media),
        thumbnail: media.thumbnail.clone(),
        at: vidcue_sync::now_ms(),
        seconds: 0.0,
        duration: media.duration.map(|d| d as f64),
        finished: false,
    };
    let id = play.id;
    let value = history_value(&play);
    state.plays.push(play);
    if state.plays.len() > MAX_PLAYS {
        let excess = state.plays.len() - MAX_PLAYS;
        state.plays.drain(..excess);
    }
    let clock = next_clock(&mut state.sync_meta);
    (
        id,
        vec![LocalOp::Personal {
            collection: "history",
            key: id.to_string(),
            value: Some(value),
            clock,
        }],
    )
}

/// `position` save: updates the resume entry and the current play event.
pub(crate) fn record_position(
    state: &mut MobileState,
    url: &str,
    ms: u64,
    duration_ms: Option<u64>,
    media: Option<&Media>,
    current_play: Option<Uuid>,
) -> Vec<LocalOp> {
    let media = slim(&media.cloned().unwrap_or_else(|| known_media(state, url)));
    let key = media_key(&media);
    let now = vidcue_sync::now_ms();
    let duration_ms = duration_ms
        .filter(|d| *d > 0)
        .or_else(|| state.resume.get(&key).and_then(|e| e.duration_ms))
        .or(media.duration.map(|d| d.saturating_mul(1000)));
    let play = current_play.and_then(|id| state.plays.iter().position(|p| p.id == id));
    // Android saves 0 once playback reached the end.
    let reached_end = |seconds: f64| {
        duration_ms.is_some_and(|d| seconds * 1000.0 + 2.0 * FINISH_MARGIN_MS as f64 >= d as f64)
    };
    let finished = duration_ms.is_some_and(|d| d > 0 && ms + FINISH_MARGIN_MS >= d)
        || ms == 0 && play.is_some_and(|i| reached_end(state.plays[i].seconds));
    if state.resume.len() >= MAX_RESUME
        && !state.resume.contains_key(&key)
        && let Some(oldest) = state
            .resume
            .iter()
            .min_by_key(|(_, e)| e.updated_at)
            .map(|(k, _)| k.clone())
    {
        state.resume.remove(&oldest);
    }
    let entry = state.resume.entry(key.clone()).or_default();
    if !media.title.is_empty() || entry.media.url.is_empty() {
        entry.media = media.clone();
    }
    entry.ms = ms;
    entry.duration_ms = duration_ms;
    entry.updated_at = now;
    entry.finished = finished;
    let entry = entry.clone();
    let mut ops = Vec::new();
    if resume_pushable(&entry) {
        let clock = next_clock(&mut state.sync_meta);
        ops.push(LocalOp::Personal {
            collection: "positions",
            key,
            value: Some(position_value(&entry)),
            clock,
        });
    }
    if let Some(index) = play {
        let play = &mut state.plays[index];
        let seconds = play.seconds.max(ms as f64 / 1000.0);
        let changed = seconds > play.seconds || finished && !play.finished;
        play.seconds = seconds;
        play.finished |= finished;
        if play.duration.is_none() {
            play.duration = duration_ms.map(|d| d as f64 / 1000.0);
        }
        if changed {
            let value = history_value(play);
            let key = play.id.to_string();
            let clock = next_clock(&mut state.sync_meta);
            ops.push(LocalOp::Personal {
                collection: "history",
                key,
                value: Some(value),
                clock,
            });
        }
    }
    ops
}

/// The saved position for a URL (through its media key).
pub(crate) fn resume_ms(state: &MobileState, url: &str) -> u64 {
    let key = media_key(&known_media(state, url));
    state.resume.get(&key).map_or(0, |e| e.ms)
}

/// `history_clear`: tombstones for every play and resume entry.
pub(crate) fn clear_history(state: &mut MobileState) -> Vec<LocalOp> {
    let mut ops = Vec::new();
    for play in std::mem::take(&mut state.plays) {
        let clock = next_clock(&mut state.sync_meta);
        ops.push(LocalOp::Personal {
            collection: "history",
            key: play.id.to_string(),
            value: None,
            clock,
        });
    }
    for key in std::mem::take(&mut state.resume).into_keys() {
        let clock = next_clock(&mut state.sync_meta);
        ops.push(LocalOp::Personal {
            collection: "positions",
            key,
            value: None,
            clock,
        });
    }
    ops
}

/// The user's filters with what they need from the library, taken without holding locks.
pub(crate) struct FilterSet {
    filters: Vec<Filter>,
    finished: HashSet<String>,
}

impl FilterSet {
    /// Whether any filter hides `media` (§8.5).
    pub(crate) fn hides(&self, media: &Media) -> bool {
        let watched = |m: &Media| self.finished.contains(&media_key(m));
        vidcue_sync::filters::hidden(&self.filters, media, &watched)
    }
}

/// Feeds that filters apply to: Home, Subscriptions, music home, search and related.
/// A channel's own page, playlists, history and local libraries are never filtered.
pub(crate) fn filtered_feed(query: &BrowseRequest, tab: u64) -> bool {
    tab != 3
        && matches!(
            query,
            BrowseRequest::Home
                | BrowseRequest::Subscriptions
                | BrowseRequest::Search(_)
                | BrowseRequest::MusicSearch { .. }
                | BrowseRequest::Related(_)
                | BrowseRequest::Category { .. }
        )
}

fn playlist_json(playlist: &LocalPlaylist) -> Value {
    json!({
        "id": playlist.id,
        "title": playlist.title,
        "count": playlist.entries.len(),
        "shared": playlist.shared_id.is_some(),
        "role": playlist.role,
        "cover": playlist.entries.first().map(|e| &e.media),
    })
}

impl Engine {
    /// The filters to apply to feeds, or `None` when there are none.
    pub(crate) fn filter_set(&self) -> Option<FilterSet> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.filters.is_empty() {
            return None;
        }
        let finished = if state.filters.iter().any(|f| f.kind == "watched") {
            state
                .resume
                .iter()
                .filter(|(_, e)| e.finished)
                .map(|(k, _)| k.clone())
                .collect()
        } else {
            HashSet::new()
        };
        Some(FilterSet {
            filters: state.filters.clone(),
            finished,
        })
    }

    /// Queues an imported YouTube session as an encrypted secret when signed in.
    pub(crate) fn hook_session(&self, session: &Session) {
        if self.sync.engine().is_none() {
            return;
        }
        if let Ok(clock) =
            self.mutate(|state| Ok(stamp(&mut state.sync_meta, "secrets", "youtube")))
        {
            self.sync.push_session(session, clock);
        }
    }

    /// Installs a provider session that arrived through sync, through the same steps as
    /// a cookie import. An identical session (our own write coming back) is a no-op.
    pub(crate) fn install_synced_session(&self, session: Session) -> Result<()> {
        let _commit = self
            .settings_commit
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = self
            .session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some(current) = current
            && serde_json::to_value(&*current)? == serde_json::to_value(&session)?
        {
            return Ok(());
        }
        self.begin_account_change();
        let notice = account::save(&self.root, &session)?;
        *self
            .account_notice
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = notice;
        let session = Arc::new(session);
        *self
            .session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(session.clone());
        self.resolver.clear();
        self.activate_context(None);
        let config = self
            .config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        *self
            .browser
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Browser::new(
            vidcue_providers::http_client(config.proxy.as_deref())?,
            Some(session),
            config.account_index,
        );
        self.sync.emit("account_updated", "");
        Ok(())
    }

    /// Hands local writes to the sync engine (no-op while signed out).
    pub(crate) fn sync_ops(&self, ops: Vec<LocalOp>) {
        self.sync.enqueue(ops);
    }

    /// Local library ops of §8.1 (playlists, follows, filters).
    pub(crate) fn library_op(&self, op: &str, r: &Value) -> Result<Value> {
        let uuid = |field: &str| -> Result<Uuid> {
            r[field]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
                .with_context(|| format!("Missing {field}"))
        };
        let read = || {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        };
        match op {
            "playlists" => Ok(json!(read().playlists.iter().map(playlist_json).collect::<Vec<_>>())),
            "playlist_entries" => {
                let id = uuid("id")?;
                let state = read();
                let playlist = state
                    .playlists
                    .iter()
                    .find(|p| p.id == id)
                    .context("This playlist no longer exists")?;
                Ok(json!(
                    playlist
                        .entries
                        .iter()
                        .map(|e| json!({"entry_id": e.id, "media": e.media}))
                        .collect::<Vec<_>>()
                ))
            }
            "playlist_create" => {
                let title = title_ok(r["title"].as_str().unwrap_or_default())?;
                let media: Option<Media> = r
                    .get("media")
                    .filter(|m| !m.is_null())
                    .map(|m| serde_json::from_value(m.clone()))
                    .transpose()?;
                if let Some(media) = &media {
                    ensure!(online(media), "Only online media can be added to a playlist");
                    ensure!(media.retained_bytes() <= 256 * 1024, "Media is too large");
                }
                let (value, ops) = self.mutate(|state| {
                    ensure!(state.playlists.len() < MAX_PLAYLISTS, "Too many playlists");
                    let id = Uuid::new_v4();
                    let created_at = vidcue_sync::now_ms();
                    let mut playlist = LocalPlaylist {
                        id,
                        title,
                        created_at,
                        shared_id: None,
                        role: None,
                        entries: Vec::new(),
                    };
                    let meta = &mut state.sync_meta;
                    let mut ops = vec![personal(
                        meta,
                        "playlists",
                        &id.to_string(),
                        Some(json!({"title": playlist.title, "created_at": created_at})),
                    )];
                    if let Some(media) = media {
                        let entry = LocalEntry {
                            id: Uuid::new_v4(),
                            position: fractional::between(None, None).unwrap_or_else(|| "V".into()),
                            media: slim(&media),
                        };
                        ops.push(personal(
                            meta,
                            "playlist_entries",
                            &format!("{id}/{}", entry.id),
                            Some(entry_value(&entry)),
                        ));
                        playlist.entries.push(entry);
                    }
                    let value = playlist_json(&playlist);
                    state.playlists.push(playlist);
                    Ok((value, ops))
                })?;
                self.sync_ops(ops);
                Ok(value)
            }
            "playlist_rename" => {
                let id = uuid("id")?;
                let title = title_ok(r["title"].as_str().unwrap_or_default())?;
                let (value, shared, ops) = self.mutate(|state| {
                    let meta = &mut state.sync_meta;
                    let playlist = state
                        .playlists
                        .iter_mut()
                        .find(|p| p.id == id)
                        .context("This playlist no longer exists")?;
                    ensure!(
                        playlist.role.as_deref() != Some("member"),
                        "Only the owner can rename a shared playlist"
                    );
                    playlist.title = title.clone();
                    let ops = if playlist.shared_id.is_none() {
                        vec![personal(
                            meta,
                            "playlists",
                            &id.to_string(),
                            Some(json!({"title": playlist.title, "created_at": playlist.created_at})),
                        )]
                    } else {
                        Vec::new()
                    };
                    Ok((playlist_json(playlist), playlist.shared_id, ops))
                })?;
                self.sync_ops(ops);
                if let Some(shared) = shared {
                    self.sync.rename_shared(shared, title);
                }
                Ok(value)
            }
            "playlist_delete" => {
                let id = uuid("id")?;
                let ops = self.mutate(|state| {
                    let index = state
                        .playlists
                        .iter()
                        .position(|p| p.id == id)
                        .context("This playlist no longer exists")?;
                    ensure!(
                        state.playlists[index].shared_id.is_none(),
                        "Leave the shared playlist first"
                    );
                    let playlist = state.playlists.remove(index);
                    let meta = &mut state.sync_meta;
                    let mut ops = vec![personal(meta, "playlists", &id.to_string(), None)];
                    for entry in &playlist.entries {
                        ops.push(personal(
                            meta,
                            "playlist_entries",
                            &format!("{id}/{}", entry.id),
                            None,
                        ));
                    }
                    Ok(ops)
                })?;
                self.sync_ops(ops);
                Ok(json!({}))
            }
            "playlist_add" => {
                let id = uuid("id")?;
                let media: Media = serde_json::from_value(r["media"].clone())?;
                ensure!(online(&media), "Only online media can be added to a playlist");
                ensure!(media.retained_bytes() <= 256 * 1024, "Media is too large");
                let (entry_id, ops) = self.mutate(|state| {
                    let playlist = state
                        .playlists
                        .iter_mut()
                        .find(|p| p.id == id)
                        .context("This playlist no longer exists")?;
                    ensure!(
                        playlist.entries.len() < MAX_PLAYLIST_ENTRIES,
                        "This playlist is full"
                    );
                    let last = playlist.entries.last().map(|e| e.position.clone());
                    let entry = LocalEntry {
                        id: Uuid::new_v4(),
                        position: fractional::between(last.as_deref(), None)
                            .context("This playlist needs reordering")?,
                        media: slim(&media),
                    };
                    let entry_id = entry.id;
                    playlist.entries.push(entry);
                    let mut ops = Vec::new();
                    let playlist = playlist.clone();
                    entry_ops(&mut state.sync_meta, &playlist, &[entry_id], &mut ops);
                    Ok((entry_id, ops))
                })?;
                self.sync_ops(ops);
                Ok(json!({"entry_id": entry_id}))
            }
            "playlist_remove" => {
                let id = uuid("id")?;
                let entry = uuid("entry_id")?;
                let ops = self.mutate(|state| {
                    let playlist = state
                        .playlists
                        .iter_mut()
                        .find(|p| p.id == id)
                        .context("This playlist no longer exists")?;
                    let before = playlist.entries.len();
                    playlist.entries.retain(|e| e.id != entry);
                    ensure!(playlist.entries.len() < before, "This entry was removed");
                    let shared = playlist.shared_id;
                    let clock = stamp(&mut state.sync_meta, "playlist_entries", &format!("{id}/{entry}"));
                    state
                        .sync_meta
                        .clocks
                        .remove(&register("playlist_entries", &format!("{id}/{entry}")));
                    Ok(vec![match shared {
                        Some(shared) => LocalOp::Shared {
                            playlist: shared,
                            entry,
                            value: None,
                            clock,
                        },
                        None => LocalOp::Personal {
                            collection: "playlist_entries",
                            key: format!("{id}/{entry}"),
                            value: None,
                            clock,
                        },
                    }])
                })?;
                self.sync_ops(ops);
                Ok(json!({}))
            }
            "playlist_move" => {
                let id = uuid("id")?;
                let entry = uuid("entry_id")?;
                let after = r["after_entry_id"]
                    .as_str()
                    .map(|s| Uuid::parse_str(s).context("Invalid entry"))
                    .transpose()?;
                let ops = self.mutate(|state| {
                    let playlist = state
                        .playlists
                        .iter_mut()
                        .find(|p| p.id == id)
                        .context("This playlist no longer exists")?;
                    let from = playlist
                        .entries
                        .iter()
                        .position(|e| e.id == entry)
                        .context("This entry was removed")?;
                    let moving = playlist.entries.remove(from);
                    let to = match after {
                        None => 0,
                        Some(after) => {
                            playlist
                                .entries
                                .iter()
                                .position(|e| e.id == after)
                                .context("The target entry was removed")?
                                + 1
                        }
                    };
                    let lo = to.checked_sub(1).map(|i| playlist.entries[i].position.clone());
                    let hi = playlist.entries.get(to).map(|e| e.position.clone());
                    let mut changed = vec![moving.id];
                    let mut moving = moving;
                    match fractional::between(lo.as_deref(), hi.as_deref()) {
                        Some(position) => {
                            moving.position = position;
                            playlist.entries.insert(to, moving);
                        }
                        None => {
                            // No room left: respace the whole playlist.
                            playlist.entries.insert(to, moving);
                            let spread = fractional::spread(playlist.entries.len());
                            for (entry, position) in playlist.entries.iter_mut().zip(spread) {
                                entry.position = position;
                            }
                            changed = playlist.entries.iter().map(|e| e.id).collect();
                        }
                    }
                    let playlist = playlist.clone();
                    let mut ops = Vec::new();
                    entry_ops(&mut state.sync_meta, &playlist, &changed, &mut ops);
                    Ok(ops)
                })?;
                self.sync_ops(ops);
                Ok(json!({}))
            }
            "follows" => Ok(json!(
                read()
                    .follows
                    .iter()
                    .map(|f| json!({"channel_id": f.channel_id, "title": f.title, "thumbnail": f.thumbnail}))
                    .collect::<Vec<_>>()
            )),
            "follow_toggle" => {
                let channel = r["channel_id"].as_str().context("Missing channel")?.trim();
                ensure!(
                    !channel.is_empty() && channel.len() <= 64 && !channel.contains(char::is_control),
                    "Invalid channel"
                );
                let title = r["title"].as_str().unwrap_or_default().trim();
                let title = if title.is_empty() { channel } else { title };
                let title: String = title.chars().take(MAX_TITLE_CHARS).collect();
                let thumbnail = r["thumbnail"].as_str().unwrap_or_default().to_owned();
                let (following, ops) = self.mutate(|state| {
                    let meta = &mut state.sync_meta;
                    if let Some(i) = state.follows.iter().position(|f| f.channel_id == channel) {
                        state.follows.remove(i);
                        return Ok((false, vec![personal(meta, "subscriptions", channel, None)]));
                    }
                    ensure!(state.follows.len() < MAX_FOLLOWS, "Too many followed channels");
                    let follow = Follow {
                        channel_id: channel.into(),
                        title,
                        thumbnail,
                        at: vidcue_sync::now_ms(),
                    };
                    let op = personal(meta, "subscriptions", channel, Some(follow_value(&follow)));
                    state.follows.push(follow);
                    Ok((true, vec![op]))
                })?;
                self.sync_ops(ops);
                Ok(json!({"following": following}))
            }
            "filters" => Ok(json!(read().filters)),
            "filter_add" => {
                let filter = Filter::new(
                    Uuid::new_v4(),
                    r["kind"].as_str().unwrap_or_default(),
                    r["value"].as_str(),
                )?;
                let ops = self.mutate(|state| {
                    ensure!(state.filters.len() < MAX_FILTERS, "Too many filters");
                    // Toggle-like kinds exist once.
                    if filter.value.is_none() {
                        ensure!(
                            !state.filters.iter().any(|f| f.kind == filter.kind),
                            "This filter is already on"
                        );
                    }
                    let op = personal(
                        &mut state.sync_meta,
                        "filters",
                        &filter.id.to_string(),
                        Some(filter.register_value()),
                    );
                    state.filters.push(filter.clone());
                    Ok(vec![op])
                })?;
                self.sync_ops(ops);
                Ok(json!(filter))
            }
            "filter_remove" => {
                let id = uuid("id")?;
                let ops = self.mutate(|state| {
                    let before = state.filters.len();
                    state.filters.retain(|f| f.id != id);
                    ensure!(state.filters.len() < before, "This filter was removed");
                    Ok(vec![personal(&mut state.sync_meta, "filters", &id.to_string(), None)])
                })?;
                self.sync_ops(ops);
                Ok(json!({}))
            }
            _ => anyhow::bail!("Unknown mobile operation"),
        }
    }

    /// Ops that put a whole local playlist on the server (first share).
    pub(crate) fn shared_entry_ops(&self, playlist: Uuid) -> Result<Vec<LocalOp>> {
        self.mutate(|state| {
            let local = state
                .playlists
                .iter()
                .find(|p| p.id == playlist)
                .cloned()
                .context("This playlist no longer exists")?;
            let ids: Vec<Uuid> = local.entries.iter().map(|e| e.id).collect();
            let mut ops = Vec::new();
            entry_ops(&mut state.sync_meta, &local, &ids, &mut ops);
            Ok(ops)
        })
    }

    /// Marks a local playlist shared (`owner`) and tombstones its personal copy.
    pub(crate) fn mark_shared(&self, playlist: Uuid) -> Result<()> {
        let ops = self.mutate(|state| {
            let meta = &mut state.sync_meta;
            let local = state
                .playlists
                .iter_mut()
                .find(|p| p.id == playlist)
                .context("This playlist no longer exists")?;
            if local.shared_id.is_some() {
                return Ok(Vec::new());
            }
            local.shared_id = Some(playlist);
            local.role = Some("owner".into());
            // The header and entries now live on the shared playlist.
            let mut ops = vec![personal(meta, "playlists", &playlist.to_string(), None)];
            for entry in &local.entries {
                ops.push(personal(
                    meta,
                    "playlist_entries",
                    &format!("{playlist}/{}", entry.id),
                    None,
                ));
            }
            Ok(ops)
        })?;
        self.sync_ops(ops);
        Ok(())
    }

    /// After leaving: the owner keeps a normal playlist, a member drops the copy.
    pub(crate) fn unshare_local(&self, playlist: Uuid) -> Result<()> {
        let ops = self.mutate(|state| {
            let Some(index) = state.playlists.iter().position(|p| p.id == playlist) else {
                return Ok(Vec::new());
            };
            if state.playlists[index].role.as_deref() != Some("owner") {
                state.playlists.remove(index);
                return Ok(Vec::new());
            }
            let local = &mut state.playlists[index];
            local.shared_id = None;
            local.role = None;
            let local = local.clone();
            let meta = &mut state.sync_meta;
            let mut ops = vec![personal(
                meta,
                "playlists",
                &playlist.to_string(),
                Some(json!({"title": local.title, "created_at": local.created_at})),
            )];
            let ids: Vec<Uuid> = local.entries.iter().map(|e| e.id).collect();
            entry_ops(meta, &local, &ids, &mut ops);
            Ok(ops)
        })?;
        self.sync_ops(ops);
        Ok(())
    }

    /// Deletes local sync stores on "sign out and forget". Saved media, offline
    /// playlists, settings, the queue and downloads stay.
    pub(crate) fn forget_synced_library(&self) -> Result<()> {
        self.mutate(|state| {
            state.playlists.clear();
            state.follows.clear();
            state.filters.clear();
            state.plays.clear();
            state.resume.clear();
            state.sync_meta = SyncMeta::default();
            Ok(())
        })
    }
}

/// Entry writes for `ids` of a playlist: shared playlists go to the shared outbox.
fn entry_ops(meta: &mut SyncMeta, playlist: &LocalPlaylist, ids: &[Uuid], ops: &mut Vec<LocalOp>) {
    for entry in playlist.entries.iter().filter(|e| ids.contains(&e.id)) {
        let key = format!("{}/{}", playlist.id, entry.id);
        let value = Some(entry_value(entry));
        match playlist.shared_id {
            Some(shared) => {
                let clock = stamp(meta, "playlist_entries", &key);
                ops.push(LocalOp::Shared {
                    playlist: shared,
                    entry: entry.id,
                    value,
                    clock,
                });
            }
            None => ops.push(personal(meta, "playlist_entries", &key, value)),
        }
    }
}

/// The engine's library as `LocalStores` for one sync run.
pub(crate) struct Stores<'a>(pub &'a Engine);

impl Stores<'_> {
    fn apply_state(&self, change: &Change) -> Result<()> {
        let engine = self.0;
        engine.mutate(|state| {
            let reg = register(&change.collection, &change.key);
            match change.value {
                Some(_) => state.sync_meta.clocks.insert(reg, change.clock),
                None => state.sync_meta.clocks.remove(&reg),
            };
            apply_to_state(state, change)
        })
    }

    fn apply_setting(&self, change: &Change) -> Result<()> {
        let engine = self.0;
        let Some(value) = &change.value else {
            // A deleted setting keeps the local value.
            return Ok(());
        };
        let _commit = engine
            .settings_commit
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = engine
            .config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut draft = serde_json::to_value(&current)?;
        match change.collection.as_str() {
            "settings" => {
                ensure!(
                    PORTABLE_SETTINGS.contains(&change.key.as_str()),
                    "Not a synced setting"
                );
                if change.key == "download" {
                    for (field, v) in value.as_object().context("Invalid download setting")? {
                        if PORTABLE_DOWNLOAD_FIELDS.contains(&field.as_str()) {
                            draft["download"][field] = v.clone();
                        }
                    }
                } else {
                    draft[&change.key] = value.clone();
                }
            }
            _ => {
                let gains = value["gains"].clone();
                draft["equalizer_custom"][&change.key] = gains;
            }
        }
        let mut next: Config = serde_json::from_value(draft)?;
        next.offline = current.offline;
        next.validate()?;
        if serde_json::to_value(&next)? == serde_json::to_value(&current)? {
            return Ok(());
        }
        engine.commit_config(next)?;
        engine.mutate(|state| {
            state
                .sync_meta
                .clocks
                .insert(register(&change.collection, &change.key), change.clock);
            Ok(())
        })
    }

    fn remove_preset(&self, name: &str) -> Result<()> {
        let engine = self.0;
        let _commit = engine
            .settings_commit
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut next = engine
            .config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if next.equalizer_custom.remove(name).is_none() {
            return Ok(());
        }
        if next.equalizer == name {
            next.equalizer = "off".into();
        }
        next.validate()?;
        engine.commit_config(next)
    }

    fn install_secret(&self, change: &Change) -> Result<()> {
        if change.key != "youtube" {
            return Ok(());
        }
        let Some(value) = &change.value else {
            return Ok(());
        };
        let session: Session = serde_json::from_value(value.clone())?;
        self.0.install_synced_session(session)
    }
}

fn apply_to_state(state: &mut MobileState, change: &Change) -> Result<()> {
    match change.collection.as_str() {
        "positions" => match &change.value {
            Some(value) => {
                let position: Position = serde_json::from_value(value.clone())?;
                let media = slim(&position.media);
                let key = media_key(&media);
                let updated_at = position.updated_at as i64;
                if state
                    .resume
                    .get(&key)
                    .is_none_or(|local| updated_at >= local.updated_at)
                {
                    state.resume.insert(
                        key,
                        ResumeEntry {
                            media,
                            ms: (position.seconds.max(0.0) * 1000.0) as u64,
                            duration_ms: position.duration.map(|d| (d.max(0.0) * 1000.0) as u64),
                            updated_at,
                            finished: position.finished,
                        },
                    );
                }
            }
            None => {
                state.resume.remove(&change.key);
            }
        },
        "history" => {
            let id = Uuid::parse_str(&change.key)?;
            match &change.value {
                Some(value) => {
                    let event: HistoryEvent = serde_json::from_value(value.clone())?;
                    vidcue_core::http_url(&event.url)?;
                    let newest = state.plays.iter().map(|p| p.at).max().unwrap_or(0);
                    let play = PlayEvent {
                        id,
                        url: event.url,
                        title: event.title,
                        creator: event.creator,
                        kind: event.kind,
                        thumbnail: event.thumbnail,
                        at: event.at,
                        seconds: event.seconds,
                        duration: event.duration,
                        finished: event.finished,
                    };
                    state.plays.retain(|p| p.id != id);
                    let at = state.plays.partition_point(|p| p.at <= play.at);
                    state.plays.insert(at, play.clone());
                    if state.plays.len() > MAX_PLAYS {
                        let excess = state.plays.len() - MAX_PLAYS;
                        state.plays.drain(..excess);
                    }
                    if play.at >= newest {
                        let media = Media {
                            url: play.url.clone(),
                            title: play.title,
                            creator: play.creator,
                            thumbnail: play.thumbnail,
                            kind: serde_json::from_value(Value::String(play.kind))
                                .unwrap_or_default(),
                            id: path_video_id(&play.url),
                            ..Default::default()
                        };
                        if state.recent.is_empty() {
                            state.recent = state.playback.items.clone();
                        }
                        state.recent.retain(|m| m.url != media.url);
                        state.recent.push(media);
                        while state.recent.len() > 100 {
                            state.recent.remove(0);
                        }
                    }
                }
                None => state.plays.retain(|p| p.id != id),
            }
        }
        "saved" => match &change.value {
            Some(value) => {
                let media: Media = serde_json::from_value(value.clone())?;
                if !state.saved.iter().any(|m| media_key(m) == change.key)
                    && state.saved.len() < MAX_SAVED
                {
                    state.saved.push(media);
                }
            }
            None => state.saved.retain(|m| media_key(m) != change.key),
        },
        "playlists" => {
            let id = Uuid::parse_str(&change.key)?;
            match &change.value {
                Some(value) => {
                    let title = title_ok(value["title"].as_str().unwrap_or_default())?;
                    if let Some(playlist) = state.playlists.iter_mut().find(|p| p.id == id) {
                        if playlist.shared_id.is_none() {
                            playlist.title = title;
                        }
                    } else {
                        ensure!(state.playlists.len() < MAX_PLAYLISTS, "Too many playlists");
                        let mut entries = state
                            .sync_meta
                            .hidden_entries
                            .remove(&id)
                            .unwrap_or_default();
                        sort_entries(&mut entries);
                        state.playlists.push(LocalPlaylist {
                            id,
                            title,
                            created_at: value["created_at"].as_i64().unwrap_or(change.clock),
                            shared_id: None,
                            role: None,
                            entries,
                        });
                    }
                }
                None => {
                    state
                        .playlists
                        .retain(|p| p.id != id || p.shared_id.is_some());
                    state.sync_meta.hidden_entries.remove(&id);
                }
            }
        }
        "playlist_entries" => {
            let (playlist, entry) = change.key.split_once('/').context("Invalid entry key")?;
            let (playlist, entry) = (Uuid::parse_str(playlist)?, Uuid::parse_str(entry)?);
            let local = state
                .playlists
                .iter_mut()
                .find(|p| p.id == playlist && p.shared_id.is_none());
            match (&change.value, local) {
                (Some(value), local) => {
                    let position = value["position"].as_str().unwrap_or_default();
                    ensure!(fractional::valid(position), "Invalid entry position");
                    let new = LocalEntry {
                        id: entry,
                        position: position.into(),
                        media: serde_json::from_value(value["media"].clone())?,
                    };
                    let entries = match local {
                        Some(local) => &mut local.entries,
                        None => state.sync_meta.hidden_entries.entry(playlist).or_default(),
                    };
                    entries.retain(|e| e.id != entry);
                    ensure!(
                        entries.len() < MAX_PLAYLIST_ENTRIES,
                        "This playlist is full"
                    );
                    entries.push(new);
                    sort_entries(entries);
                }
                (None, Some(local)) => local.entries.retain(|e| e.id != entry),
                (None, None) => {
                    if let Some(hidden) = state.sync_meta.hidden_entries.get_mut(&playlist) {
                        hidden.retain(|e| e.id != entry);
                    }
                }
            }
        }
        "offline_playlists" => {
            let mut next = state.offline_playlists.clone();
            match &change.value {
                Some(value) => {
                    let playlist: vidcue_core::offline::OfflinePlaylist =
                        serde_json::from_value(value.clone())?;
                    playlist.validate()?;
                    ensure!(playlist.id == change.key, "Invalid offline playlist key");
                    match next.iter_mut().find(|p| p.id == playlist.id) {
                        Some(existing) => *existing = playlist,
                        None => next.push(playlist),
                    }
                }
                None => next.retain(|p| p.id != change.key),
            }
            vidcue_core::offline::validate_library(&next)?;
            state.offline_playlists = next;
        }
        "subscriptions" => match &change.value {
            Some(value) => {
                let follow = Follow {
                    channel_id: change.key.clone(),
                    title: value["title"].as_str().unwrap_or_default().into(),
                    thumbnail: value["thumbnail"].as_str().unwrap_or_default().into(),
                    at: change.clock,
                };
                match state
                    .follows
                    .iter_mut()
                    .find(|f| f.channel_id == change.key)
                {
                    Some(existing) => {
                        existing.title = follow.title;
                        existing.thumbnail = follow.thumbnail;
                    }
                    None => {
                        ensure!(
                            state.follows.len() < MAX_FOLLOWS,
                            "Too many followed channels"
                        );
                        state.follows.push(follow);
                    }
                }
            }
            None => state.follows.retain(|f| f.channel_id != change.key),
        },
        "filters" => {
            let id = Uuid::parse_str(&change.key)?;
            state.filters.retain(|f| f.id != id);
            if let Some(value) = &change.value {
                let filter = Filter::new(
                    id,
                    value["kind"].as_str().unwrap_or_default(),
                    value["value"].as_str(),
                )?;
                ensure!(state.filters.len() < MAX_FILTERS, "Too many filters");
                state.filters.push(filter);
            }
        }
        _ => {}
    }
    Ok(())
}

impl vidcue_sync::LocalStores for Stores<'_> {
    fn apply(&self, change: &Change) -> Result<()> {
        match change.collection.as_str() {
            "settings" | "eq_presets" if change.value.is_some() => self.apply_setting(change),
            "eq_presets" => self.remove_preset(&change.key),
            "settings" => Ok(()),
            "secrets" => self.install_secret(change),
            _ => self.apply_state(change),
        }
    }

    fn snapshot(&self, collection: &str) -> Result<Vec<(String, Value, i64)>> {
        let engine = self.0;
        if collection == "settings" || collection == "eq_presets" {
            let config = engine
                .config
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let meta = engine
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .sync_meta
                .clone();
            if collection == "eq_presets" {
                return Ok(config
                    .equalizer_custom
                    .iter()
                    .map(|(name, gains)| {
                        (
                            name.clone(),
                            json!({"gains": gains}),
                            clock_of(&meta, "eq_presets", name),
                        )
                    })
                    .collect());
            }
            let value = serde_json::to_value(&config)?;
            return Ok(PORTABLE_SETTINGS
                .iter()
                .map(|field| {
                    (
                        (*field).to_owned(),
                        setting_value(&value, field),
                        clock_of(&meta, "settings", field),
                    )
                })
                .collect());
        }
        if collection == "secrets" {
            let session = engine
                .session
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let clock = clock_of(
                &engine
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .sync_meta,
                "secrets",
                "youtube",
            );
            return Ok(session
                .map(|s| serde_json::to_value(&*s))
                .transpose()?
                .map(|value| vec![("youtube".to_owned(), value, clock)])
                .unwrap_or_default());
        }
        let state = engine
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let meta = &state.sync_meta;
        Ok(match collection {
            "positions" => state
                .resume
                .iter()
                .filter(|(_, e)| resume_pushable(e))
                .map(|(k, e)| (k.clone(), position_value(e), e.updated_at))
                .collect(),
            "history" => state
                .plays
                .iter()
                .filter(|p| vidcue_core::http_url(&p.url).is_ok())
                .map(|p| (p.id.to_string(), history_value(p), p.at))
                .collect(),
            "saved" => state
                .saved
                .iter()
                .filter(|m| online(m))
                .map(|m| {
                    let key = media_key(m);
                    let clock = clock_of(meta, "saved", &key);
                    (
                        key,
                        serde_json::to_value(slim(m)).unwrap_or_default(),
                        clock,
                    )
                })
                .collect(),
            "playlists" => state
                .playlists
                .iter()
                .filter(|p| p.shared_id.is_none())
                .map(|p| {
                    let key = p.id.to_string();
                    let clock = clock_of(meta, "playlists", &key);
                    (
                        key,
                        json!({"title": p.title, "created_at": p.created_at}),
                        clock,
                    )
                })
                .collect(),
            "playlist_entries" => state
                .playlists
                .iter()
                .filter(|p| p.shared_id.is_none())
                .flat_map(|p| {
                    p.entries.iter().filter(|e| online(&e.media)).map(move |e| {
                        let key = format!("{}/{}", p.id, e.id);
                        let clock = clock_of(meta, "playlist_entries", &key);
                        (key, entry_value(e), clock)
                    })
                })
                .collect(),
            "offline_playlists" => state
                .offline_playlists
                .iter()
                .map(|p| {
                    (
                        p.id.clone(),
                        serde_json::to_value(p).unwrap_or_default(),
                        clock_of(meta, "offline_playlists", &p.id),
                    )
                })
                .collect(),
            "subscriptions" => state
                .follows
                .iter()
                .map(|f| (f.channel_id.clone(), follow_value(f), f.at))
                .collect(),
            "filters" => state
                .filters
                .iter()
                .map(|f| {
                    let key = f.id.to_string();
                    let clock = clock_of(meta, "filters", &key);
                    (key, f.register_value(), clock)
                })
                .collect(),
            _ => Vec::new(),
        })
    }

    fn applied(&self, collections: &[String]) {
        let mut kinds: Vec<(&str, &str)> = Vec::new();
        for collection in collections {
            let event = match collection.as_str() {
                "positions" | "history" => ("history_updated", ""),
                "offline_playlists" => ("offline_playlists_changed", ""),
                "saved" => ("library_changed", "saved"),
                "playlists" | "playlist_entries" => ("library_changed", "playlists"),
                "subscriptions" => ("library_changed", "follows"),
                "filters" => ("library_changed", "filters"),
                "settings" | "eq_presets" => ("config_changed", ""),
                _ => continue,
            };
            if !kinds.contains(&event) {
                kinds.push(event);
            }
        }
        for (kind, text) in kinds {
            self.0.sync.emit(kind, text);
        }
    }

    fn shared_playlists(&self, playlists: &[SharedPlaylist]) -> Result<()> {
        let mut changed = false;
        self.0.mutate(|state| {
            let ids: HashSet<Uuid> = playlists.iter().map(|p| p.id).collect();
            for playlist in playlists {
                match state.playlists.iter_mut().find(|p| p.id == playlist.id) {
                    Some(local) => {
                        if local.title != playlist.title
                            || local.shared_id != Some(playlist.id)
                            || local.role.as_deref() != Some(playlist.role.as_str())
                        {
                            local.title = playlist.title.clone();
                            local.shared_id = Some(playlist.id);
                            local.role = Some(playlist.role.clone());
                            changed = true;
                        }
                    }
                    None => {
                        state.playlists.push(LocalPlaylist {
                            id: playlist.id,
                            title: playlist.title.clone(),
                            created_at: playlist.updated_at,
                            shared_id: Some(playlist.id),
                            role: Some(playlist.role.clone()),
                            entries: Vec::new(),
                        });
                        changed = true;
                    }
                }
            }
            // Playlists this user no longer belongs to: members lose the copy, an
            // owner's other devices keep it as a normal playlist.
            let gone: Vec<Uuid> = state
                .playlists
                .iter()
                .filter(|p| p.shared_id.is_some_and(|id| !ids.contains(&id)))
                .map(|p| p.id)
                .collect();
            for id in gone {
                changed = true;
                let index = state
                    .playlists
                    .iter()
                    .position(|p| p.id == id)
                    .expect("listed");
                if state.playlists[index].role.as_deref() == Some("owner") {
                    let local = &mut state.playlists[index];
                    local.shared_id = None;
                    local.role = None;
                } else {
                    state.playlists.remove(index);
                }
            }
            Ok(())
        })?;
        if changed {
            self.0.sync.emit("library_changed", "playlists");
        }
        Ok(())
    }

    fn apply_shared(&self, playlist: &SharedPlaylist, changes: &[SharedChange]) -> Result<()> {
        self.0.mutate(|state| {
            let Some(local) = state
                .playlists
                .iter_mut()
                .find(|p| p.shared_id == Some(playlist.id))
            else {
                return Ok(());
            };
            local.title = playlist.title.clone();
            for change in changes {
                local.entries.retain(|e| e.id != change.entry_id);
                if let Some(value) = &change.value {
                    let position = value["position"].as_str().unwrap_or_default();
                    let Ok(media) = serde_json::from_value::<Media>(value["media"].clone()) else {
                        continue;
                    };
                    if fractional::valid(position) && local.entries.len() < MAX_PLAYLIST_ENTRIES {
                        local.entries.push(LocalEntry {
                            id: change.entry_id,
                            position: position.into(),
                            media,
                        });
                    }
                }
            }
            sort_entries(&mut local.entries);
            Ok(())
        })?;
        self.0
            .sync
            .emit("shared_playlist_changed", &playlist.id.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(id: &str) -> Media {
        Media {
            url: format!("https://www.youtube.com/watch?v={id}"),
            title: format!("Title {id}"),
            description: "long".into(),
            ..Default::default()
        }
    }

    #[test]
    fn migration_keys_positions_by_media_and_fills_titles() {
        let mut state = MobileState::default();
        state.saved.push(video("abcdefghijk"));
        state
            .positions
            .insert("https://www.youtube.com/watch?v=abcdefghijk".into(), 1234);
        state
            .positions
            .insert("https://www.youtube.com/shorts/lmnopqrstuv".into(), 99);
        state.positions.insert("https://radio.example/a".into(), 5);
        assert!(migrate(&mut state));
        assert!(state.positions.is_empty());
        let entry = &state.resume["youtube:abcdefghijk"];
        assert_eq!((entry.ms, entry.updated_at), (1234, 0));
        assert_eq!(entry.media.title, "Title abcdefghijk");
        assert!(entry.media.description.is_empty());
        assert_eq!(state.resume["youtube:lmnopqrstuv"].ms, 99);
        assert!(state.resume.contains_key("https://radio.example/a"));
        // Untitled migrated entries are never pushed; titled ones are.
        assert!(!resume_pushable(&state.resume["youtube:lmnopqrstuv"]));
        assert!(resume_pushable(&state.resume["youtube:abcdefghijk"]));
        assert!(!migrate(&mut state));
    }

    #[test]
    fn positions_update_resume_and_the_current_play() {
        let mut state = MobileState::default();
        let media = Media {
            duration: Some(100),
            ..video("abcdefghijk")
        };
        let (play, ops) = record_play(&mut state, &media);
        assert!(matches!(
            &ops[0],
            LocalOp::Personal {
                collection: "history",
                ..
            }
        ));
        let ops = record_position(
            &mut state,
            &media.url,
            40_000,
            None,
            Some(&media),
            Some(play),
        );
        assert_eq!(ops.len(), 2);
        assert_eq!(state.plays[0].seconds, 40.0);
        assert!(!state.plays[0].finished);
        let ops = record_position(
            &mut state,
            &media.url,
            96_000,
            Some(100_000),
            None,
            Some(play),
        );
        assert!(state.plays[0].finished);
        assert!(state.resume["youtube:abcdefghijk"].finished);
        let LocalOp::Personal { value, .. } = &ops[0] else {
            panic!()
        };
        assert_eq!(value.as_ref().unwrap()["finished"], true);
        assert_eq!(value.as_ref().unwrap()["seconds"], 96.0);
        // A lower position never lowers the listened seconds.
        record_position(&mut state, &media.url, 10_000, None, None, Some(play));
        assert_eq!(state.plays[0].seconds, 96.0);
        assert_eq!(resume_ms(&state, &media.url), 10_000);
        // Local files are not pushed.
        let local = Media {
            url: "file:///sdcard/a.mp3".into(),
            title: "x".into(),
            ..Default::default()
        };
        assert!(record_position(&mut state, &local.url, 5, None, Some(&local), None).is_empty());
        let clock = state.sync_meta.last_clock;
        assert!(clock > 0);
        let ops = clear_history(&mut state);
        assert!(
            ops.iter()
                .all(|op| matches!(op, LocalOp::Personal { value: None, .. }))
        );
        assert!(state.plays.is_empty() && state.resume.is_empty());
    }

    #[test]
    fn config_diff_sends_only_portable_fields() {
        let mut meta = SyncMeta::default();
        let old = Config::default();
        let mut new = old.clone();
        new.theme = "vidcue-light".into();
        new.proxy = Some("http://127.0.0.1:1".into());
        new.download.quality = "720".into();
        new.download.directory = "/elsewhere".into();
        new.equalizer_custom.insert("mine".into(), vec![1.0; 10]);
        let ops = config_ops(&mut meta, &old, &new);
        let keys: Vec<String> = ops
            .iter()
            .map(|op| match op {
                LocalOp::Personal {
                    collection, key, ..
                } => format!("{collection}/{key}"),
                LocalOp::Shared { .. } => unreachable!(),
            })
            .collect();
        assert_eq!(
            keys,
            ["settings/theme", "settings/download", "eq_presets/mine"]
        );
        let LocalOp::Personal { value, .. } = &ops[1] else {
            panic!()
        };
        let download = value.as_ref().unwrap();
        assert_eq!(download["quality"], "720");
        assert!(download.get("directory").is_none());
    }

    #[test]
    fn filtered_feeds_exclude_channels_and_playlists() {
        assert!(filtered_feed(&BrowseRequest::Home, 1));
        assert!(filtered_feed(&BrowseRequest::Search("x".into()), 2));
        assert!(!filtered_feed(&BrowseRequest::Home, 3));
        assert!(!filtered_feed(&BrowseRequest::Collection("UC".into()), 1));
        assert!(!filtered_feed(&BrowseRequest::Library, 1));
        assert!(!filtered_feed(&BrowseRequest::History, 1));
    }
}
