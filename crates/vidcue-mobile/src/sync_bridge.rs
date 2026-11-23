//! Engine side of the self-hosted backend (PROTOCOL.md §8): account and sync ops, the
//! sealed secret store, the background sync loop, the WebSocket connection and its
//! events, presence/handoff/remote, health reports and the Jam clock.
use super::*;
use crate::sync_stores::{LocalOp, Stores};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{sync::Notify, task::JoinHandle};
use uuid::Uuid;
use vidcue_sync::{
    SyncEngine,
    clock::{OffsetEstimator, PlayerState, drift_action, expected_position},
    engine::{COLLECTIONS, KeyState},
    pairing,
    protocol::{
        ClientMessage, HealthCategory, HealthKind, JamPlayback, PresenceEntry, PresenceState,
        ServerEvent,
    },
    store::{E2E_KEY, SESSION, SecretStore},
    ws::{WsClient, WsState},
};

/// Delay between a local edit and the push it triggers.
pub const DEBOUNCE: Duration = Duration::from_secs(2);
/// A hidden app keeps the socket this long (unless playing or in a Jam).
pub const HIDDEN_GRACE: Duration = Duration::from_secs(60);
const PRESENCE_MIN_GAP_MS: i64 = 2_000;
const PRESENCE_REFRESH_MS: i64 = 30_000;
const PRESENCE_SEEK_JUMP_MS: i64 = 3_000;
const STATUS_GAP_MS: i64 = 1_000;
const KEY_POLL: Duration = Duration::from_secs(5);
const JAM_PING: Duration = Duration::from_secs(60);
const MAX_HANDOFF_QUEUE: usize = 25;
/// Test builds keep emitted UI events for assertions (Android forwards them).
const MAX_KEPT_EVENTS: usize = 512;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Device token and account key, sealed by the Android Keystore (`ytui.account.v1`) in
/// files separate from the YouTube account, so a YouTube disconnect never touches them.
struct PlatformSecrets {
    dir: PathBuf,
}

impl PlatformSecrets {
    fn path(&self, name: &str) -> Result<PathBuf> {
        let file = match name {
            SESSION => "session",
            E2E_KEY => "e2e",
            _ => anyhow::bail!("Unknown secret"),
        };
        let extension = if cfg!(target_os = "android") {
            "enc"
        } else {
            "txt"
        };
        Ok(self.dir.join(format!("{file}.{extension}")))
    }
}

impl SecretStore for PlatformSecrets {
    fn load(&self, name: &str) -> Result<Option<String>> {
        use std::io::Read;
        let path = self.path(name)?;
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let mut text = String::new();
        file.take(1024 * 1024 + 1).read_to_string(&mut text)?;
        ensure!(
            text.len() <= 1024 * 1024,
            "Stored sync secret exceeds limit"
        );
        #[cfg(target_os = "android")]
        let text = crate::platform::account_cipher("openAccount", &text)?;
        Ok(Some(text))
    }
    fn save(&self, name: &str, value: &str) -> Result<()> {
        let path = self.path(name)?;
        #[cfg(target_os = "android")]
        let value = crate::platform::account_cipher("sealAccount", value)?;
        vidcue_sync::store::write_atomic(&path, value.as_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
    fn delete(&self, name: &str) -> Result<()> {
        vidcue_sync::store::remove_file(&self.path(name)?)
    }
}

#[derive(Default)]
struct Foreground {
    visible: bool,
    playing: bool,
    close_timer: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct PresenceOut {
    key: Option<String>,
    playing: bool,
    position_ms: u64,
    sent_at: i64,
    pending: Option<PresenceState>,
    timer: Option<JoinHandle<()>>,
}

#[derive(Default)]
pub(crate) struct JamLocal {
    room: Option<Uuid>,
    playback: Option<JamPlayback>,
    clock: OffsetEstimator,
    last_seek: Option<i64>,
    tasks: Vec<JoinHandle<()>>,
}

impl JamLocal {
    fn stop(&mut self) {
        for task in self.tasks.drain(..) {
            task.abort();
        }
        self.room = None;
        self.playback = None;
        self.last_seek = None;
    }
}

#[derive(Default)]
struct StatusEmit {
    last_at: i64,
    last: String,
    timer: Option<JoinHandle<()>>,
    key_state: Option<KeyState>,
}

struct Connection {
    client: Arc<WsClient>,
    tasks: Vec<JoinHandle<()>>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

pub(crate) struct Inner {
    dir: PathBuf,
    secrets: Arc<dyn SecretStore>,
    engine: Mutex<Option<Arc<SyncEngine>>>,
    notice: Mutex<Option<String>>,
    owner: OnceLock<Weak<Engine>>,
    connection: Mutex<Option<Connection>>,
    sync_task: Mutex<Option<JoinHandle<()>>>,
    key_task: Mutex<Option<JoinHandle<()>>>,
    trigger: Notify,
    urgent: AtomicBool,
    foreground: Mutex<Foreground>,
    events: Mutex<VecDeque<(String, String)>>,
    presence: Mutex<BTreeMap<Uuid, PresenceEntry>>,
    presence_out: Mutex<PresenceOut>,
    jam: Mutex<JamLocal>,
    plays: Mutex<HashMap<String, Uuid>>,
    status: Mutex<StatusEmit>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        for slot in [&self.sync_task, &self.key_task] {
            if let Some(task) = lock(slot).take() {
                task.abort();
            }
        }
        lock(&self.jam).stop();
    }
}

/// Owned by `Engine`; cheap to clone into background tasks.
#[derive(Clone)]
pub(crate) struct SyncHandle(Arc<Inner>);

impl SyncHandle {
    pub(crate) fn new(root: &Path) -> Self {
        let dir = root.join("sync");
        let secrets: Arc<dyn SecretStore> = Arc::new(PlatformSecrets { dir: dir.clone() });
        let (engine, notice) = match SyncEngine::open(&dir, secrets.clone()) {
            Ok(engine) => (engine.map(Arc::new), None),
            // An unreadable session never blocks the offline library.
            Err(_) => (
                None,
                Some("The Vidcue server sign-in is unavailable; sign in again.".to_owned()),
            ),
        };
        Self(Arc::new(Inner {
            dir,
            secrets,
            engine: Mutex::new(engine),
            notice: Mutex::new(notice),
            owner: OnceLock::new(),
            connection: Mutex::new(None),
            sync_task: Mutex::new(None),
            key_task: Mutex::new(None),
            trigger: Notify::new(),
            urgent: AtomicBool::new(false),
            foreground: Mutex::new(Foreground::default()),
            events: Mutex::new(VecDeque::new()),
            presence: Mutex::new(BTreeMap::new()),
            presence_out: Mutex::new(PresenceOut::default()),
            jam: Mutex::new(JamLocal::default()),
            plays: Mutex::new(HashMap::new()),
            status: Mutex::new(StatusEmit::default()),
        }))
    }

    /// Starts background work (sync loop) once the engine is shared.
    pub(crate) fn start(&self, owner: &Arc<Engine>) {
        let _ = self.0.owner.set(Arc::downgrade(owner));
        if self.engine().is_some() {
            self.0.start_loop();
            self.trigger(true);
        }
    }

    pub(crate) fn engine(&self) -> Option<Arc<SyncEngine>> {
        self.0.engine()
    }

    pub(crate) fn emit(&self, kind: &str, text: &str) {
        self.0.emit(kind, text);
    }

    /// Drains UI events kept by non-Android builds (tests).
    pub(crate) fn take_events(&self) -> Vec<(String, String)> {
        lock(&self.0.events).drain(..).collect()
    }

    pub(crate) fn trigger(&self, urgent: bool) {
        self.0.trigger(urgent);
    }

    /// Queues local writes when signed in; failures surface in the status, never in
    /// the local operation that already succeeded.
    pub(crate) fn enqueue(&self, ops: Vec<LocalOp>) {
        let Some(engine) = self.engine().filter(|e| !e.is_signed_out()) else {
            return;
        };
        if ops.is_empty() {
            return;
        }
        let mut personal = Vec::new();
        let mut result = Ok(());
        for op in ops {
            match op {
                LocalOp::Personal {
                    collection,
                    key,
                    value,
                    clock,
                } => personal.push((collection.to_owned(), key, value, clock)),
                LocalOp::Shared {
                    playlist,
                    entry,
                    value,
                    clock,
                } => {
                    if let Err(e) = engine.enqueue_shared(playlist, entry, value, clock) {
                        result = Err(e);
                    }
                }
            }
        }
        if let Err(e) = engine.enqueue(&personal) {
            result = Err(e);
        }
        *lock(&self.0.notice) = result.err().map(|e| e.to_string());
        self.trigger(false);
        self.0.emit_status();
    }

    /// Queues the YouTube session as an end-to-end encrypted secret (key state `ok`).
    pub(crate) fn push_session(&self, session: &Session, clock: i64) {
        let Some(engine) = self.engine().filter(|e| !e.is_signed_out()) else {
            return;
        };
        let Ok(value) = serde_json::to_value(session) else {
            return;
        };
        if engine
            .enqueue_secret("youtube", &value, clock)
            .unwrap_or(false)
        {
            self.trigger(false);
        }
    }

    pub(crate) fn rename_shared(&self, playlist: Uuid, title: String) {
        let Some(engine) = self.engine() else {
            return;
        };
        crate::runtime().spawn(async move {
            let _ = engine.api().shared_rename(playlist, &title).await;
        });
    }

    /// Remembers the play event of a URL so position saves update it.
    pub(crate) fn set_current_play(&self, url: &str, id: Uuid) {
        let mut plays = lock(&self.0.plays);
        if plays.len() > 64 {
            plays.clear();
        }
        plays.insert(url.into(), id);
    }

    pub(crate) fn current_play(&self, url: &str) -> Option<Uuid> {
        lock(&self.0.plays).get(url).copied()
    }

    /// Records the outcome of a `resolve` (not for speculative prefetches).
    pub(crate) fn health_resolve(&self, result: &Result<Value>, elapsed: Duration) {
        let Some(engine) = self.engine() else {
            return;
        };
        let category = match result {
            Ok(_) => HealthCategory::Ok,
            Err(error) => vidcue_sync::health::classify(&format!("{error:#}")),
        };
        let _ = engine.record_health(
            HealthKind::Resolve,
            category,
            env!("CARGO_PKG_VERSION"),
            "android",
            Some(elapsed.as_millis() as u64),
        );
    }
}

impl Inner {
    fn engine(&self) -> Option<Arc<SyncEngine>> {
        lock(&self.engine).clone()
    }

    fn owner(&self) -> Option<Arc<Engine>> {
        self.owner.get().and_then(Weak::upgrade)
    }

    fn emit(&self, kind: &str, text: &str) {
        #[cfg(target_os = "android")]
        crate::platform::ui_event(kind, text);
        #[cfg(not(target_os = "android"))]
        {
            let mut events = lock(&self.events);
            if events.len() >= MAX_KEPT_EVENTS {
                events.pop_front();
            }
            events.push_back((kind.into(), text.into()));
        }
    }

    fn trigger(&self, urgent: bool) {
        if urgent {
            self.urgent.store(true, Ordering::SeqCst);
        }
        self.trigger.notify_one();
    }

    fn features(&self) -> Vec<String> {
        let mut features = lock(&self.connection)
            .as_ref()
            .map(|c| c.client.features())
            .unwrap_or_default();
        if let Some(engine) = self.engine() {
            for feature in engine.status().server_features {
                if !features.contains(&feature) {
                    features.push(feature);
                }
            }
        }
        features
    }

    fn connected(&self) -> bool {
        lock(&self.connection)
            .as_ref()
            .is_some_and(|c| c.client.connected())
    }

    fn realtime(&self) -> Option<Arc<WsClient>> {
        lock(&self.connection)
            .as_ref()
            .map(|c| c.client.clone())
            .filter(|c| c.realtime())
    }

    pub(crate) fn status(&self) -> Value {
        let Some(engine) = self.engine() else {
            return json!({
                "signed_in": false,
                "server": null, "username": null, "user_id": null, "device_id": null,
                "is_admin": false,
                "state": "signed_out",
                "reason": SyncEngine::signed_out_reason(&self.dir),
                "connected": false,
                "features": [],
                "last_success": 0, "pending": 0, "rejected_count": 0,
                "key_state": "missing",
                "collections": COLLECTIONS.iter().map(|c| (*c, true)).collect::<BTreeMap<_, _>>(),
                "health_reports": false,
                "notice": lock(&self.notice).clone(),
            });
        };
        let session = engine.session();
        let status = engine.status();
        json!({
            "signed_in": !engine.is_signed_out(),
            "server": session.server,
            "username": session.username,
            "user_id": session.user_id,
            "device_id": session.device_id,
            "is_admin": session.is_admin,
            "state": status.state.name(),
            "reason": status.reason,
            "connected": self.connected(),
            "features": self.features(),
            "last_success": status.last_success,
            "pending": status.pending,
            "rejected_count": status.rejected_count,
            "key_state": status.key_state.name(),
            "collections": status.collections,
            "health_reports": status.health_reports,
            "notice": lock(&self.notice).clone().or(status.last_error),
        })
    }

    /// `sync_status` event, at most one per second; also `sync_key_state` on change.
    fn emit_status(self: &Arc<Self>) {
        let key_state = self.engine().map(|e| e.key_state());
        let key_changed = {
            let mut status = lock(&self.status);
            let changed = status.key_state != key_state;
            status.key_state = key_state;
            changed
        };
        if key_changed && let Some(state) = key_state {
            self.emit("sync_key_state", state.name());
        }
        let now = vidcue_sync::now_ms();
        let mut status = lock(&self.status);
        if status.timer.as_ref().is_some_and(|t| !t.is_finished()) {
            return;
        }
        let wait = STATUS_GAP_MS - (now - status.last_at);
        if wait > 0 {
            let inner = self.clone();
            status.timer = Some(crate::runtime().spawn(async move {
                tokio::time::sleep(Duration::from_millis(wait as u64)).await;
                lock(&inner.status).timer = None;
                inner.emit_status_now();
            }));
            return;
        }
        drop(status);
        self.emit_status_now();
    }

    fn emit_status_now(&self) {
        let text = self.status().to_string();
        let mut status = lock(&self.status);
        status.last_at = vidcue_sync::now_ms();
        if status.last == text {
            return;
        }
        status.last = text.clone();
        drop(status);
        self.emit("sync_status", &text);
    }

    fn start_loop(self: &Arc<Self>) {
        let mut slot = lock(&self.sync_task);
        if slot.as_ref().is_some_and(|t| !t.is_finished()) {
            return;
        }
        let Some(owner) = self.owner.get().cloned() else {
            return;
        };
        let inner = self.clone();
        *slot = Some(crate::runtime().spawn(sync_loop(inner, owner)));
    }

    /// One sync run with the engine's library; emits status and key events.
    async fn run_sync(self: &Arc<Self>, owner: &Engine) -> Result<()> {
        let engine = self.engine().context("Sign in to a Vidcue server first")?;
        self.emit_status();
        let result = engine.sync_once(&Stores(owner)).await;
        if engine.is_signed_out() {
            self.stop_connection();
        }
        self.emit_status();
        result
    }

    /// Records a refused token from any call and stops the connection.
    fn checked<T>(self: &Arc<Self>, result: Result<T>) -> Result<T> {
        if let Err(error) = &result
            && let Some(engine) = self.engine()
        {
            engine.note_error(error);
            if engine.is_signed_out() {
                self.stop_connection();
                self.emit_status();
            }
        }
        result
    }

    fn connect(self: &Arc<Self>) -> Result<()> {
        let Some(engine) = self.engine().filter(|e| !e.is_signed_out()) else {
            return Ok(());
        };
        let mut connection = lock(&self.connection);
        if connection.is_some() {
            return Ok(());
        }
        let session = engine.session();
        let _runtime = crate::runtime().enter();
        let client = Arc::new(WsClient::connect(
            &session.server,
            &session.token,
            session.device_id,
        )?);
        let pump = crate::runtime().spawn(event_pump(self.clone(), client.subscribe()));
        let watch = crate::runtime().spawn(state_watch(self.clone(), client.state()));
        *connection = Some(Connection {
            client,
            tasks: vec![pump, watch],
        });
        Ok(())
    }

    fn stop_connection(self: &Arc<Self>) {
        let old = lock(&self.connection).take();
        if old.is_some() {
            lock(&self.presence).clear();
            self.emit("presence_changed", "");
        }
        drop(old);
    }

    fn sign_out_local(self: &Arc<Self>) {
        self.stop_connection();
        lock(&self.jam).stop();
        if let Some(task) = lock(&self.key_task).take() {
            task.abort();
        }
        *lock(&self.engine) = None;
        *lock(&self.presence_out) = PresenceOut::default();
    }

    fn install(self: &Arc<Self>, engine: SyncEngine) -> Result<()> {
        self.sign_out_local();
        *lock(&self.engine) = Some(Arc::new(engine));
        *lock(&self.notice) = None;
        self.start_loop();
        self.connect()?;
        self.trigger(true);
        self.emit_status();
        Ok(())
    }

    fn in_jam(&self) -> bool {
        lock(&self.jam).room.is_some()
    }

    fn set_foreground(self: &Arc<Self>, visible: bool, playing: bool) -> Result<()> {
        let mut foreground = lock(&self.foreground);
        foreground.visible = visible;
        foreground.playing = playing;
        if let Some(timer) = foreground.close_timer.take() {
            timer.abort();
        }
        if visible {
            drop(foreground);
            self.connect()?;
            self.trigger(true);
            return Ok(());
        }
        let inner = self.clone();
        foreground.close_timer = Some(crate::runtime().spawn(async move {
            tokio::time::sleep(HIDDEN_GRACE).await;
            let keep = {
                let foreground = lock(&inner.foreground);
                foreground.visible || foreground.playing
            } || inner.in_jam();
            if !keep {
                inner.stop_connection();
                inner.emit_status();
            }
        }));
        Ok(())
    }

    fn send_presence(&self, state: PresenceState) {
        let Some(client) = self.realtime() else {
            return;
        };
        client.send(&ClientMessage::Presence { state });
    }

    /// Room bookkeeping after create/join/fetch: clock sampling and `jam_ping`.
    fn enter_jam(
        self: &Arc<Self>,
        room: Uuid,
        playback: Option<JamPlayback>,
        server_ms: Option<i64>,
    ) {
        let mut jam = lock(&self.jam);
        if let Some(server_ms) = server_ms {
            jam.clock.seed(vidcue_sync::now_ms(), server_ms);
        }
        if let Some(playback) = playback {
            jam.playback = Some(playback);
        }
        if jam.room == Some(room) {
            return;
        }
        for task in jam.tasks.drain(..) {
            task.abort();
        }
        jam.room = Some(room);
        jam.last_seek = None;
        let sampler = crate::runtime().spawn(clock_sampler(self.clone()));
        let pinger = crate::runtime().spawn(jam_pinger(self.clone(), room));
        jam.tasks = vec![sampler, pinger];
    }

    fn leave_jam(&self, room: Option<Uuid>) {
        let mut jam = lock(&self.jam);
        if room.is_none() || jam.room == room {
            jam.stop();
        }
    }

    fn poll_key(self: &Arc<Self>) {
        let mut slot = lock(&self.key_task);
        if slot.as_ref().is_some_and(|t| !t.is_finished()) {
            return;
        }
        let inner = self.clone();
        *slot = Some(crate::runtime().spawn(async move {
            // Poll every 5 s until granted or expired (15 min); `key_granted` wakes it.
            loop {
                let Some(engine) = inner.engine() else { return };
                if engine.relay_id().is_none() {
                    inner.emit_status();
                    return;
                }
                match engine.key_request_poll().await {
                    Ok(true) => {
                        inner.emit_status();
                        inner.trigger(true);
                        return;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        engine.note_error(&error);
                        if engine.is_signed_out() || error.to_string().contains("key did not match")
                        {
                            *lock(&inner.notice) = Some(error.to_string());
                            inner.emit_status();
                            return;
                        }
                    }
                }
                tokio::time::sleep(KEY_POLL).await;
            }
        }));
    }
}

async fn sync_loop(inner: Arc<Inner>, owner: Weak<Engine>) {
    let mut failures = 0u32;
    loop {
        let now = vidcue_sync::now_ms();
        let retry = inner.engine().and_then(|e| e.next_retry());
        let mut wait = retry.map(|at| Duration::from_millis((at - now).max(0) as u64));
        if failures > 0 {
            let backoff = vidcue_sync::outbox::backoff(failures - 1);
            wait = Some(wait.map_or(backoff, |w| w.max(backoff)));
        }
        tokio::select! {
            _ = inner.trigger.notified() => {
                if !inner.urgent.swap(false, Ordering::SeqCst) {
                    tokio::time::sleep(DEBOUNCE).await;
                    inner.urgent.store(false, Ordering::SeqCst);
                }
            }
            _ = tokio::time::sleep(wait.unwrap_or(Duration::from_secs(24 * 3600))) => {}
        }
        let Some(engine) = owner.upgrade() else {
            return;
        };
        if inner.engine().is_none_or(|e| e.is_signed_out()) {
            failures = 0;
            continue;
        }
        match inner.run_sync(&engine).await {
            Ok(()) => failures = 0,
            Err(_) => failures = failures.saturating_add(1).min(16),
        }
    }
}

async fn state_watch(inner: Arc<Inner>, mut state: tokio::sync::watch::Receiver<WsState>) {
    loop {
        if state.changed().await.is_err() {
            return;
        }
        let current = *state.borrow();
        match current {
            WsState::SignedOut => {
                if let Some(engine) = inner.engine() {
                    engine.revoked();
                }
                inner.stop_connection();
                inner.emit_status();
                return;
            }
            WsState::Connected | WsState::Disconnected => inner.emit_status(),
            _ => {}
        }
    }
}

async fn event_pump(inner: Arc<Inner>, mut events: tokio::sync::broadcast::Receiver<ServerEvent>) {
    loop {
        match events.recv().await {
            Ok(event) => on_event(&inner, event),
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => inner.trigger(true),
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        }
    }
}

fn on_event(inner: &Arc<Inner>, event: ServerEvent) {
    let Some(engine) = inner.engine() else { return };
    let own = engine.session().device_id;
    match event {
        ServerEvent::Hello { seq, server_ms, .. } => {
            if let Some(server_ms) = server_ms {
                lock(&inner.jam).clock.seed(vidcue_sync::now_ms(), server_ms);
            }
            if seq > engine.state().cursor {
                inner.trigger(true);
            }
            if let Some(client) = inner.realtime() {
                client.send(&ClientMessage::PresenceQuery);
            }
            let room = lock(&inner.jam).room;
            if let Some(room) = room {
                // Resample after reconnecting and refetch the room.
                let restart = inner.clone();
                crate::runtime().spawn(async move { sample_clock(&restart, vidcue_sync::clock::RESAMPLES).await });
                inner.emit("jam_changed", &room.to_string());
            }
            inner.emit_status();
        }
        ServerEvent::Changed { device_id, resync, .. } => {
            if resync || device_id != Some(own) {
                inner.trigger(true);
            }
            if resync {
                if let Some(client) = inner.realtime() {
                    client.send(&ClientMessage::PresenceQuery);
                }
                if let Some(room) = lock(&inner.jam).room {
                    inner.emit("jam_changed", &room.to_string());
                }
            }
        }
        ServerEvent::DeviceAdded { .. } => inner.emit_status(),
        ServerEvent::KeyRequest { id, device_id } if device_id != own => {
            let task = inner.clone();
            crate::runtime().spawn(async move {
                let Some(engine) = task.engine() else { return };
                let Ok(requests) = engine.key_requests().await else { return };
                if let Some(request) = requests.iter().find(|r| r["id"] == json!(id)) {
                    task.emit("sync_key_request", &request.to_string());
                }
            });
        }
        ServerEvent::KeyGranted { device_id, .. } if device_id == own => inner.poll_key(),
        ServerEvent::Presence(entry) => {
            if entry.device_id != own {
                lock(&inner.presence).insert(entry.device_id, entry);
                inner.emit("presence_changed", "");
            }
        }
        ServerEvent::PresenceGone { device_id } => {
            lock(&inner.presence).remove(&device_id);
            inner.emit("presence_changed", "");
        }
        ServerEvent::PresenceSnapshot { devices, .. } => {
            *lock(&inner.presence) = devices
                .into_iter()
                .filter(|d| d.device_id != own)
                .map(|d| (d.device_id, d))
                .collect();
            inner.emit("presence_changed", "");
        }
        ServerEvent::Handoff {
            to_device,
            from_name,
            media,
            position_ms,
            playing,
            queue,
            ..
        } if to_device == own => inner.emit(
            "handoff_received",
            &json!({"from_name": from_name, "media": media, "position_ms": position_ms, "playing": playing, "queue": queue}).to_string(),
        ),
        ServerEvent::Remote {
            to_device,
            from_name,
            command,
            position_ms,
            volume,
            ..
        } if to_device == own => {
            let mut text = json!({"from_name": from_name, "command": command});
            if let Some(position) = position_ms {
                text["position_ms"] = position.into();
            }
            if let Some(volume) = volume {
                text["volume"] = volume.into();
            }
            inner.emit("remote_command", &text.to_string());
        }
        ServerEvent::FriendRequest { .. }
        | ServerEvent::FriendAdded { .. }
        | ServerEvent::FriendRemoved { .. } => inner.emit("friends_changed", ""),
        ServerEvent::FriendActivity { .. } => inner.emit("friend_activity", ""),
        ServerEvent::InboxItem { id, from } => inner.emit(
            "inbox_item",
            &json!({"id": id, "from": {"username": from.username}}).to_string(),
        ),
        ServerEvent::SharedPlaylistChanged {
            playlist_id,
            device_id,
            ..
        } => {
            if device_id == Some(own) {
                return;
            }
            let task = inner.clone();
            crate::runtime().spawn(async move {
                let (Some(engine), Some(owner)) = (task.engine(), task.owner()) else {
                    return;
                };
                let result = engine.pull_shared(&Stores(&owner), playlist_id).await;
                let _ = task.checked(result);
            });
        }
        ServerEvent::SharedPlaylistRemoved { .. } => inner.trigger(true),
        ServerEvent::JamState {
            room_id, playback, ..
        } => {
            let mut jam = lock(&inner.jam);
            if jam.room != Some(room_id) {
                return;
            }
            jam.playback = Some(playback.clone());
            drop(jam);
            inner.emit("jam_state", &serde_json::to_string(&playback).unwrap_or_default());
        }
        ServerEvent::JamChanged { room_id, .. } => inner.emit("jam_changed", &room_id.to_string()),
        ServerEvent::JamInvite { room_id, title, from } => inner.emit(
            "jam_invite",
            &json!({"room_id": room_id, "title": title, "from": {"username": from.username}}).to_string(),
        ),
        ServerEvent::JamEnded { room_id, reason } => {
            inner.leave_jam(Some(room_id));
            inner.emit(
                "jam_ended",
                &json!({"room_id": room_id, "reason": reason}).to_string(),
            );
        }
        _ => {}
    }
}

/// Takes `count` clock samples 200 ms apart through `ping`/`pong`.
async fn sample_clock(inner: &Arc<Inner>, count: usize) {
    lock(&inner.jam).clock.begin_round();
    for i in 0..count {
        if i > 0 {
            tokio::time::sleep(Duration::from_millis(vidcue_sync::clock::SAMPLE_GAP_MS)).await;
        }
        let Some(client) = inner.realtime() else {
            return;
        };
        let t0 = vidcue_sync::now_ms();
        if let Ok(pong) = client.request(&ClientMessage::Ping { t0 }).await
            && let Some(server_ms) = pong["server_ms"].as_i64()
        {
            let t1 = vidcue_sync::now_ms();
            lock(&inner.jam).clock.sample(t0, t1, server_ms);
        }
    }
}

async fn clock_sampler(inner: Arc<Inner>) {
    sample_clock(&inner, vidcue_sync::clock::JOIN_SAMPLES).await;
    loop {
        tokio::time::sleep(Duration::from_millis(vidcue_sync::clock::RESAMPLE_EVERY_MS)).await;
        sample_clock(&inner, vidcue_sync::clock::RESAMPLES).await;
    }
}

async fn jam_pinger(inner: Arc<Inner>, room: Uuid) {
    loop {
        if let Some(client) = inner.realtime() {
            client.send(&ClientMessage::JamPing { room_id: room });
        }
        tokio::time::sleep(JAM_PING).await;
    }
}

fn text<'a>(r: &'a Value, field: &str) -> Result<&'a str> {
    r[field]
        .as_str()
        .with_context(|| format!("Missing {}", field.replace('_', " ")))
}

pub(crate) fn uuid_field(r: &Value, field: &str) -> Result<Uuid> {
    r[field]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .with_context(|| format!("Missing {}", field.replace('_', " ")))
}

fn device_name(r: &Value) -> String {
    let name: String = r["device_name"]
        .as_str()
        .unwrap_or("Android")
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect();
    if name.is_empty() {
        "Android".into()
    } else {
        name
    }
}

pub(crate) fn qr_rows(text: &str) -> Value {
    vidcue_sync::qr::rows(text).map_or(Value::Null, |rows| json!(rows))
}

/// Media that may leave the device (online, slim); `None` for local files.
pub(crate) fn shareable(media: &Media) -> Option<Media> {
    crate::sync_stores::online(media).then(|| crate::sync_stores::slim(media))
}

const OPS: &[&str] = &[
    "sync_status",
    "sync_sign_in",
    "sync_sign_out",
    "sync_pair_start",
    "sync_pair_claim",
    "sync_pair_inspect",
    "sync_devices",
    "sync_revoke",
    "sync_now",
    "sync_settings",
    "sync_foreground",
    "key_requests",
    "key_grant",
    "key_request_start",
    "key_fingerprint",
    "account_recovery_status",
    "account_recovery_regenerate",
    "account_password",
    "account_totp_start",
    "account_totp_confirm",
    "presence_update",
    "presence_devices",
    "handoff_send",
    "remote_send",
    "health_record",
    "playlists",
    "playlist_create",
    "playlist_rename",
    "playlist_delete",
    "playlist_entries",
    "playlist_add",
    "playlist_remove",
    "playlist_move",
    "follows",
    "follow_toggle",
    "filters",
    "filter_add",
    "filter_remove",
];

pub(crate) fn handles(op: &str) -> bool {
    OPS.contains(&op) || crate::social_ops::handles(op)
}

pub(crate) async fn dispatch(engine: &Engine, op: &str, r: &Value) -> Result<Value> {
    if crate::social_ops::handles(op) {
        return crate::social_ops::dispatch(engine, op, r).await;
    }
    let handle = &engine.sync;
    let inner = &handle.0;
    let signed_in = || {
        inner
            .engine()
            .filter(|e| !e.is_signed_out())
            .context("Sign in to a Vidcue server first")
    };
    match op {
        "playlists" | "playlist_create" | "playlist_rename" | "playlist_delete"
        | "playlist_entries" | "playlist_add" | "playlist_remove" | "playlist_move" | "follows"
        | "follow_toggle" | "filters" | "filter_add" | "filter_remove" => engine.library_op(op, r),
        "sync_status" => Ok(inner.status()),
        "sync_sign_in" => {
            let new = SyncEngine::sign_in(
                &inner.dir,
                inner.secrets.clone(),
                text(r, "server")?,
                text(r, "username")?,
                text(r, "password")?,
                text(r, "code")?,
                &device_name(r),
            )
            .await?;
            inner.install(new)?;
            Ok(inner.status())
        }
        "sync_pair_inspect" => {
            let pairing = pairing::parse_pairing_uri(text(r, "uri")?)?;
            Ok(json!({
                "server": pairing.server,
                "host": pairing::origin_host(&pairing.server),
                "has_key": pairing.key.is_some(),
            }))
        }
        "sync_pair_claim" => {
            let pairing = pairing::parse_pairing_uri(text(r, "uri")?)?;
            let new =
                SyncEngine::claim(&inner.dir, inner.secrets.clone(), &pairing, &device_name(r))
                    .await?;
            inner.install(new)?;
            Ok(inner.status())
        }
        "sync_sign_out" => {
            inner.sign_out_local();
            SyncEngine::forget(&inner.dir, inner.secrets.as_ref())?;
            if r["forget_data"].as_bool().unwrap_or(false) {
                engine.forget_synced_library()?;
            }
            *lock(&inner.notice) = None;
            inner.emit_status();
            Ok(inner.status())
        }
        "sync_pair_start" => {
            let sync = signed_in()?;
            let response = inner.checked(sync.pairing(text(r, "password")?).await)?;
            let uri = response["uri"].as_str().unwrap_or_default();
            Ok(json!({"uri": uri, "qr": qr_rows(uri), "expires_at": response["expires_at"]}))
        }
        "sync_devices" => {
            let sync = signed_in()?;
            let listing = inner.checked(sync.api().devices().await.map_err(Into::into))?;
            let presence = lock(&inner.presence).clone();
            let connected = inner.connected();
            let devices: Vec<Value> = listing["devices"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|d| {
                    let mut d = d.clone();
                    let id = d["id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
                    let entry = id.and_then(|id| presence.get(&id));
                    let current = d["current"].as_bool().unwrap_or(false);
                    d["online"] = json!(entry.is_some() || current && connected);
                    d["now_playing"] = json!(entry.and_then(|e| e.state.media.clone()));
                    d
                })
                .collect();
            Ok(json!({"devices": devices}))
        }
        "sync_revoke" => {
            let sync = signed_in()?;
            let id = uuid_field(r, "device_id")?;
            inner.checked(sync.api().revoke_device(id).await.map_err(Into::into))?;
            if id == sync.session().device_id {
                inner.sign_out_local();
                SyncEngine::forget(&inner.dir, inner.secrets.as_ref())?;
                inner.emit_status();
            }
            Ok(json!({"revoked": true}))
        }
        "sync_now" => {
            signed_in()?;
            // Failures are reported in the returned status (state/reason/notice).
            let _ = inner.run_sync(engine).await;
            Ok(inner.status())
        }
        "sync_settings" => {
            let sync = signed_in()?;
            let collections: BTreeMap<String, bool> = match r.get("collections") {
                Some(map) if !map.is_null() => serde_json::from_value(map.clone())?,
                _ => BTreeMap::new(),
            };
            sync.set_settings(&collections, r["health_reports"].as_bool())?;
            inner.trigger(true);
            inner.emit_status();
            Ok(inner.status())
        }
        "sync_foreground" => {
            let visible = r["visible"].as_bool().context("Missing visible")?;
            inner.set_foreground(visible, r["playing"].as_bool().unwrap_or(false))?;
            Ok(json!({}))
        }
        "key_requests" => {
            let sync = signed_in()?;
            Ok(json!({"requests": inner.checked(sync.key_requests().await)?}))
        }
        "key_grant" => {
            let sync = signed_in()?;
            inner.checked(sync.key_grant(uuid_field(r, "id")?).await)?;
            Ok(json!({"granted": true}))
        }
        "key_request_start" => {
            let sync = signed_in()?;
            let started = inner.checked(sync.key_request_start().await)?;
            inner.poll_key();
            inner.emit_status();
            Ok(started)
        }
        "key_fingerprint" => {
            let sync = signed_in()?;
            Ok(json!({"state": sync.key_state().name(), "fingerprint": sync.relay_fingerprint()}))
        }
        "account_recovery_status" => {
            let sync = signed_in()?;
            inner.checked(sync.api().recovery_status().await.map_err(Into::into))
        }
        "account_recovery_regenerate" => {
            let sync = signed_in()?;
            let response = inner.checked(
                sync.api()
                    .recovery_regenerate(text(r, "password")?, text(r, "code")?)
                    .await
                    .map_err(Into::into),
            )?;
            Ok(json!({"codes": response["recovery_codes"]}))
        }
        "account_password" => {
            let sync = signed_in()?;
            inner.checked(
                sync.api()
                    .change_password(
                        text(r, "current_password")?,
                        text(r, "code")?,
                        text(r, "new_password")?,
                        r["sign_out_others"].as_bool().unwrap_or(false),
                    )
                    .await
                    .map_err(Into::into),
            )?;
            Ok(json!({"changed": true}))
        }
        "account_totp_start" => {
            let sync = signed_in()?;
            let response = inner.checked(
                sync.api()
                    .totp_start(text(r, "password")?, text(r, "code")?)
                    .await
                    .map_err(Into::into),
            )?;
            let uri = response["totp_uri"].as_str().unwrap_or_default();
            Ok(json!({"secret": response["totp_secret"], "otpauth_uri": uri, "qr": qr_rows(uri)}))
        }
        "account_totp_confirm" => {
            let sync = signed_in()?;
            inner.checked(
                sync.api()
                    .totp_confirm(text(r, "code")?)
                    .await
                    .map_err(Into::into),
            )?;
            Ok(json!({"confirmed": true}))
        }
        "presence_update" => {
            presence_update(inner, r)?;
            Ok(json!({}))
        }
        "presence_devices" => {
            if let Some(client) = inner.realtime()
                && let Ok(reply) = client.request(&ClientMessage::PresenceQuery).await
                && let Ok(ServerEvent::PresenceSnapshot { devices, .. }) =
                    serde_json::from_value::<ServerEvent>(reply)
            {
                let own = signed_in()?.session().device_id;
                *lock(&inner.presence) = devices
                    .into_iter()
                    .filter(|d| d.device_id != own)
                    .map(|d| (d.device_id, d))
                    .collect();
            }
            let devices: Vec<_> = lock(&inner.presence).values().cloned().collect();
            Ok(json!({"devices": devices}))
        }
        "handoff_send" => {
            signed_in()?;
            let media: Media = serde_json::from_value(r["media"].clone())?;
            let media = shareable(&media).context("Local files cannot be handed off")?;
            let queue: Vec<Media> = match r.get("queue") {
                Some(q) if !q.is_null() => serde_json::from_value(q.clone())?,
                _ => Vec::new(),
            };
            let queue: Vec<Media> = queue
                .iter()
                .filter_map(shareable)
                .take(MAX_HANDOFF_QUEUE)
                .collect();
            let position_ms = r["position_ms"]
                .as_u64()
                .unwrap_or(0)
                .min(7 * 24 * 3600 * 1000);
            let client = inner.realtime().context(vidcue_sync::ws::OFFLINE)?;
            client
                .request(&ClientMessage::Handoff {
                    to_device: uuid_field(r, "to_device")?,
                    media,
                    position_ms,
                    playing: r["playing"].as_bool().unwrap_or(true),
                    queue,
                })
                .await?;
            Ok(json!({"sent": true}))
        }
        "remote_send" => {
            signed_in()?;
            let command = text(r, "command")?;
            ensure!(
                ["play", "pause", "toggle", "seek", "next", "prev", "volume"].contains(&command),
                "Unknown remote command"
            );
            let client = inner.realtime().context(vidcue_sync::ws::OFFLINE)?;
            client
                .request(&ClientMessage::Remote {
                    to_device: uuid_field(r, "to_device")?,
                    command: command.into(),
                    position_ms: r["position_ms"].as_u64(),
                    volume: r["volume"].as_u64(),
                })
                .await?;
            Ok(json!({"sent": true}))
        }
        "health_record" => {
            let kind = HealthKind::parse(text(r, "kind")?).context("Unknown report kind")?;
            let category = match (r["category"].as_str(), r["error_text"].as_str()) {
                (Some(category), _) => {
                    HealthCategory::parse(category).context("Unknown report category")?
                }
                // Classified here, then discarded: error text is never stored or sent.
                (None, Some(text)) => vidcue_sync::health::classify(text),
                (None, None) => HealthCategory::Ok,
            };
            if let Some(sync) = inner.engine() {
                sync.record_health(
                    kind,
                    category,
                    env!("CARGO_PKG_VERSION"),
                    "android",
                    r["duration_ms"].as_u64(),
                )?;
            }
            Ok(json!({}))
        }
        _ => anyhow::bail!("Unknown mobile operation"),
    }
}

/// Throttled presence (§8.1): changes of media, play state or a > 3 s jump, or every
/// 30 s while playing; at most one message per 2 s; local files are announced as idle.
fn presence_update(inner: &Arc<Inner>, r: &Value) -> Result<()> {
    let media: Option<Media> = match r.get("media") {
        Some(m) if !m.is_null() => Some(serde_json::from_value(m.clone())?),
        _ => None,
    };
    let media = media.as_ref().and_then(shareable);
    let key = media.as_ref().map(crate::sync_stores::media_key);
    let playing = r["playing"].as_bool().unwrap_or(false);
    let position_ms = r["position_ms"]
        .as_u64()
        .unwrap_or(0)
        .min(7 * 24 * 3600 * 1000);
    let now = vidcue_sync::now_ms();
    let state = PresenceState {
        media,
        playing,
        position_ms,
        duration_ms: r["duration_ms"]
            .as_u64()
            .filter(|d| *d > 0)
            .map(|d| d.min(7 * 24 * 3600 * 1000)),
        rate: 1.0,
        volume: r["volume"].as_u64().map(|v| v.min(100)),
        queue_len: r["queue_len"].as_u64().unwrap_or(0).min(10_000),
        at: now,
        can: vec!["remote".into(), "handoff".into()],
    };
    let mut out = lock(&inner.presence_out);
    let elapsed = now - out.sent_at;
    let expected = if out.playing {
        out.position_ms as i64 + elapsed
    } else {
        out.position_ms as i64
    };
    let significant = out.sent_at == 0
        || out.key != key
        || out.playing != playing
        || (position_ms as i64 - expected).abs() > PRESENCE_SEEK_JUMP_MS
        || playing && elapsed >= PRESENCE_REFRESH_MS;
    if !significant {
        return Ok(());
    }
    out.key = key;
    out.playing = playing;
    out.position_ms = position_ms;
    if elapsed < PRESENCE_MIN_GAP_MS {
        // Too soon: send the latest state when the window ends.
        out.pending = Some(state);
        if out.timer.as_ref().is_none_or(|t| t.is_finished()) {
            let task = inner.clone();
            let wait = (PRESENCE_MIN_GAP_MS - elapsed) as u64;
            out.timer = Some(crate::runtime().spawn(async move {
                tokio::time::sleep(Duration::from_millis(wait)).await;
                let pending = {
                    let mut out = lock(&task.presence_out);
                    out.sent_at = vidcue_sync::now_ms();
                    out.pending.take()
                };
                if let Some(state) = pending {
                    task.send_presence(state);
                }
            }));
        }
        return Ok(());
    }
    out.sent_at = now;
    out.pending = None;
    drop(out);
    inner.send_presence(state);
    Ok(())
}

/// Local Jam computations used by `social_ops` (§5.5).
impl SyncHandle {
    pub(crate) fn inner(&self) -> &Arc<Inner> {
        &self.0
    }
}

impl Inner {
    pub(crate) fn jam_enter(self: &Arc<Self>, room: &Value) {
        let Some(id) = room["id"].as_str().and_then(|s| Uuid::parse_str(s).ok()) else {
            return;
        };
        let playback = serde_json::from_value(room["playback"].clone()).ok();
        self.enter_jam(id, playback, room["server_ms"].as_i64());
    }

    pub(crate) fn jam_leave(&self, room: Uuid) {
        self.leave_jam(Some(room));
    }

    pub(crate) fn jam_room(&self) -> Option<Uuid> {
        lock(&self.jam).room
    }

    pub(crate) fn jam_playback(&self, room: Uuid, playback: &Value) {
        let mut jam = lock(&self.jam);
        if jam.room == Some(room)
            && let Ok(playback) = serde_json::from_value(playback.clone())
        {
            jam.playback = Some(playback);
        }
    }

    pub(crate) fn jam_target(&self, r: &Value) -> Value {
        let mut jam = lock(&self.jam);
        let (Some(_), Some(playback)) = (jam.room, jam.playback.clone()) else {
            return json!({"in_room": false});
        };
        let local = vidcue_sync::now_ms();
        let server_now = jam.clock.server_now(local);
        let player = PlayerState {
            media_key: r["player_media_key"].as_str().map(str::to_owned),
            position_ms: r["player_position_ms"].as_u64(),
            playing: r["player_playing"].as_bool(),
        };
        let target = expected_position(&playback, server_now);
        let action = drift_action(&playback, &player, server_now, local, &mut jam.last_seek);
        let seek_ms = match action {
            vidcue_sync::clock::DriftAction::Seek(ms) => ms,
            _ => target,
        };
        json!({
            "in_room": true,
            "media": playback.item.as_ref().map(|i| &i.media),
            "media_key": playback.item.as_ref().map(|i| &i.media_key),
            "playing": playback.state == "playing",
            "target_ms": target,
            "action": action.name(),
            "seek_ms": seek_ms,
        })
    }

    pub(crate) fn require(self: &Arc<Self>) -> Result<Arc<SyncEngine>> {
        self.engine()
            .filter(|e| !e.is_signed_out())
            .context("Sign in to a Vidcue server first")
    }

    /// Wraps an API result: a refused token signs this device out.
    pub(crate) fn net(
        self: &Arc<Self>,
        result: Result<Value, vidcue_sync::ApiError>,
    ) -> Result<Value> {
        self.checked(result.map_err(Into::into))
    }

    pub(crate) fn trigger_sync(&self) {
        self.trigger(true);
    }
}
