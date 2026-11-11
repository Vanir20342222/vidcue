//! An mpv instance embedded in the window.
//!
//! libmpv decodes, keeps A/V sync and plays audio. Its software renderer converts each video
//! frame to packed BGRA on our render thread, sized to what the window shows (never more than
//! the source or 1080p), and hands it to a Rune [`NativeSurface`]; the GPU scales, rounds and
//! composites it like any other element, so video takes part in blur, glow and shared-element
//! flights. Fast-changing values (position, levels) go to [`Live`] handles and a [`LiveText`],
//! which redraw without re-running the view; discrete changes (pause, end, error, tracks) wake
//! the app.

use std::ffi::{CString, c_void};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use anyhow::{Result, bail};
use rune::native_surface::{ByteFrame, ColorInfo, Format};
use rune::{Live, LiveText, NativeSurface, Waker};
use serde_json::Value;

use super::ffi::*;

/// What a playing file looks like right now. Cloned by the UI on each view.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub loaded: bool,
    pub time: f64,
    pub duration: Option<f64>,
    pub paused: bool,
    pub buffering: bool,
    pub cache_percent: Option<f64>,
    pub seeking: bool,
    pub eof: bool,
    pub idle: bool,
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub video_size: Option<(u32, u32)>,
    /// mpv reports width and height as separate properties; both must arrive.
    pub(crate) dims: (u32, u32),
    pub cached_until: Option<f64>,
    pub title: Option<String>,
    pub sub_text: Option<String>,
    pub tracks: Value,
    pub audio_devices: Value,
    pub audio_device: Option<String>,
    pub chapters: Value,
    pub hwdec: Option<String>,
    pub video_codec: Option<String>,
    pub error: Option<String>,
    /// Bumped each time a file ends, starts or fails, so the UI acts on each once.
    pub generation: u64,
    pub ended_generation: u64,
}

/// One file to play.
#[derive(Clone, Debug, Default)]
pub struct Load {
    pub url: String,
    pub audio_url: Option<String>,
    pub headers: Vec<(String, String)>,
    pub start: Option<f64>,
    pub title: Option<String>,
    pub subtitles: Vec<(String, String)>,
    /// Audio-only playback: no video track is decoded or drawn.
    pub audio_only: bool,
    pub paused: bool,
    pub muted: bool,
    /// For short previews: stop after this many seconds.
    pub end: Option<f64>,
}

/// Live handles the view binds to.
#[derive(Clone)]
pub struct Lives {
    /// Played fraction 0..1, for the seek bar.
    pub progress: Live<f32>,
    /// Buffered fraction 0..1.
    pub buffered: Live<f32>,
    pub level_left: Live<f32>,
    pub level_right: Live<f32>,
    pub level_peak: Live<f32>,
    /// `0:42` style position.
    pub time: LiveText,
    /// `0:42 / 3:15`.
    pub time_of: LiveText,
    /// Played-part rects for the seek bars on screen (watch page, dock, music stage).
    pub bars: Arc<[Bar; 3]>,
    /// Stereo meter fills (left, right) and the peak-hold marker, in display levels.
    pub meters: Arc<[Meter; 3]>,
    /// 1.0 at silence, up to 1.22 on peaks: halos breathe with the music.
    pub pulse: Live<f32>,
    /// 0..1 display level of the peak, for glow opacity.
    pub glow: Live<f32>,
}

/// A meter's fill (or, for the peak, a thin marker) that follows the level without a view
/// re-run. Its size comes from layout.
pub struct Meter {
    pub rect: Live<[f32; 4]>,
    size: AtomicU64,
    /// The last level, so a size change (first layout, a resize while paused) redraws it.
    last: std::sync::atomic::AtomicU32,
    marker: bool,
}

impl Meter {
    fn new(marker: bool) -> Self {
        Self {
            rect: Live::new([0.0, 0.0, 0.0, 0.0]),
            size: AtomicU64::new(0),
            last: std::sync::atomic::AtomicU32::new(0),
            marker,
        }
    }
    pub fn set_size(&self, w: f32, h: f32) {
        let packed = (w.to_bits() as u64) << 32 | h.to_bits() as u64;
        if self.size.swap(packed, Ordering::Relaxed) != packed {
            self.follow(f32::from_bits(self.last.load(Ordering::Relaxed)));
        }
    }
    fn follow(&self, v: f32) {
        self.last.store(v.to_bits(), Ordering::Relaxed);
        let packed = self.size.load(Ordering::Relaxed);
        let (w, h) = (
            f32::from_bits((packed >> 32) as u32),
            f32::from_bits(packed as u32),
        );
        let v = v.clamp(0.0, 1.0);
        self.rect.set(if self.marker {
            [(w * v - 1.5).max(0.0), 0.0, 3.0, h]
        } else {
            [0.0, 0.0, w * v, h]
        });
    }
}

/// Maps a 0..1 level (-60..0 dBFS) to a display level over -36..-6 dB, where music lives,
/// so meters move instead of sitting near the top.
pub fn display_level(level: f32) -> f32 {
    let db = level * 60.0 - 60.0;
    ((db + 36.0) / 30.0).clamp(0.0, 1.0)
}

/// A seek bar's played part: its rect follows playback; its width comes from layout.
pub struct Bar {
    pub rect: Live<[f32; 4]>,
    width: std::sync::atomic::AtomicU32,
}

impl Bar {
    fn new() -> Self {
        Self {
            rect: Live::new([0.0, 0.0, 0.0, BAR_H]),
            width: std::sync::atomic::AtomicU32::new(0),
        }
    }
    pub fn set_width(&self, w: f32, progress: f32) {
        if self.width.swap(w.to_bits(), Ordering::Relaxed) != w.to_bits() {
            self.rect
                .set([0.0, 0.0, w * progress.clamp(0.0, 1.0), BAR_H]);
        }
    }
    fn follow(&self, progress: f32) {
        let w = f32::from_bits(self.width.load(Ordering::Relaxed));
        self.rect
            .set([0.0, 0.0, w * progress.clamp(0.0, 1.0), BAR_H]);
    }
}

pub const BAR_H: f32 = 4.0;
pub const BAR_WATCH: usize = 0;
pub const BAR_DOCK: usize = 1;
pub const BAR_MUSIC: usize = 2;

impl Lives {
    /// Publishes one block of levels (0..1 over -60..0 dBFS) to every meter, halo and glow.
    pub fn set_levels(&self, l: f32, r: f32, p: f32) {
        self.level_left.set(l);
        self.level_right.set(r);
        self.level_peak.set(p);
        let (dl, dr, dp) = (display_level(l), display_level(r), display_level(p));
        self.meters[0].follow(dl);
        self.meters[1].follow(dr);
        self.meters[2].follow(dp);
        self.pulse.set(1.0 + 0.22 * dp);
        self.glow.set(dp);
    }

    /// Sets every bar's played part, for seeks and loads the event thread hasn't seen yet.
    pub fn set_progress(&self, p: f32) {
        self.progress.set(p);
        for b in self.bars.iter() {
            b.follow(p);
        }
    }
}

impl Default for Lives {
    fn default() -> Self {
        Self {
            progress: Live::new(0.0),
            buffered: Live::new(0.0),
            level_left: Live::new(0.0),
            level_right: Live::new(0.0),
            level_peak: Live::new(0.0),
            time: LiveText::new("0:00"),
            time_of: LiveText::new("0:00 / 0:00"),
            bars: Arc::new([Bar::new(), Bar::new(), Bar::new()]),
            meters: Arc::new([Meter::new(false), Meter::new(false), Meter::new(true)]),
            pulse: Live::new(1.0),
            glow: Live::new(0.0),
        }
    }
}

pub fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

struct Handle(*mut mpv_handle);
// libmpv's client API is thread-safe for one handle.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

#[derive(Default)]
struct RenderGate {
    pending: bool,
    stop: bool,
}

struct Inner {
    mpv: Handle,
    snapshot: Mutex<Snapshot>,
    lives: Lives,
    surface: NativeSurface,
    waker: Mutex<Option<Waker>>,
    gate: Mutex<RenderGate>,
    gate_cv: Condvar,
    /// The largest picture worth rendering, in device pixels: `w << 32 | h`.
    target: AtomicU64,
    stop: AtomicBool,
    pending_subs: Mutex<Vec<(String, String)>>,
    frames: AtomicU64,
}

impl Inner {
    fn wake(&self) {
        if let Some(w) = self.waker.lock().unwrap().as_ref() {
            w();
        }
    }
}

pub struct Mpv {
    inner: Arc<Inner>,
    threads: Vec<JoinHandle<()>>,
}

fn c(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

impl Mpv {
    pub fn new(surface: NativeSurface) -> Result<Self> {
        let handle = unsafe { mpv_create() };
        if handle.is_null() {
            bail!("libmpv could not be created");
        }
        let set = |k: &str, v: &str| unsafe {
            mpv_set_option_string(handle, c(k).as_ptr(), c(v).as_ptr())
        };
        for (k, v) in [
            ("vo", "libmpv"),
            ("hwdec", "auto-copy-safe"),
            ("ytdl", "no"),
            ("keep-open", "yes"),
            ("idle", "yes"),
            ("osc", "no"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            ("audio-client-name", "vidcue"),
            ("volume-max", "150"),
            ("cache", "yes"),
            ("demuxer-max-bytes", "192MiB"),
            ("demuxer-max-back-bytes", "64MiB"),
            ("demuxer-readahead-secs", "30"),
            ("network-timeout", "20"),
            ("sub-visibility", "no"),
            ("sub-auto", "no"),
            ("audio-display", "no"),
            ("video-sync", "audio"),
            ("terminal", "no"),
            ("msg-level", "all=no"),
        ] {
            set(k, v);
        }
        if unsafe { mpv_initialize(handle) } < 0 {
            unsafe { mpv_terminate_destroy(handle) };
            bail!("libmpv could not start");
        }
        let inner = Arc::new(Inner {
            mpv: Handle(handle),
            snapshot: Mutex::new(Snapshot {
                volume: 100.0,
                speed: 1.0,
                idle: true,
                ..Default::default()
            }),
            lives: Lives::default(),
            surface,
            waker: Mutex::default(),
            gate: Mutex::default(),
            gate_cv: Condvar::new(),
            target: AtomicU64::new((1280u64 << 32) | 720),
            stop: AtomicBool::new(false),
            pending_subs: Mutex::default(),
            frames: AtomicU64::new(0),
        });
        for (i, (name, format)) in OBSERVED.iter().enumerate() {
            unsafe { mpv_observe_property(handle, i as u64 + 1, c(name).as_ptr(), *format) };
        }
        let events = {
            let inner = inner.clone();
            std::thread::Builder::new()
                .name("vidcue-mpv-events".into())
                .spawn(move || event_loop(&inner))?
        };
        let render = {
            let inner = inner.clone();
            std::thread::Builder::new()
                .name("vidcue-mpv-render".into())
                .spawn(move || render_loop(&inner))?
        };
        Ok(Self {
            inner,
            threads: vec![events, render],
        })
    }

    pub fn set_waker(&self, waker: Waker) {
        *self.inner.waker.lock().unwrap() = Some(waker.clone());
        self.inner.surface.set_waker(waker);
    }

    pub fn lives(&self) -> &Lives {
        &self.inner.lives
    }

    pub fn snapshot(&self) -> Snapshot {
        self.inner.snapshot.lock().unwrap().clone()
    }

    pub fn frames(&self) -> u64 {
        self.inner.frames.load(Ordering::Relaxed)
    }

    /// The size, in device pixels, the video is drawn at. Rendering never exceeds it.
    pub fn set_target(&self, w: f32, h: f32) {
        let w = (w.round() as u64).clamp(64, 3840);
        let h = (h.round() as u64).clamp(36, 2160);
        let packed = (w << 32) | h;
        if self.inner.target.swap(packed, Ordering::Relaxed) != packed {
            self.redraw();
        }
    }

    fn redraw(&self) {
        let mut g = self.inner.gate.lock().unwrap();
        g.pending = true;
        self.inner.gate_cv.notify_one();
    }

    pub fn command(&self, args: &[&str]) -> bool {
        let owned: Vec<CString> = args.iter().map(|a| c(a)).collect();
        let mut ptrs: Vec<*const std::ffi::c_char> = owned.iter().map(|s| s.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        unsafe { mpv_command(self.inner.mpv.0, ptrs.as_mut_ptr()) >= 0 }
    }

    fn command_async(&self, args: &[&str]) {
        let owned: Vec<CString> = args.iter().map(|a| c(a)).collect();
        let mut ptrs: Vec<*const std::ffi::c_char> = owned.iter().map(|s| s.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        unsafe { mpv_command_async(self.inner.mpv.0, 0, ptrs.as_mut_ptr()) };
    }

    pub fn set(&self, name: &str, value: &str) -> bool {
        unsafe {
            mpv_set_property_string(self.inner.mpv.0, c(name).as_ptr(), c(value).as_ptr()) >= 0
        }
    }

    pub fn set_flag(&self, name: &str, value: bool) {
        let mut v: std::ffi::c_int = value as _;
        unsafe {
            mpv_set_property(
                self.inner.mpv.0,
                c(name).as_ptr(),
                MPV_FORMAT_FLAG,
                (&mut v as *mut std::ffi::c_int).cast(),
            )
        };
    }

    pub fn set_f64(&self, name: &str, value: f64) {
        let mut v = value;
        unsafe {
            mpv_set_property(
                self.inner.mpv.0,
                c(name).as_ptr(),
                MPV_FORMAT_DOUBLE,
                (&mut v as *mut f64).cast(),
            )
        };
    }

    pub fn load(&self, load: &Load) {
        {
            let mut s = self.inner.snapshot.lock().unwrap();
            s.loaded = false;
            s.eof = false;
            s.error = None;
            s.time = load.start.unwrap_or(0.0);
            s.duration = None;
            s.title = load.title.clone();
            s.sub_text = None;
            s.video_size = None;
            s.dims = (0, 0);
            s.generation += 1;
        }
        self.inner.lives.set_progress(0.0);
        self.inner.lives.buffered.set(0.0);
        self.inner.lives.time.set("0:00");
        self.inner.lives.time_of.set("0:00 / 0:00");
        self.inner.surface.clear();
        self.command(&["change-list", "http-header-fields", "clr", ""]);
        let mut user_agent = String::new();
        for (k, v) in &load.headers {
            if k.eq_ignore_ascii_case("user-agent") {
                user_agent = v.clone();
            } else if !k.contains(['\r', '\n']) && !v.contains(['\r', '\n']) {
                self.command(&[
                    "change-list",
                    "http-header-fields",
                    "append",
                    &format!("{k}: {v}"),
                ]);
            }
        }
        self.set(
            "user-agent",
            if user_agent.is_empty() {
                "Mozilla/5.0"
            } else {
                &user_agent
            },
        );
        self.command(&["change-list", "audio-files", "clr", ""]);
        if let Some(a) = &load.audio_url {
            self.command(&["change-list", "audio-files", "append", a]);
        }
        self.set(
            "start",
            &load
                .start
                .filter(|s| *s > 1.0)
                .map(|s| format!("{s:.3}"))
                .unwrap_or_else(|| "none".into()),
        );
        self.set(
            "end",
            &load
                .end
                .map(|e| format!("{e:.3}"))
                .unwrap_or_else(|| "none".into()),
        );
        self.set("vid", if load.audio_only { "no" } else { "auto" });
        self.set("force-media-title", load.title.as_deref().unwrap_or(""));
        self.set_flag("pause", load.paused);
        self.set_flag("mute", load.muted);
        *self.inner.pending_subs.lock().unwrap() = load.subtitles.clone();
        self.command_async(&["loadfile", &load.url, "replace"]);
        self.inner.wake();
    }

    pub fn stop(&self) {
        self.command(&["stop"]);
        self.inner.surface.clear();
        let mut s = self.inner.snapshot.lock().unwrap();
        s.loaded = false;
        s.idle = true;
        s.title = None;
        s.video_size = None;
        drop(s);
        self.inner.wake();
    }

    pub fn toggle_pause(&self) {
        self.command(&["cycle", "pause"]);
    }

    pub fn set_paused(&self, paused: bool) {
        self.set_flag("pause", paused);
    }

    pub fn seek_to(&self, seconds: f64) {
        let s = format!("{:.3}", seconds.max(0.0));
        self.command_async(&["seek", &s, "absolute+exact"]);
        let mut snap = self.inner.snapshot.lock().unwrap();
        snap.time = seconds.max(0.0);
        if let Some(d) = snap.duration.filter(|d| *d > 0.0) {
            self.inner
                .lives
                .set_progress((seconds / d).clamp(0.0, 1.0) as f32);
        }
    }

    pub fn set_volume(&self, volume: f64) {
        self.set_f64("volume", volume.clamp(0.0, 150.0));
    }

    pub fn set_speed(&self, speed: f64) {
        self.set_f64("speed", speed.clamp(0.25, 4.0));
    }

    /// Replaces the audio filter chain: the equalizer (if any) and the level meter.
    pub fn set_audio_filters(&self, eq: Option<&str>, meter: bool) {
        let mut chain: Vec<&str> = Vec::new();
        if let Some(eq) = eq {
            chain.push(eq);
        }
        if meter {
            chain.push(METER);
        }
        self.set("af", &chain.join(","));
    }

    pub fn set_video_filter(&self, filter: Option<&str>) {
        self.set("vf", filter.unwrap_or(""));
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        self.inner.stop.store(true, Ordering::Relaxed);
        {
            let mut g = self.inner.gate.lock().unwrap();
            g.stop = true;
            self.inner.gate_cv.notify_all();
        }
        unsafe { mpv_wakeup(self.inner.mpv.0) };
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        unsafe { mpv_terminate_destroy(self.inner.mpv.0) };
    }
}

/// Level meter: RMS and peak per block, read back through `af-metadata`.
pub const METER: &str = "@vidcue-meter:lavfi=[astats=metadata=1:reset=1:length=0.05:measure_perchannel=RMS_level+Peak_level:measure_overall=RMS_level+Peak_level]";

const OBSERVED: &[(&str, mpv_format)] = &[
    ("time-pos", MPV_FORMAT_DOUBLE),
    ("duration", MPV_FORMAT_DOUBLE),
    ("pause", MPV_FORMAT_FLAG),
    ("paused-for-cache", MPV_FORMAT_FLAG),
    ("seeking", MPV_FORMAT_FLAG),
    ("eof-reached", MPV_FORMAT_FLAG),
    ("idle-active", MPV_FORMAT_FLAG),
    ("volume", MPV_FORMAT_DOUBLE),
    ("mute", MPV_FORMAT_FLAG),
    ("speed", MPV_FORMAT_DOUBLE),
    ("dwidth", MPV_FORMAT_INT64),
    ("dheight", MPV_FORMAT_INT64),
    ("demuxer-cache-time", MPV_FORMAT_DOUBLE),
    ("cache-buffering-state", MPV_FORMAT_INT64),
    ("media-title", MPV_FORMAT_STRING),
    ("sub-text", MPV_FORMAT_STRING),
    ("track-list", MPV_FORMAT_NODE),
    ("audio-device-list", MPV_FORMAT_NODE),
    ("audio-device", MPV_FORMAT_STRING),
    ("chapter-list", MPV_FORMAT_NODE),
    ("hwdec-current", MPV_FORMAT_STRING),
    ("video-codec", MPV_FORMAT_STRING),
    ("af-metadata/vidcue-meter", MPV_FORMAT_NODE),
];

fn level(v: &Value, key: &str) -> Option<f32> {
    let v = v.get(key)?;
    let db = v.as_f64().or_else(|| v.as_str()?.parse::<f64>().ok())?;
    if db == f64::NEG_INFINITY {
        Some(0.0)
    } else if db.is_finite() {
        Some(((db + 60.0) / 60.0).clamp(0.0, 1.0) as f32)
    } else {
        None
    }
}

fn event_loop(inner: &Inner) {
    let mpv = inner.mpv.0;
    loop {
        if inner.stop.load(Ordering::Relaxed) {
            return;
        }
        let ev = unsafe { &*mpv_wait_event(mpv, 0.5) };
        match ev.event_id {
            MPV_EVENT_NONE => continue,
            MPV_EVENT_SHUTDOWN => return,
            MPV_EVENT_FILE_LOADED => {
                refresh_size(inner);
                let subs = std::mem::take(&mut *inner.pending_subs.lock().unwrap());
                for (url, title) in subs {
                    let args = [c("sub-add"), c(&url), c("auto"), c(&title)];
                    let mut ptrs: Vec<*const std::ffi::c_char> =
                        args.iter().map(|s| s.as_ptr()).collect();
                    ptrs.push(std::ptr::null());
                    unsafe { mpv_command_async(mpv, 0, ptrs.as_mut_ptr()) };
                }
                let mut s = inner.snapshot.lock().unwrap();
                s.loaded = true;
                s.idle = false;
                s.error = None;
                drop(s);
                inner.wake();
            }
            MPV_EVENT_VIDEO_RECONFIG => {
                refresh_size(inner);
                inner.wake();
            }
            MPV_EVENT_END_FILE => {
                let end = unsafe { &*(ev.data as *const mpv_event_end_file) };
                if end.reason == MPV_END_FILE_REASON_ERROR {
                    let msg = unsafe { cstr(mpv_error_string(end.error)) }
                        .unwrap_or_else(|| "playback failed".into());
                    let mut s = inner.snapshot.lock().unwrap();
                    s.error = Some(msg);
                    s.loaded = false;
                    s.ended_generation = s.generation;
                    drop(s);
                    inner.wake();
                }
            }
            MPV_EVENT_PROPERTY_CHANGE => {
                let prop = unsafe { &*(ev.data as *const mpv_event_property) };
                let Some(name) = (unsafe { cstr(prop.name) }) else {
                    continue;
                };
                if property(inner, &name, prop) {
                    inner.wake();
                }
            }
            _ => {}
        }
    }
}

/// Applies one property change; true when the view should re-run.
fn property(inner: &Inner, name: &str, prop: &mpv_event_property) -> bool {
    let f64v = || {
        (prop.format == MPV_FORMAT_DOUBLE && !prop.data.is_null())
            .then(|| unsafe { *(prop.data as *const f64) })
    };
    let flag = || {
        prop.format == MPV_FORMAT_FLAG
            && !prop.data.is_null()
            && unsafe { *(prop.data as *const std::ffi::c_int) } != 0
    };
    let int = || {
        (prop.format == MPV_FORMAT_INT64 && !prop.data.is_null())
            .then(|| unsafe { *(prop.data as *const i64) })
    };
    let string = || {
        (prop.format == MPV_FORMAT_STRING && !prop.data.is_null())
            .then(|| unsafe { cstr(*(prop.data as *const *const std::ffi::c_char)) })
            .flatten()
    };
    let node = || {
        if prop.format == MPV_FORMAT_NODE && !prop.data.is_null() {
            unsafe { node_to_json(&*(prop.data as *const mpv_node)) }
        } else {
            Value::Null
        }
    };
    let mut s = inner.snapshot.lock().unwrap();
    match name {
        "time-pos" => {
            let Some(t) = f64v() else { return false };
            let whole = s.time as u64 != t as u64;
            s.time = t;
            if let Some(d) = s.duration.filter(|d| *d > 0.0) {
                inner.lives.set_progress((t / d).clamp(0.0, 1.0) as f32);
                if whole {
                    inner
                        .lives
                        .time_of
                        .set(format!("{} / {}", clock(t), clock(d)));
                }
            }
            if whole {
                inner.lives.time.set(clock(t));
            }
            false
        }
        "duration" => {
            s.duration = f64v().filter(|d| d.is_finite() && *d > 0.0);
            true
        }
        "demuxer-cache-time" => {
            s.cached_until = f64v();
            if let (Some(c), Some(d)) = (s.cached_until, s.duration.filter(|d| *d > 0.0)) {
                inner.lives.buffered.set((c / d).clamp(0.0, 1.0) as f32);
            }
            false
        }
        "pause" => {
            s.paused = flag();
            true
        }
        "paused-for-cache" => {
            s.buffering = flag();
            true
        }
        "seeking" => {
            s.seeking = flag();
            true
        }
        "eof-reached" => {
            let eof = flag();
            let changed = eof != s.eof;
            s.eof = eof;
            if eof {
                s.ended_generation = s.generation;
            }
            changed
        }
        "idle-active" => {
            s.idle = flag();
            true
        }
        "volume" => {
            s.volume = f64v().unwrap_or(s.volume);
            true
        }
        "mute" => {
            s.muted = flag();
            true
        }
        "speed" => {
            s.speed = f64v().unwrap_or(1.0);
            true
        }
        "dwidth" | "dheight" => {
            let v = int().unwrap_or(0).max(0) as u32;
            if name == "dwidth" {
                s.dims.0 = v
            } else {
                s.dims.1 = v
            }
            let (w, h) = s.dims;
            s.video_size = (w > 0 && h > 0).then_some((w, h));
            drop(s);
            let mut g = inner.gate.lock().unwrap();
            g.pending = true;
            inner.gate_cv.notify_one();
            true
        }
        "cache-buffering-state" => {
            s.cache_percent = int().map(|v| v as f64);
            false
        }
        "media-title" => {
            let t = string();
            let changed = t != s.title;
            if t.as_deref().is_some_and(|t| !t.is_empty()) {
                s.title = t;
            }
            changed
        }
        "sub-text" => {
            let t = string().filter(|t| !t.trim().is_empty());
            let changed = t != s.sub_text;
            s.sub_text = t;
            changed
        }
        "track-list" => {
            s.tracks = node();
            true
        }
        "audio-device-list" => {
            s.audio_devices = node();
            true
        }
        "audio-device" => {
            s.audio_device = string();
            true
        }
        "chapter-list" => {
            s.chapters = node();
            true
        }
        "hwdec-current" => {
            s.hwdec = string();
            false
        }
        "video-codec" => {
            s.video_codec = string();
            false
        }
        "af-metadata/vidcue-meter" => {
            drop(s);
            let v = node();
            if let Some(l) = level(&v, "lavfi.astats.1.RMS_level")
                .or_else(|| level(&v, "lavfi.astats.Overall.RMS_level"))
            {
                let r = level(&v, "lavfi.astats.2.RMS_level").unwrap_or(l);
                let p = level(&v, "lavfi.astats.Overall.Peak_level").unwrap_or(l.max(r));
                inner.lives.set_levels(l, r, p);
            }
            false
        }
        _ => false,
    }
}

/// Reads the display size straight from mpv. Observed properties only report changes, so a
/// file with the same size as the one before it sends no `dwidth`/`dheight` event after
/// `load` cleared them; without this the renderer would never draw it (sound, no picture).
fn refresh_size(inner: &Inner) {
    let get = |name: &str| {
        let mut v: i64 = 0;
        let ok = unsafe {
            mpv_get_property(
                inner.mpv.0,
                c(name).as_ptr(),
                MPV_FORMAT_INT64,
                (&mut v as *mut i64).cast(),
            )
        } >= 0;
        (ok && v > 0).then_some(v as u32)
    };
    let (Some(w), Some(h)) = (get("dwidth"), get("dheight")) else {
        return;
    };
    {
        let mut s = inner.snapshot.lock().unwrap();
        s.dims = (w, h);
        s.video_size = Some((w, h));
    }
    let mut g = inner.gate.lock().unwrap();
    g.pending = true;
    inner.gate_cv.notify_one();
}

unsafe extern "C" fn on_update(ctx: *mut c_void) {
    let inner = unsafe { &*(ctx as *const Inner) };
    let mut g = inner.gate.lock().unwrap();
    g.pending = true;
    inner.gate_cv.notify_one();
}

/// The size to render a `vw`×`vh` picture at inside a `tw`×`th` target: aspect kept, never
/// upscaled, width a multiple of 16 (64-byte rows for mpv's SIMD paths), height even.
pub fn render_size(vw: u32, vh: u32, tw: u32, th: u32) -> (u32, u32) {
    let (vw, vh) = (vw.max(2) as f64, vh.max(2) as f64);
    // Never more than a 1080p frame, in either orientation.
    let cap = (1920.0 / vw.max(vh)).min(1080.0 / vw.min(vh));
    let scale = (tw as f64 / vw).min(th as f64 / vh).min(1.0).min(cap);
    let w = ((vw * scale) as u32 / 16 * 16).max(16);
    let h = (((w as f64) * vh / vw).round() as u32 / 2 * 2).max(2);
    (w, h)
}

fn render_loop(inner: &Arc<Inner>) {
    let mpv = inner.mpv.0;
    let mut ctx: *mut mpv_render_context = std::ptr::null_mut();
    let api = c("sw");
    let mut params = [
        mpv_render_param {
            type_: MPV_RENDER_PARAM_API_TYPE,
            data: api.as_ptr() as *mut c_void,
        },
        mpv_render_param {
            type_: MPV_RENDER_PARAM_INVALID,
            data: std::ptr::null_mut(),
        },
    ];
    if unsafe { mpv_render_context_create(&mut ctx, mpv, params.as_mut_ptr()) } < 0 || ctx.is_null()
    {
        inner.snapshot.lock().unwrap().error =
            Some("libmpv's software renderer is unavailable".into());
        inner.wake();
        return;
    }
    unsafe {
        mpv_render_context_set_update_callback(
            ctx,
            Some(on_update),
            Arc::as_ptr(inner) as *mut c_void,
        )
    };
    let format = c("bgr0");
    let mut frame_id = 0u64;
    let mut last_size = (0u32, 0u32);
    loop {
        {
            let mut g = inner.gate.lock().unwrap();
            while !g.pending && !g.stop {
                g = inner.gate_cv.wait(g).unwrap();
            }
            if g.stop {
                break;
            }
            g.pending = false;
        }
        let flags = unsafe { mpv_render_context_update(ctx) };
        let video = inner.snapshot.lock().unwrap().video_size;
        let Some((vw, vh)) = video else { continue };
        let target = inner.target.load(Ordering::Relaxed);
        let (w, h) = render_size(vw, vh, (target >> 32) as u32, (target & 0xffff_ffff) as u32);
        let resized = (w, h) != last_size;
        if flags & MPV_RENDER_UPDATE_FRAME == 0 && !resized {
            continue;
        }
        last_size = (w, h);
        let stride = w as usize * 4;
        let mut pixels = vec![0u8; stride * h as usize];
        let mut size = [w as std::ffi::c_int, h as std::ffi::c_int];
        let mut stride_v: usize = stride;
        let mut params = [
            mpv_render_param {
                type_: MPV_RENDER_PARAM_SW_SIZE,
                data: size.as_mut_ptr().cast(),
            },
            mpv_render_param {
                type_: MPV_RENDER_PARAM_SW_FORMAT,
                data: format.as_ptr() as *mut c_void,
            },
            mpv_render_param {
                type_: MPV_RENDER_PARAM_SW_STRIDE,
                data: (&mut stride_v as *mut usize).cast(),
            },
            mpv_render_param {
                type_: MPV_RENDER_PARAM_SW_POINTER,
                data: pixels.as_mut_ptr().cast(),
            },
            mpv_render_param {
                type_: MPV_RENDER_PARAM_INVALID,
                data: std::ptr::null_mut(),
            },
        ];
        if unsafe { mpv_render_context_render(ctx, params.as_mut_ptr()) } < 0 {
            continue;
        }
        // bgr0 leaves the fourth byte undefined; the surface reads BGRA, so make it opaque.
        for px in pixels.as_chunks_mut::<4>().0 {
            px[3] = 255;
        }
        frame_id += 1;
        let frame = ByteFrame {
            width: w,
            height: h,
            format: Format::Bgra,
            color: ColorInfo::default(),
            planes: [pixels, Vec::new()],
        };
        if inner.surface.push_bytes(frame, frame_id).is_ok() {
            inner.frames.fetch_add(1, Ordering::Relaxed);
        }
    }
    unsafe {
        mpv_render_context_set_update_callback(ctx, None, std::ptr::null_mut());
        mpv_render_context_free(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opt-in throughput regression: real libmpv -> software renderer -> Rune surface.
    /// Supply a local 8s 1080p60 clip; no GUI, network, accounts or real audio output.
    /// VIDCUE_BENCH_VF can replay an old filter to confirm the assertion catches the lag.
    #[test]
    #[ignore = "requires VIDCUE_BENCH_CLIP (8s 1080p60) and a performance-capable host"]
    fn graded_1080p60_keeps_up() {
        let clip = std::env::var("VIDCUE_BENCH_CLIP").expect("local 8s 1080p60 clip");
        let grade = std::env::var("VIDCUE_BENCH_GRADE").unwrap_or("cinematic".into());
        let filter = std::env::var("VIDCUE_BENCH_VF")
            .ok()
            .or_else(|| super::super::session::grade_filter(&grade));
        let surface = NativeSurface::new();
        let mpv = Mpv::new(surface).expect("libmpv starts");
        assert!(mpv.set("ao", "null"));
        mpv.set_target(1920.0, 1080.0);
        assert!(
            mpv.set("vf", filter.as_deref().unwrap_or("")),
            "filter accepted"
        );
        mpv.load(&Load {
            url: clip,
            ..Default::default()
        });
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(15) && !mpv.snapshot().eof {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let elapsed = start.elapsed().as_secs_f64();
        let snap = mpv.snapshot();
        let frames = mpv.frames();
        eprintln!(
            "grade={grade} frames={frames}/480 wall={elapsed:.2}s hwdec={:?}",
            snap.hwdec
        );
        assert!(snap.error.is_none(), "{snap:?}");
        assert!(snap.eof, "playback reached EOF");
        assert!(
            snap.duration.is_some_and(|d| (d - 8.0).abs() < 0.1),
            "8s fixture required"
        );
        assert!(
            elapsed < 10.0 && frames >= 432,
            "1080p60 must deliver >=90% of frames in real time: {frames} in {elapsed:.2}s"
        );
    }

    #[test]
    fn render_size_keeps_aspect_never_upscales_and_aligns_rows() {
        assert_eq!(render_size(1920, 1080, 4000, 4000), (1920, 1080));
        let (w, h) = render_size(1920, 1080, 960, 540);
        assert_eq!((w, h), (960, 540));
        let (w, h) = render_size(640, 360, 2000, 2000);
        assert_eq!((w, h), (640, 360));
        let (w, h) = render_size(3840, 2160, 3840, 2160);
        assert!(
            w <= 1920 && h <= 1080 && w % 16 == 0 && h % 2 == 0,
            "{w}x{h}"
        );
        let (w, h) = render_size(1080, 1920, 500, 500);
        assert!(h <= 500 && w % 16 == 0, "portrait {w}x{h}");
    }

    /// End to end, offline: a generated clip decodes through libmpv's software renderer into
    /// the Rune surface, reports its duration and size, and reaches EOF.
    #[test]
    fn a_generated_clip_plays_into_the_surface() {
        let dir = std::env::temp_dir().join(format!("vidcue-mpv-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let clip = dir.join("clip.mp4");
        let made = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=640x360:rate=30:duration=2",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=2",
                "-shortest",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ])
            .arg(&clip)
            .status();
        if !made.is_ok_and(|s| s.success()) {
            eprintln!("ffmpeg unavailable; skipping");
            return;
        }
        let surface = NativeSurface::new();
        let mpv = Mpv::new(surface.clone()).expect("libmpv starts");
        mpv.set("ao", "null");
        mpv.set_target(640.0, 360.0);
        mpv.load(&Load {
            url: clip.to_string_lossy().into(),
            title: Some("clip".into()),
            ..Default::default()
        });
        let start = std::time::Instant::now();
        let mut snap = mpv.snapshot();
        let mut seen_size = None;
        while start.elapsed() < std::time::Duration::from_secs(15) && !snap.eof {
            std::thread::sleep(std::time::Duration::from_millis(50));
            snap = mpv.snapshot();
            seen_size = seen_size.or(snap.video_size);
        }
        assert!(snap.error.is_none(), "{:?}", snap.error);
        assert!(snap.eof, "reached the end: {snap:?}");
        assert_eq!(seen_size, Some((640, 360)));
        assert!(
            snap.duration.is_some_and(|d| (d - 2.0).abs() < 0.2),
            "{:?}",
            snap.duration
        );
        assert!(mpv.frames() >= 20, "frames pushed: {}", mpv.frames());

        // The same size again: mpv reports no size change, yet the picture must still draw.
        let before = mpv.frames();
        mpv.load(&Load {
            url: clip.to_string_lossy().into(),
            ..Default::default()
        });
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(15) && mpv.frames() < before + 20 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(
            mpv.frames() >= before + 20,
            "second same-size file drew {} frames",
            mpv.frames() - before
        );
        assert_eq!(mpv.snapshot().video_size, Some((640, 360)));
        drop(mpv);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn display_levels_spread_the_range_music_uses() {
        assert_eq!(display_level(0.0), 0.0);
        assert_eq!(display_level(1.0), 1.0);
        let quiet = display_level((60.0 - 30.0) / 60.0);
        let loud = display_level((60.0 - 10.0) / 60.0);
        assert!(quiet < 0.25 && loud > 0.8, "{quiet} {loud}");
    }

    #[test]
    fn clock_formats_minutes_and_hours() {
        assert_eq!(clock(5.4), "0:05");
        assert_eq!(clock(754.0), "12:34");
        assert_eq!(clock(3725.0), "1:02:05");
    }
}
