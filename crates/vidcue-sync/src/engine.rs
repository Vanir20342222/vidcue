//! Push/pull loop, conflict handling and the end-to-end key state (PROTOCOL.md §7.3–§7.6).
use crate::{
    api::{Api, ApiError},
    crypto::{self, AccountKey},
    health::HealthBuffer,
    now_ms,
    outbox::{self, Outbox},
    pairing::Pairing,
    protocol::{
        HealthCategory, HealthKind, PullChange, PushOp, SessionInfo, SharedChange, SharedPlaylist,
    },
    store::{self, E2E_KEY, SESSION, SecretStore},
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;

/// Toggle names shown in `SyncStatus.collections` (`playlists` also covers entries).
pub const COLLECTIONS: [&str; 10] = [
    "positions",
    "history",
    "saved",
    "playlists",
    "offline_playlists",
    "subscriptions",
    "settings",
    "eq_presets",
    "filters",
    "secrets",
];
/// Server collections this client stores locally.
pub const LOCAL_COLLECTIONS: [&str; 11] = [
    "positions",
    "history",
    "saved",
    "playlists",
    "playlist_entries",
    "offline_playlists",
    "subscriptions",
    "settings",
    "eq_presets",
    "filters",
    "secrets",
];
pub const PULL_LIMIT: i64 = 1000;
pub const SHARED_PULL_LIMIT: i64 = 500;
pub const SHARED_PUSH_BATCH: usize = 200;

/// The toggle a server collection belongs to; `None` for collections never stored here.
pub fn toggle_of(collection: &str) -> Option<&'static str> {
    match collection {
        "playlist_entries" => Some("playlists"),
        other => COLLECTIONS.iter().copied().find(|c| *c == other),
    }
}

/// One remote register value handed to the app (§7.2). For `secrets`, `value` is the
/// decrypted plaintext JSON; the key check never reaches the app.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub collection: String,
    pub key: String,
    pub value: Option<Value>,
    pub clock: i64,
    pub device_id: Uuid,
    pub seq: i64,
}

/// The app's local library as seen by the sync engine (§7.2).
pub trait LocalStores: Send + Sync {
    /// Apply one remote register value (None = deleted). Called in seq order.
    fn apply(&self, change: &Change) -> Result<()>;
    /// Every register this device holds for a collection, for the first-sync union.
    /// For `secrets` the values are plaintext JSON (the engine encrypts them).
    fn snapshot(&self, collection: &str) -> Result<Vec<(String, Value, i64)>>;
    /// Called once after a pull batch was applied (UI refresh hook).
    fn applied(&self, collections: &[String]);
    /// Shared playlists the user belongs to, after each listing (create, rename, drop).
    fn shared_playlists(&self, _playlists: &[SharedPlaylist]) -> Result<()> {
        Ok(())
    }
    /// Entry changes of one shared playlist, in seq order.
    fn apply_shared(&self, _playlist: &SharedPlaylist, _changes: &[SharedChange]) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyState {
    Ok,
    Missing,
    Mismatch,
    Waiting,
}

impl KeyState {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Missing => "missing",
            Self::Mismatch => "mismatch",
            Self::Waiting => "waiting",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    Idle,
    Syncing,
    Offline,
    Error,
    SignedOut,
}

impl RunState {
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Syncing => "syncing",
            Self::Offline => "offline",
            Self::Error => "error",
            Self::SignedOut => "signed_out",
        }
    }
}

fn default_collections() -> BTreeMap<String, bool> {
    COLLECTIONS
        .iter()
        .map(|c| ((*c).to_owned(), true))
        .collect()
}

/// A remote secret that arrived while the key was unusable; still encrypted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PendingSecret {
    pub value: Option<Value>,
    pub clock: i64,
    pub device_id: Uuid,
    pub seq: i64,
}

/// `state.json` (§7.4) plus the fields this client needs to resume safely.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncState {
    #[serde(default)]
    pub cursor: i64,
    #[serde(default)]
    pub first_sync_done: bool,
    #[serde(default = "default_collections")]
    pub collections: BTreeMap<String, bool>,
    #[serde(default)]
    pub shared_cursors: BTreeMap<Uuid, i64>,
    #[serde(default)]
    pub last_success: i64,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub rejected_count: u64,
    #[serde(default)]
    pub health_reports: bool,
    /// Account the outbox belongs to; another account discards it on sign-in.
    #[serde(default)]
    pub user_id: Option<Uuid>,
    /// Set when the server refused the token (`revoked`).
    #[serde(default)]
    pub signed_out_reason: Option<String>,
    /// The latest `secrets/keycheck` envelope seen remotely.
    #[serde(default)]
    pub keycheck: Option<Value>,
    #[serde(default)]
    pub pending_secrets: BTreeMap<String, PendingSecret>,
    /// Re-push local secrets after adopting another device's key.
    #[serde(default)]
    pub resend_secrets: bool,
    /// B5 features from `GET /api/features`.
    #[serde(default)]
    pub server_features: Vec<String>,
    #[serde(default)]
    pub last_clock: i64,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            cursor: 0,
            first_sync_done: false,
            collections: default_collections(),
            shared_cursors: BTreeMap::new(),
            last_success: 0,
            last_error: None,
            rejected_count: 0,
            health_reports: false,
            user_id: None,
            signed_out_reason: None,
            keycheck: None,
            pending_secrets: BTreeMap::new(),
            resend_secrets: false,
            server_features: Vec::new(),
            last_clock: 0,
        }
    }
}

struct KeySlot {
    key: Option<AccountKey>,
    state: KeyState,
}

struct Relay {
    id: Uuid,
    request: crypto::RelayRequest,
    expires_at: i64,
}

/// Snapshot of everything `SyncStatus` (§8.2) needs from the engine.
#[derive(Clone, Debug)]
pub struct EngineStatus {
    pub state: RunState,
    pub reason: Option<String>,
    pub last_success: i64,
    pub pending: usize,
    pub rejected_count: u64,
    pub key_state: KeyState,
    pub collections: BTreeMap<String, bool>,
    pub health_reports: bool,
    pub server_features: Vec<String>,
    pub last_error: Option<String>,
}

pub struct SyncEngine {
    api: Api,
    session: SessionInfo,
    dir: PathBuf,
    secrets: Arc<dyn SecretStore>,
    state: Mutex<SyncState>,
    outbox: Mutex<Outbox>,
    shared_outbox: Mutex<Outbox>,
    health: Mutex<HealthBuffer>,
    key: Mutex<KeySlot>,
    relay: Mutex<Option<Relay>>,
    run: tokio::sync::Mutex<()>,
    run_state: Mutex<(RunState, Option<String>)>,
    signed_out: AtomicBool,
    features_checked: AtomicBool,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Whether a pending local write beats a remote one under `(clock, device_id)` ordering.
fn local_wins(local_clock: i64, own: Uuid, remote_clock: i64, remote_device: Uuid) -> bool {
    (local_clock, own) > (remote_clock, remote_device)
}

impl SyncEngine {
    fn paths(dir: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        (
            dir.join("state.json"),
            dir.join("outbox.json"),
            dir.join("shared_outbox.json"),
            dir.join("health.json"),
        )
    }

    /// Opens the signed-in engine, or `None` when this device is signed out.
    pub fn open(dir: &Path, secrets: Arc<dyn SecretStore>) -> Result<Option<Self>> {
        let Some(text) = secrets.load(SESSION)? else {
            return Ok(None);
        };
        let session: SessionInfo =
            serde_json::from_str(&text).context("The stored sync session is invalid")?;
        Self::start(dir, secrets, session, None).map(Some)
    }

    /// The reason recorded when the server last refused this device.
    pub fn signed_out_reason(dir: &Path) -> Option<String> {
        store::read_json::<SyncState>(&Self::paths(dir).0)
            .ok()
            .flatten()
            .and_then(|s| s.signed_out_reason)
    }

    /// Signs in with a password and second factor.
    pub async fn sign_in(
        dir: &Path,
        secrets: Arc<dyn SecretStore>,
        server: &str,
        username: &str,
        password: &str,
        code: &str,
        device_name: &str,
    ) -> Result<Self> {
        let origin = crate::pairing::server_origin(server)?;
        let api = Api::new(&origin, None)?;
        let login = api.login(username, password, code, device_name).await?;
        Self::finish_sign_in(dir, secrets, &api, &origin, &login, None).await
    }

    /// Claims a QR pairing code; the key from `#k=` is verified at the first pull.
    pub async fn claim(
        dir: &Path,
        secrets: Arc<dyn SecretStore>,
        pairing: &Pairing,
        device_name: &str,
    ) -> Result<Self> {
        let api = Api::new(&pairing.server, None)?;
        let claim = api.pairing_claim(&pairing.code, device_name).await?;
        Self::finish_sign_in(dir, secrets, &api, &pairing.server, &claim, pairing.key).await
    }

    async fn finish_sign_in(
        dir: &Path,
        secrets: Arc<dyn SecretStore>,
        api: &Api,
        origin: &str,
        login: &Value,
        key: Option<AccountKey>,
    ) -> Result<Self> {
        let token = login["token"]
            .as_str()
            .context("The server did not issue a device token")?
            .to_owned();
        let api = api.with_token(token.clone());
        let me = api.me().await?;
        let parse = |v: &Value| v.as_str().and_then(|s| Uuid::parse_str(s).ok());
        let session = SessionInfo {
            server: origin.into(),
            user_id: parse(&me["user_id"]).context("The server sent an invalid account")?,
            username: me["username"].as_str().unwrap_or_default().into(),
            device_id: parse(&me["device_id"]).context("The server sent an invalid device")?,
            is_admin: me["is_admin"].as_bool().unwrap_or(false),
            token,
        };
        secrets.save(SESSION, &serde_json::to_string(&session)?)?;
        Self::start(dir, secrets, session, key)
    }

    fn start(
        dir: &Path,
        secrets: Arc<dyn SecretStore>,
        session: SessionInfo,
        pairing_key: Option<AccountKey>,
    ) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let (state_path, outbox_path, shared_path, health_path) = Self::paths(dir);
        let mut state: SyncState = store::read_json(&state_path)?.unwrap_or_default();
        let mut outbox = Outbox::load(&outbox_path).unwrap_or_default();
        let mut shared = Outbox::load(&shared_path).unwrap_or_default();
        if state.user_id != Some(session.user_id) {
            // Another account (or a fresh install): its queued edits must not leak here.
            let keep = SyncState {
                collections: state.collections.clone(),
                health_reports: state.health_reports,
                ..Default::default()
            };
            state = SyncState {
                user_id: Some(session.user_id),
                ..keep
            };
            outbox = Outbox::default();
            shared = Outbox::default();
            secrets.delete(E2E_KEY)?;
        }
        state.signed_out_reason = None;
        outbox.reset_backoff();
        shared.reset_backoff();
        if let Some(key) = pairing_key {
            secrets.save(E2E_KEY, &crypto::encode_key(&key))?;
        }
        let key = secrets
            .load(E2E_KEY)?
            .map(|text| crypto::decode_key(&text))
            .transpose()?;
        let key_state = match (&key, &state.keycheck) {
            (Some(k), Some(check)) if crypto::keycheck_opens(k, check) => KeyState::Ok,
            (Some(_), Some(_)) => KeyState::Mismatch,
            (Some(_), None) => KeyState::Ok,
            (None, _) => KeyState::Missing,
        };
        let engine = Self {
            api: Api::new(&session.server, Some(session.token.clone()))?,
            session,
            dir: dir.into(),
            secrets,
            state: Mutex::new(state),
            outbox: Mutex::new(outbox),
            shared_outbox: Mutex::new(shared),
            health: Mutex::new(store::read_json(&health_path)?.unwrap_or_default()),
            key: Mutex::new(KeySlot {
                key,
                state: key_state,
            }),
            relay: Mutex::new(None),
            run: tokio::sync::Mutex::new(()),
            run_state: Mutex::new((RunState::Idle, None)),
            signed_out: AtomicBool::new(false),
            features_checked: AtomicBool::new(false),
        };
        engine.save_state()?;
        engine.save_outbox()?;
        engine.save_shared_outbox()?;
        Ok(engine)
    }

    /// Deletes the session, key, state and outboxes of this device (local sign-out).
    pub fn forget(dir: &Path, secrets: &dyn SecretStore) -> Result<()> {
        secrets.delete(SESSION)?;
        secrets.delete(E2E_KEY)?;
        let (state, outbox, shared, health) = Self::paths(dir);
        for path in [state, outbox, shared, health] {
            store::remove_file(&path)?;
        }
        Ok(())
    }

    pub fn session(&self) -> &SessionInfo {
        &self.session
    }
    pub fn api(&self) -> &Api {
        &self.api
    }
    pub fn dir(&self) -> &Path {
        &self.dir
    }
    pub fn is_signed_out(&self) -> bool {
        self.signed_out.load(Ordering::SeqCst)
    }
    pub fn state(&self) -> SyncState {
        lock(&self.state).clone()
    }

    fn save_state(&self) -> Result<()> {
        store::write_json(&Self::paths(&self.dir).0, &*lock(&self.state))
    }
    fn save_outbox(&self) -> Result<()> {
        lock(&self.outbox).save(&Self::paths(&self.dir).1)
    }
    fn save_shared_outbox(&self) -> Result<()> {
        lock(&self.shared_outbox).save(&Self::paths(&self.dir).2)
    }
    fn save_health(&self) -> Result<()> {
        store::write_json(&Self::paths(&self.dir).3, &*lock(&self.health))
    }

    pub fn status(&self) -> EngineStatus {
        let state = lock(&self.state).clone();
        let (run, reason) = lock(&self.run_state).clone();
        let (run, reason) = if self.is_signed_out() {
            (RunState::SignedOut, Some("revoked".to_owned()))
        } else {
            (run, reason)
        };
        EngineStatus {
            state: run,
            reason,
            last_success: state.last_success,
            pending: lock(&self.outbox).len() + lock(&self.shared_outbox).len(),
            rejected_count: state.rejected_count,
            key_state: self.key_state(),
            collections: state.collections,
            health_reports: state.health_reports,
            server_features: state.server_features,
            last_error: state.last_error,
        }
    }

    /// Whether a server collection is enabled on this device.
    pub fn enabled(&self, collection: &str) -> bool {
        toggle_of(collection).is_some_and(|toggle| {
            lock(&self.state)
                .collections
                .get(toggle)
                .copied()
                .unwrap_or(true)
        })
    }

    /// Changes collection toggles and the health toggle. Disabling drops queued writes
    /// of that collection; enabling it again re-runs the first-sync union.
    pub fn set_settings(
        &self,
        collections: &BTreeMap<String, bool>,
        health_reports: Option<bool>,
    ) -> Result<()> {
        let mut enabled_again = false;
        {
            let mut state = lock(&self.state);
            for (name, on) in collections {
                ensure!(
                    COLLECTIONS.contains(&name.as_str()),
                    "Unknown sync collection"
                );
                let was = state.collections.get(name).copied().unwrap_or(true);
                state.collections.insert(name.clone(), *on);
                if !*on {
                    let mut outbox = lock(&self.outbox);
                    for collection in LOCAL_COLLECTIONS {
                        if toggle_of(collection) == Some(name.as_str()) {
                            outbox.remove_collection(collection);
                        }
                    }
                } else if !was {
                    enabled_again = true;
                }
            }
            if enabled_again {
                // Pull everything again so the re-enabled collection catches up.
                state.first_sync_done = false;
                state.cursor = 0;
            }
            if let Some(on) = health_reports {
                state.health_reports = on;
                if !on {
                    *lock(&self.health) = HealthBuffer::default();
                }
            }
        }
        self.save_outbox()?;
        self.save_health()?;
        self.save_state()
    }

    /// A strictly increasing local clock: `max(now, last + 1)`.
    pub fn next_clock(&self) -> i64 {
        let mut state = lock(&self.state);
        state.last_clock = now_ms().max(state.last_clock + 1);
        state.last_clock
    }

    /// Queues local register writes of enabled collections (not `secrets`).
    pub fn enqueue(&self, ops: &[(String, String, Option<Value>, i64)]) -> Result<usize> {
        ensure!(
            ops.iter().all(|(collection, ..)| collection != "secrets"),
            "Secrets are queued through enqueue_secret"
        );
        // Decide before taking the outbox lock (lock order: state, then outbox).
        let wanted: Vec<_> = ops
            .iter()
            .filter(|(collection, ..)| self.enabled(collection))
            .collect();
        let queued = wanted.len();
        {
            let mut outbox = lock(&self.outbox);
            for (collection, key, value, clock) in wanted {
                outbox.enqueue(collection, key, value.clone(), *clock)?;
            }
        }
        if queued > 0 {
            self.save_outbox()?;
        }
        Ok(queued)
    }

    /// Encrypts and queues a provider session (`secrets/<provider>`). Returns false when
    /// the key is not usable or the collection is disabled; nothing is queued then.
    pub fn enqueue_secret(&self, provider: &str, plaintext: &Value, clock: i64) -> Result<bool> {
        ensure!(provider != crypto::KEYCHECK, "Reserved secret name");
        if !self.enabled("secrets") {
            return Ok(false);
        }
        let Some(key) = self.usable_key() else {
            return Ok(false);
        };
        let envelope =
            crypto::seal_secret(&key, provider, serde_json::to_string(plaintext)?.as_bytes())?;
        lock(&self.outbox).enqueue("secrets", provider, Some(envelope), clock)?;
        self.save_outbox()?;
        Ok(true)
    }

    /// Queues an entry write of a shared playlist (`value` = `{position, media}` or None).
    pub fn enqueue_shared(
        &self,
        playlist: Uuid,
        entry: Uuid,
        value: Option<Value>,
        clock: i64,
    ) -> Result<()> {
        lock(&self.shared_outbox).enqueue(
            &playlist.to_string(),
            &entry.to_string(),
            value,
            clock,
        )?;
        self.save_shared_outbox()
    }

    /// Forgets the shared cursor and queued writes of a playlist this device left.
    pub fn drop_shared(&self, playlist: Uuid) -> Result<()> {
        lock(&self.shared_outbox).remove_collection(&playlist.to_string());
        lock(&self.state).shared_cursors.remove(&playlist);
        self.save_shared_outbox()?;
        self.save_state()
    }

    /// Buffers one health report when the per-device toggle is on.
    pub fn record_health(
        &self,
        kind: HealthKind,
        category: HealthCategory,
        client_version: &str,
        platform: &str,
        duration_ms: Option<u64>,
    ) -> Result<bool> {
        if !lock(&self.state).health_reports {
            return Ok(false);
        }
        lock(&self.health).record(
            kind,
            category,
            client_version,
            platform,
            now_ms(),
            duration_ms,
        );
        self.save_health()?;
        Ok(true)
    }

    pub fn key_state(&self) -> KeyState {
        let slot = lock(&self.key);
        if slot.state != KeyState::Ok
            && lock(&self.relay)
                .as_ref()
                .is_some_and(|r| r.expires_at > now_ms())
        {
            return KeyState::Waiting;
        }
        slot.state
    }

    fn usable_key(&self) -> Option<AccountKey> {
        let slot = lock(&self.key);
        (slot.state == KeyState::Ok).then_some(slot.key).flatten()
    }

    /// The account key, only when it is verified or not yet contradicted.
    pub fn account_key(&self) -> Option<AccountKey> {
        self.usable_key()
    }

    fn set_key_state(&self, next: KeyState) {
        let mut slot = lock(&self.key);
        if slot.state == next {
            return;
        }
        slot.state = next;
        if next != KeyState::Ok {
            // Envelopes sealed with a contradicted key must not reach the server.
            drop(slot);
            lock(&self.outbox).remove_collection("secrets");
            let _ = self.save_outbox();
        }
    }

    fn mark_signed_out(&self) {
        self.signed_out.store(true, Ordering::SeqCst);
        let _ = self.secrets.delete(SESSION);
        lock(&self.state).signed_out_reason = Some("revoked".into());
        let _ = self.save_state();
        *lock(&self.run_state) = (RunState::SignedOut, Some("revoked".into()));
    }

    /// Records a refused token from any call (HTTP 401 or a closed WebSocket).
    pub fn note_error(&self, error: &anyhow::Error) {
        if matches!(
            error.downcast_ref::<ApiError>(),
            Some(ApiError::Unauthorized)
        ) {
            self.mark_signed_out();
        }
    }

    /// Called when the WebSocket reports this device as revoked.
    pub fn revoked(&self) {
        self.mark_signed_out();
    }

    /// One complete sync run (§7.4). Concurrent callers wait for the running one.
    pub async fn sync_once(&self, stores: &dyn LocalStores) -> Result<()> {
        ensure!(
            !self.is_signed_out(),
            "This device was signed out of the server"
        );
        let _run = self.run.lock().await;
        *lock(&self.run_state) = (RunState::Syncing, None);
        let result = self.run_inner(stores).await;
        let mut state = lock(&self.state);
        match &result {
            Ok(()) => {
                state.last_success = now_ms();
                state.last_error = None;
                *lock(&self.run_state) = (RunState::Idle, None);
            }
            Err(error) => match error.downcast_ref::<ApiError>() {
                Some(ApiError::Unauthorized) => {
                    drop(state);
                    self.mark_signed_out();
                    return result;
                }
                Some(e) if e.transient() => {
                    state.last_error = Some(e.to_string());
                    *lock(&self.run_state) = (RunState::Offline, Some("network".into()));
                }
                Some(e) if e.not_found() => {
                    state.last_error = Some(e.to_string());
                    *lock(&self.run_state) = (RunState::Error, Some("server_too_old".into()));
                }
                _ => {
                    state.last_error = Some(format!("{error:#}").chars().take(300).collect());
                    *lock(&self.run_state) = (RunState::Error, None);
                }
            },
        }
        drop(state);
        self.save_state()?;
        result
    }

    async fn run_inner(&self, stores: &dyn LocalStores) -> Result<()> {
        if !self.features_checked.load(Ordering::SeqCst) {
            match self.api.features().await {
                Ok(value) => {
                    let features = value["features"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .filter(|f| *f != "recs" || value["recs_available"] != false)
                                .map(str::to_owned)
                                .collect()
                        })
                        .unwrap_or_default();
                    lock(&self.state).server_features = features;
                    self.features_checked.store(true, Ordering::SeqCst);
                }
                Err(e) if e.not_found() => {
                    lock(&self.state).server_features.clear();
                    self.features_checked.store(true, Ordering::SeqCst);
                }
                Err(e) => return Err(e.into()),
            }
        }
        let first = !lock(&self.state).first_sync_done;
        if first {
            let remote = self.pull(stores, true).await?;
            self.resolve_key_after_first_pull()?;
            self.union(stores, &remote.unwrap_or_default())?;
            lock(&self.state).first_sync_done = true;
            self.save_state()?;
        }
        self.apply_pending_secrets(stores)?;
        self.resend_secrets(stores)?;
        self.push().await?;
        self.pull(stores, false).await?;
        self.apply_pending_secrets(stores)?;
        self.sync_shared(stores).await?;
        self.flush_health().await?;
        Ok(())
    }

    /// Pulls from the cursor while `more`. With `record`, returns every remote register's
    /// `(clock, device)` for the first-sync union.
    async fn pull(
        &self,
        stores: &dyn LocalStores,
        record: bool,
    ) -> Result<Option<HashMap<(String, String), (i64, Uuid)>>> {
        let mut remote = record.then(HashMap::new);
        loop {
            let since = lock(&self.state).cursor;
            let page = self.api.pull(since, PULL_LIMIT).await?;
            let mut touched: Vec<String> = Vec::new();
            for change in &page.changes {
                if let Some(remote) = remote.as_mut() {
                    remote.insert(
                        (change.collection.clone(), change.key.clone()),
                        (change.clock, change.device_id),
                    );
                }
                if self.apply_remote(stores, change)? && !touched.contains(&change.collection) {
                    touched.push(change.collection.clone());
                }
            }
            lock(&self.state).cursor = page.next.max(since);
            self.save_state()?;
            if !touched.is_empty() {
                stores.applied(&touched);
            }
            if !page.more || page.next <= since {
                break;
            }
        }
        Ok(remote)
    }

    /// Applies one pulled change; returns whether the app saw it.
    fn apply_remote(&self, stores: &dyn LocalStores, change: &PullChange) -> Result<bool> {
        let Some(toggle) = toggle_of(&change.collection) else {
            return Ok(false);
        };
        if change.collection == "secrets" && change.key == crypto::KEYCHECK {
            self.on_keycheck(change.value.as_ref());
            return Ok(false);
        }
        if !lock(&self.state)
            .collections
            .get(toggle)
            .copied()
            .unwrap_or(true)
        {
            return Ok(false);
        }
        if let Some(op) = lock(&self.outbox).pending(&change.collection, &change.key)
            && local_wins(
                op.clock,
                self.session.device_id,
                change.clock,
                change.device_id,
            )
        {
            return Ok(false);
        }
        let mut out = Change {
            collection: change.collection.clone(),
            key: change.key.clone(),
            value: change.value.clone(),
            clock: change.clock,
            device_id: change.device_id,
            seq: change.seq,
        };
        if change.collection == "secrets" {
            let Some(envelope) = &change.value else {
                // A deletion elsewhere never disconnects this device.
                return Ok(false);
            };
            let Some(key) = self.usable_key() else {
                lock(&self.state).pending_secrets.insert(
                    change.key.clone(),
                    PendingSecret {
                        value: change.value.clone(),
                        clock: change.clock,
                        device_id: change.device_id,
                        seq: change.seq,
                    },
                );
                return Ok(false);
            };
            let Ok(plain) = crypto::open_secret(&key, &change.key, envelope) else {
                // Sealed with an older key; the owner re-pushes after a relay.
                return Ok(false);
            };
            let Ok(value) = serde_json::from_slice::<Value>(&plain) else {
                return Ok(false);
            };
            out.value = Some(value);
        }
        // A value the app cannot apply (for example a setting from a newer app version)
        // must not stop the cursor; it is skipped.
        Ok(stores.apply(&out).is_ok())
    }

    fn on_keycheck(&self, envelope: Option<&Value>) {
        let Some(envelope) = envelope else {
            return;
        };
        lock(&self.state).keycheck = Some(envelope.clone());
        let key = lock(&self.key).key;
        let next = match key {
            Some(key) if crypto::keycheck_opens(&key, envelope) => KeyState::Ok,
            Some(_) => KeyState::Mismatch,
            None => KeyState::Missing,
        };
        self.set_key_state(next);
    }

    /// After the first pull: no key check anywhere means this device creates the key.
    fn resolve_key_after_first_pull(&self) -> Result<()> {
        if lock(&self.state).keycheck.is_some() {
            return Ok(());
        }
        let existing = lock(&self.key).key;
        let key = match existing {
            Some(key) => key,
            None => {
                let key = crypto::new_account_key();
                self.secrets.save(E2E_KEY, &crypto::encode_key(&key))?;
                lock(&self.key).key = Some(key);
                key
            }
        };
        lock(&self.key).state = KeyState::Ok;
        let clock = self.next_clock();
        lock(&self.outbox).enqueue(
            "secrets",
            crypto::KEYCHECK,
            Some(crypto::keycheck(&key)?),
            clock,
        )?;
        self.save_outbox()
    }

    /// First-sync union: local registers missing remotely or newer locally are queued.
    fn union(
        &self,
        stores: &dyn LocalStores,
        remote: &HashMap<(String, String), (i64, Uuid)>,
    ) -> Result<()> {
        for collection in LOCAL_COLLECTIONS {
            if !self.enabled(collection) {
                continue;
            }
            if collection == "secrets" && self.usable_key().is_none() {
                continue;
            }
            for (key, value, clock) in stores.snapshot(collection)? {
                let newer = match remote.get(&(collection.to_owned(), key.clone())) {
                    None => true,
                    Some((remote_clock, device)) => {
                        local_wins(clock, self.session.device_id, *remote_clock, *device)
                    }
                };
                if !newer {
                    continue;
                }
                let clock = if clock > 0 { clock } else { self.next_clock() };
                if collection == "secrets" {
                    if key != crypto::KEYCHECK {
                        self.enqueue_secret(&key, &value, clock)?;
                    }
                } else {
                    lock(&self.outbox).enqueue(collection, &key, Some(value), clock)?;
                }
            }
        }
        self.save_outbox()
    }

    fn apply_pending_secrets(&self, stores: &dyn LocalStores) -> Result<()> {
        if self.usable_key().is_none() || lock(&self.state).pending_secrets.is_empty() {
            return Ok(());
        }
        let pending = std::mem::take(&mut lock(&self.state).pending_secrets);
        let mut touched = false;
        for (key, secret) in pending {
            let change = PullChange {
                collection: "secrets".into(),
                key,
                value: secret.value,
                clock: secret.clock,
                device_id: secret.device_id,
                seq: secret.seq,
            };
            touched |= self.apply_remote(stores, &change)?;
        }
        self.save_state()?;
        if touched {
            stores.applied(&["secrets".to_owned()]);
        }
        Ok(())
    }

    fn resend_secrets(&self, stores: &dyn LocalStores) -> Result<()> {
        if !lock(&self.state).resend_secrets || self.usable_key().is_none() {
            return Ok(());
        }
        if self.enabled("secrets") {
            for (key, value, _) in stores.snapshot("secrets")? {
                if key != crypto::KEYCHECK {
                    let clock = self.next_clock();
                    self.enqueue_secret(&key, &value, clock)?;
                }
            }
        }
        lock(&self.state).resend_secrets = false;
        self.save_state()
    }

    async fn push(&self) -> Result<()> {
        loop {
            let now = now_ms();
            let key_ok = self.usable_key().is_some();
            let batch: Vec<_> = lock(&self.outbox)
                .due(now, usize::MAX)
                .into_iter()
                .filter(|op| key_ok || op.collection != "secrets")
                .take(outbox::PUSH_BATCH)
                .collect();
            if batch.is_empty() {
                return Ok(());
            }
            let ops: Vec<PushOp> = batch
                .iter()
                .map(|op| PushOp {
                    id: op.id,
                    collection: op.collection.clone(),
                    key: op.key.clone(),
                    value: op.value.clone(),
                    clock: op.clock,
                })
                .collect();
            let ids: Vec<Uuid> = batch.iter().map(|op| op.id).collect();
            match self.api.push(&ops).await {
                Ok(response) => {
                    let mut done = Vec::new();
                    let mut rejected = 0;
                    for result in &response.results {
                        if ids.contains(&result.id) {
                            done.push(result.id);
                            if result.status == "rejected" {
                                rejected += 1;
                            }
                        }
                    }
                    lock(&self.outbox).complete(&done);
                    lock(&self.state).rejected_count += rejected;
                    self.save_outbox()?;
                    self.save_state()?;
                    if batch.len() < outbox::PUSH_BATCH || done.is_empty() {
                        return Ok(());
                    }
                }
                Err(e) => {
                    self.defer(&self.outbox, &ids, &e);
                    self.save_outbox()?;
                    return Err(e.into());
                }
            }
        }
    }

    fn defer(&self, queue: &Mutex<Outbox>, ids: &[Uuid], error: &ApiError) {
        let fixed = (error.status() == Some(429)).then_some(outbox::RATE_LIMIT_BACKOFF);
        if !matches!(error, ApiError::Unauthorized) {
            lock(queue).defer(ids, now_ms(), fixed);
        }
    }

    /// The earliest time a deferred write may be retried.
    pub fn next_retry(&self) -> Option<i64> {
        [
            lock(&self.outbox).next_retry(),
            lock(&self.shared_outbox).next_retry(),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    async fn sync_shared(&self, stores: &dyn LocalStores) -> Result<()> {
        let listing = match self.api.shared_playlists().await {
            Ok(listing) => listing,
            Err(e) if e.not_found() => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let playlists: Vec<SharedPlaylist> =
            serde_json::from_value(listing["playlists"].clone()).unwrap_or_default();
        stores.shared_playlists(&playlists)?;
        let known: Vec<Uuid> = playlists.iter().map(|p| p.id).collect();
        {
            // Writes to playlists this user no longer belongs to are dropped.
            let mut shared = lock(&self.shared_outbox);
            shared
                .ops
                .retain(|op| Uuid::parse_str(&op.collection).is_ok_and(|id| known.contains(&id)));
            lock(&self.state)
                .shared_cursors
                .retain(|id, _| known.contains(id));
        }
        self.save_shared_outbox()?;
        for playlist in &playlists {
            self.push_shared(playlist.id).await?;
        }
        for playlist in &playlists {
            let cursor = lock(&self.state)
                .shared_cursors
                .get(&playlist.id)
                .copied()
                .unwrap_or(0);
            if playlist.seq > cursor {
                self.pull_shared(stores, playlist.id).await?;
            }
        }
        self.save_state()
    }

    async fn push_shared(&self, playlist: Uuid) -> Result<()> {
        loop {
            let name = playlist.to_string();
            let batch: Vec<_> = lock(&self.shared_outbox)
                .due(now_ms(), usize::MAX)
                .into_iter()
                .filter(|op| op.collection == name)
                .take(SHARED_PUSH_BATCH)
                .collect();
            if batch.is_empty() {
                return Ok(());
            }
            let ops: Vec<Value> = batch
                .iter()
                .map(|op| json!({"id": op.id, "entry_id": op.key, "value": op.value, "clock": op.clock}))
                .collect();
            let ids: Vec<Uuid> = batch.iter().map(|op| op.id).collect();
            match self.api.shared_push(playlist, &ops).await {
                Ok(response) => {
                    let done: Vec<Uuid> = response
                        .results
                        .iter()
                        .map(|r| r.id)
                        .filter(|id| ids.contains(id))
                        .collect();
                    let rejected = response
                        .results
                        .iter()
                        .filter(|r| r.status == "rejected")
                        .count() as u64;
                    lock(&self.shared_outbox).complete(&done);
                    lock(&self.state).rejected_count += rejected;
                    self.save_shared_outbox()?;
                    if batch.len() < SHARED_PUSH_BATCH || done.is_empty() {
                        return Ok(());
                    }
                }
                Err(e) if e.not_found() => {
                    // Removed from the playlist meanwhile.
                    lock(&self.shared_outbox).remove_collection(&name);
                    self.save_shared_outbox()?;
                    return Ok(());
                }
                Err(e) => {
                    self.defer(&self.shared_outbox, &ids, &e);
                    self.save_shared_outbox()?;
                    return Err(e.into());
                }
            }
        }
    }

    /// Pulls one shared playlist from its cursor (also used on `shared_playlist_changed`).
    pub async fn pull_shared(&self, stores: &dyn LocalStores, playlist: Uuid) -> Result<()> {
        loop {
            let since = lock(&self.state)
                .shared_cursors
                .get(&playlist)
                .copied()
                .unwrap_or(0);
            let page = match self
                .api
                .shared_pull(playlist, since, SHARED_PULL_LIMIT)
                .await
            {
                Ok(page) => page,
                Err(e) if e.not_found() => {
                    self.drop_shared(playlist)?;
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
            };
            let name = playlist.to_string();
            let changes: Vec<SharedChange> = {
                let shared = lock(&self.shared_outbox);
                page.changes
                    .iter()
                    .filter(|c| {
                        shared
                            .pending(&name, &c.entry_id.to_string())
                            .is_none_or(|op| {
                                !local_wins(op.clock, self.session.device_id, c.clock, c.device_id)
                            })
                    })
                    .cloned()
                    .collect()
            };
            stores.apply_shared(&page.playlist, &changes)?;
            lock(&self.state)
                .shared_cursors
                .insert(playlist, page.next.max(since));
            self.save_state()?;
            if !page.more || page.next <= since {
                return Ok(());
            }
        }
    }

    async fn flush_health(&self) -> Result<()> {
        if !lock(&self.state).health_reports {
            return Ok(());
        }
        loop {
            let batch = lock(&self.health).batch();
            if batch.is_empty() {
                return Ok(());
            }
            match self.api.health_post(&batch).await {
                Ok(_) => {
                    lock(&self.health).drop_front(batch.len());
                    self.save_health()?;
                }
                // Older servers have no health log; keep the bounded buffer.
                Err(e) if e.not_found() => return Ok(()),
                Err(ApiError::Unauthorized) => return Err(ApiError::Unauthorized.into()),
                // Reports are best effort and never fail a sync run.
                Err(_) => return Ok(()),
            }
        }
    }

    // ---- key relay and pairing ----

    /// Starts a QR pairing: server code plus the local key in the fragment.
    pub async fn pairing(&self, password: &str) -> Result<Value> {
        let key = self
            .usable_key()
            .filter(|_| self.key_state() == KeyState::Ok)
            .context("No account key yet")?;
        let mut response = self.api.pairing_start(password).await?;
        let uri = response["uri"]
            .as_str()
            .context("The server did not return a pairing link")?;
        let full = crate::pairing::with_key(uri, &key)?;
        response["uri"] = full.into();
        Ok(response)
    }

    /// Requests the key from a trusted device; returns `{id, fingerprint, expires_at}`.
    pub async fn key_request_start(&self) -> Result<Value> {
        let request = crypto::RelayRequest::new();
        let created = self.api.key_request_create(&request.public_b64()).await?;
        let id = created["id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .context("The server sent an invalid key request")?;
        let expires_at = created["expires_at"]
            .as_i64()
            .unwrap_or(now_ms() + 15 * 60 * 1000);
        let fingerprint = request.fingerprint();
        *lock(&self.relay) = Some(Relay {
            id,
            request,
            expires_at,
        });
        Ok(json!({"id": id, "fingerprint": fingerprint, "expires_at": expires_at}))
    }

    /// The fingerprint of this device's open request.
    pub fn relay_fingerprint(&self) -> Option<String> {
        lock(&self.relay)
            .as_ref()
            .filter(|r| r.expires_at > now_ms())
            .map(|r| r.request.fingerprint())
    }

    pub fn relay_id(&self) -> Option<Uuid> {
        lock(&self.relay)
            .as_ref()
            .filter(|r| r.expires_at > now_ms())
            .map(|r| r.id)
    }

    /// Checks the open request once. Returns true when a key was granted and adopted.
    pub async fn key_request_poll(&self) -> Result<bool> {
        let Some(id) = self.relay_id() else {
            *lock(&self.relay) = None;
            return Ok(false);
        };
        let response = match self.api.key_request(id).await {
            Ok(response) => response,
            Err(e) if e.not_found() => {
                *lock(&self.relay) = None;
                return Ok(false);
            }
            Err(e) => return Err(e.into()),
        };
        let Some(sealed) = response["sealed_key"].as_str() else {
            return Ok(false);
        };
        let key = {
            let relay = lock(&self.relay);
            let relay = relay.as_ref().context("The key request expired")?;
            relay.request.open(sealed)
        };
        // A granted request is consumed by the server; a bad key needs a new request.
        *lock(&self.relay) = None;
        self.adopt_key(key?)?;
        Ok(true)
    }

    /// Installs a key received from another device after checking it against `keycheck`.
    pub fn adopt_key(&self, key: AccountKey) -> Result<()> {
        let check = lock(&self.state).keycheck.clone();
        match check {
            Some(check) if crypto::keycheck_opens(&key, &check) => {}
            _ => bail!("key did not match"),
        }
        self.secrets.save(E2E_KEY, &crypto::encode_key(&key))?;
        {
            let mut slot = lock(&self.key);
            slot.key = Some(key);
            slot.state = KeyState::Ok;
        }
        lock(&self.outbox).remove_collection("secrets");
        self.save_outbox()?;
        lock(&self.state).resend_secrets = true;
        self.save_state()
    }

    /// Open requests of the user's other devices with locally computed fingerprints.
    pub async fn key_requests(&self) -> Result<Vec<Value>> {
        Ok(self
            .api
            .key_requests()
            .await?
            .into_iter()
            .filter_map(|r| {
                let public = crypto::decode_public(&r.public_key).ok()?;
                Some(json!({
                    "id": r.id,
                    "device_id": r.device_id,
                    "device_name": r.device_name,
                    "fingerprint": crypto::fingerprint(&public),
                    "created_at": r.created_at,
                }))
            })
            .collect())
    }

    /// Seals the key to a request (after the user compared fingerprints).
    pub async fn key_grant(&self, id: Uuid) -> Result<()> {
        let key = self
            .usable_key()
            .filter(|_| self.key_state() == KeyState::Ok)
            .context("No account key yet")?;
        let request = self
            .api
            .key_requests()
            .await?
            .into_iter()
            .find(|r| r.id == id)
            .context("This key request is no longer open")?;
        let sealed = crypto::seal_for(&request.public_key, &key)?;
        self.api.key_grant(id, &sealed).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_and_toggles() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        assert!(local_wins(5, a, 4, b));
        assert!(!local_wins(4, a, 5, b));
        assert!(local_wins(5, b, 5, a));
        assert!(!local_wins(5, a, 5, b));
        assert_eq!(toggle_of("playlist_entries"), Some("playlists"));
        assert_eq!(toggle_of("collection_watches"), None);
        assert_eq!(toggle_of("secrets"), Some("secrets"));
        let state: SyncState = serde_json::from_str("{}").unwrap();
        assert_eq!(state.collections.len(), 10);
        assert!(state.collections.values().all(|v| *v));
        assert!(!state.health_reports);
    }
}
