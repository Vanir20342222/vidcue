//! Friends, opt-in activity sharing, send-to-a-friend inbox and shared playlists.
//!
//! "Friend" always means an accepted friendship. Every lookup of another user's data
//! goes through a friendship or membership check, and objects a caller may not see are
//! answered with 404 so their ids cannot be probed.

use crate::api::{ApiError, ApiResult, AppState, Device};
use crate::auth;
use crate::sync;
use crate::throttle::Rule;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_postgres::GenericClient;
use uuid::Uuid;
use vidcue_core::playback_history::PlaybackHistory;

const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

pub const MAX_FRIENDS: i64 = 100;
pub const MAX_OUTGOING_REQUESTS: i64 = 50;
/// Friend requests per user, recorded on every call (also bounds username probing).
pub const FRIEND_REQUEST: Rule = Rule {
    limit: 20,
    window_ms: HOUR,
};
/// Inbox sends per sender.
pub const INBOX_SEND: Rule = Rule {
    limit: 60,
    window_ms: HOUR,
};
pub const MAX_INBOX: i64 = 200;
pub const MAX_NOTE_CHARS: usize = 200;
pub const INBOX_RETENTION_MS: i64 = 90 * DAY;
/// `FriendActivity.recent`: this many history events from this window.
pub const MAX_RECENT: i64 = 20;
pub const RECENT_WINDOW_MS: i64 = 7 * DAY;
pub const MAX_SHARED_PER_USER: i64 = 50;
pub const MAX_SHARED_MEMBERS: i64 = 20;
pub const MAX_SHARED_OPS: usize = 200;
pub const MAX_SHARED_ENTRIES: i64 = 2000;
pub const MAX_SHARED_PULL: i64 = 1000;
/// Op ids stay known (for idempotent retries) this long, like `sync_applied`.
pub const SHARED_APPLIED_RETENTION_MS: i64 = 30 * DAY;
pub const SHARE_LEVELS: &[&str] = &["off", "now_playing", "history"];

/// Events to publish after the transaction that caused them committed.
pub(crate) type Events = Vec<(Uuid, Value)>;

pub(crate) fn publish(state: &AppState, events: Events) {
    for (user, event) in events {
        state.0.hub.publish(user, event);
    }
}

pub(crate) fn not_found(message: &str) -> ApiError {
    ApiError(StatusCode::NOT_FOUND, message.into())
}

pub(crate) fn conflict(message: impl Into<String>) -> ApiError {
    ApiError(StatusCode::CONFLICT, message.into())
}

pub(crate) fn forbidden(message: &str) -> ApiError {
    ApiError(StatusCode::FORBIDDEN, message.into())
}

/// Online media in its stored (slim) form plus its `PlaybackHistory::key`. Error
/// messages are fixed so they never echo request content.
pub(crate) fn slim_online(media: &Value) -> Result<(Value, String, vidcue_core::Media), String> {
    let media = sync::online_media(media).map_err(|_| "media must be online media".to_owned())?;
    if has_nul(&serde_json::to_value(&media).unwrap_or_default()) {
        return Err("NUL characters are not allowed".into());
    }
    let slim = sync::slimmed(&media).map_err(|_| "media must be online media".to_owned())?;
    if slim.to_string().len() > sync::MAX_VALUE_BYTES {
        return Err(format!("media exceeds {} bytes", sync::MAX_VALUE_BYTES));
    }
    Ok((slim, PlaybackHistory::key(&media), media))
}

/// PostgreSQL JSONB refuses `\u0000`, so it is rejected before it reaches a statement.
pub(crate) fn has_nul(value: &Value) -> bool {
    match value {
        Value::String(s) => s.contains('\0'),
        Value::Array(items) => items.iter().any(has_nul),
        Value::Object(map) => map.iter().any(|(k, v)| k.contains('\0') || has_nul(v)),
        _ => false,
    }
}

/// Same rule as synced playlist titles.
pub(crate) fn title_ok(title: &str, max_chars: usize) -> Result<(), String> {
    if title.trim().is_empty()
        || title.chars().count() > max_chars
        || title.chars().any(char::is_control)
    {
        return Err(format!(
            "title must have 1 to {max_chars} printable characters"
        ));
    }
    Ok(())
}

/// The friendship row key: user_a < user_b in UUID byte order (PostgreSQL agrees).
pub(crate) fn pair(a: Uuid, b: Uuid) -> (Uuid, Uuid) {
    if a < b { (a, b) } else { (b, a) }
}

/// Whether `a` and `b` are friends. Inside a transaction the row stays share-locked,
/// so a concurrent removal waits and then also undoes what this transaction adds.
pub(crate) async fn are_friends(
    db: &(impl GenericClient + Sync),
    a: Uuid,
    b: Uuid,
) -> Result<bool, tokio_postgres::Error> {
    if a == b {
        return Ok(false);
    }
    let (a, b) = pair(a, b);
    Ok(db
        .query_opt(
            "SELECT 1 FROM friendships WHERE user_a = $1 AND user_b = $2 AND accepted_at IS NOT NULL FOR SHARE",
            &[&a, &b],
        )
        .await?
        .is_some())
}

pub(crate) async fn friends_of(
    db: &(impl GenericClient + Sync),
    user: Uuid,
) -> Result<Vec<Uuid>, tokio_postgres::Error> {
    Ok(db
        .query(
            "SELECT CASE WHEN user_a = $1 THEN user_b ELSE user_a END FROM friendships
             WHERE (user_a = $1 OR user_b = $1) AND accepted_at IS NOT NULL",
            &[&user],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect())
}

async fn friend_count(
    db: &(impl GenericClient + Sync),
    user: Uuid,
) -> Result<i64, tokio_postgres::Error> {
    Ok(db
        .query_one(
            "SELECT COUNT(*) FROM friendships WHERE (user_a = $1 OR user_b = $1) AND accepted_at IS NOT NULL",
            &[&user],
        )
        .await?
        .get(0))
}

async fn lock_key(
    db: &(impl GenericClient + Sync),
    key: &str,
    seed: i64,
) -> Result<(), tokio_postgres::Error> {
    db.execute(
        "SELECT pg_advisory_xact_lock(hashtextextended($1::text, $2))",
        &[&key, &seed],
    )
    .await
    .map(|_| ())
}

pub(crate) async fn lock_playlist(
    db: &(impl GenericClient + Sync),
    playlist: Uuid,
) -> Result<(), tokio_postgres::Error> {
    lock_key(db, &playlist.to_string(), 1).await
}

async fn lock_pair(
    db: &(impl GenericClient + Sync),
    a: Uuid,
    b: Uuid,
) -> Result<(), tokio_postgres::Error> {
    let (a, b) = pair(a, b);
    lock_key(db, &format!("{a}/{b}"), 4).await
}

async fn username(
    db: &(impl GenericClient + Sync),
    user: Uuid,
) -> Result<String, tokio_postgres::Error> {
    Ok(db
        .query_opt("SELECT username FROM users WHERE id = $1", &[&user])
        .await?
        .map(|r| r.get(0))
        .unwrap_or_default())
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/friends", get(friends))
        .route("/api/friends/requests", post(friend_request))
        .route("/api/friends/activity", get(activity))
        .route("/api/friends/{user_id}/accept", post(friend_accept))
        .route("/api/friends/{user_id}", delete(friend_remove))
        .route("/api/social/settings", get(settings).put(put_settings))
        .route("/api/inbox", get(inbox).post(inbox_send))
        .route("/api/inbox/{id}/read", post(inbox_read))
        .route("/api/inbox/{id}", delete(inbox_delete))
        .route(
            "/api/shared-playlists",
            get(shared_list).post(shared_create),
        )
        .route(
            "/api/shared-playlists/{id}",
            axum::routing::patch(shared_rename).delete(shared_delete),
        )
        .route(
            "/api/shared-playlists/{id}/members",
            post(shared_add_member),
        )
        .route(
            "/api/shared-playlists/{id}/members/{user_id}",
            delete(shared_remove_member),
        )
        .route("/api/shared-playlists/{id}/push", post(shared_push))
        .route("/api/shared-playlists/{id}/pull", get(shared_pull))
}

// ---------------------------------------------------------------- friends

async fn friends(State(state): State<AppState>, device: Device) -> ApiResult {
    let rows = state
        .0
        .db
        .get()
        .await?
        .query(
            "SELECT u.id, u.username, f.requested_by, f.accepted_at, f.created_at
             FROM friendships f
             JOIN users u ON u.id = CASE WHEN f.user_a = $1 THEN f.user_b ELSE f.user_a END
             WHERE f.user_a = $1 OR f.user_b = $1
             ORDER BY u.username",
            &[&device.user],
        )
        .await?;
    let (mut friends, mut incoming, mut outgoing) = (vec![], vec![], vec![]);
    for r in &rows {
        let (id, name): (Uuid, String) = (r.get("id"), r.get("username"));
        match r.get::<_, Option<i64>>("accepted_at") {
            Some(since) => friends.push(json!({ "user_id": id, "username": name, "since": since })),
            None => {
                let item =
                    json!({ "user_id": id, "username": name, "at": r.get::<_, i64>("created_at") });
                if r.get::<_, Uuid>("requested_by") == device.user {
                    outgoing.push(item);
                } else {
                    incoming.push(item);
                }
            }
        }
    }
    Ok(Json(
        json!({ "friends": friends, "incoming": incoming, "outgoing": outgoing }),
    ))
}

#[derive(Deserialize)]
struct UsernameBody {
    username: String,
}

async fn friend_request(
    State(state): State<AppState>,
    device: Device,
    Json(body): Json<UsernameBody>,
) -> ApiResult {
    let key = format!("friend-request:{}", device.user);
    state
        .attempt(
            &[(FRIEND_REQUEST, &key)],
            "Too many friend requests, try again later",
        )
        .await?;
    let no_user = || not_found("No such user");
    let name = auth::normalize_username(&body.username).map_err(|_| no_user())?;
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    let other: Uuid = tx
        .query_opt(
            "SELECT id FROM users WHERE username = $1 AND active",
            &[&name],
        )
        .await?
        .map(|r| r.get(0))
        .filter(|id| *id != device.user)
        .ok_or_else(no_user)?;
    let (a, b) = pair(device.user, other);
    lock_pair(&*tx, a, b).await?;
    let existing = tx
        .query_opt(
            "SELECT requested_by, accepted_at FROM friendships WHERE user_a = $1 AND user_b = $2",
            &[&a, &b],
        )
        .await?;
    let status = match existing {
        Some(r) if r.get::<_, Option<i64>>("accepted_at").is_some() => "accepted",
        Some(r) if r.get::<_, Uuid>("requested_by") == device.user => "requested",
        Some(_) => {
            // They had already asked: this request accepts theirs.
            friend_caps(&*tx, device.user, other).await?;
            tx.execute(
                "UPDATE friendships SET accepted_at = $3 WHERE user_a = $1 AND user_b = $2",
                &[&a, &b, &now],
            )
            .await?;
            tx.commit().await?;
            publish(&state, added_events(&device, other, &name));
            return Ok(Json(json!({ "status": "accepted" })));
        }
        None => {
            if friend_count(&*tx, device.user).await? >= MAX_FRIENDS {
                return Err(conflict(format!("At most {MAX_FRIENDS} friends")));
            }
            let outgoing: i64 = tx
                .query_one(
                    "SELECT COUNT(*) FROM friendships WHERE requested_by = $1 AND accepted_at IS NULL",
                    &[&device.user],
                )
                .await?
                .get(0);
            if outgoing >= MAX_OUTGOING_REQUESTS {
                return Err(conflict(format!(
                    "At most {MAX_OUTGOING_REQUESTS} open friend requests"
                )));
            }
            tx.execute(
                "INSERT INTO friendships (user_a, user_b, requested_by, created_at) VALUES ($1, $2, $3, $4)",
                &[&a, &b, &device.user, &now],
            )
            .await?;
            tx.commit().await?;
            state.0.hub.publish(
                other,
                json!({ "type": "friend_request", "from": { "user_id": device.user, "username": device.username } }),
            );
            "requested"
        }
    };
    Ok(Json(json!({ "status": status })))
}

async fn friend_caps(db: &(impl GenericClient + Sync), me: Uuid, other: Uuid) -> ApiResult<()> {
    if friend_count(db, me).await? >= MAX_FRIENDS || friend_count(db, other).await? >= MAX_FRIENDS {
        return Err(conflict(format!("At most {MAX_FRIENDS} friends")));
    }
    Ok(())
}

fn added_events(device: &Device, other: Uuid, other_name: &str) -> Events {
    vec![
        (
            device.user,
            json!({ "type": "friend_added", "user_id": other, "username": other_name }),
        ),
        (
            other,
            json!({ "type": "friend_added", "user_id": device.user, "username": device.username }),
        ),
    ]
}

async fn friend_accept(
    State(state): State<AppState>,
    device: Device,
    Path(other): Path<Uuid>,
) -> ApiResult {
    let now = state.now();
    let (a, b) = pair(device.user, other);
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    lock_pair(&*tx, a, b).await?;
    let pending = tx
        .query_opt(
            "SELECT 1 FROM friendships WHERE user_a = $1 AND user_b = $2 AND accepted_at IS NULL AND requested_by = $3",
            &[&a, &b, &other],
        )
        .await?
        .is_some();
    if !pending || other == device.user {
        return Err(not_found("No such friend request"));
    }
    friend_caps(&*tx, device.user, other).await?;
    tx.execute(
        "UPDATE friendships SET accepted_at = $3 WHERE user_a = $1 AND user_b = $2",
        &[&a, &b, &now],
    )
    .await?;
    let name = username(&*tx, other).await?;
    tx.commit().await?;
    publish(&state, added_events(&device, other, &name));
    Ok(Json(json!({ "status": "accepted" })))
}

async fn friend_remove(
    State(state): State<AppState>,
    device: Device,
    Path(other): Path<Uuid>,
) -> ApiResult {
    let now = state.now();
    let (a, b) = pair(device.user, other);
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    lock_pair(&*tx, a, b).await?;
    let removed = tx
        .query_opt(
            "DELETE FROM friendships WHERE user_a = $1 AND user_b = $2 RETURNING accepted_at",
            &[&a, &b],
        )
        .await?
        .ok_or_else(|| not_found("No such friend"))?;
    let mut events: Events = Vec::new();
    if removed.get::<_, Option<i64>>(0).is_some() {
        // Neither keeps access to the other's shared playlists or Jam room. The playlist
        // locks come first (in id order), then the member rows: the same order as
        // deleting a playlist or removing a member, so the two cannot deadlock.
        let affected: Vec<Uuid> = tx
            .query(
                "SELECT DISTINCT m.playlist_id FROM shared_playlist_members m
                 JOIN shared_playlists p ON p.id = m.playlist_id
                 WHERE (p.owner = $1 AND m.user_id = $2) OR (p.owner = $2 AND m.user_id = $1)
                 ORDER BY m.playlist_id",
                &[&device.user, &other],
            )
            .await?
            .iter()
            .map(|r| r.get(0))
            .collect();
        for playlist in &affected {
            lock_playlist(&*tx, *playlist).await?;
        }
        let dropped = tx
            .query(
                "DELETE FROM shared_playlist_members m USING shared_playlists p
                 WHERE m.playlist_id = p.id AND m.playlist_id = ANY($3)
                   AND ((p.owner = $1 AND m.user_id = $2) OR (p.owner = $2 AND m.user_id = $1))
                 RETURNING m.playlist_id, m.user_id",
                &[&device.user, &other, &affected],
            )
            .await?;
        for row in dropped {
            let (playlist, user): (Uuid, Uuid) = (row.get(0), row.get(1));
            events.push((
                user,
                json!({ "type": "shared_playlist_removed", "playlist_id": playlist }),
            ));
            events.extend(bump_playlist(&*tx, playlist, now, device.device).await?);
        }
        events.extend(crate::jam::unfriend(&*tx, device.user, other, now).await?);
    }
    tx.commit().await?;
    events.push((
        device.user,
        json!({ "type": "friend_removed", "user_id": other }),
    ));
    events.push((
        other,
        json!({ "type": "friend_removed", "user_id": device.user }),
    ));
    publish(&state, events);
    Ok(Json(json!({ "removed": true })))
}

// ---------------------------------------------------------------- activity

async fn share_level(
    db: &(impl GenericClient + Sync),
    user: Uuid,
) -> Result<String, tokio_postgres::Error> {
    Ok(db
        .query_opt(
            "SELECT share_activity FROM social_settings WHERE user_id = $1",
            &[&user],
        )
        .await?
        .map_or_else(|| "off".to_owned(), |r| r.get(0)))
}

async fn settings(State(state): State<AppState>, device: Device) -> ApiResult {
    let level = share_level(&**state.0.db.get().await?, device.user).await?;
    Ok(Json(json!({ "share_activity": level })))
}

#[derive(Deserialize)]
struct SettingsBody {
    share_activity: String,
}

async fn put_settings(
    State(state): State<AppState>,
    device: Device,
    Json(body): Json<SettingsBody>,
) -> ApiResult {
    if !SHARE_LEVELS.contains(&body.share_activity.as_str()) {
        return Err(ApiError::bad(
            "share_activity must be off, now_playing or history",
        ));
    }
    let client = state.0.db.get().await?;
    let previous = share_level(&**client, device.user).await?;
    client
        .execute(
            "INSERT INTO social_settings (user_id, share_activity, updated_at) VALUES ($1, $2, $3)
             ON CONFLICT (user_id) DO UPDATE SET share_activity = EXCLUDED.share_activity, updated_at = EXCLUDED.updated_at",
            &[&device.user, &body.share_activity, &state.now()],
        )
        .await?;
    if (previous == "off") != (body.share_activity == "off") {
        // Friends learn at once that sharing started or stopped.
        let now_playing = if body.share_activity == "off" {
            Value::Null
        } else {
            state.0.realtime.now_playing(device.user)
        };
        for friend in friends_of(&**client, device.user).await? {
            state.0.hub.publish(
                friend,
                json!({ "type": "friend_activity", "user_id": device.user, "now_playing": now_playing }),
            );
        }
    }
    Ok(Json(json!({ "share_activity": body.share_activity })))
}

/// Sends a sharing user's now-playing to their friends (called by `presence`).
pub(crate) async fn publish_activity(
    state: &AppState,
    user: Uuid,
    now_playing: Value,
) -> anyhow::Result<()> {
    let client = state.0.db.get().await?;
    if share_level(&**client, user).await? == "off" {
        return Ok(());
    }
    for friend in friends_of(&**client, user).await? {
        state.0.hub.publish(
            friend,
            json!({ "type": "friend_activity", "user_id": user, "now_playing": now_playing }),
        );
    }
    Ok(())
}

/// One `FriendActivity.recent` entry: only the six listed fields, cleaned.
pub(crate) fn recent_item(event: &Value) -> Option<Value> {
    let text = |field: &str| {
        vidcue_core::clean_text(event.get(field).and_then(Value::as_str).unwrap_or_default())
    };
    let url = event.get("url").and_then(Value::as_str)?;
    vidcue_core::http_url(url).ok()?;
    let at = event.get("at").and_then(Value::as_f64)?;
    let thumbnail = event
        .get("thumbnail")
        .and_then(Value::as_str)
        .filter(|t| vidcue_core::http_url(t).is_ok())
        .unwrap_or_default();
    Some(json!({
        "title": text("title"),
        "creator": text("creator"),
        "url": url,
        "kind": text("kind"),
        "thumbnail": thumbnail,
        "at": at as i64,
    }))
}

async fn activity(State(state): State<AppState>, device: Device) -> ApiResult {
    let client = state.0.db.get().await?;
    let rows = client
        .query(
            "SELECT u.id, u.username, s.share_activity FROM friendships f
             JOIN users u ON u.id = CASE WHEN f.user_a = $1 THEN f.user_b ELSE f.user_a END
             JOIN social_settings s ON s.user_id = u.id
             WHERE (f.user_a = $1 OR f.user_b = $1) AND f.accepted_at IS NOT NULL
               AND s.share_activity <> 'off'
             ORDER BY u.username",
            &[&device.user],
        )
        .await?;
    let history_users: Vec<Uuid> = rows
        .iter()
        .filter(|r| r.get::<_, String>("share_activity") == "history")
        .map(|r| r.get("id"))
        .collect();
    let mut recent: std::collections::HashMap<Uuid, Vec<Value>> = Default::default();
    if !history_users.is_empty() {
        let since = state.now() - RECENT_WINDOW_MS;
        let events = client
            .query(
                "SELECT user_id, value FROM (
                    SELECT user_id, value, row_number() OVER (
                        PARTITION BY user_id ORDER BY (value->>'at')::numeric DESC) AS n
                    FROM sync_records
                    WHERE user_id = ANY($1) AND collection = 'history' AND value IS NOT NULL
                      AND jsonb_typeof(value->'at') = 'number'
                      AND (value->>'at')::numeric >= $2::BIGINT
                 ) t WHERE n <= $3 ORDER BY user_id, n",
                &[&history_users, &since, &MAX_RECENT],
            )
            .await?;
        for row in events {
            if let Some(item) = recent_item(&row.get::<_, Value>("value")) {
                recent.entry(row.get("user_id")).or_default().push(item);
            }
        }
    }
    let activity: Vec<Value> = rows
        .iter()
        .map(|r| {
            let id: Uuid = r.get("id");
            let mut item = json!({
                "user_id": id,
                "username": r.get::<_, String>("username"),
                "now_playing": state.0.realtime.now_playing(id),
            });
            if r.get::<_, String>("share_activity") == "history" {
                item["recent"] = json!(recent.remove(&id).unwrap_or_default());
            }
            item
        })
        .collect();
    Ok(Json(json!({ "activity": activity })))
}

// ---------------------------------------------------------------- inbox

#[derive(Deserialize)]
struct InboxBody {
    to: Uuid,
    media: Value,
    #[serde(default)]
    note: String,
}

pub(crate) fn clean_note(note: &str) -> Result<String, String> {
    if note.chars().count() > MAX_NOTE_CHARS {
        return Err(format!("note has at most {MAX_NOTE_CHARS} characters"));
    }
    if note.chars().any(char::is_control) {
        return Err("note may not contain control characters".into());
    }
    Ok(vidcue_core::clean_text(note.trim()))
}

async fn inbox_send(
    State(state): State<AppState>,
    device: Device,
    Json(body): Json<InboxBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let key = format!("inbox-send:{}", device.user);
    state
        .attempt(&[(INBOX_SEND, &key)], "Too many sends, try again later")
        .await?;
    let (media, _, _) = slim_online(&body.media).map_err(ApiError::bad)?;
    let note = clean_note(&body.note).map_err(ApiError::bad)?;
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    if !are_friends(&*tx, device.user, body.to).await? {
        return Err(not_found("No such friend"));
    }
    lock_key(&*tx, &body.to.to_string(), 5).await?;
    let count: i64 = tx
        .query_one("SELECT COUNT(*) FROM inbox WHERE to_user = $1", &[&body.to])
        .await?
        .get(0);
    if count >= MAX_INBOX {
        // Make room by dropping the oldest items the recipient has already read.
        tx.execute(
            "DELETE FROM inbox WHERE id IN (
                SELECT id FROM inbox WHERE to_user = $1 AND read_at IS NOT NULL
                ORDER BY created_at, id LIMIT $2)",
            &[&body.to, &(count - MAX_INBOX + 1)],
        )
        .await?;
        let left: i64 = tx
            .query_one("SELECT COUNT(*) FROM inbox WHERE to_user = $1", &[&body.to])
            .await?
            .get(0);
        if left >= MAX_INBOX {
            return Err(conflict("Their inbox is full"));
        }
    }
    let id = Uuid::new_v4();
    tx.execute(
        "INSERT INTO inbox (id, to_user, from_user, media, note, created_at) VALUES ($1, $2, $3, $4, $5, $6)",
        &[&id, &body.to, &device.user, &media, &note, &now],
    )
    .await?;
    tx.commit().await?;
    state.0.hub.publish(
        body.to,
        json!({ "type": "inbox_item", "id": id, "from": { "user_id": device.user, "username": device.username } }),
    );
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

#[derive(Deserialize)]
struct InboxQuery {
    #[serde(default)]
    unread_only: bool,
}

async fn inbox(
    State(state): State<AppState>,
    device: Device,
    Query(q): Query<InboxQuery>,
) -> ApiResult {
    let client = state.0.db.get().await?;
    let rows = client
        .query(
            "SELECT i.id, i.from_user, u.username, i.media, i.note, i.created_at, i.read_at
             FROM inbox i JOIN users u ON u.id = i.from_user
             WHERE i.to_user = $1 AND (NOT $2 OR i.read_at IS NULL)
             ORDER BY i.created_at DESC, i.id LIMIT $3",
            &[&device.user, &q.unread_only, &MAX_INBOX],
        )
        .await?;
    let unread: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM inbox WHERE to_user = $1 AND read_at IS NULL",
            &[&device.user],
        )
        .await?
        .get(0);
    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>("id"),
                "from": { "user_id": r.get::<_, Uuid>("from_user"), "username": r.get::<_, String>("username") },
                "media": r.get::<_, Value>("media"),
                "note": r.get::<_, String>("note"),
                "created_at": r.get::<_, i64>("created_at"),
                "read": r.get::<_, Option<i64>>("read_at").is_some(),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items, "unread": unread })))
}

async fn inbox_read(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<Uuid>,
) -> ApiResult {
    let n = state
        .0
        .db
        .get()
        .await?
        .execute(
            "UPDATE inbox SET read_at = COALESCE(read_at, $3) WHERE id = $1 AND to_user = $2",
            &[&id, &device.user, &state.now()],
        )
        .await?;
    if n == 0 {
        return Err(not_found("No such item"));
    }
    Ok(Json(json!({ "read": true })))
}

async fn inbox_delete(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<Uuid>,
) -> ApiResult {
    let n = state
        .0
        .db
        .get()
        .await?
        .execute(
            "DELETE FROM inbox WHERE id = $1 AND to_user = $2",
            &[&id, &device.user],
        )
        .await?;
    if n == 0 {
        return Err(not_found("No such item"));
    }
    Ok(Json(json!({ "deleted": true })))
}

// ---------------------------------------------------------------- shared playlists

/// `SharedPlaylist` objects for `ids`, in creation order.
async fn playlists_json(
    db: &(impl GenericClient + Sync),
    ids: &[Uuid],
    caller: Uuid,
) -> Result<Vec<Value>, tokio_postgres::Error> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = db
        .query(
            "SELECT p.id, p.title, p.owner, u.username, p.updated_at,
                    GREATEST(p.seq, COALESCE((SELECT MAX(e.seq) FROM shared_playlist_entries e WHERE e.playlist_id = p.id), 0)) AS seq,
                    (SELECT COUNT(*) FROM shared_playlist_entries e WHERE e.playlist_id = p.id AND e.value IS NOT NULL) AS entry_count
             FROM shared_playlists p JOIN users u ON u.id = p.owner
             WHERE p.id = ANY($1) ORDER BY p.created_at, p.id",
            &[&ids],
        )
        .await?;
    let members = db
        .query(
            "SELECT m.playlist_id, m.user_id, u.username FROM shared_playlist_members m
             JOIN shared_playlists p ON p.id = m.playlist_id
             JOIN users u ON u.id = m.user_id
             WHERE m.playlist_id = ANY($1)
             ORDER BY (m.user_id = p.owner) DESC, u.username",
            &[&ids],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| {
            let id: Uuid = r.get("id");
            let owner: Uuid = r.get("owner");
            let list: Vec<Value> = members
                .iter()
                .filter(|m| m.get::<_, Uuid>(0) == id)
                .map(|m| json!({ "user_id": m.get::<_, Uuid>(1), "username": m.get::<_, String>(2) }))
                .collect();
            json!({
                "id": id,
                "title": r.get::<_, String>("title"),
                "owner": { "user_id": owner, "username": r.get::<_, String>("username") },
                "members": list,
                "entry_count": r.get::<_, i64>("entry_count"),
                "seq": r.get::<_, i64>("seq"),
                "updated_at": r.get::<_, i64>("updated_at"),
                "role": if owner == caller { "owner" } else { "member" },
            })
        })
        .collect())
}

async fn playlist_json(
    db: &(impl GenericClient + Sync),
    id: Uuid,
    caller: Uuid,
) -> ApiResult<Value> {
    playlists_json(db, &[id], caller)
        .await?
        .pop()
        .ok_or_else(|| not_found("No such playlist"))
}

/// The playlist's owner when `user` is a member; `None` (answered 404) otherwise.
async fn member_of(
    db: &(impl GenericClient + Sync),
    playlist: Uuid,
    user: Uuid,
) -> ApiResult<Uuid> {
    db.query_opt(
        "SELECT p.owner FROM shared_playlists p
         JOIN shared_playlist_members m ON m.playlist_id = p.id AND m.user_id = $2
         WHERE p.id = $1",
        &[&playlist, &user],
    )
    .await?
    .map(|r| r.get(0))
    .ok_or_else(|| not_found("No such playlist"))
}

async fn members(
    db: &(impl GenericClient + Sync),
    playlist: Uuid,
) -> Result<Vec<Uuid>, tokio_postgres::Error> {
    Ok(db
        .query(
            "SELECT user_id FROM shared_playlist_members WHERE playlist_id = $1",
            &[&playlist],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect())
}

/// Bumps the playlist's own seq (title or membership change; caller holds the
/// playlist lock) and returns `shared_playlist_changed` for every member.
async fn bump_playlist(
    db: &(impl GenericClient + Sync),
    playlist: Uuid,
    now: i64,
    device: Uuid,
) -> Result<Events, tokio_postgres::Error> {
    let seq: i64 = db
        .query_one(
            "UPDATE shared_playlists SET seq = nextval('shared_seq'), updated_at = $2 WHERE id = $1 RETURNING seq",
            &[&playlist, &now],
        )
        .await?
        .get(0);
    Ok(changed_events(
        &members(db, playlist).await?,
        playlist,
        seq,
        device,
    ))
}

fn changed_events(members: &[Uuid], playlist: Uuid, seq: i64, device: Uuid) -> Events {
    members
        .iter()
        .map(|m| {
            (
                *m,
                json!({ "type": "shared_playlist_changed", "playlist_id": playlist, "seq": seq, "device_id": device }),
            )
        })
        .collect()
}

async fn memberships(
    db: &(impl GenericClient + Sync),
    user: Uuid,
) -> Result<i64, tokio_postgres::Error> {
    Ok(db
        .query_one(
            "SELECT COUNT(*) FROM shared_playlist_members WHERE user_id = $1",
            &[&user],
        )
        .await?
        .get(0))
}

async fn shared_list(State(state): State<AppState>, device: Device) -> ApiResult {
    let client = state.0.db.get().await?;
    let ids: Vec<Uuid> = client
        .query(
            "SELECT playlist_id FROM shared_playlist_members WHERE user_id = $1",
            &[&device.user],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect();
    let playlists = playlists_json(&**client, &ids, device.user).await?;
    Ok(Json(json!({ "playlists": playlists })))
}

#[derive(Deserialize)]
struct CreateBody {
    id: Uuid,
    title: String,
}

async fn shared_create(
    State(state): State<AppState>,
    device: Device,
    Json(body): Json<CreateBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    title_ok(&body.title, 200).map_err(ApiError::bad)?;
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    lock_key(&*tx, &device.user.to_string(), 6).await?;
    lock_playlist(&*tx, body.id).await?;
    let owner: Option<Uuid> = tx
        .query_opt(
            "SELECT owner FROM shared_playlists WHERE id = $1",
            &[&body.id],
        )
        .await?
        .map(|r| r.get(0));
    match owner {
        Some(owner) if owner == device.user => {
            let playlist = playlist_json(&*tx, body.id, device.user).await?;
            tx.commit().await?;
            return Ok((StatusCode::OK, Json(playlist)));
        }
        Some(_) => return Err(conflict("This playlist id is taken")),
        None => {}
    }
    if memberships(&*tx, device.user).await? >= MAX_SHARED_PER_USER {
        return Err(conflict(format!(
            "At most {MAX_SHARED_PER_USER} shared playlists"
        )));
    }
    tx.execute(
        "INSERT INTO shared_playlists (id, owner, title, created_at, updated_at, seq)
         VALUES ($1, $2, $3, $4, $4, nextval('shared_seq'))",
        &[&body.id, &device.user, &body.title, &now],
    )
    .await?;
    tx.execute(
        "INSERT INTO shared_playlist_members (playlist_id, user_id, added_at) VALUES ($1, $2, $3)",
        &[&body.id, &device.user, &now],
    )
    .await?;
    let playlist = playlist_json(&*tx, body.id, device.user).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(playlist)))
}

#[derive(Deserialize)]
struct TitleBody {
    title: String,
}

async fn shared_rename(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<Uuid>,
    Json(body): Json<TitleBody>,
) -> ApiResult {
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    lock_playlist(&*tx, id).await?;
    if member_of(&*tx, id, device.user).await? != device.user {
        return Err(forbidden("Only the owner can do this"));
    }
    title_ok(&body.title, 200).map_err(ApiError::bad)?;
    tx.execute(
        "UPDATE shared_playlists SET title = $2 WHERE id = $1",
        &[&id, &body.title],
    )
    .await?;
    let events = bump_playlist(&*tx, id, now, device.device).await?;
    let playlist = playlist_json(&*tx, id, device.user).await?;
    tx.commit().await?;
    publish(&state, events);
    Ok(Json(playlist))
}

async fn shared_delete(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<Uuid>,
) -> ApiResult {
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    lock_playlist(&*tx, id).await?;
    if member_of(&*tx, id, device.user).await? != device.user {
        return Err(forbidden("Only the owner can do this"));
    }
    let everyone = members(&*tx, id).await?;
    tx.execute("DELETE FROM shared_playlists WHERE id = $1", &[&id])
        .await?;
    tx.commit().await?;
    let event = json!({ "type": "shared_playlist_removed", "playlist_id": id });
    publish(
        &state,
        everyone.into_iter().map(|m| (m, event.clone())).collect(),
    );
    Ok(Json(json!({ "deleted": true })))
}

#[derive(Deserialize)]
struct MemberBody {
    user_id: Uuid,
}

async fn shared_add_member(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<Uuid>,
    Json(body): Json<MemberBody>,
) -> ApiResult {
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    // Friendship first (share-locked), then the playlist: the same order as unfriending.
    if !are_friends(&*tx, device.user, body.user_id).await? {
        return Err(not_found("No such friend"));
    }
    lock_playlist(&*tx, id).await?;
    if member_of(&*tx, id, device.user).await? != device.user {
        return Err(forbidden("Only the owner can do this"));
    }
    let everyone = members(&*tx, id).await?;
    if !everyone.contains(&body.user_id) {
        if everyone.len() as i64 >= MAX_SHARED_MEMBERS {
            return Err(conflict(format!(
                "At most {MAX_SHARED_MEMBERS} members per playlist"
            )));
        }
        if memberships(&*tx, body.user_id).await? >= MAX_SHARED_PER_USER {
            return Err(conflict("They are in too many shared playlists"));
        }
        tx.execute(
            "INSERT INTO shared_playlist_members (playlist_id, user_id, added_at) VALUES ($1, $2, $3)",
            &[&id, &body.user_id, &now],
        )
        .await?;
    }
    let events = bump_playlist(&*tx, id, now, device.device).await?;
    let playlist = playlist_json(&*tx, id, device.user).await?;
    tx.commit().await?;
    publish(&state, events);
    Ok(Json(playlist))
}

async fn shared_remove_member(
    State(state): State<AppState>,
    device: Device,
    Path((id, user)): Path<(Uuid, Uuid)>,
) -> ApiResult {
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    lock_playlist(&*tx, id).await?;
    let owner = member_of(&*tx, id, device.user).await?;
    if user == owner {
        return Err(conflict(
            "The owner cannot leave; delete the playlist instead",
        ));
    }
    if user != device.user && owner != device.user {
        return Err(forbidden("Only the owner can do this"));
    }
    let removed = tx
        .execute(
            "DELETE FROM shared_playlist_members WHERE playlist_id = $1 AND user_id = $2",
            &[&id, &user],
        )
        .await?;
    if removed == 0 {
        return Err(not_found("No such member"));
    }
    let mut events = bump_playlist(&*tx, id, now, device.device).await?;
    tx.commit().await?;
    events.push((
        user,
        json!({ "type": "shared_playlist_removed", "playlist_id": id }),
    ));
    publish(&state, events);
    Ok(Json(json!({ "removed": true })))
}

#[derive(Deserialize)]
struct SharedOp {
    id: Uuid,
    entry_id: Uuid,
    #[serde(default)]
    value: Option<Value>,
    clock: i64,
}

#[derive(Deserialize)]
struct SharedPushBody {
    ops: Vec<SharedOp>,
}

/// Validates a shared entry exactly like a `playlist_entries` register value.
pub(crate) fn entry_value(value: &Value) -> Result<Value, String> {
    if has_nul(value) {
        return Err("NUL characters are not allowed".into());
    }
    if serde_json::to_vec(value).map_or(usize::MAX, |v| v.len()) > sync::MAX_VALUE_BYTES {
        return Err(format!("value exceeds {} bytes", sync::MAX_VALUE_BYTES));
    }
    let position = value
        .get("position")
        .and_then(Value::as_str)
        .ok_or("entry needs a position string")?;
    if position.is_empty()
        || position.len() > 64
        || !position.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        return Err("position must be 1 to 64 letters or digits".into());
    }
    let (media, _, _) = slim_online(value.get("media").ok_or("entry needs media")?)?;
    Ok(json!({ "position": position, "media": media }))
}

async fn shared_push(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<Uuid>,
    Json(body): Json<SharedPushBody>,
) -> ApiResult {
    if body.ops.len() > MAX_SHARED_OPS {
        return Err(ApiError::bad(format!(
            "Send at most {MAX_SHARED_OPS} operations per push"
        )));
    }
    let now = state.now();
    let mut client = state.0.db.get().await?;
    let tx = client.transaction().await?;
    lock_playlist(&*tx, id).await?;
    member_of(&*tx, id, device.user).await?;
    let mut live: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM shared_playlist_entries WHERE playlist_id = $1 AND value IS NOT NULL",
            &[&id],
        )
        .await?
        .get(0);
    let mut results = Vec::with_capacity(body.ops.len());
    let mut latest: Option<i64> = None;
    for op in &body.ops {
        let value = match (op.clock > 0, &op.value) {
            (false, _) => Err("clock must be a Unix millisecond time".to_owned()),
            (true, None) => Ok(None),
            (true, Some(v)) => entry_value(v).map(Some),
        };
        let value = match value {
            Ok(v) => v,
            Err(reason) => {
                results.push(sync::OpResult::Rejected { reason });
                continue;
            }
        };
        let seen = tx
            .query_opt(
                "SELECT 1 FROM shared_applied WHERE playlist_id = $1 AND op_id = $2",
                &[&id, &op.id],
            )
            .await?
            .is_some();
        if seen {
            results.push(sync::OpResult::Duplicate);
            continue;
        }
        let clock = sync::clamp_clock(op.clock, now);
        let stored = tx
            .query_opt(
                "SELECT clock, device_id, value IS NOT NULL FROM shared_playlist_entries
                 WHERE playlist_id = $1 AND entry_id = $2",
                &[&id, &op.entry_id],
            )
            .await?;
        let was_live = stored.as_ref().is_some_and(|r| r.get::<_, bool>(2));
        if let Some(row) = &stored
            && !sync::wins(
                (clock, device.device),
                (row.get("clock"), row.get("device_id")),
            )
        {
            mark_applied(&*tx, id, op.id, now).await?;
            results.push(sync::OpResult::Superseded);
            continue;
        }
        if value.is_some() && !was_live && live >= MAX_SHARED_ENTRIES {
            results.push(sync::OpResult::Rejected {
                reason: "playlist is full".into(),
            });
            continue;
        }
        mark_applied(&*tx, id, op.id, now).await?;
        let seq: i64 = tx
            .query_one(
                "INSERT INTO shared_playlist_entries (playlist_id, entry_id, value, clock, device_id, user_id, seq)
                 VALUES ($1, $2, $3, $4, $5, $6, nextval('shared_seq'))
                 ON CONFLICT (playlist_id, entry_id) DO UPDATE
                   SET value = EXCLUDED.value, clock = EXCLUDED.clock, device_id = EXCLUDED.device_id,
                       user_id = EXCLUDED.user_id, seq = EXCLUDED.seq
                 RETURNING seq",
                &[&id, &op.entry_id, &value, &clock, &device.device, &device.user],
            )
            .await?
            .get(0);
        live += i64::from(value.is_some()) - i64::from(was_live);
        latest = Some(seq);
        results.push(sync::OpResult::Applied { seq });
    }
    let mut events = Events::new();
    if let Some(seq) = latest {
        tx.execute(
            "UPDATE shared_playlists SET updated_at = $2 WHERE id = $1",
            &[&id, &now],
        )
        .await?;
        events = changed_events(&members(&*tx, id).await?, id, seq, device.device);
    }
    tx.commit().await?;
    publish(&state, events);
    let results: Vec<Value> = body
        .ops
        .iter()
        .zip(results)
        .map(|(op, r)| {
            let mut v = serde_json::to_value(r).unwrap_or_default();
            v["id"] = json!(op.id);
            v
        })
        .collect();
    Ok(Json(json!({ "results": results, "seq": latest })))
}

async fn mark_applied(
    db: &(impl GenericClient + Sync),
    playlist: Uuid,
    op: Uuid,
    now: i64,
) -> Result<u64, tokio_postgres::Error> {
    db.execute(
        "INSERT INTO shared_applied (playlist_id, op_id, applied_at) VALUES ($1, $2, $3)",
        &[&playlist, &op, &now],
    )
    .await
}

#[derive(Deserialize)]
struct PullQuery {
    #[serde(default)]
    since: i64,
    limit: Option<i64>,
}

async fn shared_pull(
    State(state): State<AppState>,
    device: Device,
    Path(id): Path<Uuid>,
    Query(q): Query<PullQuery>,
) -> ApiResult {
    let limit = q.limit.unwrap_or(500).clamp(1, MAX_SHARED_PULL);
    let mut client = state.0.db.get().await?;
    // One snapshot for the entries and the playlist's seq.
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await?;
    member_of(&*tx, id, device.user).await?;
    let rows = tx
        .query(
            "SELECT entry_id, value, clock, device_id, user_id, seq FROM shared_playlist_entries
             WHERE playlist_id = $1 AND seq > $2 ORDER BY seq LIMIT $3",
            &[&id, &q.since, &(limit + 1)],
        )
        .await?;
    let playlist = playlist_json(&*tx, id, device.user).await?;
    tx.commit().await?;
    let more = rows.len() as i64 > limit;
    let changes: Vec<Value> = rows
        .iter()
        .take(limit as usize)
        .map(|r| {
            json!({
                "entry_id": r.get::<_, Uuid>("entry_id"),
                "value": r.get::<_, Option<Value>>("value"),
                "clock": r.get::<_, i64>("clock"),
                "device_id": r.get::<_, Uuid>("device_id"),
                "user_id": r.get::<_, Uuid>("user_id"),
                "seq": r.get::<_, i64>("seq"),
            })
        })
        .collect();
    let mut next = changes
        .last()
        .and_then(|c| c["seq"].as_i64())
        .unwrap_or(q.since);
    if !more {
        // Title and membership changes advance the playlist seq without an entry; a
        // complete pull moves the cursor past them so the client does not pull again.
        next = next.max(playlist["seq"].as_i64().unwrap_or(0));
    }
    Ok(Json(json!({
        "playlist": playlist,
        "changes": changes,
        "next": next,
        "more": more,
    })))
}

pub async fn maintenance(db: &deadpool_postgres::Pool, now: i64) -> anyhow::Result<()> {
    let client = db.get().await?;
    client
        .execute(
            "DELETE FROM inbox WHERE created_at < $1",
            &[&(now - INBOX_RETENTION_MS)],
        )
        .await?;
    client
        .execute(
            "DELETE FROM shared_applied WHERE applied_at < $1",
            &[&(now - SHARED_APPLIED_RETENTION_MS)],
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_are_ordered_like_postgres_uuids() {
        let (lo, hi) = (Uuid::from_u128(1), Uuid::from_u128(u128::MAX));
        assert_eq!(pair(hi, lo), (lo, hi));
        assert_eq!(pair(lo, hi), (lo, hi));
    }

    #[test]
    fn notes_are_short_and_printable() {
        assert_eq!(clean_note("  listen to this ").unwrap(), "listen to this");
        assert!(clean_note(&"x".repeat(201)).is_err());
        assert!(clean_note(&"é".repeat(200)).is_ok());
        assert!(clean_note("line\nbreak").is_err());
        assert!(clean_note("bell\u{7}").is_err());
    }

    #[test]
    fn entries_follow_the_playlist_entry_rules() {
        let media = json!({"id": "abcdefghijk", "url": "https://youtu.be/abcdefghijk", "description": "long"});
        let v = entry_value(&json!({"position": "a0", "media": media, "extra": 1})).unwrap();
        assert_eq!(v["media"]["description"], "");
        assert!(v.get("extra").is_none());
        assert!(entry_value(&json!({"position": "a 0", "media": media})).is_err());
        assert!(entry_value(&json!({"position": "x".repeat(65), "media": media})).is_err());
        assert!(entry_value(&json!({"position": "a", "media": {"url": "/sdcard/a.mp3"}})).is_err());
        assert!(entry_value(&json!({"position": "a"})).is_err());
        let nul =
            json!({"position": "a", "media": {"url": "https://youtu.be/x", "title": "a\u{0}b"}});
        assert!(entry_value(&nul).unwrap_err().contains("NUL"));
        // Error messages never echo the request.
        let err =
            slim_online(&json!({"url": "https://secret.example/?token=abc", "kind": "sekrit"}))
                .unwrap_err();
        assert!(!err.contains("secret") && !err.contains("sekrit"), "{err}");
    }

    #[test]
    fn recent_items_keep_six_clean_fields() {
        let item = recent_item(&json!({"url": "https://youtu.be/abc", "title": "T\u{1b}[31m", "creator": "C",
                                       "kind": "song", "thumbnail": "/data/local/x.jpg", "at": 1_763_890_000_000i64,
                                       "seconds": 30, "finished": true}))
        .unwrap();
        assert_eq!(item["title"], "T[31m");
        assert_eq!(item["thumbnail"], "");
        assert_eq!(item["at"], 1_763_890_000_000i64);
        assert_eq!(item.as_object().unwrap().len(), 6);
        assert!(recent_item(&json!({"url": "file:///x", "at": 1})).is_none());
        assert!(recent_item(&json!({"url": "https://youtu.be/abc"})).is_none());
    }

    #[test]
    fn titles() {
        assert!(title_ok("Road trip", 200).is_ok());
        assert!(title_ok(" ", 200).is_err());
        assert!(title_ok(&"x".repeat(81), 80).is_err());
        assert!(title_ok("tab\there", 200).is_err());
    }
}
