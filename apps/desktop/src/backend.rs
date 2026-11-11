//! The bridge between Rune's UI thread and the shared Vidcue engine.
//!
//! Every engine call and background task runs on a Tokio runtime. Its answer comes back as a
//! closure queued for the UI thread: `poll` drains the queue, runs each closure with `&mut App`
//! and returns true, so the view re-runs once per batch of answers and an idle window stays idle.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rune::{Tx, Waker};
use serde_json::{Value, json};
use vidcue_mobile::Engine;

use crate::app::App;

/// A closure that lands an answer in app state, on the UI thread.
pub type Landing = Box<dyn FnOnce(&mut App, &mut Tx) + Send>;

/// An engine failure classified the way the Android client classifies it, so the UI can offer
/// the same actions (sign in, retry after a delay, search instead of an empty Home).
#[derive(Clone, Debug)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
    pub retry_after_ms: Option<u64>,
}

impl ApiError {
    pub fn from_anyhow(error: &anyhow::Error) -> Self {
        let text = vidcue_providers::diagnostic(&format!("{error:#}"));
        let lower = text.to_ascii_lowercase();
        let (code, message) = if lower.contains("no anonymous home recommendations") {
            (
                "anonymous_home_empty",
                "YouTube did not provide anonymous recommendations. Search for something, or sign in for a personal feed."
                    .to_owned(),
            )
        } else if lower.contains("sign in first") || lower.contains("session expired") {
            (
                "auth_required",
                "Sign in to open this feed. Open Account to sign in or import cookies.".to_owned(),
            )
        } else if text.contains("F2") || text.contains("vidcue auth") {
            (
                "consent_required",
                "YouTube needs consent or a refreshed session. Open Account to sign in again."
                    .to_owned(),
            )
        } else if lower.contains("likely being blocked")
            || lower.contains("http 429")
            || lower.contains("rate limit")
        {
            ("provider_rejected", text)
        } else {
            ("provider_error", text)
        };
        let retry_after_ms = error
            .downcast_ref::<vidcue_providers::browse::FeedRetry>()
            .map(|r| r.after.as_millis().min(u64::MAX as u128) as u64);
        Self {
            code,
            message,
            retry_after_ms,
        }
    }

    pub fn retryable(&self) -> bool {
        self.retry_after_ms.is_some()
    }

    pub fn needs_account(&self) -> bool {
        matches!(self.code, "auth_required" | "consent_required")
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Where the desktop app keeps its engine state. These sit beside the terminal app's folders
/// (`~/.local/state/vidcue`, `~/.cache/vidcue`), after the ytui → vidcue folder migration.
#[derive(Clone, Debug)]
pub struct DesktopPaths {
    pub shared: vidcue_providers::paths::Paths,
    pub root: PathBuf,
    pub cache: PathBuf,
}

impl DesktopPaths {
    pub fn discover() -> Result<Self> {
        let shared = vidcue_providers::paths::Paths::discover()?;
        let state_root = shared
            .state
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| shared.state.clone());
        let cache_root = shared
            .cache
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| shared.cache.clone());
        Ok(Self {
            root: state_root.join("desktop"),
            cache: cache_root.join("desktop"),
            shared,
        })
    }

    /// A throwaway location for headless renders and tests: nothing touches the user's files.
    pub fn ephemeral() -> Result<Self> {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("vidcue-desktop-{}-{n}", std::process::id()));
        let shared = vidcue_providers::paths::Paths {
            config_override: None,
            xdg_state: dir.join("state"),
            config: dir.join("config"),
            state: dir.join("state/rust"),
            cache: dir.join("cache/rust"),
            legacy_data: dir.join("legacy"),
            home: dir.clone(),
        };
        Ok(Self {
            root: dir.join("state/desktop"),
            cache: dir.join("cache/desktop"),
            shared,
        })
    }

    pub fn file(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

/// Seeds the engine's settings from the terminal app's `rust.toml` the first time the desktop
/// app starts, so a theme, proxy, radio servers or download folders carry over. Never writes
/// to the old file.
fn import_terminal_settings(paths: &DesktopPaths) {
    let target = paths.root.join("config.json");
    if target.exists() {
        return;
    }
    let source = paths.shared.config_file();
    if !source.is_file() {
        return;
    }
    let Ok(config) = paths.shared.load_config() else {
        return;
    };
    if config.validate().is_err() {
        return;
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(&config) {
        let _ = vidcue_providers::paths::atomic_write(&target, &bytes);
    }
}

/// Engine operations that only touch local state. Offline (headless renders and tests) they run
/// against the throwaway engine directory; everything else fails at once without the network.
const LOCAL_OPS: &[&str] = &[
    "settings",
    "save_settings",
    "themes",
    "palettes",
    "theme",
    "import_theme",
    "equalizer_presets",
    "saved",
    "save_media",
    "queue",
    "enqueue",
    "queue_peek",
    "queue_commit",
    "queue_remove",
    "queue_move",
    "up_next",
    "seed_context",
    "offline_context",
    "radio_context",
    "context_status",
    "history_record",
    "history_list",
    "history_peek",
    "history_clear",
    "position",
    "offline_playlists",
    "offline_playlist_create",
    "offline_playlist_update",
    "offline_playlist_delete",
    "offline_playlist_download",
    "lyrics_get",
    "lyrics_save",
    "lyrics_remove",
    "account",
    "resolve_cache_clear",
];

#[derive(Clone)]
pub struct Backend {
    pub rt: Arc<tokio::runtime::Runtime>,
    pub engine: Arc<Engine>,
    pub paths: DesktopPaths,
    inbox: Arc<Mutex<Vec<Landing>>>,
    waker: Arc<Mutex<Option<Waker>>>,
    /// True in headless renders: nothing reaches the network, so snapshots stay deterministic.
    pub offline: bool,
}

impl Backend {
    pub fn start(offline: bool) -> Result<Self> {
        let paths = if offline {
            DesktopPaths::ephemeral()?
        } else {
            DesktopPaths::discover()?
        };
        vidcue_providers::paths::private_dir(&paths.root)?;
        vidcue_providers::paths::private_dir(&paths.cache)?;
        if !offline {
            import_terminal_settings(&paths);
        }
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("vidcue-engine")
            .enable_all()
            .build()
            .context("Cannot start the engine runtime")?;
        let engine = {
            let _guard = rt.enter();
            Engine::new(&paths.root, &paths.cache)?
        };
        Ok(Self {
            rt: Arc::new(rt),
            engine: Arc::new(engine),
            paths,
            inbox: Arc::default(),
            waker: Arc::default(),
            offline,
        })
    }

    pub fn set_waker(&self, waker: Waker) {
        *self.waker.lock().unwrap() = Some(waker);
    }

    /// Queues a closure for the UI thread and wakes the window.
    pub fn land(&self, landing: Landing) {
        self.inbox.lock().unwrap().push(landing);
        if let Some(w) = self.waker.lock().unwrap().as_ref() {
            w();
        }
    }

    pub fn drain(&self) -> Vec<Landing> {
        std::mem::take(&mut *self.inbox.lock().unwrap())
    }

    /// Runs one engine request (`{"op": …}`) and lands its answer.
    pub fn call(
        &self,
        request: Value,
        then: impl FnOnce(&mut App, Result<Value, ApiError>, &mut Tx) + Send + 'static,
    ) {
        if self.offline && !LOCAL_OPS.contains(&request["op"].as_str().unwrap_or_default()) {
            let op = request["op"].as_str().unwrap_or_default().to_owned();
            self.land(Box::new(move |app, tx| {
                then(
                    app,
                    Err(ApiError {
                        code: "offline",
                        message: format!("{op} is unavailable offline"),
                        retry_after_ms: None,
                    }),
                    tx,
                )
            }));
            return;
        }
        let engine = self.engine.clone();
        let me = self.clone();
        self.rt.spawn(async move {
            let name = request["op"].as_str().unwrap_or_default().to_owned();
            let started = std::time::Instant::now();
            let answer = engine
                .request(request)
                .await
                .map_err(|e| ApiError::from_anyhow(&e));
            if std::env::var_os("VIDCUE_DESKTOP_TRACE").is_some() {
                // Op names and error codes only: no URLs, headers or account material.
                match &answer {
                    Ok(_) => eprintln!("vidcue: {name} ok in {} ms", started.elapsed().as_millis()),
                    Err(e) => eprintln!(
                        "vidcue: {name} failed ({}) in {} ms",
                        e.code,
                        started.elapsed().as_millis()
                    ),
                }
            }
            me.land(Box::new(move |app, tx| then(app, answer, tx)));
        });
    }

    /// Runs any future on the engine runtime and lands its output.
    pub fn task<T: Send + 'static>(
        &self,
        work: impl Future<Output = T> + Send + 'static,
        then: impl FnOnce(&mut App, T, &mut Tx) + Send + 'static,
    ) {
        let me = self.clone();
        self.rt.spawn(async move {
            let out = work.await;
            me.land(Box::new(move |app, tx| then(app, out, tx)));
        });
    }

    /// Runs a blocking closure on Tokio's blocking pool and lands its output.
    pub fn blocking<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
        then: impl FnOnce(&mut App, T, &mut Tx) + Send + 'static,
    ) {
        let me = self.clone();
        self.rt.spawn(async move {
            if let Ok(out) = tokio::task::spawn_blocking(work).await {
                me.land(Box::new(move |app, tx| then(app, out, tx)));
            }
        });
    }
}

/// Shorthand for an engine request object.
pub fn op(name: &str, mut fields: Value) -> Value {
    if !fields.is_object() {
        fields = json!({});
    }
    fields["op"] = Value::String(name.to_owned());
    fields
}
