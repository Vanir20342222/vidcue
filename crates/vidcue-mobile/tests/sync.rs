//! Engine sync integration against the in-process mock server (offline, synthetic data).
#[path = "../../vidcue-sync/tests/support/mock.rs"]
mod mock;
use mock::{CODE, Mock};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
use vidcue_mobile::Engine;

struct Device {
    engine: Arc<Engine>,
    root: tempfile::TempDir,
}

impl Device {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let engine =
            Arc::new(Engine::new(&root.path().join("files"), &root.path().join("cache")).unwrap());
        engine.start_sync();
        Self { engine, root }
    }
    async fn call(&self, request: Value) -> Value {
        self.engine
            .request(request.clone())
            .await
            .unwrap_or_else(|e| panic!("{request}: {e:#}"))
    }
    async fn sign_in(&self, mock: &Mock, user: &str) -> Value {
        self.call(json!({"op": "sync_sign_in", "server": mock.origin, "username": user, "password": "pw", "code": CODE, "device_name": "Tablet"}))
            .await
    }
    fn events(&self) -> Vec<(String, String)> {
        self.engine.take_sync_events()
    }
    fn files(&self) -> std::path::PathBuf {
        self.root.path().join("files")
    }
}

fn media(id: &str) -> Value {
    json!({"id": id, "title": format!("Song {id}"), "creator": "Band", "url": format!("https://www.youtube.com/watch?v={id}"),
           "thumbnail": "", "thumbnails": [], "section": "", "duration": 200, "description": "long text", "kind": "song",
           "channel_id": "UCaaaaaaaaaaaaaaaaaaaaaa", "browse_id": null, "tracks_url": null, "live": false, "chapters": []})
}

/// A finished run; a background run may already have started again (`syncing`).
fn assert_synced(status: &Value) {
    assert!(
        matches!(status["state"].as_str(), Some("idle" | "syncing")),
        "{status}"
    );
    assert!(status["last_success"].as_i64().unwrap() > 0, "{status}");
}

/// Polls `check` for up to 15 s.
async fn eventually(mut check: impl AsyncFnMut() -> bool) {
    for _ in 0..150 {
        if check().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("condition not reached");
}

#[tokio::test(flavor = "multi_thread")]
async fn old_library_positions_migrate_to_media_keyed_resume_entries() {
    let root = tempfile::tempdir().unwrap();
    let files = root.path().join("files");
    std::fs::create_dir_all(&files).unwrap();
    // A library.json written by an older version (no sync fields).
    std::fs::write(
        files.join("library.json"),
        json!({
            "saved": [media("abcdefghijk")],
            "queue": [],
            "positions": {
                "https://www.youtube.com/watch?v=abcdefghijk": 61000,
                "https://music.youtube.com/watch?v=lmnopqrstuv": 5000
            },
            "recent": []
        })
        .to_string(),
    )
    .unwrap();
    let engine = Engine::new(&files, &root.path().join("cache")).unwrap();
    // Lookups go through the media key, so another URL form of a video resumes too.
    for url in [
        "https://www.youtube.com/watch?v=abcdefghijk",
        "https://youtu.be/abcdefghijk",
    ] {
        assert_eq!(
            engine
                .request(json!({"op": "position", "url": url}))
                .await
                .unwrap()["milliseconds"],
            61000
        );
    }
    assert_eq!(
        engine
            .request(
                json!({"op": "position", "url": "https://www.youtube.com/watch?v=lmnopqrstuv"})
            )
            .await
            .unwrap()["milliseconds"],
        5000
    );
    let stored: Value =
        serde_json::from_slice(&std::fs::read(files.join("library.json")).unwrap()).unwrap();
    assert_eq!(stored["positions"], json!({}));
    let entry = &stored["resume"]["youtube:abcdefghijk"];
    assert_eq!(entry["ms"], 61000);
    assert_eq!(entry["updated_at"], 0);
    assert_eq!(entry["media"]["title"], "Song abcdefghijk");
    assert_eq!(entry["media"]["description"], "");
    // Saved media and the rest of the library are intact.
    assert_eq!(
        engine.request(json!({"op": "saved"})).await.unwrap()[0]["url"],
        media("abcdefghijk")["url"]
    );
    // Signed out: status works offline and sync ops refuse politely.
    let status = engine.request(json!({"op": "sync_status"})).await.unwrap();
    assert_eq!(
        (status["signed_in"].clone(), status["state"].clone()),
        (json!(false), json!("signed_out"))
    );
    assert!(engine.request(json!({"op": "sync_now"})).await.is_err());
    assert_eq!(
        engine.request(json!({"op": "jam_target"})).await.unwrap(),
        json!({"in_room": false})
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn local_changes_queue_the_expected_ops_and_sync_now_pushes_them() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    let device = Device::new();
    let status = device.sign_in(&mock, "alice").await;
    assert_eq!(status["signed_in"], true);
    assert_eq!(status["username"], "alice");
    assert!(
        status.to_string().find("vcd_").is_none(),
        "status must not carry the token"
    );
    device.call(json!({"op": "sync_now"})).await;
    let song = media("abcdefghijk");
    device
        .call(json!({"op": "save_media", "media": song}))
        .await;
    device
        .call(json!({"op": "history_record", "media": song}))
        .await;
    device
        .call(json!({"op": "position", "url": song["url"], "milliseconds": 90000, "duration_ms": 200000}))
        .await;
    let playlist = device
        .call(json!({"op": "playlist_create", "title": "Mix", "media": media("bbbbbbbbbbb")}))
        .await;
    let id = playlist["id"].as_str().unwrap().to_owned();
    let second = device
        .call(json!({"op": "playlist_add", "id": id, "media": song}))
        .await["entry_id"]
        .as_str()
        .unwrap()
        .to_owned();
    device
        .call(json!({"op": "playlist_move", "id": id, "entry_id": second}))
        .await;
    let entries = device
        .call(json!({"op": "playlist_entries", "id": id}))
        .await;
    assert_eq!(entries[0]["entry_id"], second);
    device
        .call(json!({"op": "follow_toggle", "channel_id": "UCbbbbbbbbbbbbbbbbbbbbbb", "title": "Band"}))
        .await;
    let filter = device
        .call(json!({"op": "filter_add", "kind": "keyword", "value": "live"}))
        .await;
    let mut config = device.call(json!({"op": "settings"})).await;
    config["theme"] = json!("vidcue-light");
    config["proxy"] = json!("http://127.0.0.1:9");
    config["download"]["quality"] = json!("720");
    device
        .call(json!({"op": "save_settings", "config": config}))
        .await;
    let mut item = song.clone();
    item["offline_id"] = json!("a".repeat(64));
    let offline = device
        .call(json!({"op": "offline_playlist_create", "title": "Trip", "items": [item]}))
        .await;
    let status = device.call(json!({"op": "sync_now"})).await;
    assert_synced(&status);
    assert_eq!(status["pending"], 0);
    assert_eq!(status["key_state"], "ok");
    let get = |c: &str, k: &str| mock.get(alice, c, k).and_then(|r| r.value);
    let saved = get("saved", "youtube:abcdefghijk").unwrap();
    assert_eq!(saved["description"], "");
    let position = get("positions", "youtube:abcdefghijk").unwrap();
    assert_eq!(position["seconds"], 90.0);
    assert_eq!(position["duration"], 200.0);
    let history: Vec<Value> = mock
        .data
        .lock()
        .unwrap()
        .records
        .iter()
        .filter(|((u, c, _), r)| *u == alice && c == "history" && r.value.is_some())
        .map(|(_, r)| r.value.clone().unwrap())
        .collect();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0]["seconds"], 90.0);
    assert_eq!(history[0]["kind"], "song");
    assert!(history[0]["at"].as_i64().unwrap() > 0);
    assert_eq!(get("playlists", &id).unwrap()["title"], "Mix");
    assert_eq!(
        get("playlist_entries", &format!("{id}/{second}")).unwrap()["media"]["url"],
        song["url"]
    );
    assert_eq!(
        get("subscriptions", "UCbbbbbbbbbbbbbbbbbbbbbb").unwrap()["title"],
        "Band"
    );
    assert_eq!(
        get("filters", filter["id"].as_str().unwrap()).unwrap(),
        json!({"kind": "keyword", "value": "live"})
    );
    assert_eq!(get("settings", "theme").unwrap(), "vidcue-light");
    // Device-local settings never leave the device.
    assert!(get("settings", "proxy").is_none());
    assert!(get("settings", "download").is_some_and(|d| d.get("directory").is_none()));
    // Untouched defaults are not pushed: they must not override other devices' choices.
    assert!(get("settings", "seek_seconds").is_none());
    assert!(get("offline_playlists", offline[0]["id"].as_str().unwrap()).is_some());
    // Deletions are tombstones.
    device
        .call(json!({"op": "save_media", "media": song}))
        .await;
    device.call(json!({"op": "history_clear"})).await;
    device.call(json!({"op": "sync_now"})).await;
    assert!(
        mock.get(alice, "saved", "youtube:abcdefghijk")
            .unwrap()
            .value
            .is_none()
    );
    assert!(
        mock.get(alice, "positions", "youtube:abcdefghijk")
            .unwrap()
            .value
            .is_none()
    );
    let events = device.events();
    assert!(events.iter().any(|(k, _)| k == "sync_status"));
    assert!(events.iter().all(|(_, text)| !text.contains("vcd_")));
}

#[tokio::test(flavor = "multi_thread")]
async fn remote_changes_apply_to_local_stores_and_keep_device_local_settings() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    let device = Device::new();
    let mut config = device.call(json!({"op": "settings"})).await;
    config["proxy"] = json!("http://127.0.0.1:9");
    config["download"]["directory"] = json!("/device/local");
    device
        .call(json!({"op": "save_settings", "config": config}))
        .await;
    let other = Uuid::new_v4();
    let now = vidcue_sync::now_ms() + 60_000;
    let playlist = Uuid::new_v4();
    let entry = Uuid::new_v4();
    let filter = Uuid::new_v4();
    let play = Uuid::new_v4();
    // An entry before its playlist header stays hidden until the header arrives.
    mock.put(
        alice,
        "playlist_entries",
        &format!("{playlist}/{entry}"),
        Some(json!({"position": "V", "media": media("ccccccccccc")})),
        now,
        other,
    );
    mock.put(
        alice,
        "playlists",
        &playlist.to_string(),
        Some(json!({"title": "Shared mix", "created_at": 5})),
        now,
        other,
    );
    mock.put(
        alice,
        "saved",
        "youtube:ddddddddddd",
        Some(media("ddddddddddd")),
        now,
        other,
    );
    mock.put(
        alice,
        "subscriptions",
        "UCcccccccccccccccccccccc",
        Some(json!({"channel_id": "UCcccccccccccccccccccccc", "title": "Chan"})),
        now,
        other,
    );
    mock.put(
        alice,
        "filters",
        &filter.to_string(),
        Some(json!({"kind": "shorts"})),
        now,
        other,
    );
    mock.put(
        alice,
        "settings",
        "theme",
        Some(json!("vidcue-light")),
        now,
        other,
    );
    mock.put(
        alice,
        "settings",
        "download",
        Some(json!({"quality": "720"})),
        now,
        other,
    );
    mock.put(
        alice,
        "settings",
        "proxy",
        Some(json!("http://evil.example:1")),
        now,
        other,
    );
    // The choice arrives before its custom curve: it is applied once the curve lands.
    mock.put(
        alice,
        "settings",
        "equalizer",
        Some(json!("mine")),
        now,
        other,
    );
    mock.put(
        alice,
        "eq_presets",
        "mine",
        Some(json!({"gains": vec![1.0; 10]})),
        now,
        other,
    );
    mock.put(alice, "positions", "youtube:ddddddddddd", Some(json!({"media": media("ddddddddddd"), "seconds": 42.5, "duration": 200.0, "updated_at": now, "finished": false})), now, other);
    mock.put(alice, "history", &play.to_string(), Some(json!({"url": media("ddddddddddd")["url"], "title": "Song ddddddddddd", "creator": "Band", "kind": "song", "thumbnail": "", "at": now, "seconds": 42.5, "finished": false})), now, other);
    mock.put(
        alice,
        "collection_watches",
        &Uuid::new_v4().to_string(),
        Some(json!({"ignored": true})),
        now,
        other,
    );
    device.sign_in(&mock, "alice").await;
    let status = device.call(json!({"op": "sync_now"})).await;
    assert_synced(&status);
    let playlists = device.call(json!({"op": "playlists"})).await;
    assert_eq!(playlists[0]["title"], "Shared mix");
    assert_eq!(playlists[0]["count"], 1);
    assert_eq!(playlists[0]["shared"], false);
    let saved = device.call(json!({"op": "saved"})).await;
    assert!(
        saved
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["url"] == media("ddddddddddd")["url"])
    );
    assert_eq!(
        device.call(json!({"op": "follows"})).await[0]["title"],
        "Chan"
    );
    assert_eq!(
        device.call(json!({"op": "filters"})).await[0]["kind"],
        "shorts"
    );
    let config = device.call(json!({"op": "settings"})).await;
    assert_eq!(config["theme"], "vidcue-light");
    assert_eq!(config["download"]["quality"], "720");
    assert_eq!(config["equalizer_custom"]["mine"][0], 1.0);
    assert_eq!(config["equalizer"], "mine");
    // Device-local fields are untouched even when a server sends them.
    assert_eq!(config["proxy"], "http://127.0.0.1:9");
    assert_eq!(config["download"]["directory"], "/device/local");
    assert_eq!(
        device
            .call(json!({"op": "position", "url": "https://youtu.be/ddddddddddd"}))
            .await["milliseconds"],
        42500
    );
    let history = device.call(json!({"op": "history_list"})).await;
    assert_eq!(history[0]["url"], media("ddddddddddd")["url"]);
    let events: Vec<String> = device
        .events()
        .into_iter()
        .map(|(k, t)| format!("{k}:{t}"))
        .collect();
    for expected in [
        "library_changed:playlists",
        "library_changed:saved",
        "library_changed:follows",
        "library_changed:filters",
        "config_changed:",
        "history_updated:",
    ] {
        assert!(
            events.iter().any(|e| e == expected),
            "{expected} in {events:?}"
        );
    }
    // Header tombstone removes the playlist.
    mock.put(
        alice,
        "playlists",
        &playlist.to_string(),
        None,
        now + 1,
        other,
    );
    device.call(json!({"op": "sync_now"})).await;
    assert_eq!(device.call(json!({"op": "playlists"})).await, json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn synced_youtube_sessions_wait_for_the_key_relay() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    let a = Device::new();
    a.sign_in(&mock, "alice").await;
    a.call(json!({"op": "sync_now"})).await;
    a.call(
        json!({"op": "import_cookies", "header": "SAPISID=synthetic-session; SID=synthetic-only"}),
    )
    .await;
    a.call(json!({"op": "sync_now"})).await;
    let envelope = mock
        .get(alice, "secrets", "youtube")
        .unwrap()
        .value
        .unwrap();
    assert!(!envelope.to_string().contains("synthetic"));
    // B signs in with a password: the session stays sealed until the key arrives.
    let b = Device::new();
    b.sign_in(&mock, "alice").await;
    let status = b.call(json!({"op": "sync_now"})).await;
    assert_eq!(status["key_state"], "missing");
    assert_eq!(b.call(json!({"op": "account"})).await["connected"], false);
    let request = b.call(json!({"op": "key_request_start"})).await;
    let fingerprint = request["fingerprint"].as_str().unwrap().to_owned();
    assert_eq!(fingerprint.len(), 15);
    assert_eq!(
        b.call(json!({"op": "key_fingerprint"})).await,
        json!({"state": "waiting", "fingerprint": fingerprint})
    );
    // A hears about the request over the WebSocket (fingerprint computed locally),
    // before anything is granted.
    let mut a_events = Vec::new();
    eventually(async || {
        a_events.extend(a.events());
        a_events
            .iter()
            .any(|(k, t)| k == "sync_key_request" && t.contains(&fingerprint))
    })
    .await;
    let requests = a.call(json!({"op": "key_requests"})).await;
    assert_eq!(requests["requests"][0]["fingerprint"], fingerprint);
    assert_eq!(requests["requests"][0]["device_name"], "Tablet");
    a.call(json!({"op": "key_grant", "id": requests["requests"][0]["id"]}))
        .await;
    eventually(async || b.call(json!({"op": "key_fingerprint"})).await["state"] == "ok").await;
    b.call(json!({"op": "sync_now"})).await;
    eventually(async || b.call(json!({"op": "account"})).await["connected"] == true).await;
    let events = b.events();
    assert!(
        events
            .iter()
            .any(|(k, t)| k == "account_updated" && t.is_empty())
    );
    assert!(
        events
            .iter()
            .any(|(k, t)| k == "sync_key_state" && t == "ok")
    );
    // The key and token are stored apart from the YouTube account and never in plain
    // library files.
    let library = std::fs::read_to_string(b.files().join("library.json")).unwrap();
    assert!(!library.contains("synthetic") && !library.contains("vcd_"));
    assert!(b.files().join("sync/session.txt").is_file());
    assert!(b.files().join("sync/e2e.txt").is_file());
    // Disconnecting YouTube locally does not delete the synced copy.
    b.call(json!({"op": "disconnect"})).await;
    b.call(json!({"op": "sync_now"})).await;
    assert!(
        mock.get(alice, "secrets", "youtube")
            .unwrap()
            .value
            .is_some()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn qr_pairing_handoff_presence_and_revocation() {
    let mock = Mock::start().await;
    mock.add_user("alice", "pw");
    let a = Device::new();
    a.sign_in(&mock, "alice").await;
    a.call(json!({"op": "sync_now"})).await;
    let pairing = a
        .call(json!({"op": "sync_pair_start", "password": "pw"}))
        .await;
    let uri = pairing["uri"].as_str().unwrap();
    assert!(uri.starts_with("vidcue://pair?") && uri.contains("#k="));
    assert!(pairing["qr"].as_array().unwrap().len() > 20);
    let b = Device::new();
    let inspect = b.call(json!({"op": "sync_pair_inspect", "uri": uri})).await;
    assert_eq!(inspect["host"], "127.0.0.1");
    assert_eq!(
        inspect["server"],
        mock.origin.as_str(),
        "the full origin is shown"
    );
    assert_eq!(inspect["secure"], false);
    assert_eq!(inspect["has_key"], true);
    assert!(b.engine.request(json!({"op": "sync_pair_inspect", "uri": "vidcue://pair?server=http%3A%2F%2Fevil.example&code=ABCDEFGH23"})).await.is_err());
    let claimed = b
        .call(json!({"op": "sync_pair_claim", "uri": uri, "device_name": "Phone"}))
        .await;
    // Nothing syncs (and no socket opens) until the user confirms the account.
    assert_eq!(claimed["awaiting_confirmation"], true);
    assert_eq!(claimed["username"], "alice");
    assert_eq!(claimed["collections"]["secrets"], false);
    let requests = mock.log().len();
    let status = b.call(json!({"op": "sync_now"})).await;
    assert_eq!(status["last_success"], 0);
    assert_eq!(mock.log().len(), requests);
    let confirmed = b.call(json!({"op": "sync_pair_confirm"})).await;
    assert_eq!(confirmed["awaiting_confirmation"], false);
    assert_eq!(
        confirmed["collections"]["secrets"], false,
        "stays off unless chosen"
    );
    let status = b.call(json!({"op": "sync_now"})).await;
    assert_eq!(status["key_state"], "ok");
    for device in [&a, &b] {
        device
            .call(json!({"op": "sync_foreground", "visible": true}))
            .await;
        eventually(async || device.call(json!({"op": "sync_status"})).await["connected"] == true)
            .await;
    }
    let b_id = b.call(json!({"op": "sync_status"})).await["device_id"].clone();
    // Presence from A shows up on B.
    a.call(json!({"op": "presence_update", "media": media("eeeeeeeeeee"), "playing": true, "position_ms": 1000}))
        .await;
    eventually(async || b.events().iter().any(|(k, _)| k == "presence_changed")).await;
    // Handoff and remote reach only the target device, with the sender's name.
    a.call(json!({"op": "handoff_send", "to_device": b_id, "media": media("eeeeeeeeeee"), "position_ms": 5000, "playing": true, "queue": [media("fffffffffff")]}))
        .await;
    a.call(json!({"op": "remote_send", "to_device": b_id, "command": "seek", "position_ms": 9000}))
        .await;
    let mut seen = Vec::new();
    eventually(async || {
        seen.extend(b.events());
        seen.iter().any(|(k, _)| k == "remote_command")
    })
    .await;
    let handoff = seen
        .iter()
        .find(|(k, _)| k == "handoff_received")
        .map(|(_, t)| serde_json::from_str::<Value>(t).unwrap())
        .unwrap();
    assert_eq!(handoff["position_ms"], 5000);
    assert_eq!(handoff["queue"][0]["url"], media("fffffffffff")["url"]);
    assert_eq!(handoff["media"]["description"], "");
    let remote = seen
        .iter()
        .find(|(k, _)| k == "remote_command")
        .map(|(_, t)| serde_json::from_str::<Value>(t).unwrap())
        .unwrap();
    assert_eq!(
        remote,
        json!({"from_name": "Tablet", "command": "seek", "position_ms": 9000})
    );
    // Local files are never handed off.
    let mut local = media("ggggggggggg");
    local["url"] = json!("file:///sdcard/Music/a.mp3");
    assert!(a.engine.request(json!({"op": "handoff_send", "to_device": b_id, "media": local, "position_ms": 0, "playing": true})).await.is_err());
    // Devices list marks the current device and online state.
    let devices = a.call(json!({"op": "sync_devices"})).await;
    assert_eq!(devices["devices"].as_array().unwrap().len(), 2);
    // B removes itself: signed out locally, library kept.
    b.call(json!({"op": "save_media", "media": media("hhhhhhhhhhh")}))
        .await;
    b.call(json!({"op": "sync_revoke", "device_id": b_id}))
        .await;
    let status = b.call(json!({"op": "sync_status"})).await;
    assert_eq!(status["signed_in"], false);
    assert_eq!(
        b.call(json!({"op": "saved"}))
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(!b.files().join("sync/session.txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn health_reports_classify_locally_and_never_store_error_text() {
    let mock = Mock::start().await;
    mock.add_user("alice", "pw");
    let device = Device::new();
    device.sign_in(&mock, "alice").await;
    device.call(json!({"op": "health_record", "kind": "error", "error_text": "HTTP Error 403: secret-marker"})).await;
    assert!(!device.files().join("sync/health.json").exists());
    device.call(json!({"op": "sync_settings", "health_reports": true, "collections": {"history": false}})).await;
    device.call(json!({"op": "health_record", "kind": "error", "error_text": "HTTP Error 403: secret-marker https://x.example/?token=1"})).await;
    device
        .call(json!({"op": "health_record", "kind": "start", "duration_ms": 800}))
        .await;
    assert!(
        device
            .engine
            .request(json!({"op": "health_record", "kind": "bogus"}))
            .await
            .is_err()
    );
    let buffered = std::fs::read_to_string(device.files().join("sync/health.json")).unwrap();
    assert!(!buffered.contains("secret-marker") && !buffered.contains("x.example"));
    let status = device.call(json!({"op": "sync_now"})).await;
    assert_eq!(status["collections"]["history"], false);
    assert_eq!(status["health_reports"], true);
    let reports = mock.data.lock().unwrap().health.clone();
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["category"], "http_403");
    assert_eq!(reports[1]["category"], "ok");
    assert_eq!(reports[1]["duration_ms"], 800);
    // A disabled collection is neither pushed nor pulled.
    device
        .call(json!({"op": "history_record", "media": media("iiiiiiiiiii")}))
        .await;
    let status = device.call(json!({"op": "sync_now"})).await;
    assert_eq!(status["pending"], 0);
    assert!(
        !mock
            .data
            .lock()
            .unwrap()
            .records
            .keys()
            .any(|(_, c, _)| c == "history")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn shared_playlists_flow_between_owner_and_member() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    let bob = mock.add_user("bob", "pw");
    let a = Device::new();
    a.sign_in(&mock, "alice").await;
    a.call(json!({"op": "sync_now"})).await;
    let playlist = a
        .call(json!({"op": "playlist_create", "title": "Road", "media": media("aaaaaaaaaaa")}))
        .await;
    let id = playlist["id"].as_str().unwrap().to_owned();
    a.call(json!({"op": "sync_now"})).await;
    assert!(mock.get(alice, "playlists", &id).unwrap().value.is_some());
    let shared = a
        .call(json!({"op": "shared_playlist_share", "playlist_id": id, "user_ids": [bob]}))
        .await;
    assert_eq!(shared["members"].as_array().unwrap().len(), 2);
    a.call(json!({"op": "sync_now"})).await;
    // The personal copy is tombstoned; the entries live on the shared playlist.
    assert!(mock.get(alice, "playlists", &id).unwrap().value.is_none());
    let uuid: Uuid = id.parse().unwrap();
    assert_eq!(mock.data.lock().unwrap().shared[&uuid].entries.len(), 1);
    let listed = a.call(json!({"op": "playlists"})).await;
    assert_eq!(
        (listed[0]["shared"].clone(), listed[0]["role"].clone()),
        (json!(true), json!("owner"))
    );
    // Bob sees it after a sync and can add to it.
    let b = Device::new();
    b.sign_in(&mock, "bob").await;
    b.call(json!({"op": "sync_now"})).await;
    let listed = b.call(json!({"op": "playlists"})).await;
    assert_eq!(listed[0]["title"], "Road");
    assert_eq!(listed[0]["role"], "member");
    assert_eq!(listed[0]["count"], 1);
    b.call(json!({"op": "playlist_add", "id": id, "media": media("bbbbbbbbbbb")}))
        .await;
    assert!(
        b.engine
            .request(json!({"op": "playlist_rename", "id": id, "title": "Mine"}))
            .await
            .is_err()
    );
    b.call(json!({"op": "sync_now"})).await;
    a.call(json!({"op": "sync_now"})).await;
    let entries = a.call(json!({"op": "playlist_entries", "id": id})).await;
    assert_eq!(entries.as_array().unwrap().len(), 2);
    assert!(
        a.events()
            .iter()
            .any(|(k, t)| k == "shared_playlist_changed" && t == &id)
    );
    // Bob leaves: his copy is gone; Alice keeps hers.
    b.call(json!({"op": "shared_playlist_leave", "playlist_id": id}))
        .await;
    assert_eq!(b.call(json!({"op": "playlists"})).await, json!([]));
    a.call(json!({"op": "sync_now"})).await;
    assert_eq!(a.call(json!({"op": "playlists"})).await[0]["count"], 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn jam_target_follows_the_room_clock_and_drift_rule() {
    let mock = Mock::start().await;
    mock.add_user("alice", "pw");
    let device = Device::new();
    device.sign_in(&mock, "alice").await;
    device
        .call(json!({"op": "sync_foreground", "visible": true}))
        .await;
    eventually(async || device.call(json!({"op": "sync_status"})).await["connected"] == true).await;
    let room = device.call(json!({"op": "jam_create"})).await;
    assert_eq!(room["title"], "Friday");
    let target = device.call(json!({"op": "jam_target"})).await;
    assert_eq!(target["in_room"], true);
    assert_eq!(target["action"], "load");
    assert_eq!(target["media"]["url"], mock::JAM_MEDIA);
    assert_eq!(target["rate"], 1.0);
    let at = target["target_ms"].as_u64().unwrap();
    // Within clock-estimate error of 10 s plus the time since the room was created.
    assert!((9_000..15_000).contains(&at), "{at}");
    // Same media, playing, far behind: seek ahead of the target, then back off.
    let behind = json!({"op": "jam_target", "player_media_key": "youtube:jjjjjjjjjjj", "player_position_ms": 0, "player_playing": true});
    let target = device.call(behind.clone()).await;
    assert_eq!(target["action"], "seek");
    assert!(target["seek_ms"].as_u64().unwrap() >= at + 150);
    assert_eq!(device.call(behind).await["action"], "none");
    let paused = device
        .call(json!({"op": "jam_target", "player_media_key": "youtube:jjjjjjjjjjj", "player_position_ms": at, "player_playing": false}))
        .await;
    assert_eq!(paused["action"], "play");
    // Leaving clears the local room.
    let id = room["id"].clone();
    device.call(json!({"op": "jam_leave", "room_id": id})).await;
    assert_eq!(
        device.call(json!({"op": "jam_target"})).await,
        json!({"in_room": false})
    );
    // `jam_current` adopts the server's view: a room open there is tracked locally,
    // and a room gone there is dropped locally.
    device.call(json!({"op": "jam_create"})).await;
    device.call(json!({"op": "jam_current"})).await;
    assert_eq!(
        device.call(json!({"op": "jam_target"})).await["in_room"],
        true
    );
    mock.data.lock().unwrap().jam = None;
    device.call(json!({"op": "jam_current"})).await;
    assert_eq!(
        device.call(json!({"op": "jam_target"})).await["in_room"],
        false
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn shared_playlist_event_before_the_first_listing_keeps_its_entries() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    let bob = mock.add_user("bob", "pw");
    let a = Device::new();
    a.sign_in(&mock, "alice").await;
    a.call(json!({"op": "sync_now"})).await;
    let b = Device::new();
    b.sign_in(&mock, "bob").await;
    b.call(json!({"op": "sync_now"})).await;
    b.call(json!({"op": "sync_foreground", "visible": true}))
        .await;
    eventually(async || b.call(json!({"op": "sync_status"})).await["connected"] == true).await;

    let playlist = a
        .call(json!({"op": "playlist_create", "title": "Road", "media": media("aaaaaaaaaaa")}))
        .await;
    let id = playlist["id"].as_str().unwrap().to_owned();
    a.call(json!({"op": "playlist_add", "id": id, "media": media("bbbbbbbbbbb")}))
        .await;
    a.call(json!({"op": "sync_now"})).await;
    a.call(json!({"op": "shared_playlist_share", "playlist_id": id, "user_ids": [bob]}))
        .await;
    a.call(json!({"op": "sync_now"})).await;
    let uuid: Uuid = id.parse().unwrap();
    let seq = {
        let data = mock.data.lock().unwrap();
        assert_eq!(data.shared[&uuid].entries.len(), 2);
        data.seq
    };
    // The member add announces the playlist to Bob before he has listed it.
    let _ = mock.hub.send((
        bob,
        json!({"type": "shared_playlist_changed", "playlist_id": id, "seq": seq, "device_id": Uuid::new_v4()}),
    ));
    eventually(async || {
        let listed = b.call(json!({"op": "playlists"})).await;
        listed
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == id.as_str() && p["count"] == 2)
    })
    .await;
    // The next listing keeps the entries (the cursor did not skip them).
    b.call(json!({"op": "sync_now"})).await;
    let entries = b.call(json!({"op": "playlist_entries", "id": id})).await;
    assert_eq!(entries.as_array().unwrap().len(), 2);
    let listed = b.call(json!({"op": "playlists"})).await;
    assert_eq!(listed[0]["role"], "member");
    let _ = alice;
}

#[tokio::test(flavor = "multi_thread")]
async fn owner_stopping_sharing_keeps_late_entries_on_other_owner_devices() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    let bob = mock.add_user("bob", "pw");
    let a = Device::new();
    a.sign_in(&mock, "alice").await;
    a.call(json!({"op": "sync_now"})).await;
    let playlist = a
        .call(json!({"op": "playlist_create", "title": "Road", "media": media("aaaaaaaaaaa")}))
        .await;
    let id = playlist["id"].as_str().unwrap().to_owned();
    a.call(json!({"op": "sync_now"})).await;
    a.call(json!({"op": "shared_playlist_share", "playlist_id": id, "user_ids": [bob]}))
        .await;
    a.call(json!({"op": "sync_now"})).await;
    // Alice's tablet has the shared copy.
    let tablet = Device::new();
    tablet.sign_in(&mock, "alice").await;
    tablet.call(json!({"op": "sync_now"})).await;
    let listed = tablet.call(json!({"op": "playlists"})).await;
    assert_eq!(
        (listed[0]["role"].clone(), listed[0]["count"].clone()),
        (json!("owner"), json!(1))
    );
    // Let the run triggered by the sign-in finish, so the next run does the personal
    // pull before the listing (the order that loses the entries).
    tokio::time::sleep(Duration::from_millis(500)).await;
    eventually(async || tablet.call(json!({"op": "sync_status"})).await["state"] == "idle").await;
    // The phone adds an entry and stops sharing before the tablet pulls it.
    a.call(json!({"op": "playlist_add", "id": id, "media": media("bbbbbbbbbbb")}))
        .await;
    a.call(json!({"op": "shared_playlist_leave", "playlist_id": id}))
        .await;
    a.call(json!({"op": "sync_now"})).await;
    assert_eq!(a.call(json!({"op": "playlists"})).await[0]["count"], 2);
    tablet.call(json!({"op": "sync_now"})).await;
    let listed = tablet.call(json!({"op": "playlists"})).await;
    assert_eq!(listed.as_array().unwrap().len(), 1, "{listed}");
    assert_eq!(listed[0]["shared"], false, "{listed}");
    assert_eq!(listed[0]["count"], 2, "{listed}");
    let _ = alice;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_shared_rename_is_reported_and_not_applied() {
    let mock = Mock::start().await;
    let bob = mock.add_user("bob", "pw");
    mock.add_user("alice", "pw");
    let a = Device::new();
    a.sign_in(&mock, "alice").await;
    a.call(json!({"op": "sync_now"})).await;
    let playlist = a
        .call(json!({"op": "playlist_create", "title": "Road"}))
        .await;
    let id = playlist["id"].as_str().unwrap().to_owned();
    a.call(json!({"op": "shared_playlist_share", "playlist_id": id, "user_ids": [bob]}))
        .await;
    a.call(json!({"op": "sync_now"})).await;
    mock.data.lock().unwrap().fail_rename = Some(503);
    assert!(
        a.engine
            .request(json!({"op": "playlist_rename", "id": id, "title": "Trip"}))
            .await
            .is_err()
    );
    assert_eq!(a.call(json!({"op": "playlists"})).await[0]["title"], "Road");
    a.call(json!({"op": "playlist_rename", "id": id, "title": "Trip"}))
        .await;
    let uuid: Uuid = id.parse().unwrap();
    assert_eq!(mock.data.lock().unwrap().shared[&uuid].title, "Trip");
    a.call(json!({"op": "sync_now"})).await;
    assert_eq!(a.call(json!({"op": "playlists"})).await[0]["title"], "Trip");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_older_jam_playback_never_replaces_a_newer_one() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    let device = Device::new();
    device.sign_in(&mock, "alice").await;
    device
        .call(json!({"op": "sync_foreground", "visible": true}))
        .await;
    eventually(async || device.call(json!({"op": "sync_status"})).await["connected"] == true).await;
    let room = device.call(json!({"op": "jam_create"})).await;
    let item = room["playback"]["item"].clone();
    let state = |version: i64, state: &str, position: u64| {
        json!({"type": "jam_state", "room_id": room["id"], "server_ms": vidcue_sync::now_ms(),
               "playback": {"item": item, "state": state, "position_ms": position,
                            "at_server_ms": vidcue_sync::now_ms(), "rate": 1.0, "version": version}})
    };
    // A newer state over the socket (version 5: paused at 1 s).
    let _ = mock.hub.send((alice, state(5, "paused", 1_000)));
    eventually(async || {
        let target = device.call(json!({"op": "jam_target"})).await;
        target["playing"] == false && target["target_ms"] == 1_000
    })
    .await;
    // An older socket event and a refetch that started earlier (version 1) are ignored.
    let _ = mock.hub.send((alice, state(3, "playing", 50_000)));
    device.call(json!({"op": "jam_current"})).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let target = device.call(json!({"op": "jam_target"})).await;
    assert_eq!(
        (target["playing"].clone(), target["target_ms"].clone()),
        (json!(false), json!(1_000))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn presence_is_sent_again_after_a_reconnect() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    let device = Device::new();
    device.sign_in(&mock, "alice").await;
    device
        .call(json!({"op": "sync_foreground", "visible": true}))
        .await;
    eventually(async || device.call(json!({"op": "sync_status"})).await["connected"] == true).await;
    // A play-state report from the hidden app (no `visible`) keeps the socket.
    device
        .call(json!({"op": "sync_foreground", "playing": true}))
        .await;
    assert_eq!(
        device.call(json!({"op": "sync_status"})).await["connected"],
        true
    );
    let mut seen = mock.hub.subscribe();
    device
        .call(json!({"op": "presence_update", "media": media("eeeeeeeeeee"), "playing": false, "position_ms": 7000}))
        .await;
    let presence = |seen: &mut tokio::sync::broadcast::Receiver<(Uuid, Value)>| {
        let mut found = None;
        while let Ok((user, event)) = seen.try_recv() {
            if user == alice && event["type"] == "presence" {
                found = Some(event);
            }
        }
        found
    };
    let mut first = None;
    eventually(async || {
        first = first.take().or_else(|| presence(&mut seen));
        first.is_some()
    })
    .await;
    // The server forgets presence when the socket closes; a new `hello` re-sends it.
    device.engine.take_sync_events();
    let _ = mock.hub.send((alice, json!({"type": "hello", "seq": 0, "device_id": Uuid::new_v4(), "server_ms": vidcue_sync::now_ms(), "features": ["realtime"]})));
    let mut again = None;
    eventually(async || {
        again = again.take().or_else(|| presence(&mut seen));
        again.is_some()
    })
    .await;
    let again = again.unwrap();
    assert_eq!(again["state"]["media"]["url"], media("eeeeeeeeeee")["url"]);
    assert_eq!(
        (
            again["state"]["playing"].clone(),
            again["state"]["position_ms"].clone()
        ),
        (json!(false), json!(7000))
    );
    assert!(device.events().iter().any(|(k, _)| k == "presence_reset"));
}
