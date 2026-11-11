//! What plays, and what plays next.
//!
//! The engine resolves streams, keeps the queue, the playback context (a playlist's tail,
//! related videos, nearby stations), history and resume positions; this module drives the
//! embedded mpv with them, exactly as Android's `PlaybackService` drives Media3: manual queue
//! first, then the context; history records only once the decoder accepted the stream; a
//! failed CDN stream is renewed once at the same position.

use std::time::Duration;

use rune::{LiveText, NativeSurface, Tx, Waker};
use serde_json::{Value, json};
use vidcue_core::{BrowseRequest, Config, Media, MediaKind};
use vidcue_providers::sponsorblock::Segment;

use super::mpv::{Lives, Load, Mpv, Snapshot};
use crate::app::{App, Prefs, Route, ToastAction, ToastKind};
use crate::backend::{Backend, op};

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Stream,
    Radio,
    Local(String),
    Movie(String),
}

#[derive(Clone, Debug)]
pub struct Now {
    pub media: Media,
    pub audio: bool,
    pub source: Source,
    pub quality: String,
    pub audio_language: Option<String>,
    pub resolving: bool,
    pub error: Option<String>,
    pub live: bool,
    pub qualities: Vec<u32>,
    pub audio_tracks: Vec<(String, String)>,
    pub captions: Vec<(String, String)>,
    pub caption: Option<usize>,
    pub context_id: Option<String>,
    pub context_entry: bool,
    pub from_history: bool,
    pub committed: bool,
    pub renewed: bool,
    pub token: u64,
    pub resume_at: Option<f64>,
    pub details: Value,
    pub headers: Vec<(String, String)>,
    pub resolution_route: Option<String>,
    /// Extra lines describing where a stream came from (movie providers, failover).
    pub provenance: Vec<String>,
}

impl Now {
    pub(crate) fn new(
        media: Media,
        audio: bool,
        source: Source,
        quality: String,
        token: u64,
    ) -> Self {
        let live = media.live;
        Self {
            media,
            audio,
            source,
            quality,
            audio_language: None,
            resolving: true,
            error: None,
            live,
            qualities: Vec::new(),
            audio_tracks: Vec::new(),
            captions: Vec::new(),
            caption: None,
            context_id: None,
            context_entry: false,
            from_history: false,
            committed: false,
            renewed: false,
            token,
            resume_at: None,
            details: Value::Null,
            headers: Vec::new(),
            resolution_route: None,
            provenance: Vec::new(),
        }
    }
    pub fn is_music(&self) -> bool {
        self.audio || matches!(self.media.kind, MediaKind::Song | MediaKind::Station)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sleep {
    At(f64),
    EndOfTrack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    Off,
    All,
    One,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerMenu {
    Settings,
    Quality,
    Speed,
    Captions,
    Audio,
    Sleep,
    Output,
    Grade,
    Equalizer,
}

/// Where the next item comes from once this one ends.
#[derive(Clone, Debug, Default)]
pub enum Context {
    /// Related videos (YouTube), when autoplay is on.
    #[default]
    Auto,
    None,
    /// The rest of a list the item was picked from.
    List(Vec<Media>),
    /// Stations near this one.
    Radio(Vec<Media>),
    /// A whole playlist, album or channel, paged by the engine.
    Collection(BrowseRequest, bool),
    /// An offline playlist.
    Offline(Vec<Media>),
}

#[derive(Clone, Debug, Default)]
pub struct PlayOptions {
    pub audio: Option<bool>,
    pub context: Context,
    pub start: Option<f64>,
    /// Stay on the current page (radio and music start in the dock).
    pub background: bool,
    pub(crate) from_queue: Option<(String, bool)>,
    pub(crate) from_history: Option<usize>,
    pub(crate) keep_context: Option<String>,
}

pub struct Player {
    pub mpv: Option<Mpv>,
    pub unavailable: Option<String>,
    pub surface: NativeSurface,
    pub lives: Lives,
    pub snap: Snapshot,
    signature: u64,
    pub now: Option<Now>,
    pub queue: Vec<Media>,
    pub context: Value,
    pub sleep: Option<Sleep>,
    pub sponsor: Vec<Segment>,
    sponsor_ignore_until: f64,
    pub repeat: Repeat,
    pub shuffle: bool,
    pub menu: Option<PlayerMenu>,
    pub theater: bool,
    pub fullscreen: bool,
    pub lyrics: crate::lyrics::Lyrics,
    handled_end: u64,
    last_saved: f64,
    token: u64,
    pub volume: f64,
    pub speed: f64,
    pub captions_on: bool,
    pub seek_seconds: f64,
    resume: bool,
    autoplay: bool,
    sponsor_mode: String,
    pub quality: String,
    eq: Option<String>,
    pub meter: bool,
    grade: String,
    pub hist_index: Option<usize>,
    pub seek_preview: Option<f32>,
    /// A label naming the player's last action ("Speed 1.5×"), shown briefly over the video.
    pub flash: Option<(String, f64)>,
    pub sleep_text: LiveText,
}

impl Player {
    pub fn new() -> Self {
        Self {
            mpv: None,
            unavailable: None,
            surface: NativeSurface::new(),
            lives: Lives::default(),
            snap: Snapshot {
                volume: 100.0,
                speed: 1.0,
                idle: true,
                ..Default::default()
            },
            signature: 0,
            now: None,
            queue: Vec::new(),
            context: Value::Null,
            sleep: None,
            sponsor: Vec::new(),
            sponsor_ignore_until: 0.0,
            repeat: Repeat::Off,
            shuffle: false,
            menu: None,
            theater: false,
            fullscreen: false,
            lyrics: Default::default(),
            handled_end: 0,
            last_saved: 0.0,
            token: 0,
            volume: 100.0,
            speed: 1.0,
            captions_on: false,
            seek_seconds: 10.0,
            resume: true,
            autoplay: true,
            sponsor_mode: "skip".into(),
            quality: "1080".into(),
            eq: None,
            meter: true,
            grade: "off".into(),
            hist_index: None,
            seek_preview: None,
            flash: None,
            sleep_text: LiveText::new("00:00"),
        }
    }

    pub fn setup(&mut self, waker: Waker, prefs: &Prefs, offline: bool) {
        self.volume = prefs.volume;
        self.speed = prefs.speed;
        self.captions_on = prefs.captions;
        if offline {
            self.unavailable = Some("Playback is off in headless renders".into());
            return;
        }
        match Mpv::new(self.surface.clone()) {
            Ok(mpv) => {
                mpv.set_waker(waker);
                mpv.set_volume(self.volume);
                self.lives = mpv.lives().clone();
                self.mpv = Some(mpv);
                self.apply_filters();
            }
            Err(e) => {
                self.unavailable = Some(format!(
                    "Embedded playback is unavailable: {e}. Install mpv (libmpv) 0.38 or newer."
                ))
            }
        }
    }

    pub fn apply_config(&mut self, c: &Config) {
        self.resume = c.resume_on_play;
        self.autoplay = c.autoplay_related;
        self.seek_seconds = c.seek_seconds.clamp(1, 600) as f64;
        self.sponsor_mode = c.playback_sponsorblock.clone();
        self.quality = c.download.quality.clone();
        self.eq = vidcue_core::equalizer::gains(&c.equalizer, &c.equalizer_custom)
            .and_then(|g| vidcue_core::equalizer::filter(&g));
        self.grade = c.color_grade.clone();
        self.apply_filters();
    }

    fn apply_filters(&self) {
        if let Some(m) = &self.mpv {
            m.set_audio_filters(self.eq.as_deref(), self.meter);
            m.set_video_filter(grade_filter(&self.grade).as_deref());
        }
    }

    pub fn playing(&self) -> bool {
        self.now.is_some() && !self.snap.paused && self.snap.loaded
    }

    pub fn has_video(&self) -> bool {
        self.now.as_ref().is_some_and(|n| !n.audio) && self.snap.video_size.is_some()
    }

    pub fn toggle_pause(&mut self) {
        if let Some(m) = &self.mpv {
            if self.snap.eof {
                m.seek_to(0.0);
                m.set_paused(false);
            } else {
                m.toggle_pause();
            }
            self.snap.paused = !self.snap.paused;
        }
    }

    pub fn seek_to(&mut self, seconds: f64, user: bool) {
        if let Some(m) = &self.mpv {
            m.seek_to(seconds);
            self.snap.time = seconds;
            if user {
                // A seek back into a skipped segment means the user wants to watch it.
                self.sponsor_ignore_until = seconds + 1.0;
            }
        }
    }

    pub fn seek_by(&mut self, delta: f64) {
        let t = (self.snap.time + delta).max(0.0);
        let t = self.snap.duration.map_or(t, |d| t.min((d - 0.5).max(0.0)));
        self.seek_to(t, true);
    }

    pub fn set_volume(&mut self, v: f64) {
        self.volume = v.clamp(0.0, 150.0);
        if let Some(m) = &self.mpv {
            m.set_volume(self.volume);
            if self.snap.muted && v > 0.0 {
                m.set_flag("mute", false);
            }
        }
    }

    pub fn toggle_mute(&mut self) {
        if let Some(m) = &self.mpv {
            m.set_flag("mute", !self.snap.muted);
        }
    }

    pub fn set_speed(&mut self, s: f64) {
        self.speed = s.clamp(0.25, 4.0);
        if let Some(m) = &self.mpv {
            m.set_speed(self.speed);
        }
    }

    pub fn set_caption(&mut self, index: Option<usize>) {
        let Some(now) = &mut self.now else { return };
        now.caption = index;
        self.captions_on = index.is_some();
        if let Some(m) = &self.mpv {
            match index {
                // Captions added on load keep their order: track n+1 is caption n.
                Some(i) => {
                    m.set("sid", &(i + 1).to_string());
                }
                None => {
                    m.set("sid", "no");
                }
            }
        }
    }

    pub fn set_audio_device(&mut self, name: &str) {
        if let Some(m) = &self.mpv {
            m.set("audio-device", name);
        }
    }

    pub fn refresh_up_next(&self, be: &Backend) {
        be.call(op("up_next", json!({ "limit": 50 })), |app, r, _| {
            if let Ok(v) = r {
                app.player.queue = v["queue"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|m| serde_json::from_value(m.clone()).ok())
                    .collect();
                app.player.context = v["context"].clone();
            }
        });
    }

    /// Context items (the tail after the queue), as the engine reports them.
    pub fn context_items(&self) -> Vec<Media> {
        self.context["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| serde_json::from_value(m.clone()).ok())
            .collect()
    }

    fn signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let s = &self.snap;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (
            s.loaded,
            s.paused,
            s.buffering,
            s.eof,
            s.idle,
            s.muted,
            s.seeking,
        )
            .hash(&mut h);
        (
            s.volume as i64,
            (s.speed * 100.0) as i64,
            s.duration.map(|d| d as i64),
            s.video_size,
        )
            .hash(&mut h);
        (
            &s.title,
            &s.sub_text,
            &s.error,
            &s.audio_device,
            s.generation,
        )
            .hash(&mut h);
        s.tracks.to_string().len().hash(&mut h);
        s.audio_devices.to_string().len().hash(&mut h);
        h.finish()
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

/// An mpv video filter for a colour-grade preset (the sw renderer applies no VO equalizer).
pub fn grade_filter(name: &str) -> Option<String> {
    let g = vidcue_services::color_grade::preset(name);
    if g == vidcue_services::color_grade::Grade::default() {
        return None;
    }
    let mut chain = vec![format!(
        "eq=brightness={:.3}:contrast={:.3}:saturation={:.3}:gamma={:.3}",
        g.brightness / 100.0 * 0.25,
        1.0 + g.contrast / 100.0,
        1.0 + g.saturation / 100.0,
        1.0 + g.gamma / 100.0
    )];
    if g.temperature != 0.0 {
        let t = (g.temperature / 100.0).clamp(-0.3, 0.3);
        chain.push(format!("colorbalance=rm={:.3}:bm={:.3}", t, -t));
    }
    Some(format!("@vidcue-grade:lavfi=[{}]", chain.join(",")))
}

fn is_audio_kind(m: &Media) -> bool {
    matches!(m.kind, MediaKind::Song | MediaKind::Station)
}

fn youtube(m: &Media) -> bool {
    vidcue_core::http_url(&m.url)
        .ok()
        .and_then(|u| u.host_str().map(vidcue_core::youtube_host))
        .unwrap_or(false)
}

/// Plays `media`: collections open as pages, everything else resolves and loads.
pub fn play(app: &mut App, media: Media, opts: PlayOptions) {
    if media.collection() {
        let tab = if matches!(media.kind, MediaKind::Album | MediaKind::Artist) {
            crate::app::Tab::Music
        } else {
            crate::app::Tab::Home
        };
        app.open_collection(tab, &media);
        return;
    }
    let p = &mut app.player;
    if let Some(prev) = p.now.take() {
        save_position(&app.be, &prev, &p.snap);
    }
    p.token += 1;
    let token = p.token;
    let station =
        media.kind == MediaKind::Station || (!youtube(&media) && !media.url.starts_with('/'));
    let audio = opts
        .audio
        .unwrap_or_else(|| is_audio_kind(&media) || station);
    let local = if media.url.starts_with('/') {
        Some(media.url.clone())
    } else {
        crate::downloads::local_file(app, &media, audio)
    };
    let source = if let Some(path) = &local {
        Source::Local(path.clone())
    } else if station {
        Source::Radio
    } else {
        Source::Stream
    };
    let p = &mut app.player;
    let mut now = Now::new(
        media.clone(),
        audio,
        source.clone(),
        p.quality.clone(),
        token,
    );
    now.from_history = opts.from_history.is_some();
    if let Some((ctx, entry)) = &opts.from_queue {
        now.context_id = Some(ctx.clone());
        now.context_entry = *entry;
    }
    if let Some(i) = opts.from_history {
        p.hist_index = Some(i);
    } else if opts.from_queue.is_none() {
        p.hist_index = None;
    }
    p.sponsor.clear();
    p.sponsor_ignore_until = 0.0;
    p.menu = None;
    p.seek_preview = None;
    p.now = Some(now);
    p.lyrics.reset();
    if p.sleep == Some(Sleep::EndOfTrack) {
        // The timer ends "this track": a new pick starts a fresh session.
        p.sleep = None;
    }
    // Videos and songs open the player; stations start in the dock.
    if !opts.background && (!audio || media.kind == MediaKind::Song) {
        app.go(Route::Watch);
    }
    // The context starts with the first item; continuing items keep it.
    let context_id = opts
        .keep_context
        .clone()
        .or(opts.from_queue.as_ref().map(|q| q.0.clone()))
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    if let Some(n) = &mut app.player.now {
        n.context_id = Some(context_id.clone());
    }
    if opts.from_queue.is_none() && opts.keep_context.is_none() {
        seed_context(app, &media, &context_id, opts.context.clone());
    }
    match source {
        Source::Local(path) => {
            load(
                app,
                token,
                Load {
                    url: path,
                    start: opts.start,
                    title: Some(media.title.clone()),
                    audio_only: audio,
                    ..Default::default()
                },
            );
            after_start(app, &media);
        }
        Source::Movie(_) => {}
        Source::Radio | Source::Stream => {
            let want_resume = app.player.resume
                && !audio
                && !media.live
                && opts.start.is_none()
                && source == Source::Stream;
            if want_resume {
                let url = media.url.clone();
                app.be
                    .call(op("position", json!({ "url": url })), move |app, r, _| {
                        if let (Ok(v), Some(n)) =
                            (r, app.player.now.as_mut().filter(|n| n.token == token))
                        {
                            let ms = v["milliseconds"].as_u64().unwrap_or(0);
                            n.resume_at = (ms > 5_000).then(|| ms as f64 / 1000.0);
                        }
                        resolve(app, token, false, None);
                    });
            } else {
                if let Some(n) = &mut app.player.now {
                    n.resume_at = opts.start;
                }
                resolve(app, token, false, None);
            }
            after_start(app, &media);
        }
    }
}

/// Details, SponsorBlock segments and lyrics, fetched beside the stream.
fn after_start(app: &mut App, media: &Media) {
    let token = app.player.token;
    if youtube(media) {
        app.be.call(
            op("watch_details", json!({ "media": media })),
            move |app, r, _| {
                if let (Ok(v), Some(n)) = (r, app.player.now.as_mut().filter(|n| n.token == token))
                {
                    n.details = v;
                }
            },
        );
        if app.player.sponsor_mode != "off" && !media.live {
            app.be.call(
                op("sponsor", json!({ "media": media })),
                move |app, r, _| {
                    if app.player.now.as_ref().is_some_and(|n| n.token == token)
                        && let Ok(v) = r
                    {
                        app.player.sponsor = serde_json::from_value(v).unwrap_or_default();
                    }
                },
            );
        }
    }
    if app.prefs.show_lyrics || matches!(media.kind, MediaKind::Song) {
        crate::lyrics::load(app, false);
    }
}

fn seed_context(app: &mut App, media: &Media, id: &str, context: Context) {
    let engine = app.be.engine.clone();
    engine.activate_context(Some(id.to_owned()));
    let source = media.url.clone();
    let request = match context {
        Context::None => return,
        Context::Auto => {
            if !app.player.autoplay || !youtube(media) || media.live {
                return;
            }
            let Some(vid) = vidcue_providers::sponsorblock::video_id(media) else {
                return;
            };
            op(
                "prepare_context",
                json!({ "context_id": id, "request": BrowseRequest::Related(vid), "music": media.kind == MediaKind::Song, "source_url": source }),
            )
        }
        Context::List(items) => op(
            "seed_context",
            json!({ "context_id": id, "items": items, "source_url": source }),
        ),
        Context::Radio(items) => op(
            "radio_context",
            json!({ "context_id": id, "items": items, "source_url": source }),
        ),
        Context::Offline(items) => op(
            "offline_context",
            json!({ "context_id": id, "items": items, "source_url": source }),
        ),
        Context::Collection(request, music) => op(
            "prepare_context",
            json!({ "context_id": id, "request": request, "music": music, "source_url": source }),
        ),
    };
    app.be
        .call(request, |app, _, _| app.player.refresh_up_next(&app.be));
}

/// Resolves the current item (again, with `fresh` after a failure) and loads it.
fn resolve(app: &mut App, token: u64, fresh: bool, start: Option<f64>) {
    let Some(now) = app.player.now.as_mut().filter(|n| n.token == token) else {
        return;
    };
    now.resolving = true;
    now.error = None;
    let mut req =
        json!({ "media": now.media, "audio": now.audio, "quality": now.quality, "fresh": fresh });
    if let Some(l) = &now.audio_language {
        req["audio_language"] = json!(l);
    }
    app.be.call(op("resolve", req), move |app, r, _| {
        let Some(now) = app.player.now.as_mut().filter(|n| n.token == token) else {
            return;
        };
        now.resolving = false;
        match r {
            Ok(v) => {
                let url = v["url"].as_str().unwrap_or_default().to_owned();
                if url.is_empty() {
                    now.error = Some("The provider returned no playable stream".into());
                    return;
                }
                now.live = v["live"].as_bool().unwrap_or(now.media.live);
                now.qualities = v["video_qualities"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_u64)
                    .map(|q| q as u32)
                    .collect();
                now.audio_tracks = v["audio_tracks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|t| {
                        Some((
                            t["language"].as_str()?.to_owned(),
                            t["label"].as_str().unwrap_or_default().to_owned(),
                        ))
                    })
                    .collect();
                if now.audio_language.is_none() {
                    now.audio_language = v["audio_language"].as_str().map(str::to_owned);
                }
                now.captions = v["catalog"]["captions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| {
                        Some((
                            c["label"].as_str()?.to_owned(),
                            c["url"].as_str()?.to_owned(),
                        ))
                    })
                    .take(40)
                    .collect();
                now.resolution_route = v["resolution_route"].as_str().map(str::to_owned);
                fill_metadata(&mut now.media, &v);
                now.headers = v["headers"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                    .collect();
                let captions_on = app.player.captions_on;
                let caption = captions_on
                    .then(|| preferred_caption(&now.captions))
                    .flatten();
                now.caption = caption;
                let load_spec = Load {
                    url,
                    audio_url: v["audio_url"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned),
                    headers: now.headers.clone(),
                    start: start.or(now.resume_at),
                    title: Some(now.media.title.clone()),
                    subtitles: now
                        .captions
                        .iter()
                        .map(|(l, u)| (u.clone(), l.clone()))
                        .collect(),
                    audio_only: now.audio,
                    ..Default::default()
                };
                load(app, token, load_spec);
                if let Some(i) = caption {
                    app.player.set_caption(Some(i));
                }
            }
            Err(e) => {
                now.error = Some(e.message.clone());
                if e.needs_account() {
                    app.toast_with(
                        ToastKind::Error,
                        e.message,
                        Some(("Account".into(), ToastAction::OpenAccount)),
                    );
                } else if app.route != Route::Watch {
                    // The stage shows the error on the Watch page; elsewhere offer a retry.
                    app.toast_with(
                        ToastKind::Error,
                        e.message,
                        Some(("Retry".into(), ToastAction::Retry)),
                    );
                }
            }
        }
    });
}

fn preferred_caption(captions: &[(String, String)]) -> Option<usize> {
    let lang = std::env::var("LANG")
        .unwrap_or_default()
        .chars()
        .take(2)
        .collect::<String>()
        .to_ascii_lowercase();
    captions
        .iter()
        .position(|(l, _)| {
            !lang.is_empty() && l.to_ascii_lowercase().starts_with(&lang) && !l.contains("auto")
        })
        .or_else(|| {
            captions
                .iter()
                .position(|(l, _)| l.to_ascii_lowercase().starts_with("english"))
        })
        .or(if captions.is_empty() { None } else { Some(0) })
}

fn fill_metadata(m: &mut Media, v: &Value) {
    let s = |k: &str| v[k].as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    if (m.title.is_empty() || m.title == "Linked video")
        && let Some(t) = s("title")
    {
        m.title = t;
    }
    if m.creator.is_empty()
        && let Some(c) = s("creator")
    {
        m.creator = c;
    }
    if m.thumbnail.is_empty()
        && let Some(t) = s("thumbnail")
    {
        m.thumbnail = t;
    }
    if m.description.is_empty()
        && let Some(d) = s("description")
    {
        m.description = d;
    }
    if m.duration.is_none() {
        m.duration = v["duration"].as_f64().map(|d| d as u64);
    }
    if m.view_count.is_none() {
        m.view_count = v["view_count"].as_u64();
    }
    if m.like_count.is_none() {
        m.like_count = v["like_count"].as_u64();
    }
    if m.upload_date.is_none() {
        m.upload_date = s("upload_date");
    }
}

pub fn load(app: &mut App, token: u64, mut spec: Load) {
    let Some(mpv) = &app.player.mpv else {
        if let Some(n) = app.player.now.as_mut().filter(|n| n.token == token) {
            n.error = app.player.unavailable.clone();
        }
        return;
    };
    spec.muted = false;
    mpv.load(&spec);
    mpv.set_speed(app.player.speed);
    app.player.handled_end = mpv.snapshot().generation;
    app.player.last_saved = app.clock;
}

/// Plays a stream resolved elsewhere (movies): the same session, with its own source.
pub fn play_resolved(
    app: &mut App,
    media: Media,
    spec: Load,
    provenance: Vec<String>,
    captions: Vec<(String, String)>,
) {
    let renewed = app
        .player
        .now
        .as_ref()
        .is_some_and(|n| n.media.url == media.url && n.renewed);
    if let Some(prev) = app.player.now.take() {
        save_position(&app.be, &prev, &app.player.snap);
    }
    app.player.token += 1;
    let token = app.player.token;
    let mut now = Now::new(
        media.clone(),
        false,
        Source::Movie(media.url.clone()),
        app.player.quality.clone(),
        token,
    );
    now.resolving = false;
    // A re-resolve of the same title after a failure keeps its one renewal spent.
    now.renewed = renewed;
    now.provenance = provenance;
    now.captions = captions;
    now.headers = spec.headers.clone();
    app.player.sponsor.clear();
    app.player.now = Some(now);
    app.go(Route::Watch);
    load(app, token, spec);
}

/// Re-resolves the current item from scratch at the current position.
pub fn retry(app: &mut App, _tx: &mut Tx) {
    let Some(now) = &app.player.now else { return };
    let token = now.token;
    let start = Some(app.player.snap.time).filter(|t| *t > 1.0);
    match now.source.clone() {
        Source::Local(path) => {
            let spec = Load {
                url: path,
                start,
                title: Some(now.media.title.clone()),
                audio_only: now.audio,
                ..Default::default()
            };
            load(app, token, spec);
        }
        Source::Movie(_) => crate::movies::retry(app),
        _ => resolve(app, token, true, start),
    }
}

pub fn change_quality(app: &mut App, quality: String) {
    let Some(now) = app.player.now.as_mut() else {
        return;
    };
    if now.quality == quality {
        return;
    }
    now.quality = quality.clone();
    let token = now.token;
    let at = Some(app.player.snap.time);
    app.player.flash = Some((
        format!("Quality {}", quality_label(&quality)),
        app.clock + 1.5,
    ));
    resolve(app, token, false, at);
}

pub fn change_audio_language(app: &mut App, language: String) {
    let Some(now) = app.player.now.as_mut() else {
        return;
    };
    now.audio_language = Some(language);
    let token = now.token;
    let at = Some(app.player.snap.time);
    resolve(app, token, false, at);
}

pub fn quality_label(q: &str) -> String {
    match q {
        "best" => "Best".into(),
        "2160" => "4K".into(),
        "1440" => "1440p".into(),
        h => format!("{h}p"),
    }
}

/// Saves where an item stopped, for resume (ordinary non-live videos only).
fn save_position(be: &Backend, now: &Now, snap: &Snapshot) {
    if now.audio || now.live || now.source != Source::Stream || !snap.loaded {
        return;
    }
    let near_end = snap.duration.is_some_and(|d| snap.time > d - 15.0);
    let ms = if near_end {
        0
    } else {
        (snap.time * 1000.0) as u64
    };
    be.call(
        op(
            "position",
            json!({ "url": now.media.url, "milliseconds": ms }),
        ),
        |_, _, _| {},
    );
}

/// Plays whatever comes next: the queue, then the context; with repeat-all, history wraps.
pub fn next(app: &mut App, automatic: bool) {
    let Some(now) = app.player.now.clone() else {
        return;
    };
    if automatic && app.player.repeat == Repeat::One {
        app.player.seek_to(0.0, false);
        if let Some(m) = &app.player.mpv {
            m.set_paused(false);
        }
        return;
    }
    save_position(&app.be, &now, &app.player.snap);
    let context_id = now.context_id.clone();
    let mut req = json!({ "shuffle": app.player.shuffle });
    if let Some(c) = &context_id {
        req["context_id"] = json!(c);
    }
    app.be.call(op("queue_peek", req), move |app, r, _| {
        let next = r.ok().filter(|v| v.get("url").is_some());
        match next {
            Some(v) => {
                let entry = v["context_entry"].as_bool().unwrap_or(false);
                if let Ok(media) = serde_json::from_value::<Media>(v) {
                    let ctx = context_id.clone().unwrap_or_default();
                    let audio = app
                        .player
                        .now
                        .as_ref()
                        .map(|n| n.audio && !is_audio_kind(&media))
                        .unwrap_or(false);
                    play(
                        app,
                        media,
                        PlayOptions {
                            from_queue: Some((ctx, entry)),
                            background: true,
                            audio: audio.then_some(true),
                            ..Default::default()
                        },
                    );
                }
            }
            None if app.player.repeat == Repeat::All => {
                app.player.seek_to(0.0, false);
                if let Some(m) = &app.player.mpv {
                    m.set_paused(false);
                }
            }
            None => {
                if automatic {
                    app.toast(ToastKind::Info, "Nothing else is queued");
                } else {
                    app.toast(ToastKind::Info, "You're at the end of the queue");
                }
            }
        }
    });
}

/// Plays an item picked from the queue (`queued`) or the context tail; the engine commits it
/// once the decoder accepts it, as if it had come up on its own.
pub fn play_next_item(app: &mut App, media: Media, queued: bool) {
    let ctx = app
        .player
        .now
        .as_ref()
        .and_then(|n| n.context_id.clone())
        .unwrap_or_default();
    play(
        app,
        media,
        PlayOptions {
            from_queue: Some((ctx, !queued)),
            background: true,
            ..Default::default()
        },
    );
}

/// Restarts after 3 s; otherwise steps back through history.
pub fn previous(app: &mut App) {
    if app.player.snap.time > 3.0 || app.player.now.is_none() {
        app.player.seek_to(0.0, true);
        return;
    }
    let mut req = json!({ "direction": "previous" });
    if let Some(i) = app.player.hist_index {
        req["from_index"] = json!(i);
    }
    app.be.call(op("history_peek", req), |app, r, _| {
        let Ok(v) = r else { return };
        let index = v["index"].as_u64().map(|i| i as usize);
        match (serde_json::from_value::<Media>(v["media"].clone()), index) {
            (Ok(media), Some(i)) => play(
                app,
                media,
                PlayOptions {
                    from_history: Some(i),
                    background: true,
                    context: Context::None,
                    ..Default::default()
                },
            ),
            _ => app.player.seek_to(0.0, true),
        }
    });
}

pub fn stop(app: &mut App) {
    if let Some(now) = app.player.now.take() {
        save_position(&app.be, &now, &app.player.snap);
    }
    if let Some(m) = &app.player.mpv {
        m.stop();
    }
    app.player.sleep = None;
    app.player.sponsor.clear();
    app.be.engine.activate_context(None);
    if app.route == Route::Watch {
        app.go_back();
    }
}

pub fn set_sleep(app: &mut App, sleep: Option<Sleep>) {
    app.player.sleep = sleep;
    app.toast(
        ToastKind::Info,
        match sleep {
            None => "Sleep timer off".to_owned(),
            Some(Sleep::EndOfTrack) => "Playback stops at the end of this track".to_owned(),
            Some(Sleep::At(t)) => {
                format!("Playback stops in {} min", ((t - app.clock) / 60.0).round())
            }
        },
    );
}

/// Runs each poll: reads mpv, records history, skips sponsors, advances at the end.
pub fn tick(app: &mut App, tx: &mut Tx) -> bool {
    let Some(mpv) = &app.player.mpv else {
        return false;
    };
    app.player.snap = mpv.snapshot();
    let sig = app.player.signature();
    let mut changed = sig != app.player.signature;
    app.player.signature = sig;
    let Some(now) = app.player.now.clone() else {
        return changed;
    };
    let snap = app.player.snap.clone();
    let clock = app.clock;

    // The decoder accepted the stream: history and queue commit, once.
    if !now.committed && snap.loaded && (snap.time > 0.3 || now.audio) && snap.error.is_none() {
        if let Some(n) = &mut app.player.now {
            n.committed = true;
        }
        if !now.from_history {
            app.be.call(
                op("history_record", json!({ "media": now.media })),
                |_, _, _| {},
            );
        }
        if let Some(ctx) = &now.context_id {
            app.be.call(op("queue_commit", json!({ "url": now.media.url, "context_entry": now.context_entry, "context_id": ctx })), |app, _, _| {
                app.player.refresh_up_next(&app.be)
            });
        }
        changed = true;
    }

    // A failed stream: renew once at the same place, then show the error.
    if let Some(err) = snap.error.clone()
        && snap.ended_generation != app.player.handled_end
    {
        app.player.handled_end = snap.ended_generation;
        let renewable = matches!(now.source, Source::Stream | Source::Movie(_)) && !now.renewed;
        if renewable {
            if let Some(n) = &mut app.player.now {
                n.renewed = true;
            }
            retry(app, tx);
        } else if let Some(n) = &mut app.player.now {
            n.error = Some(format!("Playback stopped: {err}"));
        }
        return true;
    }

    // The end of the item.
    if snap.eof && snap.ended_generation != app.player.handled_end {
        app.player.handled_end = snap.ended_generation;
        save_position(&app.be, &now, &snap);
        if app.player.sleep == Some(Sleep::EndOfTrack) {
            app.player.sleep = None;
            app.toast(
                ToastKind::Info,
                "Sleep timer: stopped at the end of the track",
            );
        } else if !now.live {
            next(app, true);
        }
        return true;
    }

    // SponsorBlock: skip, or just mark segments.
    if app.player.sponsor_mode == "skip"
        && !snap.paused
        && let Some(to) = vidcue_providers::sponsorblock::skip_target(
            &app.player.sponsor,
            snap.time,
            app.player.sponsor_ignore_until,
        )
    {
        let from = snap.time;
        app.player.seek_to(to, false);
        app.player.sponsor_ignore_until = to;
        app.toast_with(
            ToastKind::Info,
            "Skipped a sponsor segment",
            Some(("Undo".into(), ToastAction::UndoSponsorSkip(from))),
        );
        changed = true;
    }

    // Sleep timer.
    if let Some(Sleep::At(at)) = app.player.sleep {
        let left = at - clock;
        app.player.sleep_text.set(super::mpv::clock(left.max(0.0)));
        if left <= 0.0 {
            app.player.sleep = None;
            if let Some(m) = &app.player.mpv {
                m.set_paused(true);
            }
            app.toast(ToastKind::Info, "Sleep timer: playback paused");
            changed = true;
        } else {
            tx.wake_after(Duration::from_millis(500));
        }
    }

    // Resume position, every ten seconds of playback.
    if !snap.paused && snap.loaded && clock - app.player.last_saved > 10.0 {
        app.player.last_saved = clock;
        save_position(&app.be, &now, &snap);
    }

    if app
        .player
        .flash
        .as_ref()
        .is_some_and(|(_, until)| *until <= clock)
    {
        app.player.flash = None;
        changed = true;
    }

    changed |= crate::lyrics::tick(app);

    if !snap.paused && snap.loaded {
        tx.wake_after(Duration::from_millis(if app.player.lyrics.visible() {
            120
        } else {
            250
        }));
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grade_presets_become_labelled_filters_and_off_sends_none() {
        assert!(grade_filter("off").is_none());
        let warm = grade_filter("warm").unwrap();
        assert!(warm.starts_with("@vidcue-grade:lavfi=[eq="), "{warm}");
        assert!(warm.contains("colorbalance"), "{warm}");
    }

    #[test]
    fn captions_prefer_a_human_track_in_the_user_language() {
        let c = vec![
            ("Deutsch".to_owned(), "a".to_owned()),
            ("English (auto-generated)".to_owned(), "b".to_owned()),
            ("English".to_owned(), "c".to_owned()),
        ];
        let i = preferred_caption(&c).unwrap();
        assert!(i < c.len());
        assert_eq!(preferred_caption(&[]), None);
    }

    #[test]
    fn quality_labels_read_as_people_say_them() {
        assert_eq!(quality_label("2160"), "4K");
        assert_eq!(quality_label("720"), "720p");
        assert_eq!(quality_label("best"), "Best");
    }
}
