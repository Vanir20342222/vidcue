//! Client serde types for every message of PROTOCOL.md (§2, §3, §5, §6, §7).
//!
//! The server builds the same JSON in its own modules (§0.4); the tests below parse the
//! literal examples of the contract.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use vidcue_core::Media;

/// `Config` fields that sync (`settings` collection), mirroring the server's
/// `PORTABLE_SETTINGS`: everything else stays on each device.
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

/// `DownloadOptions` fields inside the `download` setting (the server's
/// `PORTABLE_DOWNLOAD_FIELDS`). Hooks, folders, templates and endpoints never sync.
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

/// The signed-in device, stored sealed under the `session` secret name.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Canonical origin, `https://host[:port]`.
    pub server: String,
    pub user_id: Uuid,
    pub username: String,
    pub device_id: Uuid,
    pub is_admin: bool,
    pub token: String,
}

impl std::fmt::Debug for SessionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionInfo")
            .field("server", &self.server)
            .field("user_id", &self.user_id)
            .field("username", &self.username)
            .field("device_id", &self.device_id)
            .field("is_admin", &self.is_admin)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// `{user_id, username}` as used in events and listings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserRef {
    #[serde(default)]
    pub user_id: Option<Uuid>,
    #[serde(default)]
    pub username: String,
}

/// A device's playback state (§3.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresenceState {
    #[serde(default)]
    pub media: Option<Media>,
    #[serde(default)]
    pub playing: bool,
    #[serde(default)]
    pub position_ms: u64,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default = "one")]
    pub rate: f64,
    #[serde(default)]
    pub volume: Option<u64>,
    #[serde(default)]
    pub queue_len: u64,
    #[serde(default)]
    pub at: i64,
    #[serde(default)]
    pub can: Vec<String>,
}

fn one() -> f64 {
    1.0
}

/// One entry of `presence_snapshot` and the `presence` event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresenceEntry {
    pub device_id: Uuid,
    #[serde(default)]
    pub device_name: String,
    pub state: PresenceState,
    #[serde(default)]
    pub server_at: i64,
}

/// Client → server WebSocket messages (§3.2, §5.5).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Ping {
        t0: i64,
    },
    Presence {
        state: PresenceState,
    },
    PresenceQuery,
    Handoff {
        to_device: Uuid,
        media: Media,
        position_ms: u64,
        playing: bool,
        queue: Vec<Media>,
    },
    Remote {
        to_device: Uuid,
        command: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        position_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        volume: Option<u64>,
    },
    JamPing {
        room_id: Uuid,
    },
}

impl ClientMessage {
    /// The JSON text frame, with `rid` when the caller waits for an acknowledgement.
    pub fn frame(&self, rid: Option<u32>) -> String {
        let mut value = serde_json::to_value(self).unwrap_or_default();
        if let Some(rid) = rid {
            value["rid"] = rid.into();
        }
        value.to_string()
    }
}

/// Server → client events. Unknown or malformed events are kept as [`ServerEvent::Other`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    Hello {
        seq: i64,
        device_id: Uuid,
        #[serde(default)]
        server_ms: Option<i64>,
        #[serde(default)]
        features: Vec<String>,
    },
    Changed {
        seq: i64,
        #[serde(default)]
        device_id: Option<Uuid>,
        #[serde(default)]
        resync: bool,
    },
    DeviceAdded {
        device_id: Uuid,
        #[serde(default)]
        paired_by: Option<Uuid>,
    },
    DeviceRevoked {
        device_id: Uuid,
    },
    DevicesRevoked {
        #[serde(default)]
        keep: Option<Uuid>,
    },
    KeyRequest {
        id: Uuid,
        device_id: Uuid,
    },
    KeyGranted {
        id: Uuid,
        device_id: Uuid,
    },
    Presence(PresenceEntry),
    PresenceGone {
        device_id: Uuid,
    },
    PresenceSnapshot {
        devices: Vec<PresenceEntry>,
        #[serde(default)]
        rid: Option<u32>,
    },
    Handoff {
        from_device: Uuid,
        #[serde(default)]
        from_name: String,
        media: Media,
        #[serde(default)]
        position_ms: u64,
        #[serde(default)]
        playing: bool,
        #[serde(default)]
        queue: Vec<Media>,
        to_device: Uuid,
    },
    Remote {
        from_device: Uuid,
        #[serde(default)]
        from_name: String,
        command: String,
        #[serde(default)]
        position_ms: Option<u64>,
        #[serde(default)]
        volume: Option<u64>,
        to_device: Uuid,
    },
    Pong {
        t0: i64,
        server_ms: i64,
        #[serde(default)]
        rid: Option<u32>,
    },
    Ack {
        rid: u32,
    },
    Error {
        #[serde(default)]
        rid: Option<u32>,
        #[serde(default)]
        code: String,
        #[serde(default)]
        message: String,
    },
    FriendRequest {
        from: UserRef,
    },
    FriendAdded {
        user_id: Uuid,
        #[serde(default)]
        username: String,
    },
    FriendRemoved {
        user_id: Uuid,
    },
    FriendActivity {
        user_id: Uuid,
        #[serde(default)]
        now_playing: Option<Value>,
    },
    InboxItem {
        id: Uuid,
        from: UserRef,
    },
    SharedPlaylistChanged {
        playlist_id: Uuid,
        #[serde(default)]
        seq: Option<i64>,
        #[serde(default)]
        device_id: Option<Uuid>,
    },
    SharedPlaylistRemoved {
        playlist_id: Uuid,
    },
    JamState {
        room_id: Uuid,
        playback: JamPlayback,
        #[serde(default)]
        server_ms: Option<i64>,
    },
    JamChanged {
        room_id: Uuid,
        #[serde(default)]
        version: i64,
        #[serde(default)]
        parts: Vec<String>,
    },
    JamInvite {
        room_id: Uuid,
        #[serde(default)]
        title: String,
        from: UserRef,
    },
    JamEnded {
        room_id: Uuid,
        #[serde(default)]
        reason: String,
    },
    #[serde(skip)]
    Other(Value),
}

impl ServerEvent {
    pub fn parse(text: &str) -> Self {
        match serde_json::from_str::<Value>(text) {
            Ok(value) => Self::from_value(value),
            Err(_) => Self::Other(Value::Null),
        }
    }
    pub fn from_value(value: Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or(Self::Other(value))
    }
}

/// The room's current item (§5.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JamItem {
    pub item_id: Uuid,
    pub media: Media,
    pub media_key: String,
    pub added_by: Uuid,
}

/// `JamPlayback` (§5.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JamPlayback {
    #[serde(default)]
    pub item: Option<JamItem>,
    pub state: String,
    #[serde(default)]
    pub position_ms: u64,
    #[serde(default)]
    pub at_server_ms: i64,
    #[serde(default = "one")]
    pub rate: f64,
    #[serde(default)]
    pub version: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JamMember {
    pub user_id: Uuid,
    pub username: String,
    pub role: String,
    #[serde(default)]
    pub online: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JamQueueItem {
    pub item_id: Uuid,
    pub media: Media,
    pub media_key: String,
    pub added_by: UserRef,
    pub added_at: i64,
    #[serde(default)]
    pub votes: i64,
    #[serde(default)]
    pub voted: bool,
}

/// `JamRoom` (§5.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JamRoom {
    pub id: Uuid,
    pub title: String,
    pub host: UserRef,
    #[serde(default)]
    pub guests_can_control: bool,
    #[serde(default)]
    pub version: i64,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub members: Vec<JamMember>,
    #[serde(default)]
    pub invited: Vec<UserRef>,
    #[serde(default)]
    pub queue: Vec<JamQueueItem>,
    pub playback: JamPlayback,
    #[serde(default)]
    pub server_ms: i64,
}

/// `SharedPlaylist` (§2.5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SharedPlaylist {
    pub id: Uuid,
    pub title: String,
    pub owner: UserRef,
    #[serde(default)]
    pub members: Vec<UserRef>,
    #[serde(default)]
    pub entry_count: i64,
    #[serde(default)]
    pub seq: i64,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub role: String,
}

/// One change of `GET /api/shared-playlists/{id}/pull`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SharedChange {
    pub entry_id: Uuid,
    #[serde(default)]
    pub value: Option<Value>,
    pub clock: i64,
    pub device_id: Uuid,
    #[serde(default)]
    pub user_id: Option<Uuid>,
    pub seq: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SharedPull {
    pub playlist: SharedPlaylist,
    #[serde(default)]
    pub changes: Vec<SharedChange>,
    pub next: i64,
    #[serde(default)]
    pub more: bool,
}

/// `FriendActivity` (§2.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FriendActivity {
    pub user_id: Uuid,
    pub username: String,
    #[serde(default)]
    pub now_playing: Option<Value>,
    #[serde(default)]
    pub recent: Vec<Value>,
}

/// A `history` register value (§6.3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub creator: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub thumbnail: String,
    #[serde(default)]
    pub at: i64,
    #[serde(default)]
    pub seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default)]
    pub finished: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthKind {
    Resolve,
    Start,
    Stall,
    Error,
    Download,
}

impl HealthKind {
    pub fn parse(text: &str) -> Option<Self> {
        serde_json::from_value(Value::String(text.into())).ok()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthCategory {
    Ok,
    NetworkTimeout,
    NetworkUnreachable,
    Tls,
    #[serde(rename = "http_403")]
    Http403,
    #[serde(rename = "http_404")]
    Http404,
    #[serde(rename = "http_429")]
    Http429,
    #[serde(rename = "http_5xx")]
    Http5xx,
    ExtractorFailed,
    JsChallengeFailed,
    SignatureFailed,
    FormatUnavailable,
    MembersOnly,
    AgeRestricted,
    GeoBlocked,
    Drm,
    LiveUnsupported,
    DecoderError,
    AudioOutputError,
    OfflineFileMissing,
    StorageFull,
    Cancelled,
    Unknown,
}

impl HealthCategory {
    pub fn parse(text: &str) -> Option<Self> {
        serde_json::from_value(Value::String(text.into())).ok()
    }
}

/// One sanitized playback outcome (§6.2). Nothing else is ever sent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthReport {
    pub kind: HealthKind,
    pub category: HealthCategory,
    pub client_version: String,
    pub platform: String,
    pub at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
}

/// A personal sync operation as sent to `POST /api/sync/push`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PushOp {
    pub id: Uuid,
    pub collection: String,
    pub key: String,
    pub value: Option<Value>,
    pub clock: i64,
}

/// Per-op push outcome (`applied`, `superseded`, `duplicate`, `rejected`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PushResult {
    pub id: Uuid,
    pub status: String,
    #[serde(default)]
    pub seq: Option<i64>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PushResponse {
    pub results: Vec<PushResult>,
    #[serde(default)]
    pub seq: Option<i64>,
}

/// One change of `GET /api/sync/pull`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PullChange {
    pub collection: String,
    pub key: String,
    #[serde(default)]
    pub value: Option<Value>,
    pub clock: i64,
    pub device_id: Uuid,
    pub seq: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PullPage {
    #[serde(default)]
    pub changes: Vec<PullChange>,
    pub next: i64,
    #[serde(default)]
    pub more: bool,
}

/// An open key request from another device (`GET /api/keys/requests`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyRequest {
    pub id: Uuid,
    pub device_id: Uuid,
    #[serde(default)]
    pub device_name: String,
    pub public_key: String,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub expires_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const MEDIA: &str = r#"{"id":"abc","title":"T","creator":"C","url":"https://www.youtube.com/watch?v=abcdefghijk","thumbnail":"","thumbnails":[],"section":"","duration":215,"description":"","kind":"song","browse_id":null,"tracks_url":null,"live":false,"chapters":[]}"#;
    const DEV: &str = "6f1c1f0e-8a52-4c3e-9d0f-3b8f6e2f3a11";

    fn literal(template: &str) -> String {
        template.replace("{MEDIA}", MEDIA).replace("{DEV}", DEV)
    }

    #[test]
    fn contract_events_parse() {
        let cases = [
            r#"{"type":"hello","seq":812,"device_id":"{DEV}","server_ms":1763900000000,"features":["realtime","social","jam"]}"#,
            r#"{"type":"hello","seq":3,"device_id":"{DEV}"}"#,
            r#"{"type":"changed","seq":9,"device_id":"{DEV}"}"#,
            r#"{"type":"changed","seq":9,"resync":true}"#,
            r#"{"type":"device_revoked","device_id":"{DEV}"}"#,
            r#"{"type":"devices_revoked"}"#,
            r#"{"type":"key_request","id":"{DEV}","device_id":"{DEV}"}"#,
            r#"{"type":"presence","device_id":"{DEV}","device_name":"Tab","state":{"media":{MEDIA},"playing":true,"position_ms":51000,"duration_ms":215000,"rate":1.0,"volume":80,"queue_len":4,"at":1763900000000,"can":["remote","handoff"]},"server_at":1763900000000}"#,
            r#"{"type":"presence_gone","device_id":"{DEV}"}"#,
            r#"{"type":"presence_snapshot","devices":[{"device_id":"{DEV}","device_name":"Tab","state":{"media":null,"playing":false,"position_ms":0,"duration_ms":null,"rate":1.0,"volume":null,"queue_len":0,"at":5,"can":[]},"server_at":5}]}"#,
            r#"{"type":"handoff","from_device":"{DEV}","from_name":"Phone","media":{MEDIA},"position_ms":5,"playing":true,"queue":[],"to_device":"{DEV}"}"#,
            r#"{"type":"remote","from_device":"{DEV}","from_name":"Phone","command":"seek","position_ms":9,"to_device":"{DEV}"}"#,
            r#"{"type":"pong","t0":1,"server_ms":2,"rid":4}"#,
            r#"{"type":"ack","rid":4}"#,
            r#"{"type":"error","rid":4,"code":"not_found","message":"That device is not connected"}"#,
            r#"{"type":"friend_request","from":{"user_id":"{DEV}","username":"bob"}}"#,
            r#"{"type":"friend_added","user_id":"{DEV}","username":"bob"}"#,
            r#"{"type":"friend_removed","user_id":"{DEV}"}"#,
            r#"{"type":"friend_activity","user_id":"{DEV}","now_playing":null}"#,
            r#"{"type":"inbox_item","id":"{DEV}","from":{"user_id":"{DEV}","username":"bob"}}"#,
            r#"{"type":"shared_playlist_changed","playlist_id":"{DEV}","seq":9,"device_id":"{DEV}"}"#,
            r#"{"type":"shared_playlist_removed","playlist_id":"{DEV}"}"#,
            r#"{"type":"jam_state","room_id":"{DEV}","playback":{"item":{"item_id":"{DEV}","media":{MEDIA},"media_key":"youtube:abc","added_by":"{DEV}"},"state":"playing","position_ms":51000,"at_server_ms":1763900000000,"rate":1.0,"version":17},"server_ms":1763900000000}"#,
            r#"{"type":"jam_state","room_id":"{DEV}","playback":{"item":null,"state":"stopped","position_ms":0,"at_server_ms":1,"rate":1.0,"version":2},"server_ms":1}"#,
            r#"{"type":"jam_changed","room_id":"{DEV}","version":17,"parts":["queue"]}"#,
            r#"{"type":"jam_invite","room_id":"{DEV}","title":"Friday","from":{"user_id":"{DEV}","username":"alice"}}"#,
            r#"{"type":"jam_ended","room_id":"{DEV}","reason":"expired"}"#,
        ];
        for case in cases {
            let text = literal(case);
            let event = ServerEvent::parse(&text);
            assert!(!matches!(event, ServerEvent::Other(_)), "{text}");
        }
        let unknown = ServerEvent::parse(r#"{"type":"future_thing","x":1}"#);
        assert_eq!(
            unknown,
            ServerEvent::Other(json!({"type":"future_thing","x":1}))
        );
        assert!(matches!(
            ServerEvent::parse(r#"{"type":"hello","seq":"x"}"#),
            ServerEvent::Other(_)
        ));
        assert!(matches!(
            ServerEvent::parse("not json"),
            ServerEvent::Other(_)
        ));
    }

    #[test]
    fn contract_objects_parse() {
        let room = literal(
            r#"{"id":"{DEV}","title":"Friday","host":{"user_id":"{DEV}","username":"alice"},
             "guests_can_control":false,"version":17,"created_at":1,"role":"guest",
             "members":[{"user_id":"{DEV}","username":"alice","role":"host","online":true}],
             "invited":[{"user_id":"{DEV}","username":"carol"}],
             "queue":[{"item_id":"{DEV}","media":{MEDIA},"media_key":"youtube:abc","added_by":{"user_id":"{DEV}","username":"bob"},"added_at":2,"votes":2,"voted":true}],
             "playback":{"item":null,"state":"stopped","position_ms":0,"at_server_ms":1,"rate":1.0,"version":17},"server_ms":1763900000123}"#,
        );
        let room: JamRoom = serde_json::from_str(&room).unwrap();
        assert_eq!(room.queue[0].votes, 2);
        let playlist = literal(
            r#"{"id":"{DEV}","title":"Road trip","owner":{"user_id":"{DEV}","username":"alice"},
             "members":[{"user_id":"{DEV}","username":"alice"},{"user_id":"{DEV}","username":"bob"}],
             "entry_count":12,"seq":41,"updated_at":1763900000000,"role":"owner"}"#,
        );
        let playlist: SharedPlaylist = serde_json::from_str(&playlist).unwrap();
        assert_eq!(playlist.seq, 41);
        let activity = literal(
            r#"{"user_id":"{DEV}","username":"bob",
             "now_playing":{"media":{MEDIA},"playing":true,"position_ms":51000,"server_at":1763900000000},
             "recent":[{"title":"t","creator":"c","url":"https://x.example/","kind":"song","thumbnail":"https://x.example/t","at":1763890000000}]}"#,
        );
        let activity: FriendActivity = serde_json::from_str(&activity).unwrap();
        assert_eq!(activity.recent.len(), 1);
        let report: HealthReport = serde_json::from_value(json!({"kind":"start","category":"ok","client_version":"0.2.0-beta.3","platform":"android","at":1763900000000_i64,"duration_ms":840})).unwrap();
        assert_eq!(report.category, HealthCategory::Ok);
        assert!(
            serde_json::from_value::<HealthReport>(json!({"kind":"start","category":"ok","client_version":"1","platform":"android","at":1,"url":"x"})).is_err()
        );
        for (text, category) in [
            ("http_403", HealthCategory::Http403),
            ("http_5xx", HealthCategory::Http5xx),
            ("js_challenge_failed", HealthCategory::JsChallengeFailed),
            ("offline_file_missing", HealthCategory::OfflineFileMissing),
        ] {
            assert_eq!(HealthCategory::parse(text), Some(category));
            assert_eq!(serde_json::to_value(category).unwrap(), json!(text));
        }
        assert_eq!(HealthKind::parse("stall"), Some(HealthKind::Stall));
        assert_eq!(HealthKind::parse("other"), None);
    }

    #[test]
    fn client_messages_serialize_as_the_contract() {
        let to = Uuid::parse_str(DEV).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&ClientMessage::Ping { t0: 5 }.frame(Some(3))).unwrap(),
            json!({"type":"ping","t0":5,"rid":3})
        );
        assert_eq!(
            serde_json::to_value(ClientMessage::PresenceQuery).unwrap(),
            json!({"type":"presence_query"})
        );
        assert_eq!(
            serde_json::to_value(ClientMessage::Remote {
                to_device: to,
                command: "pause".into(),
                position_ms: None,
                volume: None
            })
            .unwrap(),
            json!({"type":"remote","to_device":DEV,"command":"pause"})
        );
        assert_eq!(
            serde_json::to_value(ClientMessage::JamPing { room_id: to }).unwrap(),
            json!({"type":"jam_ping","room_id":DEV})
        );
    }
}
