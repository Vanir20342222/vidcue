//! Social, shared playlist, Jam, personal (stats, recommendations, health) and admin
//! ops (PROTOCOL.md §8.1). Thin wrappers returning the server JSON unless noted.
use super::*;
use crate::sync_bridge::{qr_rows, shareable, uuid_field};
use uuid::Uuid;

const OPS: &[&str] = &[
    "friends",
    "friend_request",
    "friend_accept",
    "friend_remove",
    "friends_activity",
    "social_settings",
    "inbox",
    "inbox_send",
    "inbox_read",
    "inbox_delete",
    "shared_playlist_share",
    "shared_playlist_leave",
    "shared_playlist_members",
    "shared_playlist_info",
    "jam_create",
    "jam_current",
    "jam_room",
    "jam_update",
    "jam_end",
    "jam_leave",
    "jam_join",
    "jam_decline",
    "jam_invite",
    "jam_kick",
    "jam_queue_add",
    "jam_queue_remove",
    "jam_vote",
    "jam_control",
    "jam_target",
    "health_summary",
    "stats",
    "stats_wrapped",
    "recs",
    "recs_settings",
    "recs_refresh",
    "admin_users",
    "admin_invite",
    "admin_reset",
    "admin_devices",
    "admin_revoke",
    "admin_disable",
    "admin_enable",
    "admin_health",
];
const JAM_ACTIONS: &[&str] = &[
    "play",
    "pause",
    "seek",
    "next",
    "play_item",
    "ended",
    "rate",
];
const MAX_IDS: usize = 20;

pub(crate) fn handles(op: &str) -> bool {
    OPS.contains(&op)
}

fn text<'a>(r: &'a Value, field: &str) -> Result<&'a str> {
    r[field]
        .as_str()
        .with_context(|| format!("Missing {}", field.replace('_', " ")))
}

fn uuids(r: &Value, field: &str) -> Result<Vec<Uuid>> {
    let list: Vec<Uuid> = match r.get(field) {
        Some(v) if !v.is_null() => {
            serde_json::from_value(v.clone()).context("Invalid user list")?
        }
        _ => Vec::new(),
    };
    ensure!(list.len() <= MAX_IDS, "Too many people at once");
    Ok(list)
}

fn online_media(r: &Value) -> Result<Media> {
    let media: Media = serde_json::from_value(r["media"].clone())?;
    ensure!(media.retained_bytes() <= 256 * 1024, "Media is too large");
    shareable(&media).context("Local files cannot be shared")
}

fn days(r: &Value) -> u32 {
    r["days"].as_u64().unwrap_or(7).clamp(1, 90) as u32
}

fn tz(r: &Value) -> Result<i32> {
    let offset = r["tz_offset_min"].as_i64().unwrap_or(0);
    ensure!((-840..=840).contains(&offset), "Invalid time zone offset");
    Ok(offset as i32)
}

/// Replaces the server's SVG with QR rows computed locally from the link.
fn with_qr(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.remove("qr_svg");
        if let Some(url) = object.get("url").and_then(Value::as_str) {
            let rows = qr_rows(url);
            object.insert("qr".into(), rows);
        }
    }
    value
}

pub(crate) async fn dispatch(engine: &Engine, op: &str, r: &Value) -> Result<Value> {
    let inner = engine.sync.inner();
    if op == "jam_target" {
        return Ok(inner.jam_target(r));
    }
    let sync = inner.require()?;
    let api = sync.api();
    match op {
        "friends" => inner.net(api.friends().await),
        "friend_request" => {
            let username = text(r, "username")?.trim();
            ensure!(
                !username.is_empty() && username.len() <= 64,
                "Enter a user name"
            );
            inner.net(api.friend_request(username).await)
        }
        "friend_accept" => inner.net(api.friend_accept(uuid_field(r, "user_id")?).await),
        "friend_remove" => inner.net(api.friend_remove(uuid_field(r, "user_id")?).await),
        "friends_activity" => inner.net(api.friends_activity().await),
        "social_settings" => match r["share_activity"].as_str() {
            Some(level) => {
                ensure!(
                    ["off", "now_playing", "history"].contains(&level),
                    "Unknown sharing level"
                );
                inner.net(api.set_social_settings(level).await)
            }
            None => inner.net(api.social_settings().await),
        },
        "inbox" => inner.net(api.inbox(r["unread_only"].as_bool().unwrap_or(false)).await),
        "inbox_send" => {
            let note = r["note"].as_str().unwrap_or_default().trim();
            ensure!(
                note.chars().count() <= 200,
                "Keep the note under 200 characters"
            );
            inner.net(
                api.inbox_send(uuid_field(r, "to")?, &online_media(r)?, note)
                    .await,
            )
        }
        "inbox_read" => inner.net(api.inbox_read(uuid_field(r, "id")?).await),
        "inbox_delete" => inner.net(api.inbox_delete(uuid_field(r, "id")?).await),
        "shared_playlist_share" => {
            let id = uuid_field(r, "playlist_id")?;
            let users = uuids(r, "user_ids")?;
            let (title, shared, role) = {
                let state = engine
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let local = state
                    .playlists
                    .iter()
                    .find(|p| p.id == id)
                    .context("This playlist no longer exists")?;
                (local.title.clone(), local.shared_id, local.role.clone())
            };
            ensure!(
                shared.is_none() || role.as_deref() == Some("owner"),
                "Only the owner can add people"
            );
            let mut playlist = if shared.is_none() {
                let created = inner.net(api.shared_create(id, &title).await)?;
                engine.mark_shared(id)?;
                let ops = engine.shared_entry_ops(id)?;
                engine.sync_ops(ops);
                created
            } else {
                Value::Null
            };
            for user in users {
                playlist = inner.net(api.shared_add_member(id, user).await)?;
            }
            inner.trigger_sync();
            if playlist.is_null() {
                let listing = inner.net(api.shared_playlists().await)?;
                playlist = listing["playlists"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|p| p["id"] == json!(id))
                    .cloned()
                    .unwrap_or(Value::Null);
            }
            Ok(playlist)
        }
        "shared_playlist_leave" => {
            let id = uuid_field(r, "playlist_id")?;
            let role = engine
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .playlists
                .iter()
                .find(|p| p.id == id && p.shared_id.is_some())
                .and_then(|p| p.role.clone())
                .context("This playlist is not shared")?;
            let response = if role == "owner" {
                inner.net(api.shared_delete(id).await)?
            } else {
                inner.net(api.shared_remove_member(id, sync.session().user_id).await)?
            };
            sync.drop_shared(id)?;
            engine.unshare_local(id)?;
            engine.sync.emit("library_changed", "playlists");
            Ok(response)
        }
        "shared_playlist_info" => {
            // The server's view of one shared playlist (members included), so the share
            // dialog starts from the real member list.
            let id = uuid_field(r, "playlist_id")?;
            let listing = inner.net(api.shared_playlists().await)?;
            Ok(listing["playlists"]
                .as_array()
                .and_then(|all| all.iter().find(|p| p["id"] == json!(id)).cloned())
                .unwrap_or(Value::Null))
        }
        "shared_playlist_members" => {
            let id = uuid_field(r, "playlist_id")?;
            let mut last = Value::Null;
            for user in uuids(r, "add")? {
                last = inner.net(api.shared_add_member(id, user).await)?;
            }
            for user in uuids(r, "remove")? {
                last = inner.net(api.shared_remove_member(id, user).await)?;
            }
            Ok(last)
        }
        "jam_create" => {
            let mut body = json!({});
            if let Some(title) = r["title"].as_str() {
                body["title"] = title.trim().into();
            }
            if let Some(control) = r["guests_can_control"].as_bool() {
                body["guests_can_control"] = control.into();
            }
            let room = inner.net(api.jam_create(&body).await)?;
            inner.jam_enter(&room);
            Ok(room)
        }
        "jam_current" => {
            let current = inner.net(api.jam_current().await)?;
            match current.get("current").filter(|c| !c.is_null()) {
                Some(room) => inner.jam_enter(room),
                None => {
                    if let Some(room) = inner.jam_room() {
                        inner.jam_leave(room);
                    }
                }
            }
            Ok(current)
        }
        "jam_room" => {
            let room = inner.net(api.jam_room(uuid_field(r, "room_id")?).await)?;
            inner.jam_enter(&room);
            Ok(room)
        }
        "jam_update" => {
            let mut body = json!({});
            if let Some(title) = r["title"].as_str() {
                body["title"] = title.trim().into();
            }
            if let Some(control) = r["guests_can_control"].as_bool() {
                body["guests_can_control"] = control.into();
            }
            inner.net(api.jam_update(uuid_field(r, "room_id")?, &body).await)
        }
        "jam_end" => {
            let room = uuid_field(r, "room_id")?;
            let response = inner.net(api.jam_end(room).await)?;
            inner.jam_leave(room);
            Ok(response)
        }
        "jam_leave" | "jam_decline" => {
            let room = uuid_field(r, "room_id")?;
            let action = if op == "jam_leave" {
                "leave"
            } else {
                "decline"
            };
            let response = inner.net(api.jam_member_action(room, action).await)?;
            if op == "jam_leave" {
                inner.jam_leave(room);
            }
            Ok(response)
        }
        "jam_join" => {
            let room = inner.net(
                api.jam_member_action(uuid_field(r, "room_id")?, "join")
                    .await,
            )?;
            inner.jam_enter(&room);
            Ok(room)
        }
        "jam_invite" => inner.net(
            api.jam_invite(uuid_field(r, "room_id")?, &uuids(r, "user_ids")?)
                .await,
        ),
        "jam_kick" => inner.net(
            api.jam_kick(uuid_field(r, "room_id")?, uuid_field(r, "user_id")?)
                .await,
        ),
        "jam_queue_add" => inner.net(
            api.jam_queue_add(uuid_field(r, "room_id")?, &online_media(r)?)
                .await,
        ),
        "jam_queue_remove" => inner.net(
            api.jam_queue_remove(uuid_field(r, "room_id")?, uuid_field(r, "item_id")?)
                .await,
        ),
        "jam_vote" => inner.net(
            api.jam_vote(
                uuid_field(r, "room_id")?,
                uuid_field(r, "item_id")?,
                r["up"].as_bool().unwrap_or(true),
            )
            .await,
        ),
        "jam_control" => {
            let room = uuid_field(r, "room_id")?;
            let action = text(r, "action")?;
            ensure!(JAM_ACTIONS.contains(&action), "Unknown Jam action");
            let mut body = json!({"action": action});
            if let Some(position) = r["position_ms"].as_u64() {
                body["position_ms"] = position.into();
            }
            if let Some(item) = r["item_id"].as_str() {
                body["item_id"] = Uuid::parse_str(item)
                    .context("Invalid item")?
                    .to_string()
                    .into();
            }
            if let Some(rate) = r["rate"].as_f64() {
                body["rate"] = rate.into();
            }
            let playback = inner.net(api.jam_playback(room, &body).await)?;
            inner.jam_playback(room, &playback);
            Ok(playback)
        }
        "health_summary" => inner.net(api.health_summary(days(r)).await),
        "stats" => {
            let from = r["from"].as_i64().context("Missing from")?;
            let to = r["to"].as_i64().context("Missing to")?;
            inner.net(api.stats(from, to, tz(r)?).await)
        }
        "stats_wrapped" => {
            let year = r["year"].as_i64().context("Missing year")?;
            ensure!((2020..=9999).contains(&year), "Invalid year");
            inner.net(api.stats_wrapped(year as i32, tz(r)?).await)
        }
        "recs" => {
            let mut recs = inner.net(api.recs().await)?;
            // Items as Media cards carrying their reason.
            if let Some(items) = recs.get_mut("items").and_then(Value::as_array_mut) {
                for item in items.iter_mut() {
                    let mut media = item["media"].clone();
                    if media.is_object() {
                        media["reason"] = item["reason"].clone();
                        media["source"] = item["source"].clone();
                        *item = media;
                    }
                }
            }
            Ok(recs)
        }
        "recs_settings" => {
            let mut body = serde_json::Map::new();
            for field in ["enabled", "music"] {
                if let Some(on) = r[field].as_bool() {
                    body.insert(field.into(), on.into());
                }
            }
            if body.is_empty() {
                inner.net(api.recs_settings().await)
            } else {
                inner.net(api.set_recs_settings(&Value::Object(body)).await)
            }
        }
        "recs_refresh" => inner.net(api.recs_refresh().await),
        "admin_users" => inner.net(api.admin_users().await),
        "admin_invite" => {
            let hours = r["hours"].as_i64().unwrap_or(72);
            let invite = inner.net(
                api.admin_invite(
                    text(r, "password")?,
                    r["admin"].as_bool().unwrap_or(false),
                    hours,
                )
                .await,
            )?;
            Ok(with_qr(invite))
        }
        "admin_reset" => {
            let reset = inner.net(
                api.admin_reset(
                    uuid_field(r, "user_id")?,
                    text(r, "password")?,
                    text(r, "code")?,
                )
                .await,
            )?;
            Ok(with_qr(reset))
        }
        "admin_devices" => inner.net(api.admin_devices(uuid_field(r, "user_id")?).await),
        "admin_revoke" => {
            let device = r["device_id"]
                .as_str()
                .map(|s| Uuid::parse_str(s).context("Invalid device"))
                .transpose()?;
            inner.net(
                api.admin_revoke(uuid_field(r, "user_id")?, text(r, "password")?, device)
                    .await,
            )
        }
        "admin_disable" | "admin_enable" => inner.net(
            api.admin_set_enabled(
                uuid_field(r, "user_id")?,
                text(r, "password")?,
                op == "admin_enable",
            )
            .await,
        ),
        "admin_health" => inner.net(api.admin_health(days(r)).await),
        _ => anyhow::bail!("Unknown mobile operation"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn invite_qr_replaces_the_svg() {
        let value = super::with_qr(
            serde_json::json!({"url": "https://a.example/join#vci_x", "qr_svg": "<svg/>", "expires_at": 1}),
        );
        assert!(value.get("qr_svg").is_none());
        assert!(value["qr"].as_array().is_some_and(|rows| !rows.is_empty()));
        assert_eq!(value["expires_at"], 1);
    }
}
