//! Offline-first record sync.
//!
//! Every synced value is a register addressed by `(collection, key)`. Devices push
//! operations that set or delete (tombstone) a register; the server keeps the winner
//! per register and hands out changes in sequence order. Collections decide what a
//! valid value is and how two concurrent writes are ordered. Collections that need
//! finer merges (playlists in B2) model each entry as its own register.

use deadpool_postgres::Pool;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use vidcue_core::playback_history::{PlaybackHistory, Position};

pub const MAX_OPS_PER_PUSH: usize = 500;
pub const MAX_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_PULL: i64 = 1000;
/// Clients may not stamp writes further ahead than this, so one device with a wrong
/// clock cannot make its writes win forever.
pub const MAX_CLOCK_SKEW_MS: i64 = 5 * 60 * 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Collection {
    /// Resume positions, keyed by `PlaybackHistory::key`.
    Positions,
    /// Watch/listen history events: grow-only, one register per event.
    History,
    /// Saved songs/videos, keyed by `PlaybackHistory::key`; value is a `Media`.
    Saved,
    /// Playlist headers, keyed by playlist UUID.
    Playlists,
    /// One register per playlist entry, keyed `<playlist uuid>/<entry uuid>`, ordered by a
    /// fractional `position` string so concurrent inserts and moves never conflict.
    PlaylistEntries,
    /// `vidcue_core::offline::OfflinePlaylist` definitions (metadata only), keyed by id.
    OfflinePlaylists,
    /// `vidcue_core::watch::CollectionWatch`, keyed by id.
    Watches,
    /// Channels followed inside Vidcue, keyed by channel id.
    Subscriptions,
    /// One portable `Config` field per register, keyed by field name.
    Settings,
    /// Custom equalizer curves, keyed by preset name.
    EqPresets,
    /// Content filters (blocked channels, muted keywords, ...), keyed by UUID.
    Filters,
    /// End-to-end encrypted provider sessions, keyed by provider. Opaque to the server.
    Secrets,
}

impl Collection {
    pub const ALL: [(&str, Self); 12] = [
        ("positions", Self::Positions),
        ("history", Self::History),
        ("saved", Self::Saved),
        ("playlists", Self::Playlists),
        ("playlist_entries", Self::PlaylistEntries),
        ("offline_playlists", Self::OfflinePlaylists),
        ("collection_watches", Self::Watches),
        ("subscriptions", Self::Subscriptions),
        ("settings", Self::Settings),
        ("eq_presets", Self::EqPresets),
        ("filters", Self::Filters),
        ("secrets", Self::Secrets),
    ];
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
    }
}

/// `Config` fields that mean the same on every device. Paths to local tools, proxies,
/// credential stores and the active account index stay per device.
pub const PORTABLE_SETTINGS: &[&str] = &[
    "theme",
    "language",
    "thumbnails",
    "music_player_style",
    "reduced_motion",
    "playback_history",
    "resume_on_play",
    "autoplay_related",
    "seek_seconds",
    "prefer_offline",
    "preload_next",
    "color_grade",
    "auto_color_grade",
    "card_info",
    "show_hints",
    "playback_sponsorblock",
    "playback_sponsor_categories",
    "equalizer",
    "download",
    "concurrency",
    "max_retries",
    "retry_delay_seconds",
    "radio_pinned_countries",
    "custom_stations",
    "keybindings",
    "notifications",
    "notification_sound",
    "schedule",
];

/// `DownloadOptions` fields that sync. Everything else (post-download hook, folders,
/// filename template, extractor/SponsorBlock endpoints, format selector) is device-local:
/// syncing it would let a stolen token make other devices run programs or write elsewhere.
pub const PORTABLE_DOWNLOAD_FIELDS: &[&str] = &[
    "audio",
    "quality",
    "container",
    "audio_container",
    "audio_quality",
    "subtitles",
    "auto_subtitles",
    "embed_subtitles",
    "metadata",
    "thumbnail",
    "bandwidth_kib",
    "global_bandwidth_kib",
    "sponsorblock",
    "sponsorblock_remove",
    "split_chapters",
    "skip_duplicates",
];

/// Per-user storage limits.
pub const MAX_RECORDS_PER_USER: i64 = 100_000;
pub const MAX_BYTES_PER_USER: i64 = 64 * 1024 * 1024;

pub const FILTER_KINDS: &[&str] = &["channel", "keyword", "shorts", "watched", "live"];

#[derive(Clone, Debug, Deserialize)]
pub struct Op {
    /// Client-generated, unique per operation; retries reuse it.
    pub id: Uuid,
    pub collection: String,
    pub key: String,
    /// `None` deletes the register.
    #[serde(default)]
    pub value: Option<Value>,
    /// Client wall clock in Unix milliseconds when the change was made (offline edits
    /// keep their original time).
    pub clock: i64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum OpResult {
    Applied {
        seq: i64,
    },
    /// A newer write already holds the register; the client should pull.
    Superseded,
    Duplicate,
    Rejected {
        reason: String,
    },
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Change {
    pub collection: String,
    pub key: String,
    pub value: Option<Value>,
    pub clock: i64,
    pub device_id: Uuid,
    pub seq: i64,
}

/// Checks an operation and returns its value in canonical form.
pub fn validate(op: &Op) -> Result<(Collection, Option<Value>), String> {
    let collection = Collection::parse(&op.collection)
        .ok_or_else(|| format!("unknown collection {:?}", op.collection))?;
    if op.key.is_empty() || op.key.len() > 512 {
        return Err("key must have 1 to 512 bytes".into());
    }
    if op.key.contains('\0') || op.value.as_ref().is_some_and(has_nul) {
        return Err("NUL characters are not allowed".into());
    }
    if op.clock <= 0 {
        return Err("clock must be a Unix millisecond time".into());
    }
    let Some(value) = &op.value else {
        return Ok((collection, None));
    };
    if serde_json::to_vec(value).map_or(usize::MAX, |v| v.len()) > MAX_VALUE_BYTES {
        return Err(format!("value exceeds {MAX_VALUE_BYTES} bytes"));
    }
    match collection {
        Collection::Positions => {
            let position: Position = serde_json::from_value(value.clone())
                .map_err(|e| format!("invalid position: {e}"))?;
            if vidcue_core::http_url(&position.media.url).is_err() {
                // Local file paths differ per device and must not leave it.
                return Err("only online media positions sync".into());
            }
            if PlaybackHistory::key(&position.media) != op.key {
                return Err("key does not match the media".into());
            }
            if !position.seconds.is_finite() || position.seconds < 0.0 {
                return Err("seconds must be finite and non-negative".into());
            }
            let mut canonical = serde_json::to_value(&position).map_err(|e| e.to_string())?;
            if let Some(media) = canonical.get_mut("media") {
                slim_media(media);
            }
            Ok((collection, Some(canonical)))
        }
        Collection::History => {
            let object = value.as_object().ok_or("history event must be an object")?;
            let url = object
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if vidcue_core::http_url(url).is_err() {
                return Err("history event needs an online media url".into());
            }
            Ok((collection, Some(history_event(object)?)))
        }
        Collection::Saved => {
            let media = online_media(value)?;
            if PlaybackHistory::key(&media) != op.key {
                return Err("key does not match the media".into());
            }
            Ok((collection, Some(slimmed(&media)?)))
        }
        Collection::Playlists => {
            uuid_key(&op.key)?;
            let title = value
                .get("title")
                .and_then(Value::as_str)
                .ok_or("playlist needs a title")?;
            title_ok(title)?;
            let mut out = serde_json::json!({ "title": title });
            if let Some(created) = value.get("created_at").and_then(Value::as_i64) {
                out["created_at"] = created.into();
            }
            Ok((collection, Some(out)))
        }
        Collection::PlaylistEntries => {
            let (playlist, entry) = op
                .key
                .split_once('/')
                .ok_or("key must be <playlist uuid>/<entry uuid>")?;
            uuid_key(playlist)?;
            uuid_key(entry)?;
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
            let media = online_media(value.get("media").ok_or("entry needs media")?)?;
            Ok((
                collection,
                Some(serde_json::json!({ "position": position, "media": slimmed(&media)? })),
            ))
        }
        Collection::OfflinePlaylists => {
            let playlist: vidcue_core::offline::OfflinePlaylist =
                serde_json::from_value(value.clone())
                    .map_err(|e| format!("invalid offline playlist: {e}"))?;
            playlist.validate().map_err(|e| e.to_string())?;
            if playlist.id != op.key {
                return Err("key must be the playlist id".into());
            }
            let canonical = serde_json::to_value(&playlist).map_err(|e| e.to_string())?;
            Ok((collection, Some(canonical)))
        }
        Collection::Watches => {
            let watch: vidcue_core::watch::CollectionWatch = serde_json::from_value(value.clone())
                .map_err(|e| format!("invalid collection watch: {e}"))?;
            watch.validate().map_err(|e| e.to_string())?;
            if watch.id.to_string() != op.key {
                return Err("key must be the watch id".into());
            }
            device_local_download_fields_unset(&watch.options)?;
            let canonical = serde_json::to_value(&watch).map_err(|e| e.to_string())?;
            Ok((collection, Some(canonical)))
        }
        Collection::Subscriptions => {
            let channel = value.get("channel_id").and_then(Value::as_str);
            if channel != Some(op.key.as_str()) || op.key.len() > 64 {
                return Err("key must equal channel_id (at most 64 bytes)".into());
            }
            let title = value
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default();
            title_ok(title)?;
            let mut out = serde_json::json!({ "channel_id": op.key, "title": title });
            if let Some(thumb) = value.get("thumbnail").and_then(Value::as_str) {
                if thumb.len() > 2048 || vidcue_core::http_url(thumb).is_err() {
                    return Err("thumbnail must be an http(s) URL".into());
                }
                out["thumbnail"] = thumb.into();
            }
            Ok((collection, Some(out)))
        }
        Collection::Settings => Ok((collection, Some(validate_setting(&op.key, value)?))),
        Collection::EqPresets => {
            use vidcue_core::equalizer;
            if !equalizer::valid_name(&op.key) || equalizer::is_builtin(&op.key) {
                return Err("preset names use letters, digits and '-', up to 24 characters, and cannot reuse a built-in name".into());
            }
            let gains: Vec<f64> = value
                .get("gains")
                .cloned()
                .and_then(|g| serde_json::from_value(g).ok())
                .ok_or("preset needs a gains array")?;
            if gains.len() != equalizer::BANDS.len()
                || !gains
                    .iter()
                    .all(|g| g.is_finite() && g.abs() <= equalizer::MAX_GAIN)
            {
                return Err(format!(
                    "gains need {} values between -{} and {} dB",
                    equalizer::BANDS.len(),
                    equalizer::MAX_GAIN,
                    equalizer::MAX_GAIN
                ));
            }
            Ok((collection, Some(serde_json::json!({ "gains": gains }))))
        }
        Collection::Filters => {
            uuid_key(&op.key)?;
            let kind = value
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !FILTER_KINDS.contains(&kind) {
                return Err(format!(
                    "filter kind must be one of {}",
                    FILTER_KINDS.join(", ")
                ));
            }
            let text = value
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if matches!(kind, "channel" | "keyword")
                && (text.trim().is_empty() || text.chars().count() > 200)
            {
                return Err("channel and keyword filters need a value up to 200 characters".into());
            }
            let mut out = serde_json::json!({ "kind": kind });
            if !text.is_empty() {
                out["value"] = text.chars().take(200).collect::<String>().into();
            }
            Ok((collection, Some(out)))
        }
        Collection::Secrets => {
            validate_secret(&op.key, value)?;
            Ok((collection, Some(value.clone())))
        }
    }
}

fn uuid_key(key: &str) -> Result<(), String> {
    Uuid::parse_str(key)
        .map(|_| ())
        .map_err(|_| "key must be a UUID".into())
}

fn title_ok(title: &str) -> Result<(), String> {
    if title.trim().is_empty() || title.chars().count() > 200 || title.chars().any(char::is_control)
    {
        return Err("title must have 1 to 200 printable characters".into());
    }
    Ok(())
}

fn online_media(value: &Value) -> Result<vidcue_core::Media, String> {
    let media: vidcue_core::Media =
        serde_json::from_value(value.clone()).map_err(|e| format!("invalid media: {e}"))?;
    vidcue_core::http_url(&media.url).map_err(|_| "only online media syncs".to_owned())?;
    Ok(media)
}

/// Long descriptions and chapter lists are refetched; they need not be stored per entry.
fn slim_media(media: &mut Value) {
    if let Some(media) = media.as_object_mut() {
        media.insert("description".into(), Value::String(String::new()));
        media.insert("chapters".into(), Value::Array(Vec::new()));
        media.remove("offline_id");
    }
}

fn slimmed(media: &vidcue_core::Media) -> Result<Value, String> {
    let mut value = serde_json::to_value(media).map_err(|e| e.to_string())?;
    slim_media(&mut value);
    Ok(value)
}

/// A setting is valid when the default configuration with this one field replaced still
/// passes `Config::validate`. Returns the value as the configuration serialises it.
pub fn validate_setting(key: &str, value: &Value) -> Result<Value, String> {
    use vidcue_core::equalizer;
    if !PORTABLE_SETTINGS.contains(&key) {
        return Err(format!("{key:?} is not a synced setting"));
    }
    let mut config =
        serde_json::to_value(vidcue_core::Config::default()).map_err(|e| e.to_string())?;
    if key == "download" {
        let fields = value.as_object().ok_or("download must be an object")?;
        if let Some(field) = fields
            .keys()
            .find(|k| !PORTABLE_DOWNLOAD_FIELDS.contains(&k.as_str()))
        {
            return Err(format!(
                "download.{field} stays on each device and does not sync"
            ));
        }
        for (field, v) in fields {
            config["download"][field] = v.clone();
        }
    } else {
        config[key] = value.clone();
    }
    // A custom preset name refers to the `eq_presets` collection, which syncs separately.
    if key == "equalizer"
        && let Some(name) = value.as_str()
        && !equalizer::is_builtin(name)
        && equalizer::valid_name(name)
    {
        config["equalizer_custom"] = serde_json::json!({ name: vec![0.0; equalizer::BANDS.len()] });
    }
    let parsed: vidcue_core::Config =
        serde_json::from_value(config).map_err(|e| format!("invalid {key}: {e}"))?;
    parsed.validate().map_err(|e| e.to_string())?;
    let canonical = serde_json::to_value(&parsed).map_err(|e| e.to_string())?;
    Ok(if key == "download" {
        let all = &canonical["download"];
        PORTABLE_DOWNLOAD_FIELDS
            .iter()
            .filter(|f| value.get(**f).is_some())
            .map(|f| ((*f).to_owned(), all[*f].clone()))
            .collect::<serde_json::Map<_, _>>()
            .into()
    } else {
        canonical[key].clone()
    })
}

/// Download options embedded in synced records must leave device-local fields unset.
fn device_local_download_fields_unset(
    options: &vidcue_core::DownloadOptions,
) -> Result<(), String> {
    let default = vidcue_core::DownloadOptions::default();
    if options.post_download_hook.is_some()
        || options.directory != default.directory
        || options.audio_directory != default.audio_directory
        || options.filename_template != default.filename_template
        || options.sponsorblock_api != default.sponsorblock_api
        || options.format_selector.is_some()
    {
        return Err("download hook, folders, filename template, format selector and SponsorBlock server stay on each device".into());
    }
    Ok(())
}

fn has_nul(value: &Value) -> bool {
    match value {
        Value::String(s) => s.contains('\0'),
        Value::Array(items) => items.iter().any(has_nul),
        Value::Object(map) => map.iter().any(|(k, v)| k.contains('\0') || has_nul(v)),
        _ => false,
    }
}

/// History events keep only known fields with the expected types.
fn history_event(object: &serde_json::Map<String, Value>) -> Result<Value, String> {
    let mut out = serde_json::Map::new();
    for (field, value) in object {
        let ok = match (field.as_str(), value) {
            ("url", Value::String(s)) => s.len() <= 2048,
            ("title" | "creator" | "kind" | "thumbnail", Value::String(s)) => {
                s.chars().count() <= 512
            }
            ("at" | "seconds" | "duration", Value::Number(_)) => true,
            ("finished", Value::Bool(_)) => true,
            _ => return Err(format!("history events do not accept {field:?}")),
        };
        if !ok {
            return Err(format!("history field {field:?} is too long"));
        }
        out.insert(field.clone(), value.clone());
    }
    Ok(out.into())
}

/// Shape check only: the server cannot and must not decrypt.
pub fn validate_secret(key: &str, value: &Value) -> Result<(), String> {
    use data_encoding::BASE64;
    if key.is_empty()
        || key.len() > 32
        || !key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("secret keys are provider names: a-z, 0-9 and '-'".into());
    }
    let object = value.as_object().ok_or("secret must be an object")?;
    let allowed = ["v", "alg", "nonce", "ciphertext"];
    if object.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err("secret may only contain v, alg, nonce and ciphertext".into());
    }
    if object.get("v") != Some(&Value::from(1))
        || object.get("alg") != Some(&Value::from("xchacha20poly1305"))
    {
        return Err("secret must be v 1, alg xchacha20poly1305".into());
    }
    let decode = |field: &str| {
        object
            .get(field)
            .and_then(Value::as_str)
            .and_then(|s| BASE64.decode(s.as_bytes()).ok())
    };
    if decode("nonce").map(|n| n.len()) != Some(24) {
        return Err("nonce must be 24 bytes of base64".into());
    }
    if !decode("ciphertext").is_some_and(|c| c.len() >= 16) {
        return Err("ciphertext must be base64 with an authentication tag".into());
    }
    Ok(())
}

/// Whether an incoming write replaces the stored one: later clock wins, the device id
/// breaks ties so every replica orders the same way.
pub fn wins(incoming: (i64, Uuid), stored: (i64, Uuid)) -> bool {
    incoming > stored
}

pub fn clamp_clock(clock: i64, now_ms: i64) -> i64 {
    clock.min(now_ms + MAX_CLOCK_SKEW_MS)
}

/// Applies a batch for one user. Returns per-op results and the highest new seq.
pub async fn push(
    db: &Pool,
    user: Uuid,
    device: Uuid,
    ops: &[Op],
    now_ms: i64,
) -> anyhow::Result<(Vec<OpResult>, Option<i64>)> {
    let mut client = db.get().await?;
    let tx = client.transaction().await?;
    // Serialise pushes per user so sequence numbers commit in order (see migration).
    tx.execute(
        "SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))",
        &[&user.to_string()],
    )
    .await?;
    let usage = tx
        .query_one(
            "SELECT COUNT(*), COALESCE(SUM(octet_length(value::text)), 0)::BIGINT
             FROM sync_records WHERE user_id = $1",
            &[&user],
        )
        .await?;
    let (mut records, mut bytes): (i64, i64) = (usage.get(0), usage.get(1));
    let mut results = Vec::with_capacity(ops.len());
    let mut latest = None;
    for op in ops {
        let (_, value) = match validate(op) {
            Ok(v) => v,
            Err(reason) => {
                results.push(OpResult::Rejected { reason });
                continue;
            }
        };
        // Deletes are always allowed so a full account can free space. The estimate
        // counts replacements as growth, which errs on the safe side.
        if let Some(v) = &value {
            let size = v.to_string().len() as i64;
            if records + 1 > MAX_RECORDS_PER_USER || bytes + size > MAX_BYTES_PER_USER {
                results.push(OpResult::Rejected {
                    reason: "storage quota reached; delete old history or items".into(),
                });
                continue;
            }
            records += 1;
            bytes += size;
        }
        let fresh = tx
            .execute(
                "INSERT INTO sync_applied (user_id, op_id, applied_at) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
                &[&user, &op.id, &now_ms],
            )
            .await?
            == 1;
        if !fresh {
            results.push(OpResult::Duplicate);
            continue;
        }
        let clock = clamp_clock(op.clock, now_ms);
        let stored = tx
            .query_opt(
                "SELECT clock, device_id FROM sync_records
                 WHERE user_id = $1 AND collection = $2 AND key = $3 FOR UPDATE",
                &[&user, &op.collection, &op.key],
            )
            .await?;
        if let Some(row) = stored
            && !wins((clock, device), (row.get("clock"), row.get("device_id")))
        {
            results.push(OpResult::Superseded);
            continue;
        }
        let seq: i64 = tx
            .query_one(
                "INSERT INTO sync_records (user_id, collection, key, value, clock, device_id, seq)
                 VALUES ($1, $2, $3, $4, $5, $6, nextval('sync_seq'))
                 ON CONFLICT (user_id, collection, key) DO UPDATE
                   SET value = EXCLUDED.value, clock = EXCLUDED.clock,
                       device_id = EXCLUDED.device_id, seq = EXCLUDED.seq
                 RETURNING seq",
                &[&user, &op.collection, &op.key, &value, &clock, &device],
            )
            .await?
            .get(0);
        latest = Some(seq);
        results.push(OpResult::Applied { seq });
    }
    tx.commit().await?;
    Ok((results, latest))
}

pub async fn pull(db: &Pool, user: Uuid, since: i64, limit: i64) -> anyhow::Result<Vec<Change>> {
    let rows = db
        .get()
        .await?
        .query(
            "SELECT collection, key, value, clock, device_id, seq FROM sync_records
             WHERE user_id = $1 AND seq > $2 ORDER BY seq LIMIT $3",
            &[&user, &since, &limit.clamp(1, MAX_PULL + 1)],
        )
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| Change {
            collection: r.get("collection"),
            key: r.get("key"),
            value: r.get("value"),
            clock: r.get("clock"),
            device_id: r.get("device_id"),
            seq: r.get("seq"),
        })
        .collect())
}

pub async fn latest_seq(db: &Pool, user: Uuid) -> anyhow::Result<i64> {
    Ok(db
        .get()
        .await?
        .query_one(
            "SELECT COALESCE(MAX(seq), 0) FROM sync_records WHERE user_id = $1",
            &[&user],
        )
        .await?
        .get(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn op(collection: &str, key: &str, value: Option<Value>) -> Op {
        Op {
            id: Uuid::new_v4(),
            collection: collection.into(),
            key: key.into(),
            value,
            clock: 1_800_000_000_000,
        }
    }

    fn position(url: &str, seconds: f64) -> Value {
        json!({
            "media": {"id": "abcdefghijk", "title": "T", "url": url, "description": "long text"},
            "seconds": seconds, "duration": 300.0, "updated_at": 1_800_000_000u64, "finished": false
        })
    }

    fn ok(collection: &str, key: &str, value: Value) -> Value {
        validate(&op(collection, key, Some(value)))
            .unwrap_or_else(|e| panic!("{collection}/{key}: {e}"))
            .1
            .unwrap()
    }

    fn err(collection: &str, key: &str, value: Value) -> String {
        validate(&op(collection, key, Some(value))).unwrap_err()
    }

    #[test]
    fn every_collection_name_parses() {
        for (name, c) in Collection::ALL {
            assert_eq!(Collection::parse(name), Some(c));
        }
    }

    #[test]
    fn saved_and_playlist_entries_hold_slim_online_media() {
        let media = json!({"id": "abcdefghijk", "title": "T", "url": "https://youtu.be/abcdefghijk",
                           "description": "long", "offline_id": "x"});
        let saved = ok("saved", "youtube:abcdefghijk", media.clone());
        assert_eq!(saved["description"], "");
        assert!(saved.get("offline_id").is_none());
        assert!(err("saved", "youtube:other", media.clone()).contains("key"));
        let (p, e) = (Uuid::new_v4(), Uuid::new_v4());
        let entry = ok(
            "playlist_entries",
            &format!("{p}/{e}"),
            json!({"position": "a0", "media": media}),
        );
        assert_eq!(entry["position"], "a0");
        assert!(
            err(
                "playlist_entries",
                &format!("{p}/{e}"),
                json!({"position": "a 0", "media": {"url": "https://youtu.be/x"}})
            )
            .contains("position")
        );
        assert!(err("playlist_entries", "nope", json!({})).contains("playlist uuid"));
        assert!(
            err(
                "playlist_entries",
                &format!("{p}/{e}"),
                json!({"position": "a", "media": {"url": "/sdcard/x.mp3"}})
            )
            .contains("online")
        );
        ok("playlists", &p.to_string(), json!({"title": "Road trip"}));
        assert!(err("playlists", &p.to_string(), json!({"title": " "})).contains("title"));
        assert!(err("playlists", "not-a-uuid", json!({"title": "x"})).contains("UUID"));
    }

    #[test]
    fn offline_playlists_use_the_core_validation() {
        let id = Uuid::new_v4().to_string();
        ok(
            "offline_playlists",
            &id,
            json!({"id": id, "title": "Commute", "items": []}),
        );
        assert!(
            err(
                "offline_playlists",
                &Uuid::new_v4().to_string(),
                json!({"id": id, "title": "C", "items": []})
            )
            .contains("id")
        );
        assert!(
            !err(
                "offline_playlists",
                &id,
                json!({"id": id, "title": "", "items": []})
            )
            .is_empty()
        );
    }

    #[test]
    fn settings_are_portable_and_validated_like_config() {
        ok("settings", "theme", json!("dracula"));
        ok("settings", "seek_seconds", json!(10));
        ok("settings", "language", json!("de"));
        ok("settings", "equalizer", json!("my-curve"));
        assert!(err("settings", "seek_seconds", json!(0)).contains("seek_seconds"));
        assert!(err("settings", "language", json!("fr")).contains("language"));
        assert!(err("settings", "seek_seconds", json!("ten")).contains("invalid"));
        // Device-local settings never sync.
        for key in [
            "proxy",
            "yt_dlp",
            "native_js_runtime",
            "auth_store",
            "account_index",
            "equalizer_custom",
        ] {
            assert!(
                err("settings", key, json!(null)).contains("not a synced setting"),
                "{key}"
            );
        }
    }

    #[test]
    fn embedded_download_options_keep_device_local_fields_unset() {
        let mut options = vidcue_core::DownloadOptions::default();
        assert!(device_local_download_fields_unset(&options).is_ok());
        options.post_download_hook = Some("/tmp/x.sh".into());
        assert!(device_local_download_fields_unset(&options).is_err());
        let options = vidcue_core::DownloadOptions {
            directory: "/tmp".into(),
            ..Default::default()
        };
        assert!(device_local_download_fields_unset(&options).is_err());
        let options = vidcue_core::DownloadOptions {
            sponsorblock_api: "https://evil.example".into(),
            ..Default::default()
        };
        assert!(device_local_download_fields_unset(&options).is_err());
        assert!(
            err("settings", "download", json!({"filename_template": "../x"}))
                .contains("stays on each device")
        );
        assert_eq!(
            ok("settings", "download", json!({"quality": "720"})),
            json!({"quality": "720"})
        );
    }

    #[test]
    fn eq_presets_filters_and_subscriptions() {
        ok(
            "eq_presets",
            "warm",
            json!({"gains": [1, 2, 3, 0, 0, 0, 0, -1, -2, -3]}),
        );
        assert!(
            err(
                "eq_presets",
                "warm",
                json!({"gains": [99, 0, 0, 0, 0, 0, 0, 0, 0, 0]})
            )
            .contains("gains")
        );
        assert!(err("eq_presets", "warm", json!({"gains": [0]})).contains("gains"));
        assert!(
            err("eq_presets", "bad name!", json!({"gains": vec![0; 10]})).contains("preset names")
        );
        let id = Uuid::new_v4().to_string();
        ok("filters", &id, json!({"kind": "channel", "value": "UCxyz"}));
        ok("filters", &id, json!({"kind": "shorts"}));
        assert!(err("filters", &id, json!({"kind": "keyword", "value": ""})).contains("value"));
        assert!(err("filters", &id, json!({"kind": "mood"})).contains("kind"));
        ok(
            "subscriptions",
            "UC123",
            json!({"channel_id": "UC123", "title": "Channel"}),
        );
        assert!(
            err(
                "subscriptions",
                "UC123",
                json!({"channel_id": "UC999", "title": "x"})
            )
            .contains("channel_id")
        );
    }

    #[test]
    fn secrets_are_opaque_but_well_formed() {
        use data_encoding::BASE64;
        let good = json!({"v": 1, "alg": "xchacha20poly1305",
                          "nonce": BASE64.encode(&[7; 24]), "ciphertext": BASE64.encode(&[9; 80])});
        ok("secrets", "youtube", good.clone());
        let mut extra = good.clone();
        extra["cookies"] = json!("SID=plaintext");
        assert!(err("secrets", "youtube", extra).contains("only contain"));
        let mut short = good.clone();
        short["nonce"] = json!(BASE64.encode(&[1; 12]));
        assert!(err("secrets", "youtube", short).contains("nonce"));
        assert!(err("secrets", "YouTube Account", good).contains("provider"));
    }

    #[test]
    fn positions_use_the_shared_media_key_and_drop_bulky_fields() {
        let url = "https://www.youtube.com/watch?v=abcdefghijk";
        let (c, value) = validate(&op(
            "positions",
            "youtube:abcdefghijk",
            Some(position(url, 42.0)),
        ))
        .unwrap();
        assert_eq!(c, Collection::Positions);
        let value = value.unwrap();
        assert_eq!(value["seconds"], 42.0);
        assert_eq!(value["media"]["description"], "");
        assert!(validate(&op("positions", "youtube:other", Some(position(url, 1.0)))).is_err());
    }

    #[test]
    fn local_files_and_garbage_never_sync() {
        assert!(
            validate(&op(
                "positions",
                "/tmp/a.mp4",
                Some(position("/tmp/a.mp4", 9.0))
            ))
            .is_err()
        );
        assert!(validate(&op("positions", "k", Some(json!({"nope": 1})))).is_err());
        assert!(validate(&op("history", "e1", Some(json!({"url": "file:///x"})))).is_err());
        assert!(validate(&op("history", "e1", Some(json!("text")))).is_err());
        assert!(validate(&op("cookies", "k", None)).is_err());
        assert!(validate(&op("history", "", None)).is_err());
        let mut stale = op("history", "e1", None);
        stale.clock = 0;
        assert!(validate(&stale).is_err());
        let big = json!({"url": "https://youtu.be/x", "pad": "x".repeat(MAX_VALUE_BYTES)});
        assert!(validate(&op("history", "e1", Some(big))).is_err());
    }

    #[test]
    fn deletes_are_tombstones() {
        assert_eq!(
            validate(&op("positions", "youtube:x", None)).unwrap().1,
            None
        );
    }

    #[test]
    fn later_clock_wins_and_device_breaks_ties() {
        let (a, b) = (Uuid::from_u128(1), Uuid::from_u128(2));
        assert!(wins((2, a), (1, b)));
        assert!(!wins((1, b), (2, a)));
        assert!(wins((5, b), (5, a)));
        assert!(!wins((5, a), (5, b)));
        assert!(
            !wins((5, a), (5, a)),
            "re-sending the stored write is a no-op"
        );
    }

    #[test]
    fn future_clocks_are_clamped() {
        assert_eq!(clamp_clock(10, 0), 10);
        assert_eq!(clamp_clock(i64::MAX, 1000), 1000 + MAX_CLOCK_SKEW_MS);
    }
}
