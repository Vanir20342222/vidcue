//! Jam rooms: friends listening or watching together.
//!
//! Coordinate-only: the server stores the queue and the playback state and keeps the
//! clock; it never fetches, resolves or proxies media. Every device starts the current
//! item itself and aligns to `JamPlayback` (expected position at server time `T` is
//! `position_ms + (T - at_server_ms) * rate` while playing).
//!
//! Locking: room changes hold the room row (`FOR UPDATE`). Changes to a user's room
//! membership also hold a per-user advisory lock and lock all involved rooms in id
//! order. Friendship rows are always locked before room rows (see `social`).

use crate::api::{ApiError, ApiResult, AppState, Device};
use crate::presence::{NOT_FOUND, WsError};
use crate::social::{Events, conflict, forbidden, not_found, publish, slim_online, title_ok};
use crate::throttle::Rule;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::cmp::Ordering;
use tokio_postgres::GenericClient;
use uuid::Uuid;

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;

pub const MAX_JAM_MEMBERS: i64 = 10;
pub const MAX_JAM_QUEUE: i64 = 200;
pub const MAX_JAM_INVITES: i64 = 20;
/// User ids per invite request.
pub const MAX_INVITE_BATCH: usize = 10;
pub const MAX_JAM_TITLE: usize = 80;
/// A room ends when nobody bumped `last_active` (room action or `jam_ping`) for this long.
pub const ROOM_IDLE_MS: i64 = 30 * MINUTE;
pub const ROOM_MAX_AGE_MS: i64 = 12 * HOUR;
/// Ended rooms are deleted after this long.
pub const ENDED_RETENTION_MS: i64 = 24 * HOUR;
pub const MAX_JAM_POSITION_MS: i64 = 24 * HOUR;
pub const MIN_RATE: f64 = 0.5;
pub const MAX_RATE: f64 = 2.0;

/// Queue additions per user.
pub const JAM_ADD: Rule = Rule {
    limit: 30,
    window_ms: 10 * MINUTE,
};
/// Playback actions per user.
pub const JAM_CONTROL: Rule = Rule {
    limit: 60,
    window_ms: MINUTE,
};
/// Room edits per user: votes, queue removals, settings, invites and declines. Each one
/// bumps the room version and notifies every member.
pub const JAM_EDIT: Rule = Rule {
    limit: 60,
    window_ms: MINUTE,
};
/// Invite requests per user.
pub const JAM_INVITE: Rule = Rule {
    limit: 30,
    window_ms: 10 * MINUTE,
};
/// A guest without control may report `ended` only this close to the item's end.
pub const ENDED_TOLERANCE_MS: i64 = 5_000;
/// With an unknown duration, a guest's `ended` needs this much played first.
pub const ENDED_UNKNOWN_MIN_MS: i64 = 30_000;
/// Rooms created per user.
pub const JAM_CREATE: Rule = Rule {
    limit: 10,
    window_ms: HOUR,
};

const PLAYING: &str = "playing";
const PAUSED: &str = "paused";
const STOPPED: &str = "stopped";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Current {
    pub item_id: Uuid,
    pub media: Value,
    pub media_key: String,
    pub added_by: Uuid,
}

/// `JamPlayback`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Playback {
    pub item: Option<Current>,
    pub state: String,
    pub position_ms: i64,
    pub at_server_ms: i64,
    pub rate: f64,
    pub version: i64,
}

impl Playback {
    pub fn stopped(now: i64, rate: f64) -> Self {
        Self {
            item: None,
            state: STOPPED.into(),
            position_ms: 0,
            at_server_ms: now,
            rate,
            version: 1,
        }
    }

    /// Length of the current item in ms, when known.
    pub fn duration_ms(&self) -> Option<i64> {
        self.item
            .as_ref()
            .and_then(|i| i.media.get("duration"))
            .and_then(Value::as_i64)
            .filter(|d| *d > 0)
            .map(|d| d.saturating_mul(1000))
    }

    /// The position every device should be at, at server time `now`.
    pub fn expected(&self, now: i64) -> i64 {
        let mut position = self.position_ms;
        if self.state == PLAYING {
            let elapsed = (now - self.at_server_ms).max(0) as f64 * self.rate;
            position = position.saturating_add(elapsed.round() as i64);
        }
        let end = self.duration_ms().unwrap_or(i64::MAX);
        position.clamp(0, end)
    }

    pub fn check_position(&self, position: i64) -> Result<(), String> {
        if !(0..=MAX_JAM_POSITION_MS).contains(&position)
            || self.duration_ms().is_some_and(|d| position > d)
        {
            return Err("position_ms must be within the media and at most 24 hours".into());
        }
        Ok(())
    }

    /// Whether a member without playback control may report the current item as ended:
    /// only when it has (nearly) reached its end, so `ended` cannot be used to skip.
    pub fn ended_plausible(&self, now: i64) -> bool {
        let position = self.expected(now);
        match self.duration_ms() {
            Some(duration) => position >= duration - ENDED_TOLERANCE_MS,
            None => position >= ENDED_UNKNOWN_MIN_MS,
        }
    }

    /// `item` becomes current at 0 and plays.
    pub fn start(&self, item: Option<Current>, now: i64) -> Self {
        match item {
            Some(item) => Self {
                item: Some(item),
                state: PLAYING.into(),
                position_ms: 0,
                at_server_ms: now,
                rate: self.rate,
                version: self.version,
            },
            None => Self::stopped(now, self.rate),
        }
    }

    fn anchored(&self, state: &str, position: Option<i64>, now: i64) -> Result<Self, String> {
        if let Some(p) = position {
            self.check_position(p)?;
        }
        Ok(Self {
            state: state.into(),
            position_ms: position.unwrap_or_else(|| self.expected(now)),
            at_server_ms: now,
            ..self.clone()
        })
    }

    pub fn play(&self, position: Option<i64>, now: i64) -> Result<Self, String> {
        self.anchored(PLAYING, position, now)
    }

    pub fn pause(&self, position: Option<i64>, now: i64) -> Result<Self, String> {
        self.anchored(PAUSED, position, now)
    }

    pub fn seek(&self, position: i64, now: i64) -> Result<Self, String> {
        let state = self.state.clone();
        self.anchored(&state, Some(position), now)
    }

    pub fn with_rate(&self, rate: f64, now: i64) -> Result<Self, String> {
        if !(MIN_RATE..=MAX_RATE).contains(&rate) {
            return Err("rate must be 0.5 to 2.0".into());
        }
        let mut next = self.seek(self.expected(now), now)?;
        next.rate = rate;
        Ok(next)
    }
}

#[derive(Clone, Debug)]
pub struct QueueItem {
    pub item_id: Uuid,
    pub media: Value,
    pub added_by: Uuid,
    pub added_by_name: String,
    pub added_at: i64,
    pub votes: i64,
    pub voted: bool,
}

/// Queue order: most votes first, then oldest, then item id.
pub fn queue_order(a: &QueueItem, b: &QueueItem) -> Ordering {
    b.votes
        .cmp(&a.votes)
        .then(a.added_at.cmp(&b.added_at))
        .then(a.item_id.cmp(&b.item_id))
}

fn media_key(media: &Value) -> String {
    serde_json::from_value::<vidcue_core::Media>(media.clone())
        .map(|m| vidcue_core::playback_history::PlaybackHistory::key(&m))
        .unwrap_or_default()
}

impl QueueItem {
    fn json(&self) -> Value {
        json!({
            "item_id": self.item_id,
            "media": self.media,
            "media_key": media_key(&self.media),
            "added_by": { "user_id": self.added_by, "username": self.added_by_name },
            "added_at": self.added_at,
            "votes": self.votes,
            "voted": self.voted,
        })
    }
    fn current(&self) -> Current {
        Current {
            item_id: self.item_id,
            media: self.media.clone(),
            media_key: media_key(&self.media),
            added_by: self.added_by,
        }
    }
}

/// `JamAction`.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Play(Option<i64>),
    Pause(Option<i64>),
    Seek(i64),
    Next,
    PlayItem(Uuid),
    Ended(Uuid),
    Rate(f64),
}

#[derive(Deserialize)]
struct ActionBody {
    action: String,
    #[serde(default)]
    position_ms: Option<i64>,
    #[serde(default)]
    item_id: Option<Uuid>,
    #[serde(default)]
    rate: Option<f64>,
}

impl Action {
    fn parse(body: &ActionBody) -> Result<Self, String> {
        let item = || {
            body.item_id
                .ok_or_else(|| format!("{} needs item_id", body.action))
        };
        Ok(match body.action.as_str() {
            "play" => Self::Play(body.position_ms),
            "pause" => Self::Pause(body.position_ms),
            "seek" => Self::Seek(body.position_ms.ok_or("seek needs position_ms")?),
            "next" => Self::Next,
            "play_item" => Self::PlayItem(item()?),
            "ended" => Self::Ended(item()?),
            "rate" => Self::Rate(body.rate.ok_or("rate needs rate")?),
            _ => {
                return Err(
                    "action must be play, pause, seek, next, play_item, ended or rate".into(),
                );
            }
        })
    }
}

struct Room {
    id: Uuid,
    guests_can_control: bool,
    playback: Playback,
    version: i64,
}

const ROOM_COLUMNS: &str = "id, guests_can_control, playback, version";

fn room_from(row: &tokio_postgres::Row) -> anyhow::Result<Room> {
    Ok(Room {
        id: row.get("id"),
        guests_can_control: row.get("guests_can_control"),
        playback: serde_json::from_value(row.get("playback"))?,
        version: row.get("version"),
    })
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/jam", get(current).post(create))
        .route("/api/jam/{id}", get(room).patch(update).delete(end))
        .route("/api/jam/{id}/invite", post(invite))
        .route("/api/jam/{id}/join", post(join))
        .route("/api/jam/{id}/decline", post(decline))
        .route("/api/jam/{id}/leave", post(leave))
        .route("/api/jam/{id}/members/{user_id}", delete(kick))
        .route("/api/jam/{id}/queue", post(queue_add))
        .route("/api/jam/{id}/queue/{item_id}", delete(queue_remove))
        .route("/api/jam/{id}/queue/{item_id}/vote", post(vote))
        .route("/api/jam/{id}/playback", post(playback))
}

// ---------------------------------------------------------------- helpers

/// Records one room edit for `user` (before any work), refused over `JAM_EDIT`.
async fn edit_attempt(state: &AppState, user: Uuid) -> ApiResult<()> {
    let key = format!("jam-edit:{user}");
    state
        .attempt(&[(JAM_EDIT, &key)], "Too many Jam changes, slow down")
        .await?;
    Ok(())
}

fn no_room() -> ApiError {
    not_found("No such Jam")
}

fn to_all(members: &[Uuid], event: &Value) -> Events {
    members.iter().map(|m| (*m, event.clone())).collect()
}

fn changed(room: Uuid, version: i64, parts: &[&str]) -> Value {
    json!({ "type": "jam_changed", "room_id": room, "version": version, "parts": parts })
}

fn state_event(room: Uuid, playback: &Playback, now: i64) -> Value {
    json!({ "type": "jam_state", "room_id": room, "playback": playback, "server_ms": now })
}

async fn member_ids(
    db: &(impl GenericClient + Sync),
    room: Uuid,
) -> Result<Vec<Uuid>, tokio_postgres::Error> {
    Ok(db
        .query(
            "SELECT user_id FROM jam_members WHERE room_id = $1",
            &[&room],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect())
}

async fn role(
    db: &(impl GenericClient + Sync),
    room: Uuid,
    user: Uuid,
) -> Result<Option<String>, tokio_postgres::Error> {
    Ok(db
        .query_opt(
            "SELECT role FROM jam_members WHERE room_id = $1 AND user_id = $2",
            &[&room, &user],
        )
        .await?
        .map(|r| r.get(0)))
}

/// Locks an active room for this transaction.
async fn lock_room(db: &(impl GenericClient + Sync), id: Uuid, now: i64) -> ApiResult<Room> {
    let row = db
        .query_opt(
            &format!(
                "SELECT {ROOM_COLUMNS} FROM jam_rooms
                 WHERE id = $1 AND ended_at IS NULL AND created_at > $2 AND last_active > $3
                 FOR UPDATE"
            ),
            &[&id, &(now - ROOM_MAX_AGE_MS), &(now - ROOM_IDLE_MS)],
        )
        .await?
        .ok_or_else(no_room)?;
    Ok(room_from(&row)?)
}

/// Locks an active room the caller is a member of; returns it with the caller's role.
async fn member_room(
    db: &(impl GenericClient + Sync),
    id: Uuid,
    user: Uuid,
    now: i64,
) -> ApiResult<(Room, String)> {
    let room = lock_room(db, id, now).await?;
    let role = role(db, id, user).await?.ok_or_else(no_room)?;
    Ok((room, role))
}

async fn bump(
    db: &(impl GenericClient + Sync),
    room: Uuid,
    now: i64,
) -> Result<i64, tokio_postgres::Error> {
    Ok(db
        .query_one(
            "UPDATE jam_rooms SET version = version + 1, last_active = $2 WHERE id = $1 RETURNING version",
            &[&room, &now],
        )
        .await?
        .get(0))
}

/// Ends a room; returns `jam_ended` for every member.
async fn end_room(
    db: &(impl GenericClient + Sync),
    room: Uuid,
    reason: &str,
    now: i64,
) -> Result<Events, tokio_postgres::Error> {
    let ended = db
        .execute(
            "UPDATE jam_rooms SET ended_at = $2, version = version + 1 WHERE id = $1 AND ended_at IS NULL",
            &[&room, &now],
        )
        .await?;
    if ended == 0 {
        return Ok(vec![]);
    }
    db.execute("DELETE FROM jam_invites WHERE room_id = $1", &[&room])
        .await?;
    let event = json!({ "type": "jam_ended", "room_id": room, "reason": reason });
    Ok(to_all(&member_ids(db, room).await?, &event))
}

/// Removes a guest (and their votes); returns `jam_changed` for the remaining members,
/// or `None` when they were not a member.
async fn remove_member(
    db: &(impl GenericClient + Sync),
    room: Uuid,
    user: Uuid,
    now: i64,
) -> Result<Option<Events>, tokio_postgres::Error> {
    let removed = db
        .execute(
            "DELETE FROM jam_members WHERE room_id = $1 AND user_id = $2 AND role = 'guest'",
            &[&room, &user],
        )
        .await?;
    if removed == 0 {
        return Ok(None);
    }
    db.execute(
        "DELETE FROM jam_votes WHERE room_id = $1 AND user_id = $2",
        &[&room, &user],
    )
    .await?;
    let version = bump(db, room, now).await?;
    Ok(Some(to_all(
        &member_ids(db, room).await?,
        &changed(room, version, &["members", "queue"]),
    )))
}

/// Active rooms the user belongs to (normally at most one).
async fn active_rooms(
    db: &(impl GenericClient + Sync),
    user: Uuid,
) -> Result<Vec<Uuid>, tokio_postgres::Error> {
    Ok(db
        .query(
            "SELECT r.id FROM jam_members m JOIN jam_rooms r ON r.id = m.room_id
             WHERE m.user_id = $1 AND r.ended_at IS NULL ORDER BY r.id",
            &[&user],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect())
}

/// Holds the per-user membership lock and row locks on `rooms` (in id order).
async fn lock_user_rooms(
    db: &(impl GenericClient + Sync),
    user: Uuid,
    extra: Option<Uuid>,
) -> Result<Vec<Uuid>, tokio_postgres::Error> {
    db.execute(
        "SELECT pg_advisory_xact_lock(hashtextextended($1::text, 3))",
        &[&user.to_string()],
    )
    .await?;
    let current = active_rooms(db, user).await?;
    let mut all: Vec<Uuid> = current.iter().copied().chain(extra).collect();
    all.sort();
    all.dedup();
    db.query(
        "SELECT id FROM jam_rooms WHERE id = ANY($1) ORDER BY id FOR UPDATE",
        &[&all],
    )
    .await?;
    Ok(current)
}

/// Leaves (or, as host, ends) the user's current rooms.
async fn leave_current(
    db: &(impl GenericClient + Sync),
    user: Uuid,
    rooms: &[Uuid],
    now: i64,
) -> Result<Events, tokio_postgres::Error> {
    let mut events = Events::new();
    for room in rooms {
        match role(db, *room, user).await?.as_deref() {
            Some("host") => events.extend(end_room(db, *room, "ended", now).await?),
            Some(_) => events.extend(
                remove_member(db, *room, user, now)
                    .await?
                    .unwrap_or_default(),
            ),
            None => {}
        }
    }
    Ok(events)
}

enum Scope {
    Room(Uuid),
    User(Uuid),
}

/// Ends rooms past their lifetime and tells their members (checked lazily on access;
/// `maintenance` catches the rest without events).
async fn expire(state: &AppState, scope: Scope) -> ApiResult<()> {
    let now = state.now();
    let client = state.0.db.get().await?;
    let due =
        "ended_at IS NULL AND (created_at <= $1 - $2::BIGINT OR last_active <= $1 - $3::BIGINT)";
    let rows = match scope {
        Scope::Room(id) => {
            client
                .query(
                    &format!(
                        "UPDATE jam_rooms SET ended_at = $1, version = version + 1 WHERE id = $4 AND {due} RETURNING id"
                    ),
                    &[&now, &ROOM_MAX_AGE_MS, &ROOM_IDLE_MS, &id],
                )
                .await?
        }
        Scope::User(user) => {
            client
                .query(
                    &format!(
                        "UPDATE jam_rooms SET ended_at = $1, version = version + 1
                         WHERE id IN (SELECT room_id FROM jam_members WHERE user_id = $4) AND {due} RETURNING id"
                    ),
                    &[&now, &ROOM_MAX_AGE_MS, &ROOM_IDLE_MS, &user],
                )
                .await?
        }
    };
    let mut events = Events::new();
    for row in rows {
        let room: Uuid = row.get(0);
        client
            .execute("DELETE FROM jam_invites WHERE room_id = $1", &[&room])
            .await?;
        let event = json!({ "type": "jam_ended", "room_id": room, "reason": "expired" });
        events.extend(to_all(&member_ids(&**client, room).await?, &event));
    }
    publish(state, events);
    Ok(())
}

async fn load_queue(
    db: &(impl GenericClient + Sync),
    room: Uuid,
    caller: Uuid,
) -> Result<Vec<QueueItem>, tokio_postgres::Error> {
    let rows = db
        .query(
            "SELECT q.item_id, q.media, q.added_by, COALESCE(u.username, '') AS username, q.added_at,
                    (SELECT COUNT(*) FROM jam_votes v WHERE v.room_id = q.room_id AND v.item_id = q.item_id) AS votes,
                    EXISTS (SELECT 1 FROM jam_votes v WHERE v.room_id = q.room_id AND v.item_id = q.item_id AND v.user_id = $2) AS voted
             FROM jam_queue q LEFT JOIN users u ON u.id = q.added_by
             WHERE q.room_id = $1",
            &[&room, &caller],
        )
        .await?;
    let mut items: Vec<QueueItem> = rows
        .iter()
        .map(|r| QueueItem {
            item_id: r.get("item_id"),
            media: r.get("media"),
            added_by: r.get("added_by"),
            added_by_name: r.get("username"),
            added_at: r.get("added_at"),
            votes: r.get("votes"),
            voted: r.get("voted"),
        })
        .collect();
    items.sort_by(queue_order);
    Ok(items)
}

/// Removes and returns the top of the queue.
async fn take_top(
    db: &(impl GenericClient + Sync),
    room: Uuid,
) -> Result<Option<Current>, tokio_postgres::Error> {
    let Some(top) = load_queue(db, room, Uuid::nil()).await?.into_iter().next() else {
        return Ok(None);
    };
    take_item(db, room, top.item_id).await
}

async fn take_item(
    db: &(impl GenericClient + Sync),
    room: Uuid,
    item: Uuid,
) -> Result<Option<Current>, tokio_postgres::Error> {
    Ok(db
        .query_opt(
            "DELETE FROM jam_queue WHERE room_id = $1 AND item_id = $2 RETURNING item_id, media, added_by",
            &[&room, &item],
        )
        .await?
        .map(|r| {
            let media: Value = r.get(1);
            Current {
                item_id: r.get(0),
                media_key: media_key(&media),
                media,
                added_by: r.get(2),
            }
        }))
}

/// `JamRoom` as the caller sees it.
async fn room_json(
    state: &AppState,
    db: &(impl GenericClient + Sync),
    id: Uuid,
    caller: Uuid,
) -> ApiResult<Value> {
    let row = db
        .query_opt(
            "SELECT r.id, r.title, r.host, u.username, r.guests_can_control, r.version, r.created_at, r.playback
             FROM jam_rooms r JOIN users u ON u.id = r.host WHERE r.id = $1",
            &[&id],
        )
        .await?
        .ok_or_else(no_room)?;
    let members = db
        .query(
            "SELECT m.user_id, u.username, m.role FROM jam_members m JOIN users u ON u.id = m.user_id
             WHERE m.room_id = $1 ORDER BY (m.role = 'host') DESC, m.joined_at, m.user_id",
            &[&id],
        )
        .await?;
    let invited = db
        .query(
            "SELECT i.user_id, u.username FROM jam_invites i JOIN users u ON u.id = i.user_id
             WHERE i.room_id = $1 ORDER BY i.created_at, i.user_id",
            &[&id],
        )
        .await?;
    let mut role = Value::Null;
    let members: Vec<Value> = members
        .iter()
        .map(|m| {
            let user: Uuid = m.get(0);
            let r: String = m.get(2);
            if user == caller {
                role = json!(r);
            }
            json!({
                "user_id": user,
                "username": m.get::<_, String>(1),
                "role": r,
                "online": state.0.realtime.user_online(user),
            })
        })
        .collect();
    let invited: Vec<Value> = invited
        .iter()
        .map(|i| json!({ "user_id": i.get::<_, Uuid>(0), "username": i.get::<_, String>(1) }))
        .collect();
    let queue: Vec<Value> = load_queue(db, id, caller)
        .await?
        .iter()
        .map(QueueItem::json)
        .collect();
    Ok(json!({
        "id": id,
        "title": row.get::<_, String>("title"),
        "host": { "user_id": row.get::<_, Uuid>("host"), "username": row.get::<_, String>("username") },
        "guests_can_control": row.get::<_, bool>("guests_can_control"),
        "version": row.get::<_, i64>("version"),
        "created_at": row.get::<_, i64>("created_at"),
        "role": role,
        "members": members,
        "invited": invited,
        "queue": queue,
        "playback": row.get::<_, Value>("playback"),
        "server_ms": state.now(),
    }))
}

fn jam_title(title: Option<&str>, username: &str) -> ApiResult<String> {
    match title {
        Some(t) => {
            let t = t.trim();
            title_ok(t, MAX_JAM_TITLE).map_err(ApiError::bad)?;
            Ok(t.to_owned())
        }
        None => Ok(format!("{username}'s Jam")),
    }
}

// ---------------------------------------------------------------- endpoints

#[derive(Deserialize, Default)]
struct CreateBody {
    title: Option<String>,
    #[serde(default)]
    guests_can_control: bool,
}

async fn create(
    State(state): State<AppState>,
    device: Device,
    body: Option<Json<CreateBody>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let body = body.map(|b| b.0).unwrap_or_default();
    let title = jam_title(body.title.as_deref(), &device.username)?;
    let key = format!("jam-create:{}", device.user);
    state
        .attempt(
            &[(JAM_CREATE, &key)],
            "Too many Jams started, try again later",
        )
        .await?;
    expire(&state, Scope::User(device.user)).await?;
    let now = state.now();
    let id = Uuid::new_v4();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    let current = lock_user_rooms(&*tx, device.user, None).await?;
    let events = leave_current(&*tx, device.user, &current, now).await?;
    let playback =
        serde_json::to_value(Playback::stopped(now, 1.0)).map_err(anyhow::Error::from)?;
    tx.execute(
        "INSERT INTO jam_rooms (id, host, title, guests_can_control, playback, version, created_at, last_active)
         VALUES ($1, $2, $3, $4, $5, 1, $6, $6)",
        &[&id, &device.user, &title, &body.guests_can_control, &playback, &now],
    )
    .await?;
    tx.execute(
        "INSERT INTO jam_members (room_id, user_id, role, joined_at) VALUES ($1, $2, 'host', $3)",
        &[&id, &device.user, &now],
    )
    .await?;
    let room = room_json(&state, &*tx, id, device.user).await?;
    tx.commit().await?;
    publish(&state, events);
    Ok((StatusCode::CREATED, Json(room)))
}

async fn current(State(state): State<AppState>, device: Device) -> ApiResult {
    expire(&state, Scope::User(device.user)).await?;
    let now = state.now();
    let client = state.0.db.get().await?;
    let room = match active_rooms(&**client, device.user).await?.first() {
        Some(id) => room_json(&state, &**client, *id, device.user).await?,
        None => Value::Null,
    };
    let invites: Vec<Value> = client
        .query(
            "SELECT i.room_id, r.title, i.invited_by, u.username, i.created_at
             FROM jam_invites i JOIN jam_rooms r ON r.id = i.room_id JOIN users u ON u.id = i.invited_by
             WHERE i.user_id = $1 AND r.ended_at IS NULL AND r.created_at > $2 AND r.last_active > $3
             ORDER BY i.created_at DESC",
            &[&device.user, &(now - ROOM_MAX_AGE_MS), &(now - ROOM_IDLE_MS)],
        )
        .await?
        .iter()
        .map(|r| {
            json!({
                "room_id": r.get::<_, Uuid>(0),
                "title": r.get::<_, String>(1),
                "from": { "user_id": r.get::<_, Uuid>(2), "username": r.get::<_, String>(3) },
                "created_at": r.get::<_, i64>(4),
            })
        })
        .collect();
    Ok(Json(json!({ "current": room, "invites": invites })))
}

async fn room(State(state): State<AppState>, device: Device, Path(id): Path<Uuid>) -> ApiResult {
    expire(&state, Scope::Room(id)).await?;
    let client = state.0.db.get().await?;
    let active: bool = client
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM jam_rooms r JOIN jam_members m ON m.room_id = r.id
                            WHERE r.id = $1 AND m.user_id = $2 AND r.ended_at IS NULL)",
            &[&id, &device.user],
        )
        .await?
        .get(0);
    if !active {
        return Err(no_room());
    }
    Ok(Json(room_json(&state, &**client, id, device.user).await?))
}

#[derive(Deserialize)]
struct UpdateBody {
    title: Option<String>,
    guests_can_control: Option<bool>,
}

async fn update(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateBody>,
) -> ApiResult {
    edit_attempt(&state, device.user).await?;
    expire(&state, Scope::Room(id)).await?;
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    let (room, role) = member_room(&*tx, id, device.user, now).await?;
    if role != "host" {
        return Err(forbidden("Only the host can do this"));
    }
    if let Some(title) = &body.title {
        let title = jam_title(Some(title), &device.username)?;
        tx.execute(
            "UPDATE jam_rooms SET title = $2 WHERE id = $1",
            &[&id, &title],
        )
        .await?;
    }
    if let Some(control) = body.guests_can_control {
        tx.execute(
            "UPDATE jam_rooms SET guests_can_control = $2 WHERE id = $1",
            &[&id, &control],
        )
        .await?;
    }
    let version = bump(&*tx, room.id, now).await?;
    let events = to_all(
        &member_ids(&*tx, id).await?,
        &changed(id, version, &["settings"]),
    );
    let out = room_json(&state, &*tx, id, device.user).await?;
    tx.commit().await?;
    publish(&state, events);
    Ok(Json(out))
}

async fn end(State(state): State<AppState>, device: Device, Path(id): Path<Uuid>) -> ApiResult {
    expire(&state, Scope::Room(id)).await?;
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    let (_, role) = member_room(&*tx, id, device.user, now).await?;
    if role != "host" {
        return Err(forbidden("Only the host can do this"));
    }
    let events = end_room(&*tx, id, "ended", now).await?;
    tx.commit().await?;
    publish(&state, events);
    Ok(Json(json!({ "ended": true })))
}

#[derive(Deserialize)]
struct InviteBody {
    user_ids: Vec<Uuid>,
}

async fn invite(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<Uuid>,
    Json(body): Json<InviteBody>,
) -> ApiResult {
    if body.user_ids.len() > MAX_INVITE_BATCH {
        return Err(ApiError::bad(format!(
            "Invite at most {MAX_INVITE_BATCH} people at once"
        )));
    }
    edit_attempt(&state, device.user).await?;
    let key = format!("jam-invite:{}", device.user);
    state
        .attempt(&[(JAM_INVITE, &key)], "Too many invites, try again later")
        .await?;
    expire(&state, Scope::Room(id)).await?;
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    // Friendships first (share-locked), then the room: the same order as unfriending.
    let mut friends = Vec::new();
    for user in &body.user_ids {
        if !friends.contains(user) && crate::social::are_friends(&*tx, device.user, *user).await? {
            friends.push(*user);
        }
    }
    let (room, _) = member_room(&*tx, id, device.user, now).await?;
    let members = member_ids(&*tx, id).await?;
    friends.retain(|u| !members.contains(u));
    let open: Vec<Uuid> = tx
        .query("SELECT user_id FROM jam_invites WHERE room_id = $1", &[&id])
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect();
    let fresh = friends.iter().filter(|u| !open.contains(u)).count() as i64;
    if open.len() as i64 + fresh > MAX_JAM_INVITES {
        return Err(conflict(format!(
            "At most {MAX_JAM_INVITES} open invites per Jam"
        )));
    }
    // Someone with an open invite to this room is not invited (or notified) again.
    let fresh: Vec<Uuid> = friends
        .iter()
        .copied()
        .filter(|u| !open.contains(u))
        .collect();
    let mut events = Events::new();
    if !fresh.is_empty() {
        for user in &fresh {
            tx.execute(
                "INSERT INTO jam_invites (room_id, user_id, invited_by, created_at) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (room_id, user_id) DO NOTHING",
                &[&id, user, &device.user, &now],
            )
            .await?;
        }
        let title: String = tx
            .query_one("SELECT title FROM jam_rooms WHERE id = $1", &[&id])
            .await?
            .get(0);
        let event = json!({
            "type": "jam_invite",
            "room_id": id,
            "title": title,
            "from": { "user_id": device.user, "username": device.username },
        });
        events.extend(to_all(&fresh, &event));
        let version = bump(&*tx, room.id, now).await?;
        events.extend(to_all(&members, &changed(id, version, &["members"])));
    }
    tx.commit().await?;
    publish(&state, events);
    Ok(Json(json!({ "invited": friends })))
}

async fn join(State(state): State<AppState>, device: Device, Path(id): Path<Uuid>) -> ApiResult {
    expire(&state, Scope::Room(id)).await?;
    expire(&state, Scope::User(device.user)).await?;
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    let current = lock_user_rooms(&*tx, device.user, Some(id)).await?;
    let room = lock_room(&*tx, id, now).await?;
    if role(&*tx, id, device.user).await?.is_some() {
        let out = room_json(&state, &*tx, id, device.user).await?;
        tx.commit().await?;
        return Ok(Json(out));
    }
    let invited = tx
        .execute(
            "DELETE FROM jam_invites WHERE room_id = $1 AND user_id = $2",
            &[&id, &device.user],
        )
        .await?;
    if invited == 0 {
        return Err(no_room());
    }
    if member_ids(&*tx, id).await?.len() as i64 >= MAX_JAM_MEMBERS {
        return Err(conflict("This Jam is full"));
    }
    let mut events = leave_current(&*tx, device.user, &current, now).await?;
    tx.execute(
        "INSERT INTO jam_members (room_id, user_id, role, joined_at) VALUES ($1, $2, 'guest', $3)",
        &[&id, &device.user, &now],
    )
    .await?;
    let version = bump(&*tx, room.id, now).await?;
    events.extend(to_all(
        &member_ids(&*tx, id).await?,
        &changed(id, version, &["members"]),
    ));
    let out = room_json(&state, &*tx, id, device.user).await?;
    tx.commit().await?;
    publish(&state, events);
    Ok(Json(out))
}

async fn decline(State(state): State<AppState>, device: Device, Path(id): Path<Uuid>) -> ApiResult {
    edit_attempt(&state, device.user).await?;
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    let room = match lock_room(&*tx, id, now).await {
        Ok(room) => Some(room),
        // An ended room still lets the invitee drop the invite.
        Err(e) if e.0 == StatusCode::NOT_FOUND => None,
        Err(e) => return Err(e),
    };
    let declined = tx
        .execute(
            "DELETE FROM jam_invites WHERE room_id = $1 AND user_id = $2",
            &[&id, &device.user],
        )
        .await?;
    if declined == 0 {
        return Err(no_room());
    }
    let mut events = Events::new();
    if let Some(room) = room {
        let version = bump(&*tx, room.id, now).await?;
        events = to_all(
            &member_ids(&*tx, id).await?,
            &changed(id, version, &["members"]),
        );
    }
    tx.commit().await?;
    publish(&state, events);
    Ok(Json(json!({ "declined": true })))
}

async fn leave(State(state): State<AppState>, device: Device, Path(id): Path<Uuid>) -> ApiResult {
    expire(&state, Scope::Room(id)).await?;
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    let (_, role) = member_room(&*tx, id, device.user, now).await?;
    let events = if role == "host" {
        end_room(&*tx, id, "ended", now).await?
    } else {
        remove_member(&*tx, id, device.user, now)
            .await?
            .unwrap_or_default()
    };
    tx.commit().await?;
    publish(&state, events);
    Ok(Json(json!({ "left": true })))
}

async fn kick(
    State(state): State<AppState>,
    device: Device,
    Path((id, user)): Path<(Uuid, Uuid)>,
) -> ApiResult {
    expire(&state, Scope::Room(id)).await?;
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    let (_, role) = member_room(&*tx, id, device.user, now).await?;
    if role != "host" {
        return Err(forbidden("Only the host can do this"));
    }
    if user == device.user {
        return Err(ApiError::bad("Leave or end the Jam instead"));
    }
    let mut events = remove_member(&*tx, id, user, now)
        .await?
        .ok_or_else(|| not_found("No such member"))?;
    tx.commit().await?;
    events.push((
        user,
        json!({ "type": "jam_ended", "room_id": id, "reason": "removed" }),
    ));
    publish(&state, events);
    Ok(Json(json!({ "removed": true })))
}

#[derive(Deserialize)]
struct QueueBody {
    media: Value,
}

async fn queue_add(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<Uuid>,
    Json(body): Json<QueueBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let key = format!("jam-add:{}", device.user);
    state
        .attempt(&[(JAM_ADD, &key)], "Too many additions, try again later")
        .await?;
    let (media, _, _) = slim_online(&body.media).map_err(ApiError::bad)?;
    expire(&state, Scope::Room(id)).await?;
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    let (room, _) = member_room(&*tx, id, device.user, now).await?;
    let queued: i64 = tx
        .query_one("SELECT COUNT(*) FROM jam_queue WHERE room_id = $1", &[&id])
        .await?
        .get(0);
    if queued >= MAX_JAM_QUEUE {
        return Err(conflict("The queue is full"));
    }
    let item = QueueItem {
        item_id: Uuid::new_v4(),
        media,
        added_by: device.user,
        added_by_name: device.username.clone(),
        added_at: now,
        votes: 0,
        voted: false,
    };
    let version = bump(&*tx, room.id, now).await?;
    let members = member_ids(&*tx, id).await?;
    let mut events = to_all(&members, &changed(id, version, &["queue"]));
    if room.playback.item.is_none() {
        // Nothing is current: the new item starts right away.
        let mut playback = room.playback.start(Some(item.current()), now);
        playback.version = version;
        tx.execute(
            "UPDATE jam_rooms SET playback = $2 WHERE id = $1",
            &[
                &id,
                &serde_json::to_value(&playback).map_err(anyhow::Error::from)?,
            ],
        )
        .await?;
        events.extend(to_all(&members, &state_event(id, &playback, now)));
    } else {
        tx.execute(
            "INSERT INTO jam_queue (room_id, item_id, media, added_by, added_at) VALUES ($1, $2, $3, $4, $5)",
            &[&id, &item.item_id, &item.media, &device.user, &now],
        )
        .await?;
    }
    tx.commit().await?;
    publish(&state, events);
    Ok((StatusCode::CREATED, Json(item.json())))
}

async fn queue_remove(
    State(state): State<AppState>,
    device: Device,
    Path((id, item)): Path<(Uuid, Uuid)>,
) -> ApiResult {
    edit_attempt(&state, device.user).await?;
    expire(&state, Scope::Room(id)).await?;
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    let (room, role) = member_room(&*tx, id, device.user, now).await?;
    let added_by: Uuid = tx
        .query_opt(
            "SELECT added_by FROM jam_queue WHERE room_id = $1 AND item_id = $2",
            &[&id, &item],
        )
        .await?
        .map(|r| r.get(0))
        .ok_or_else(|| not_found("No such queue item"))?;
    if role != "host" && added_by != device.user {
        return Err(forbidden("Only the host or who added it can remove it"));
    }
    tx.execute(
        "DELETE FROM jam_queue WHERE room_id = $1 AND item_id = $2",
        &[&id, &item],
    )
    .await?;
    let version = bump(&*tx, room.id, now).await?;
    let events = to_all(
        &member_ids(&*tx, id).await?,
        &changed(id, version, &["queue"]),
    );
    tx.commit().await?;
    publish(&state, events);
    Ok(Json(json!({ "removed": true })))
}

#[derive(Deserialize)]
struct VoteBody {
    up: bool,
}

async fn vote(
    State(state): State<AppState>,
    device: Device,
    Path((id, item)): Path<(Uuid, Uuid)>,
    Json(body): Json<VoteBody>,
) -> ApiResult {
    edit_attempt(&state, device.user).await?;
    expire(&state, Scope::Room(id)).await?;
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    let (room, _) = member_room(&*tx, id, device.user, now).await?;
    let exists = tx
        .query_opt(
            "SELECT 1 FROM jam_queue WHERE room_id = $1 AND item_id = $2",
            &[&id, &item],
        )
        .await?
        .is_some();
    if !exists {
        return Err(not_found("No such queue item"));
    }
    let changed_rows = if body.up {
        tx.execute(
            "INSERT INTO jam_votes (room_id, item_id, user_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
            &[&id, &item, &device.user],
        )
        .await?
    } else {
        tx.execute(
            "DELETE FROM jam_votes WHERE room_id = $1 AND item_id = $2 AND user_id = $3",
            &[&id, &item, &device.user],
        )
        .await?
    };
    let votes: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM jam_votes WHERE room_id = $1 AND item_id = $2",
            &[&id, &item],
        )
        .await?
        .get(0);
    let mut events = Events::new();
    if changed_rows > 0 {
        let version = bump(&*tx, room.id, now).await?;
        events = to_all(
            &member_ids(&*tx, id).await?,
            &changed(id, version, &["queue"]),
        );
    }
    tx.commit().await?;
    publish(&state, events);
    Ok(Json(json!({ "votes": votes, "voted": body.up })))
}

async fn playback(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<Uuid>,
    Json(body): Json<ActionBody>,
) -> ApiResult<Json<Playback>> {
    let action = Action::parse(&body).map_err(ApiError::bad)?;
    let key = format!("jam-control:{}", device.user);
    state
        .attempt(
            &[(JAM_CONTROL, &key)],
            "Too many playback changes, slow down",
        )
        .await?;
    expire(&state, Scope::Room(id)).await?;
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    let (room, role) = member_room(&*tx, id, device.user, now).await?;
    let ended = matches!(action, Action::Ended(_));
    let controls = role == "host" || room.guests_can_control;
    if !controls && !ended {
        return Err(forbidden("Only the host can control playback"));
    }
    if !controls && ended && !room.playback.ended_plausible(now) {
        // A guest without control cannot use `ended` to skip: the item is not near its
        // end yet, so the current playback stays.
        tx.commit().await?;
        return Ok(Json(room.playback));
    }
    let pb = &room.playback;
    let bad = ApiError::bad;
    let nothing = || conflict("Nothing is playing");
    let mut queue_changed = false;
    let next = match action {
        Action::Ended(item) if pb.item.as_ref().map(|i| i.item_id) != Some(item) => {
            // Another member's device already advanced: nothing to do.
            tx.commit().await?;
            return Ok(Json(room.playback));
        }
        Action::Ended(_) | Action::Next => {
            queue_changed = true;
            pb.start(take_top(&*tx, id).await?, now)
        }
        Action::PlayItem(item) => {
            let item = take_item(&*tx, id, item)
                .await?
                .ok_or_else(|| not_found("No such queue item"))?;
            queue_changed = true;
            pb.start(Some(item), now)
        }
        Action::Play(position) if pb.item.is_none() => {
            if position.is_some() {
                return Err(nothing());
            }
            let top = take_top(&*tx, id)
                .await?
                .ok_or_else(|| conflict("The queue is empty"))?;
            queue_changed = true;
            pb.start(Some(top), now)
        }
        Action::Play(position) => pb.play(position, now).map_err(bad)?,
        _ if pb.item.is_none() => return Err(nothing()),
        Action::Pause(position) => pb.pause(position, now).map_err(bad)?,
        Action::Seek(position) => pb.seek(position, now).map_err(bad)?,
        Action::Rate(rate) => pb.with_rate(rate, now).map_err(bad)?,
    };
    let mut next = next;
    next.version = room.version + 1;
    tx.execute(
        "UPDATE jam_rooms SET playback = $2, version = $3, last_active = $4 WHERE id = $1",
        &[
            &id,
            &serde_json::to_value(&next).map_err(anyhow::Error::from)?,
            &next.version,
            &now,
        ],
    )
    .await?;
    let members = member_ids(&*tx, id).await?;
    tx.commit().await?;
    let mut events = to_all(&members, &state_event(id, &next, now));
    if queue_changed {
        events.extend(to_all(&members, &changed(id, next.version, &["queue"])));
    }
    publish(&state, events);
    Ok(Json(next))
}

// ---------------------------------------------------------------- other modules

/// After an accepted friendship ends: each leaves the room the other hosts, and open
/// invites between them are withdrawn. Caller holds the friendship lock.
pub(crate) async fn unfriend(
    db: &(impl GenericClient + Sync),
    a: Uuid,
    b: Uuid,
    now: i64,
) -> Result<Events, tokio_postgres::Error> {
    let mut events = Events::new();
    let rooms = db
        .query(
            "SELECT r.id, m.user_id FROM jam_rooms r JOIN jam_members m ON m.room_id = r.id
             WHERE r.ended_at IS NULL AND m.role = 'guest'
               AND ((r.host = $1 AND m.user_id = $2) OR (r.host = $2 AND m.user_id = $1))
             ORDER BY r.id",
            &[&a, &b],
        )
        .await?;
    for row in rooms {
        let (room, user): (Uuid, Uuid) = (row.get(0), row.get(1));
        db.execute("SELECT 1 FROM jam_rooms WHERE id = $1 FOR UPDATE", &[&room])
            .await?;
        if let Some(more) = remove_member(db, room, user, now).await? {
            events.extend(more);
            events.push((
                user,
                json!({ "type": "jam_ended", "room_id": room, "reason": "removed" }),
            ));
        }
    }
    let withdrawn = db
        .query(
            "DELETE FROM jam_invites
             WHERE (user_id = $1 AND invited_by = $2) OR (user_id = $2 AND invited_by = $1)
             RETURNING room_id",
            &[&a, &b],
        )
        .await?;
    for row in withdrawn {
        let room: Uuid = row.get(0);
        let version = bump(db, room, now).await?;
        events.extend(to_all(
            &member_ids(db, room).await?,
            &changed(room, version, &["members"]),
        ));
    }
    Ok(events)
}

/// `jam_ping` from a member's socket: keeps the room alive (at most one write per room
/// per `JAM_PING_WRITE_MS`).
pub(crate) async fn ping(state: &AppState, user: Uuid, room: Uuid) -> Result<(), WsError> {
    let internal = |e: &dyn std::fmt::Display| {
        tracing::warn!("jam ping: {e}");
        WsError::new(crate::presence::INVALID, "try again later")
    };
    let now = state.now();
    let client = state.0.db.get().await.map_err(|e| internal(&e))?;
    let member = client
        .query_opt(
            "SELECT 1 FROM jam_members m JOIN jam_rooms r ON r.id = m.room_id
             WHERE m.room_id = $1 AND m.user_id = $2 AND r.ended_at IS NULL",
            &[&room, &user],
        )
        .await
        .map_err(|e| internal(&e))?
        .is_some();
    if !member {
        return Err(WsError::new(NOT_FOUND, "Not in this Jam"));
    }
    // Writes (expiry and the keep-alive) only when due: at most once per room per
    // `JAM_PING_WRITE_MS`, whatever the message rate.
    if state.0.realtime.jam_ping_due(room, now) {
        drop(client);
        expire(state, Scope::Room(room))
            .await
            .map_err(|e| internal(&e.1))?;
        let client = state.0.db.get().await.map_err(|e| internal(&e))?;
        let alive = client
            .execute(
                "UPDATE jam_rooms SET last_active = GREATEST(last_active, $2) WHERE id = $1 AND ended_at IS NULL",
                &[&room, &now],
            )
            .await
            .map_err(|e| internal(&e))?;
        if alive == 0 {
            // It just expired (and its members were told).
            return Err(WsError::new(NOT_FOUND, "Not in this Jam"));
        }
    }
    Ok(())
}

pub async fn maintenance(db: &deadpool_postgres::Pool, now: i64) -> anyhow::Result<()> {
    let client = db.get().await?;
    client
        .execute(
            "UPDATE jam_rooms SET ended_at = $1, version = version + 1
             WHERE ended_at IS NULL AND (created_at <= $1 - $2::BIGINT OR last_active <= $1 - $3::BIGINT)",
            &[&now, &ROOM_MAX_AGE_MS, &ROOM_IDLE_MS],
        )
        .await?;
    client
        .execute(
            "DELETE FROM jam_invites i USING jam_rooms r WHERE i.room_id = r.id AND r.ended_at IS NOT NULL",
            &[],
        )
        .await?;
    client
        .execute(
            "DELETE FROM jam_rooms WHERE ended_at < $1",
            &[&(now - ENDED_RETENTION_MS)],
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(duration: Option<u64>) -> Current {
        Current {
            item_id: Uuid::from_u128(1),
            media: json!({"url": "https://youtu.be/abc", "duration": duration}),
            media_key: "youtube:abc".into(),
            added_by: Uuid::from_u128(2),
        }
    }

    fn playing(at: i64, position: i64, rate: f64) -> Playback {
        Playback {
            item: Some(item(Some(300))),
            state: PLAYING.into(),
            position_ms: position,
            at_server_ms: at,
            rate,
            version: 3,
        }
    }

    #[test]
    fn ended_needs_the_item_near_its_end() {
        // 300 s item playing from 0 at t=0.
        let pb = playing(0, 0, 1.0);
        assert!(!pb.ended_plausible(1_000));
        assert!(!pb.ended_plausible(294_000));
        assert!(pb.ended_plausible(295_000));
        assert!(pb.ended_plausible(400_000));
        // Paused early never counts as ended.
        let paused = pb.pause(Some(10_000), 0).unwrap();
        assert!(!paused.ended_plausible(1_000_000));
        // Unknown duration: at least 30 s played.
        let mut unknown = playing(0, 0, 1.0);
        unknown.item = Some(item(None));
        assert!(!unknown.ended_plausible(29_000));
        assert!(unknown.ended_plausible(30_000));
    }

    #[test]
    fn expected_position_follows_the_server_clock() {
        let pb = playing(1_000, 51_000, 1.0);
        assert_eq!(pb.expected(1_000), 51_000);
        assert_eq!(pb.expected(3_500), 53_500);
        assert_eq!(playing(1_000, 0, 2.0).expected(2_000), 2_000);
        assert_eq!(playing(1_000, 0, 0.5).expected(2_001), 501);
        // Clamped to the media length and never negative.
        assert_eq!(pb.expected(10_000_000), 300_000);
        assert_eq!(pb.expected(0), 51_000);
        let paused = pb.pause(None, 2_000).unwrap();
        assert_eq!(
            (paused.state.as_str(), paused.position_ms),
            (PAUSED, 52_000)
        );
        assert_eq!(paused.expected(99_000), 52_000);
    }

    #[test]
    fn actions_reanchor_at_the_server_time() {
        let pb = playing(1_000, 10_000, 1.0);
        let played = pb.play(Some(20_000), 5_000).unwrap();
        assert_eq!((played.position_ms, played.at_server_ms), (20_000, 5_000));
        let seeked = pb.pause(None, 2_000).unwrap().seek(40_000, 9_000).unwrap();
        assert_eq!(
            (seeked.state.as_str(), seeked.position_ms),
            (PAUSED, 40_000)
        );
        let faster = pb.with_rate(2.0, 3_000).unwrap();
        assert_eq!(
            (faster.position_ms, faster.at_server_ms, faster.rate),
            (12_000, 3_000, 2.0)
        );
        assert_eq!(faster.expected(4_000), 14_000);
        assert!(pb.with_rate(2.5, 3_000).is_err());
        assert!(pb.with_rate(0.25, 3_000).is_err());
        assert!(pb.seek(300_001, 3_000).is_err(), "past the end");
        assert!(pb.seek(-1, 3_000).is_err());
        let unknown = Playback {
            item: Some(item(None)),
            ..pb.clone()
        };
        assert!(unknown.seek(MAX_JAM_POSITION_MS, 0).is_ok());
        assert!(unknown.seek(MAX_JAM_POSITION_MS + 1, 0).is_err());
        let started = pb.with_rate(1.5, 0).unwrap().start(Some(item(None)), 7_000);
        assert_eq!(
            (started.state.as_str(), started.position_ms, started.rate),
            (PLAYING, 0, 1.5)
        );
        let stopped = pb.start(None, 8_000);
        assert_eq!(
            (stopped.state.as_str(), stopped.item.is_none()),
            (STOPPED, true)
        );
    }

    #[test]
    fn queue_orders_by_votes_then_age_then_id() {
        let q = |id: u128, votes: i64, at: i64| QueueItem {
            item_id: Uuid::from_u128(id),
            media: Value::Null,
            added_by: Uuid::nil(),
            added_by_name: String::new(),
            added_at: at,
            votes,
            voted: false,
        };
        let mut items = [
            q(1, 0, 10),
            q(2, 2, 30),
            q(3, 0, 5),
            q(4, 2, 20),
            q(6, 0, 5),
            q(5, 0, 5),
        ];
        items.sort_by(queue_order);
        let order: Vec<u128> = items.iter().map(|i| i.item_id.as_u128()).collect();
        assert_eq!(order, vec![4, 2, 3, 5, 6, 1]);
    }

    #[test]
    fn actions_parse_with_required_fields() {
        let parse = |v: Value| Action::parse(&serde_json::from_value(v).unwrap());
        let id = Uuid::new_v4();
        assert_eq!(parse(json!({"action": "play"})), Ok(Action::Play(None)));
        assert_eq!(
            parse(json!({"action": "seek", "position_ms": 5})),
            Ok(Action::Seek(5))
        );
        assert_eq!(
            parse(json!({"action": "ended", "item_id": id})),
            Ok(Action::Ended(id))
        );
        assert_eq!(
            parse(json!({"action": "rate", "rate": 1.25})),
            Ok(Action::Rate(1.25))
        );
        assert!(parse(json!({"action": "seek"})).is_err());
        assert!(parse(json!({"action": "play_item"})).is_err());
        assert!(parse(json!({"action": "rate"})).is_err());
        assert!(parse(json!({"action": "rewind"})).is_err());
    }

    #[test]
    fn playback_matches_the_contract_shape() {
        let pb = playing(1_763_900_000_000, 51_000, 1.0);
        let v = serde_json::to_value(&pb).unwrap();
        for field in [
            "item",
            "state",
            "position_ms",
            "at_server_ms",
            "rate",
            "version",
        ] {
            assert!(v.get(field).is_some(), "{field}");
        }
        for field in ["item_id", "media", "media_key", "added_by"] {
            assert!(v["item"].get(field).is_some(), "{field}");
        }
        assert_eq!(
            serde_json::to_value(Playback::stopped(5, 1.0)).unwrap()["item"],
            Value::Null
        );
    }
}
