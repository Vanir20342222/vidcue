//! Presence, handoff and remote control: the client-to-server half of the WebSocket.
//!
//! Presence is kept in memory only (it is stale after a restart anyway). Each socket
//! has its own token buckets, so no client message causes a database write except the
//! rate-limited `jam_ping`. Handoff and remote commands are forwarded to one device of
//! the same user; the per-user broadcast channel carries them with a `to_device` field
//! and every other socket drops them (`delivers`).

use crate::api::{AppState, Device};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Advertised in the `hello` event; clients send messages only when `realtime` is listed.
pub const FEATURES: &[&str] = &["realtime", "social", "jam"];

/// `presence`: one message per 2 s.
const PRESENCE_BURST: f64 = 1.0;
const PRESENCE_PER_SEC: f64 = 0.5;
/// Every other client message together: 20 per 10 s, burst 20.
const MESSAGE_BURST: f64 = 20.0;
const MESSAGE_PER_SEC: f64 = 2.0;
/// This many `bad_message`/`unknown_type` errors within the window close the socket.
const MAX_BAD_MESSAGES: usize = 3;
const BAD_MESSAGE_WINDOW: Duration = Duration::from_secs(60);
/// Close codes (RFC 6455).
pub const CLOSE_UNSUPPORTED: u16 = 1003;
pub const CLOSE_POLICY: u16 = 1008;

pub const MAX_HANDOFF_QUEUE: usize = 25;
/// Positions in presence, handoff and remote messages.
pub const MAX_POSITION_MS: u64 = 7 * 24 * 3_600_000;
pub const MAX_QUEUE_LEN: u64 = 10_000;
const MAX_RID: i64 = (1 << 31) - 1;
const MAX_TYPE_LEN: usize = 32;
/// Friends get at most one `friend_activity` per sharing user in this window; changes
/// inside it are coalesced into one event at the end of the window.
pub const ACTIVITY_WINDOW: Duration = Duration::from_secs(10);
/// `jam_ping` writes `last_active` at most once per room in this interval.
pub const JAM_PING_WRITE_MS: i64 = 30_000;

pub const BAD_MESSAGE: &str = "bad_message";
pub const UNKNOWN_TYPE: &str = "unknown_type";
pub const RATE_LIMITED: &str = "rate_limited";
pub const NOT_FOUND: &str = "not_found";
/// Listed in the protocol; no current message is refused this way.
#[allow(dead_code)]
pub const FORBIDDEN: &str = "forbidden";
pub const TOO_LARGE: &str = "too_large";
pub const INVALID: &str = "invalid";

/// An error answered on the socket as `{"type":"error","rid","code","message"}`.
#[derive(Debug, PartialEq)]
pub struct WsError {
    pub code: &'static str,
    pub message: String,
}

impl WsError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    fn bad(message: impl Into<String>) -> Self {
        Self::new(BAD_MESSAGE, message)
    }
    fn invalid(message: impl Into<String>) -> Self {
        Self::new(INVALID, message)
    }
}

/// Token bucket; time is passed in so tests are deterministic.
#[derive(Clone, Debug)]
pub struct Bucket {
    capacity: f64,
    per_sec: f64,
    tokens: f64,
    last: Instant,
}

impl Bucket {
    pub fn new(capacity: f64, per_sec: f64, now: Instant) -> Self {
        Self {
            capacity,
            per_sec,
            tokens: capacity,
            last: now,
        }
    }
    pub fn take(&mut self, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.per_sec).min(self.capacity);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

struct LiveDevice {
    user: Uuid,
    name: String,
    sockets: usize,
    presence: Option<StoredPresence>,
    /// Server time the device came online (reported until it sends presence).
    since: i64,
}

#[derive(Clone)]
struct StoredPresence {
    state: Value,
    server_at: i64,
    media_key: Option<String>,
    playing: bool,
}

#[derive(Default)]
struct Activity {
    last_emit: Option<Instant>,
    /// `(media key, playing)` of the last `friend_activity` decision.
    emitted: (Option<String>, bool),
    pending: bool,
}

#[derive(Default)]
struct Live {
    devices: HashMap<Uuid, LiveDevice>,
    /// Online devices per user.
    users: HashMap<Uuid, HashSet<Uuid>>,
    activity: HashMap<Uuid, Activity>,
    /// Last `last_active` write per Jam room (server ms).
    jam_pings: HashMap<Uuid, i64>,
}

/// In-memory presence of every connected device.
#[derive(Default)]
pub struct Realtime {
    live: Mutex<Live>,
}

enum ActivityStep {
    Nothing,
    Wait(Duration),
    Emit(Value),
}

impl Realtime {
    fn lock(&self) -> std::sync::MutexGuard<'_, Live> {
        self.live.lock().expect("realtime lock")
    }

    fn add_socket(&self, device: Uuid, user: Uuid, name: &str, now: i64) {
        let mut live = self.lock();
        let entry = live.devices.entry(device).or_insert_with(|| LiveDevice {
            user,
            name: name.to_owned(),
            sockets: 0,
            presence: None,
            since: now,
        });
        entry.sockets += 1;
        entry.name = name.to_owned();
        live.users.entry(user).or_default().insert(device);
    }

    /// Returns true when this was the device's last socket.
    fn remove_socket(&self, device: Uuid) -> bool {
        let mut live = self.lock();
        let Some(entry) = live.devices.get_mut(&device) else {
            return false;
        };
        entry.sockets = entry.sockets.saturating_sub(1);
        if entry.sockets > 0 {
            return false;
        }
        let user = entry.user;
        live.devices.remove(&device);
        if let Some(set) = live.users.get_mut(&user) {
            set.remove(&device);
            if set.is_empty() {
                live.users.remove(&user);
            }
        }
        true
    }

    /// Whether the user has at least one open socket.
    pub fn user_online(&self, user: Uuid) -> bool {
        self.lock().users.contains_key(&user)
    }

    /// The owner of a connected device.
    pub fn device_user(&self, device: Uuid) -> Option<Uuid> {
        self.lock()
            .devices
            .get(&device)
            .filter(|d| d.sockets > 0)
            .map(|d| d.user)
    }

    fn set_presence(&self, device: Uuid, presence: StoredPresence) {
        if let Some(entry) = self.lock().devices.get_mut(&device) {
            entry.presence = Some(presence);
        }
    }

    fn snapshot(&self, user: Uuid, except: Uuid) -> Vec<Value> {
        let live = self.lock();
        let mut devices: Vec<Value> = live
            .users
            .get(&user)
            .into_iter()
            .flatten()
            .filter(|d| **d != except)
            .filter_map(|d| live.devices.get(d).map(|e| (d, e)))
            .map(|(id, e)| {
                // A device that has not reported yet is online and idle.
                let (state, server_at) = e
                    .presence
                    .as_ref()
                    .map_or_else(|| (idle_state(e.since), e.since), |p| (p.state.clone(), p.server_at));
                json!({ "device_id": id, "device_name": e.name, "state": state, "server_at": server_at })
            })
            .collect();
        devices.sort_by(|a, b| a["device_id"].as_str().cmp(&b["device_id"].as_str()));
        devices
    }

    fn latest_playing(live: &Live, user: Uuid) -> Option<StoredPresence> {
        live.users
            .get(&user)?
            .iter()
            .filter_map(|d| live.devices.get(d)?.presence.clone())
            .filter(|p| p.media_key.is_some())
            .max_by_key(|p| p.server_at)
    }

    /// `FriendActivity.now_playing`: the most recently updated presence with media.
    pub fn now_playing(&self, user: Uuid) -> Value {
        Self::latest_playing(&self.lock(), user).map_or(Value::Null, |p| now_playing_json(&p))
    }

    fn activity_step(&self, user: Uuid, now: Instant) -> ActivityStep {
        let mut live = self.lock();
        let current = Self::latest_playing(&live, user);
        let key = current
            .as_ref()
            .map_or((None, false), |p| (p.media_key.clone(), p.playing));
        if live.activity.len() > 10_000 {
            live.activity.retain(|_, a| {
                a.pending
                    || a.last_emit
                        .is_some_and(|t| now.duration_since(t) < ACTIVITY_WINDOW)
            });
        }
        let entry = live.activity.entry(user).or_default();
        if entry.emitted == key || entry.pending {
            return ActivityStep::Nothing;
        }
        if let Some(last) = entry.last_emit {
            let elapsed = now.saturating_duration_since(last);
            if elapsed < ACTIVITY_WINDOW {
                entry.pending = true;
                return ActivityStep::Wait(ACTIVITY_WINDOW - elapsed);
            }
        }
        entry.last_emit = Some(now);
        entry.emitted = key;
        ActivityStep::Emit(current.map_or(Value::Null, |p| now_playing_json(&p)))
    }

    fn activity_wait_over(&self, user: Uuid) {
        if let Some(entry) = self.lock().activity.get_mut(&user) {
            entry.pending = false;
        }
    }

    /// Whether a `jam_ping` for this room should write `last_active` now.
    pub fn jam_ping_due(&self, room: Uuid, now: i64) -> bool {
        let mut live = self.lock();
        if live.jam_pings.len() > 10_000 {
            live.jam_pings.retain(|_, at| *at > now - JAM_PING_WRITE_MS);
        }
        match live.jam_pings.get(&room) {
            Some(at) if *at > now - JAM_PING_WRITE_MS => false,
            _ => {
                live.jam_pings.insert(room, now);
                true
            }
        }
    }
}

/// `PresenceState` of a connected device that has not sent `presence` yet.
fn idle_state(at: i64) -> Value {
    json!({
        "media": null, "playing": false, "position_ms": 0, "duration_ms": null, "rate": 1.0,
        "volume": null, "queue_len": 0, "at": at, "can": [],
    })
}

fn now_playing_json(p: &StoredPresence) -> Value {
    json!({
        "media": p.state["media"],
        "playing": p.playing,
        "position_ms": p.state["position_ms"],
        "server_at": p.server_at,
    })
}

/// One open WebSocket. Dropping it removes the socket from `Realtime` and announces
/// `presence_gone` when it was the device's last one.
pub struct Connection {
    state: AppState,
    device: Uuid,
    user: Uuid,
    name: String,
    presence: Bucket,
    messages: Bucket,
    bad: VecDeque<Instant>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        disconnected(&self.state, self.device, self.user);
    }
}

/// Registers a new socket of `device`. Keep the returned value alive for the session.
pub async fn connected(state: &AppState, device: &Device) -> Connection {
    let name = match state.0.db.get().await {
        Ok(client) => client
            .query_opt("SELECT name FROM devices WHERE id = $1", &[&device.device])
            .await
            .ok()
            .flatten()
            .map(|r| r.get::<_, String>(0)),
        Err(_) => None,
    }
    .unwrap_or_else(|| "Device".into());
    state
        .0
        .realtime
        .add_socket(device.device, device.user, &name, state.now());
    let now = Instant::now();
    Connection {
        state: state.clone(),
        device: device.device,
        user: device.user,
        name,
        presence: Bucket::new(PRESENCE_BURST, PRESENCE_PER_SEC, now),
        messages: Bucket::new(MESSAGE_BURST, MESSAGE_PER_SEC, now),
        bad: VecDeque::new(),
    }
}

fn disconnected(state: &AppState, device: Uuid, user: Uuid) {
    if !state.0.realtime.remove_socket(device) {
        return;
    }
    state.0.hub.publish(
        user,
        json!({ "type": "presence_gone", "device_id": device }),
    );
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(activity_changed(state.clone(), user));
    }
}

/// Whether a published user event goes to this device's socket: targeted events only
/// reach their `to_device`, and a device never hears its own presence.
pub fn delivers(event: &Value, device: Uuid) -> bool {
    if let Some(to) = event.get("to_device").and_then(Value::as_str) {
        return Uuid::parse_str(to).is_ok_and(|to| to == device);
    }
    let own = event.get("device_id").and_then(Value::as_str) == Some(&device.to_string());
    !(own && matches!(event["type"].as_str(), Some("presence" | "presence_gone")))
}

/// What the session does after a client message.
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    pub reply: Option<Value>,
    /// Close the socket with this code.
    pub close: Option<u16>,
}

struct Envelope {
    kind: String,
    rid: Option<i64>,
    body: Value,
}

fn envelope(text: &str) -> Result<Envelope, (Option<i64>, WsError)> {
    let body: Value =
        serde_json::from_str(text).map_err(|_| (None, WsError::bad("not a JSON object")))?;
    if !body.is_object() {
        return Err((None, WsError::bad("not a JSON object")));
    }
    let rid = match body.get("rid") {
        None | Some(Value::Null) => None,
        Some(v) => match v.as_i64() {
            Some(n) if (1..=MAX_RID).contains(&n) => Some(n),
            _ => return Err((None, WsError::bad("rid must be an integer 1..=2^31-1"))),
        },
    };
    let kind = match body.get("type").and_then(Value::as_str) {
        Some(t) if !t.is_empty() && t.len() <= MAX_TYPE_LEN => t.to_owned(),
        _ => return Err((rid, WsError::bad("type must be a short string"))),
    };
    Ok(Envelope { kind, rid, body })
}

fn error_json(rid: Option<i64>, e: &WsError) -> Value {
    let mut v = json!({ "type": "error", "code": e.code, "message": e.message });
    if let Some(rid) = rid {
        v["rid"] = rid.into();
    }
    v
}

fn typed<T: serde::de::DeserializeOwned>(body: &Value) -> Result<T, WsError> {
    serde_json::from_value(body.clone()).map_err(|_| WsError::bad("wrong field types"))
}

/// Media that may be announced to other devices: online only, stored slim.
fn slim_online(media: &Value) -> Result<(Value, String), WsError> {
    let (slim, key, _) = crate::social::slim_online(media).map_err(WsError::invalid)?;
    Ok((slim, key))
}

#[derive(Deserialize)]
struct PresenceIn {
    #[serde(default)]
    media: Option<Value>,
    #[serde(default)]
    playing: bool,
    #[serde(default)]
    position_ms: u64,
    #[serde(default)]
    duration_ms: Option<u64>,
    #[serde(default = "unit_rate")]
    rate: f64,
    #[serde(default)]
    volume: Option<u64>,
    #[serde(default)]
    queue_len: u64,
    #[serde(default)]
    at: Option<i64>,
    #[serde(default)]
    can: Vec<String>,
}

fn unit_rate() -> f64 {
    1.0
}

/// Validates a `PresenceState` and returns its canonical form and media key.
pub fn presence_state(raw: &Value, now: i64) -> Result<(Value, Option<String>, bool), WsError> {
    if !raw.is_object() {
        return Err(WsError::bad("state must be an object"));
    }
    let p: PresenceIn = typed(raw)?;
    let (media, key) = match &p.media {
        None | Some(Value::Null) => (Value::Null, None),
        Some(m) => {
            let (m, k) = slim_online(m)?;
            (m, Some(k))
        }
    };
    if p.position_ms > MAX_POSITION_MS || p.duration_ms.is_some_and(|d| d > MAX_POSITION_MS) {
        return Err(WsError::invalid(
            "position_ms and duration_ms must be at most 7 days",
        ));
    }
    if !(0.25..=4.0).contains(&p.rate) {
        return Err(WsError::invalid("rate must be 0.25 to 4.0"));
    }
    if p.volume.is_some_and(|v| v > 100) {
        return Err(WsError::invalid("volume must be 0 to 100"));
    }
    if p.queue_len > MAX_QUEUE_LEN {
        return Err(WsError::invalid("queue_len must be at most 10000"));
    }
    if p.at.is_some_and(|at| at <= 0) {
        return Err(WsError::invalid("at must be a Unix millisecond time"));
    }
    let mut can: Vec<&str> = Vec::new();
    for c in &p.can {
        match c.as_str() {
            "remote" | "handoff" if !can.contains(&c.as_str()) => can.push(c),
            "remote" | "handoff" => {}
            _ => return Err(WsError::invalid("can may only list remote and handoff")),
        }
    }
    let state = json!({
        "media": media,
        "playing": p.playing,
        "position_ms": p.position_ms,
        "duration_ms": p.duration_ms,
        "rate": p.rate,
        "volume": p.volume,
        "queue_len": p.queue_len,
        "at": p.at.unwrap_or(now),
        "can": can,
    });
    Ok((state, key, p.playing))
}

#[derive(Deserialize)]
struct HandoffIn {
    to_device: Uuid,
    media: Value,
    #[serde(default)]
    position_ms: u64,
    #[serde(default)]
    playing: bool,
    #[serde(default)]
    queue: Vec<Value>,
}

/// Validates a handoff and returns the forwarded fields (without sender fields).
pub fn handoff_fields(body: &Value) -> Result<(Uuid, Value), WsError> {
    let h: HandoffIn = typed(body)?;
    let (media, _) = slim_online(&h.media)?;
    if h.position_ms > MAX_POSITION_MS {
        return Err(WsError::invalid("position_ms must be at most 7 days"));
    }
    if h.queue.len() > MAX_HANDOFF_QUEUE {
        return Err(WsError::new(TOO_LARGE, "queue holds at most 25 items"));
    }
    let queue = h
        .queue
        .iter()
        .map(|m| slim_online(m).map(|(m, _)| m))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        h.to_device,
        json!({
            "type": "handoff",
            "media": media,
            "position_ms": h.position_ms,
            "playing": h.playing,
            "queue": queue,
            "to_device": h.to_device,
        }),
    ))
}

#[derive(Deserialize)]
struct RemoteIn {
    to_device: Uuid,
    command: String,
    #[serde(default)]
    position_ms: Option<u64>,
    #[serde(default)]
    volume: Option<u64>,
}

pub const REMOTE_COMMANDS: &[&str] = &["play", "pause", "toggle", "seek", "next", "prev", "volume"];

pub fn remote_fields(body: &Value) -> Result<(Uuid, Value), WsError> {
    let r: RemoteIn = typed(body)?;
    if !REMOTE_COMMANDS.contains(&r.command.as_str()) {
        return Err(WsError::invalid(format!(
            "command must be one of {}",
            REMOTE_COMMANDS.join(", ")
        )));
    }
    let mut event = json!({ "type": "remote", "command": r.command, "to_device": r.to_device });
    match r.command.as_str() {
        "seek" => match r.position_ms {
            Some(p) if p <= MAX_POSITION_MS => event["position_ms"] = p.into(),
            _ => return Err(WsError::invalid("seek needs position_ms up to 7 days")),
        },
        "volume" => match r.volume {
            Some(v) if v <= 100 => event["volume"] = v.into(),
            _ => return Err(WsError::invalid("volume needs volume 0 to 100")),
        },
        _ => {}
    }
    Ok((r.to_device, event))
}

impl Connection {
    /// Records a protocol error; true when the socket must close.
    fn strike(&mut self, now: Instant) -> bool {
        while self
            .bad
            .front()
            .is_some_and(|t| now.duration_since(*t) >= BAD_MESSAGE_WINDOW)
        {
            self.bad.pop_front();
        }
        self.bad.push_back(now);
        self.bad.len() >= MAX_BAD_MESSAGES
    }

    /// The target of a handoff/remote must be another connected device of this user.
    fn target(&self, to: Uuid) -> Result<(), WsError> {
        let ours = self.state.0.realtime.device_user(to) == Some(self.user);
        if to == self.device || !ours || !self.state.0.hub.device_online(to) {
            return Err(WsError::new(NOT_FOUND, "That device is not connected"));
        }
        Ok(())
    }

    async fn dispatch(&mut self, env: &Envelope) -> Result<Option<Value>, WsError> {
        let state = self.state.clone();
        let now = state.now();
        match env.kind.as_str() {
            "ping" => {
                let t0 = env
                    .body
                    .get("t0")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| WsError::bad("ping needs t0"))?;
                Ok(Some(
                    json!({ "type": "pong", "t0": t0, "server_ms": state.now() }),
                ))
            }
            "presence" => {
                let raw = env
                    .body
                    .get("state")
                    .ok_or_else(|| WsError::bad("presence needs state"))?;
                let (presence, media_key, playing) = presence_state(raw, now)?;
                state.0.realtime.set_presence(
                    self.device,
                    StoredPresence {
                        state: presence.clone(),
                        server_at: now,
                        media_key,
                        playing,
                    },
                );
                state.0.hub.publish(
                    self.user,
                    json!({
                        "type": "presence",
                        "device_id": self.device,
                        "device_name": self.name,
                        "state": presence,
                        "server_at": now,
                    }),
                );
                tokio::spawn(activity_changed(state.clone(), self.user));
                Ok(None)
            }
            "presence_query" => Ok(Some(json!({
                "type": "presence_snapshot",
                "devices": state.0.realtime.snapshot(self.user, self.device),
            }))),
            "handoff" => {
                let (to, mut event) = handoff_fields(&env.body)?;
                self.target(to)?;
                event["from_device"] = json!(self.device);
                event["from_name"] = json!(self.name);
                state.0.hub.publish(self.user, event);
                Ok(None)
            }
            "remote" => {
                let (to, mut event) = remote_fields(&env.body)?;
                self.target(to)?;
                event["from_device"] = json!(self.device);
                event["from_name"] = json!(self.name);
                state.0.hub.publish(self.user, event);
                Ok(None)
            }
            "jam_ping" => {
                let room = env
                    .body
                    .get("room_id")
                    .and_then(Value::as_str)
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .ok_or_else(|| WsError::bad("jam_ping needs room_id"))?;
                crate::jam::ping(&state, self.user, room).await?;
                Ok(None)
            }
            _ => Err(WsError::new(UNKNOWN_TYPE, "unknown message type")),
        }
    }
}

const KNOWN: &[&str] = &[
    "ping",
    "presence",
    "presence_query",
    "handoff",
    "remote",
    "jam_ping",
];

/// Handles one text frame and returns the reply for this socket, if any.
pub async fn on_message(conn: &mut Connection, text: &str) -> Outcome {
    let now = Instant::now();
    let env = match envelope(text) {
        Ok(env) => env,
        Err((rid, e)) => {
            let close = conn.strike(now).then_some(CLOSE_POLICY);
            return Outcome {
                reply: Some(error_json(rid, &e)),
                close,
            };
        }
    };
    let rid = env.rid;
    let fail = |e: WsError, conn: &mut Connection| {
        let protocol = matches!(e.code, BAD_MESSAGE | UNKNOWN_TYPE);
        let close = (protocol && conn.strike(now)).then_some(CLOSE_POLICY);
        // Without a rid, only bad_message is answered.
        let reply = (rid.is_some() || e.code == BAD_MESSAGE).then(|| error_json(rid, &e));
        Outcome { reply, close }
    };
    if !KNOWN.contains(&env.kind.as_str()) {
        return fail(WsError::new(UNKNOWN_TYPE, "unknown message type"), conn);
    }
    let allowed = if env.kind == "presence" {
        conn.presence.take(now)
    } else {
        conn.messages.take(now)
    };
    if !allowed {
        return fail(WsError::new(RATE_LIMITED, "slow down"), conn);
    }
    match conn.dispatch(&env).await {
        Ok(Some(mut reply)) => {
            if let Some(rid) = rid {
                reply["rid"] = rid.into();
            }
            Outcome {
                reply: Some(reply),
                close: None,
            }
        }
        Ok(None) => Outcome {
            reply: rid.map(|rid| json!({ "type": "ack", "rid": rid })),
            close: None,
        },
        Err(e) => fail(e, conn),
    }
}

/// Re-evaluates a user's shared now-playing and tells their friends when the media key
/// or the playing flag changed, at most once per `ACTIVITY_WINDOW`.
pub async fn activity_changed(state: AppState, user: Uuid) {
    loop {
        match state.0.realtime.activity_step(user, Instant::now()) {
            ActivityStep::Nothing => return,
            ActivityStep::Wait(d) => {
                tokio::time::sleep(d).await;
                state.0.realtime.activity_wait_over(user);
            }
            ActivityStep::Emit(now_playing) => {
                if let Err(e) = crate::social::publish_activity(&state, user, now_playing).await {
                    tracing::warn!("friend activity: {e:#}");
                }
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(url: &str) -> Value {
        json!({"id": "abcdefghijk", "title": "Song", "url": url, "description": "long", "chapters": [{"title": "x", "start": 1.0}]})
    }

    #[test]
    fn buckets_refill_over_time() {
        let t0 = Instant::now();
        let mut presence = Bucket::new(PRESENCE_BURST, PRESENCE_PER_SEC, t0);
        assert!(presence.take(t0));
        assert!(!presence.take(t0 + Duration::from_millis(1900)));
        assert!(presence.take(t0 + Duration::from_millis(4000)));
        let mut messages = Bucket::new(MESSAGE_BURST, MESSAGE_PER_SEC, t0);
        assert_eq!((0..25).filter(|_| messages.take(t0)).count(), 20);
        assert!(messages.take(t0 + Duration::from_millis(500)));
        assert!(!messages.take(t0 + Duration::from_millis(500)));
        // Never more than the burst, however long the pause.
        let later = t0 + Duration::from_secs(3600);
        assert_eq!((0..25).filter(|_| messages.take(later)).count(), 20);
    }

    #[test]
    fn envelopes_check_rid_and_type() {
        assert_eq!(envelope("nope").err().unwrap().1.code, BAD_MESSAGE);
        assert_eq!(envelope("[1]").err().unwrap().1.code, BAD_MESSAGE);
        assert_eq!(
            envelope(r#"{"rid":0,"type":"ping"}"#).err().unwrap().1.code,
            BAD_MESSAGE
        );
        assert_eq!(
            envelope(r#"{"rid":2147483648,"type":"ping"}"#)
                .err()
                .unwrap()
                .1
                .code,
            BAD_MESSAGE
        );
        assert_eq!(
            envelope(r#"{"rid":"1","type":"ping"}"#)
                .err()
                .unwrap()
                .1
                .code,
            BAD_MESSAGE
        );
        let (rid, e) = envelope(r#"{"rid":7,"type":5}"#).err().unwrap();
        assert_eq!((rid, e.code), (Some(7), BAD_MESSAGE));
        let long = format!(r#"{{"type":"{}"}}"#, "x".repeat(33));
        assert!(envelope(&long).is_err());
        let ok = envelope(r#"{"rid":2147483647,"type":"ping","t0":5,"extra":1}"#).unwrap();
        assert_eq!((ok.kind.as_str(), ok.rid), ("ping", Some(2_147_483_647)));
    }

    #[test]
    fn presence_states_are_validated_and_slimmed() {
        let raw = json!({"media": media("https://youtu.be/abcdefghijk"), "playing": true, "position_ms": 51000,
                         "duration_ms": 215000, "rate": 1.0, "volume": 80, "queue_len": 4,
                         "at": 1_763_900_000_000i64, "can": ["remote", "handoff", "remote"], "extra": true});
        let (state, key, playing) = presence_state(&raw, 1).unwrap();
        assert_eq!(key.as_deref(), Some("youtube:abcdefghijk"));
        assert!(playing);
        assert_eq!(state["media"]["description"], "");
        assert_eq!(state["media"]["chapters"], json!([]));
        assert_eq!(state["can"], json!(["remote", "handoff"]));
        assert!(state.get("extra").is_none());
        let (idle, key, _) = presence_state(&json!({"media": null}), 9).unwrap();
        assert_eq!(
            (idle["media"].clone(), key, idle["at"].clone()),
            (Value::Null, None, json!(9))
        );
        let code = |v: Value| presence_state(&v, 1).unwrap_err().code;
        assert_eq!(code(json!({"media": media("/sdcard/song.mp3")})), INVALID);
        assert_eq!(code(json!({"rate": 5.0})), INVALID);
        assert_eq!(code(json!({"volume": 101})), INVALID);
        assert_eq!(code(json!({"queue_len": 10_001})), INVALID);
        assert_eq!(code(json!({"can": ["admin"]})), INVALID);
        assert_eq!(code(json!({"position_ms": MAX_POSITION_MS + 1})), INVALID);
        assert_eq!(code(json!({"position_ms": -1})), BAD_MESSAGE);
        assert_eq!(code(json!({"playing": "yes"})), BAD_MESSAGE);
        assert_eq!(code(json!("x")), BAD_MESSAGE);
    }

    #[test]
    fn handoff_and_remote_validation() {
        let to = Uuid::new_v4();
        let online = media("https://youtu.be/abcdefghijk");
        let (target, event) = handoff_fields(
            &json!({"to_device": to, "media": online, "position_ms": 5, "playing": true,
                                                     "queue": [online.clone()]}),
        )
        .unwrap();
        assert_eq!(target, to);
        assert_eq!(event["queue"][0]["description"], "");
        assert_eq!(event["to_device"], json!(to));
        let queue: Vec<Value> = (0..26).map(|_| online.clone()).collect();
        assert_eq!(
            handoff_fields(&json!({"to_device": to, "media": online, "queue": queue}))
                .unwrap_err()
                .code,
            TOO_LARGE
        );
        assert_eq!(
            handoff_fields(
                &json!({"to_device": to, "media": online, "queue": [media("file:///x")]})
            )
            .unwrap_err()
            .code,
            INVALID
        );
        assert_eq!(
            handoff_fields(&json!({"to_device": "nope", "media": online}))
                .unwrap_err()
                .code,
            BAD_MESSAGE
        );
        let (_, seek) = remote_fields(
            &json!({"to_device": to, "command": "seek", "position_ms": 9000, "volume": 3}),
        )
        .unwrap();
        assert_eq!(seek["position_ms"], 9000);
        assert!(seek.get("volume").is_none());
        let (_, vol) =
            remote_fields(&json!({"to_device": to, "command": "volume", "volume": 40})).unwrap();
        assert_eq!(vol["volume"], 40);
        for bad in [
            json!({"to_device": to, "command": "seek"}),
            json!({"to_device": to, "command": "volume", "volume": 101}),
            json!({"to_device": to, "command": "shutdown"}),
        ] {
            assert_eq!(remote_fields(&bad).unwrap_err().code, INVALID, "{bad}");
        }
    }

    #[test]
    fn targeted_events_reach_only_their_device() {
        let (me, other) = (Uuid::new_v4(), Uuid::new_v4());
        assert!(delivers(&json!({"type": "handoff", "to_device": me}), me));
        assert!(!delivers(
            &json!({"type": "handoff", "to_device": other}),
            me
        ));
        assert!(!delivers(
            &json!({"type": "remote", "to_device": "garbage"}),
            me
        ));
        assert!(!delivers(&json!({"type": "presence", "device_id": me}), me));
        assert!(!delivers(
            &json!({"type": "presence_gone", "device_id": me}),
            me
        ));
        assert!(delivers(
            &json!({"type": "presence", "device_id": other}),
            me
        ));
        assert!(delivers(
            &json!({"type": "changed", "seq": 3, "device_id": me}),
            me
        ));
    }

    #[test]
    fn activity_is_coalesced_per_window() {
        let rt = Realtime::default();
        let (user, device) = (Uuid::new_v4(), Uuid::new_v4());
        rt.add_socket(device, user, "Phone", 1);
        let other = Uuid::new_v4();
        rt.add_socket(other, user, "Tablet", 5);
        let snap = rt.snapshot(user, device);
        assert_eq!((snap.len(), snap[0]["server_at"].as_i64()), (1, Some(5)));
        assert_eq!(snap[0]["state"]["media"], Value::Null, "online but idle");
        assert!(rt.remove_socket(other));
        let t0 = Instant::now();
        // Idle at start: nothing to tell.
        assert!(matches!(rt.activity_step(user, t0), ActivityStep::Nothing));
        let play = |key: &str, playing: bool, at: i64| StoredPresence {
            state: json!({"media": {"url": key}, "position_ms": 0}),
            server_at: at,
            media_key: Some(key.into()),
            playing,
        };
        rt.set_presence(device, play("youtube:a", true, 1));
        assert!(
            matches!(rt.activity_step(user, t0), ActivityStep::Emit(v) if v["playing"] == true)
        );
        assert!(matches!(rt.activity_step(user, t0), ActivityStep::Nothing));
        rt.set_presence(device, play("youtube:a", false, 2));
        let wait = rt.activity_step(user, t0 + Duration::from_secs(4));
        assert!(matches!(wait, ActivityStep::Wait(d) if d == Duration::from_secs(6)));
        // A second change while waiting does not schedule another event.
        rt.set_presence(device, play("youtube:b", true, 3));
        assert!(matches!(
            rt.activity_step(user, t0 + Duration::from_secs(5)),
            ActivityStep::Nothing
        ));
        rt.activity_wait_over(user);
        assert!(matches!(
            rt.activity_step(user, t0 + Duration::from_secs(10)),
            ActivityStep::Emit(v) if v["media"]["url"] == "youtube:b"
        ));
        assert!(rt.remove_socket(device));
        assert!(!rt.user_online(user));
        assert!(matches!(
            rt.activity_step(user, t0 + Duration::from_secs(30)),
            ActivityStep::Emit(Value::Null)
        ));
    }

    #[test]
    fn jam_pings_write_at_most_every_30_seconds() {
        let rt = Realtime::default();
        let room = Uuid::new_v4();
        assert!(rt.jam_ping_due(room, 1_000_000));
        assert!(!rt.jam_ping_due(room, 1_029_000));
        assert!(rt.jam_ping_due(room, 1_030_001));
        assert!(rt.jam_ping_due(Uuid::new_v4(), 1_030_001));
    }
}
