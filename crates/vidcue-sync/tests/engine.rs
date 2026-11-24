//! Sync engine and WebSocket client against the in-process mock server (offline).
mod support {
    pub mod mock;
}
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use support::mock::{CODE, Mock};
use uuid::Uuid;
use vidcue_sync::{
    Change, LocalStores, SyncEngine,
    engine::{KeyState, RunState},
    protocol::{ServerEvent, SharedChange, SharedPlaylist},
    store::{MemoryStore, SecretStore},
    ws::{WsClient, WsState},
};

#[derive(Default)]
struct Fake {
    registers: Mutex<BTreeMap<(String, String), (Value, i64)>>,
    applied: Mutex<Vec<Change>>,
    shared: Mutex<Vec<(Uuid, Vec<SharedChange>)>>,
    listed: Mutex<Vec<SharedPlaylist>>,
}

impl Fake {
    fn set(&self, collection: &str, key: &str, value: Value, clock: i64) {
        self.registers
            .lock()
            .unwrap()
            .insert((collection.into(), key.into()), (value, clock));
    }
    fn value(&self, collection: &str, key: &str) -> Option<Value> {
        self.registers
            .lock()
            .unwrap()
            .get(&(collection.into(), key.into()))
            .map(|(v, _)| v.clone())
    }
}

impl LocalStores for Fake {
    fn apply(&self, change: &Change) -> anyhow::Result<()> {
        let mut registers = self.registers.lock().unwrap();
        let slot = (change.collection.clone(), change.key.clone());
        match &change.value {
            Some(value) => {
                registers.insert(slot, (value.clone(), change.clock));
            }
            None => {
                registers.remove(&slot);
            }
        }
        self.applied.lock().unwrap().push(change.clone());
        Ok(())
    }
    fn snapshot(&self, collection: &str) -> anyhow::Result<Vec<(String, Value, i64)>> {
        Ok(self
            .registers
            .lock()
            .unwrap()
            .iter()
            .filter(|((c, _), _)| c == collection)
            .map(|((_, k), (v, clock))| (k.clone(), v.clone(), *clock))
            .collect())
    }
    fn applied(&self, _collections: &[String]) {}
    fn shared_playlists(&self, playlists: &[SharedPlaylist]) -> anyhow::Result<()> {
        *self.listed.lock().unwrap() = playlists.to_vec();
        Ok(())
    }
    fn apply_shared(
        &self,
        playlist: &SharedPlaylist,
        changes: &[SharedChange],
    ) -> anyhow::Result<()> {
        self.shared
            .lock()
            .unwrap()
            .push((playlist.id, changes.to_vec()));
        Ok(())
    }
}

fn media(id: &str) -> Value {
    json!({"id": id, "title": format!("T {id}"), "creator": "C", "url": format!("https://www.youtube.com/watch?v={id}"),
           "thumbnail": "", "thumbnails": [], "section": "", "duration": 100, "description": "", "kind": "video",
           "browse_id": null, "tracks_url": null, "live": false, "chapters": []})
}

async fn sign_in(
    mock: &Mock,
    dir: &std::path::Path,
    store: Arc<MemoryStore>,
    user: &str,
) -> SyncEngine {
    SyncEngine::sign_in(dir, store, &mock.origin, user, "pw", CODE, "Test")
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn first_sync_unions_both_sides_and_creates_the_key() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    let other = Uuid::new_v4();
    let now = vidcue_sync::now_ms();
    // Remote: A is newer than the local copy, C exists only remotely.
    mock.put(alice, "saved", "A", Some(media("aaaaaaaaaaa")), now, other);
    mock.put(alice, "saved", "C", Some(media("ccccccccccc")), 5, other);
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let engine = sign_in(&mock, dir.path(), store.clone(), "alice").await;
    assert_eq!(engine.key_state(), KeyState::Missing);
    let local = Fake::default();
    local.set("saved", "A", json!({"stale": true}), 10);
    local.set("saved", "B", media("bbbbbbbbbbb"), 10);
    engine.sync_once(&local).await.unwrap();
    // Remote A and C were applied; local B was pushed; local A was not.
    assert_eq!(local.value("saved", "A").unwrap()["title"], "T aaaaaaaaaaa");
    assert!(local.value("saved", "C").is_some());
    assert_eq!(mock.get(alice, "saved", "B").unwrap().clock, 10);
    assert_eq!(mock.get(alice, "saved", "A").unwrap().device, other);
    // No key check anywhere: this device created the key and pushed the check.
    assert_eq!(engine.key_state(), KeyState::Ok);
    let check = mock
        .get(alice, "secrets", "keycheck")
        .unwrap()
        .value
        .unwrap();
    let key = engine.account_key().unwrap();
    assert!(vidcue_sync::crypto::keycheck_opens(&key, &check));
    assert!(store.load("e2e_key").unwrap().is_some());
    let status = engine.status();
    assert_eq!(status.state, RunState::Idle);
    assert_eq!(status.pending, 0);
    assert!(status.server_features.contains(&"health".to_owned()));
    // The key check never reaches the app.
    assert!(local.value("secrets", "keycheck").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn first_sync_keeps_and_pushes_newer_local_registers() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    let other = Uuid::new_v4();
    // Remote values written earlier by another device.
    mock.put(
        alice,
        "settings",
        "theme",
        Some(json!("remote-old")),
        100,
        other,
    );
    mock.put(
        alice,
        "filters",
        "F",
        Some(json!({"kind": "remote"})),
        100,
        other,
    );
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let engine = sign_in(&mock, dir.path(), store.clone(), "alice").await;
    let local = Fake::default();
    // Edited here later (while signed out or with the collection off).
    local.set("settings", "theme", json!("local-new"), 200);
    local.set("filters", "F", json!({"kind": "local"}), 200);
    // Never edited here (clock 0) and missing remotely: pushed with the lowest clock.
    local.set("settings", "volume", json!(80), 0);
    engine.sync_once(&local).await.unwrap();
    assert_eq!(
        local.value("settings", "theme").unwrap(),
        json!("local-new")
    );
    assert_eq!(
        local.value("filters", "F").unwrap(),
        json!({"kind": "local"})
    );
    let theme = mock.get(alice, "settings", "theme").unwrap();
    assert_eq!((theme.value, theme.clock), (Some(json!("local-new")), 200));
    assert_eq!(
        mock.get(alice, "filters", "F").unwrap().value,
        Some(json!({"kind": "local"}))
    );
    assert_eq!(mock.get(alice, "settings", "volume").unwrap().clock, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn pending_local_writes_win_conflicts_and_the_cursor_persists() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let engine = sign_in(&mock, dir.path(), store.clone(), "alice").await;
    let local = Fake::default();
    engine.sync_once(&local).await.unwrap();
    // Another device writes X at clock 100; this device has a newer pending write.
    let other = Uuid::new_v4();
    mock.put(
        alice,
        "filters",
        "X",
        Some(json!({"kind": "remote"})),
        100,
        other,
    );
    let clock = engine.next_clock();
    engine
        .enqueue(&[(
            "filters".into(),
            "X".into(),
            Some(json!({"kind": "local"})),
            clock,
        )])
        .unwrap();
    // Push fails once: the remote change is pulled while the local write is pending.
    mock.data.lock().unwrap().fail_push.push(503);
    let error = engine.sync_once(&local).await.unwrap_err();
    assert!(error.to_string().contains("Injected"), "{error}");
    assert_eq!(engine.status().state, RunState::Offline);
    assert_eq!(engine.status().pending, 1);
    assert!(engine.next_retry().unwrap() > vidcue_sync::now_ms());
    // Backoff: the op is deferred, so the next run pushes nothing and skips the
    // older remote value because the pending local write wins.
    engine.sync_once(&local).await.unwrap();
    assert!(local.value("filters", "X").is_none());
    assert_eq!(engine.status().pending, 1);
    let cursor = engine.state().cursor;
    assert!(cursor > 0);
    drop(engine);
    // Reopened: pulls from the persisted cursor, not from zero.
    let reopened = SyncEngine::open(dir.path(), store).unwrap().unwrap();
    assert_eq!(reopened.state().cursor, cursor);
    reopened.sync_once(&local).await.unwrap();
    assert!(
        mock.log()
            .iter()
            .any(|l| l == &format!("GET /api/sync/pull?since={cursor}"))
    );
    // Reopening reset the backoff: the local write reached the server and wins.
    assert_eq!(
        mock.get(alice, "filters", "X").unwrap().value.unwrap()["kind"],
        "local"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rejected_ops_are_counted_and_revocation_keeps_the_outbox_for_the_same_account() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    mock.add_user("bob", "pw");
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let engine = sign_in(&mock, dir.path(), store.clone(), "alice").await;
    let local = Fake::default();
    engine.sync_once(&local).await.unwrap();
    engine
        .enqueue(&[(
            "saved".into(),
            "REJECT".into(),
            Some(media("rrrrrrrrrrr")),
            engine.next_clock(),
        )])
        .unwrap();
    engine.sync_once(&local).await.unwrap();
    assert_eq!(engine.status().rejected_count, 1);
    assert_eq!(engine.status().pending, 0);
    let clock = engine.next_clock();
    engine
        .enqueue(&[(
            "saved".into(),
            "K".into(),
            Some(media("kkkkkkkkkkk")),
            clock,
        )])
        .unwrap();
    mock.revoke_all(alice);
    let error = engine.sync_once(&local).await.unwrap_err();
    assert!(error.to_string().contains("signed out"), "{error}");
    assert!(engine.is_signed_out());
    assert_eq!(engine.status().state, RunState::SignedOut);
    assert_eq!(engine.status().reason.as_deref(), Some("revoked"));
    assert!(store.load("session").unwrap().is_none());
    assert_eq!(
        SyncEngine::signed_out_reason(dir.path()).as_deref(),
        Some("revoked")
    );
    drop(engine);
    // Same account again: the queued write is pushed.
    let engine = sign_in(&mock, dir.path(), store.clone(), "alice").await;
    assert_eq!(engine.status().pending, 1);
    engine.sync_once(&local).await.unwrap();
    assert!(mock.get(alice, "saved", "K").is_some());
    let clock = engine.next_clock();
    engine
        .enqueue(&[(
            "saved".into(),
            "L".into(),
            Some(media("lllllllllll")),
            clock,
        )])
        .unwrap();
    drop(engine);
    // Another account: the previous account's queue is dropped.
    let engine = sign_in(&mock, dir.path(), store, "bob").await;
    assert_eq!(engine.status().pending, 0);
    assert!(!engine.state().first_sync_done);
}

#[tokio::test(flavor = "multi_thread")]
async fn key_relay_moves_the_key_and_unlocks_pending_secrets() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    let (dir_a, dir_b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let a = sign_in(
        &mock,
        dir_a.path(),
        Arc::new(MemoryStore::default()),
        "alice",
    )
    .await;
    let local_a = Fake::default();
    local_a.set("secrets", "youtube", json!({"cookies": ["synthetic"]}), 50);
    a.sync_once(&local_a).await.unwrap();
    let envelope = mock
        .get(alice, "secrets", "youtube")
        .unwrap()
        .value
        .unwrap();
    assert_eq!(envelope["alg"], "xchacha20poly1305");
    assert!(!envelope.to_string().contains("synthetic"));
    // Device B signs in with a password: no key, the secret waits encrypted.
    let b = sign_in(
        &mock,
        dir_b.path(),
        Arc::new(MemoryStore::default()),
        "alice",
    )
    .await;
    let local_b = Fake::default();
    b.sync_once(&local_b).await.unwrap();
    assert_eq!(b.key_state(), KeyState::Missing);
    assert!(local_b.value("secrets", "youtube").is_none());
    assert!(!b.state().pending_secrets.is_empty());
    // B cannot create a key check of its own nor push secrets.
    assert!(
        !b.enqueue_secret("youtube", &json!({"x": 1}), b.next_clock())
            .unwrap()
    );
    let request = b.key_request_start().await.unwrap();
    assert_eq!(b.key_state(), KeyState::Waiting);
    let fingerprint = request["fingerprint"].as_str().unwrap();
    assert_eq!(b.relay_fingerprint().as_deref(), Some(fingerprint));
    // A sees the same fingerprint and grants.
    let requests = a.key_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["fingerprint"], fingerprint);
    a.key_grant(requests[0]["id"].as_str().unwrap().parse().unwrap())
        .await
        .unwrap();
    assert!(b.key_request_poll().await.unwrap());
    assert_eq!(b.key_state(), KeyState::Ok);
    assert_eq!(b.account_key(), a.account_key());
    b.sync_once(&local_b).await.unwrap();
    assert_eq!(
        local_b.value("secrets", "youtube").unwrap(),
        json!({"cookies": ["synthetic"]})
    );
    // A pairing link from A carries the key in the fragment only.
    let pairing = a.pairing("pw").await.unwrap();
    let uri = pairing["uri"].as_str().unwrap();
    let parsed = vidcue_sync::pairing::parse_pairing_uri(uri).unwrap();
    assert_eq!(parsed.key, a.account_key());
    let dir_c = tempfile::tempdir().unwrap();
    let c = SyncEngine::claim(dir_c.path(), Arc::new(MemoryStore::default()), &parsed, "C")
        .await
        .unwrap();
    let local_c = Fake::default();
    // A link sign-in syncs nothing until confirmed, and the YouTube sign-in stays off
    // unless the user switches it on.
    assert!(c.awaiting_confirmation());
    assert!(!c.enabled("secrets"));
    let requests = mock.log().len();
    c.sync_once(&local_c).await.unwrap();
    assert_eq!(
        mock.log().len(),
        requests,
        "nothing sent before confirmation"
    );
    assert_eq!(c.state().cursor, 0);
    assert_eq!(c.status().last_success, 0);
    c.confirm_pairing(true).unwrap();
    assert!(!c.awaiting_confirmation());
    c.sync_once(&local_c).await.unwrap();
    assert_eq!(c.key_state(), KeyState::Ok);
    assert!(local_c.value("secrets", "youtube").is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wrong_pairing_key_is_a_mismatch_and_queues_no_secrets() {
    let mock = Mock::start().await;
    mock.add_user("alice", "pw");
    let dir_a = tempfile::tempdir().unwrap();
    let a = sign_in(
        &mock,
        dir_a.path(),
        Arc::new(MemoryStore::default()),
        "alice",
    )
    .await;
    a.sync_once(&Fake::default()).await.unwrap();
    let pairing = a.pairing("pw").await.unwrap();
    let mut parsed =
        vidcue_sync::pairing::parse_pairing_uri(pairing["uri"].as_str().unwrap()).unwrap();
    parsed.key = Some([9u8; 32]);
    let dir = tempfile::tempdir().unwrap();
    let c = SyncEngine::claim(dir.path(), Arc::new(MemoryStore::default()), &parsed, "C")
        .await
        .unwrap();
    c.confirm_pairing(true).unwrap();
    // Provisionally usable until the first pull contradicts it.
    let local = Fake::default();
    local.set("secrets", "youtube", json!({"c": 1}), 5);
    c.sync_once(&local).await.unwrap();
    assert_eq!(c.key_state(), KeyState::Mismatch);
    assert!(c.account_key().is_none());
    assert_eq!(c.status().pending, 0);
    assert!(c.adopt_key([9u8; 32]).is_err());
    assert!(c.adopt_key(a.account_key().unwrap()).is_ok());
    assert_eq!(c.key_state(), KeyState::Ok);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_secret_that_does_not_open_waits_for_the_right_key() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    let dir_a = tempfile::tempdir().unwrap();
    let a = sign_in(
        &mock,
        dir_a.path(),
        Arc::new(MemoryStore::default()),
        "alice",
    )
    .await;
    let local_a = Fake::default();
    local_a.set("secrets", "youtube", json!({"cookies": ["synthetic"]}), 50);
    a.sync_once(&local_a).await.unwrap();
    // The key check is written again after the session: a device holding a wrong key
    // meets the session before the check contradicts its key.
    let check = mock.get(alice, "secrets", "keycheck").unwrap();
    mock.put(
        alice,
        "secrets",
        "keycheck",
        check.value,
        check.clock,
        check.device,
    );
    let pairing = a.pairing("pw").await.unwrap();
    let mut parsed =
        vidcue_sync::pairing::parse_pairing_uri(pairing["uri"].as_str().unwrap()).unwrap();
    parsed.key = Some([9u8; 32]);
    let dir = tempfile::tempdir().unwrap();
    let c = SyncEngine::claim(dir.path(), Arc::new(MemoryStore::default()), &parsed, "C")
        .await
        .unwrap();
    c.confirm_pairing(true).unwrap();
    let local = Fake::default();
    c.sync_once(&local).await.unwrap();
    assert_eq!(c.key_state(), KeyState::Mismatch);
    assert!(local.value("secrets", "youtube").is_none());
    assert!(c.state().pending_secrets.contains_key("youtube"));
    c.adopt_key(a.account_key().unwrap()).unwrap();
    c.sync_once(&local).await.unwrap();
    assert_eq!(
        local.value("secrets", "youtube").unwrap(),
        json!({"cookies": ["synthetic"]})
    );
    assert!(c.state().pending_secrets.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn shared_playlists_push_and_pull_with_their_own_cursor() {
    let mock = Mock::start().await;
    mock.add_user("alice", "pw");
    let dir = tempfile::tempdir().unwrap();
    let engine = sign_in(&mock, dir.path(), Arc::new(MemoryStore::default()), "alice").await;
    let local = Fake::default();
    engine.sync_once(&local).await.unwrap();
    let playlist = Uuid::new_v4();
    engine
        .api()
        .shared_create(playlist, "Road trip")
        .await
        .unwrap();
    let entry = Uuid::new_v4();
    engine
        .enqueue_shared(
            playlist,
            entry,
            Some(json!({"position": "V", "media": media("sssssssssss")})),
            engine.next_clock(),
        )
        .unwrap();
    engine.sync_once(&local).await.unwrap();
    assert_eq!(local.listed.lock().unwrap()[0].title, "Road trip");
    let pulled = local.shared.lock().unwrap().clone();
    assert_eq!(pulled.len(), 1);
    assert_eq!(pulled[0].1[0].entry_id, entry);
    let cursor = engine.state().shared_cursors[&playlist];
    assert!(cursor > 0);
    // Nothing new: no second pull.
    engine.sync_once(&local).await.unwrap();
    assert_eq!(local.shared.lock().unwrap().len(), 1);
    let pulls = mock
        .log()
        .iter()
        .filter(|l| l.starts_with(&format!("GET /api/shared-playlists/{playlist}/pull")))
        .count();
    assert_eq!(pulls, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn health_reports_flush_only_when_enabled() {
    use vidcue_sync::protocol::{HealthCategory, HealthKind};
    let mock = Mock::start().await;
    mock.add_user("alice", "pw");
    let dir = tempfile::tempdir().unwrap();
    let engine = sign_in(&mock, dir.path(), Arc::new(MemoryStore::default()), "alice").await;
    assert!(
        !engine
            .record_health(
                HealthKind::Start,
                HealthCategory::Ok,
                "1.0",
                "android",
                Some(5)
            )
            .unwrap()
    );
    engine.set_settings(&BTreeMap::new(), Some(true)).unwrap();
    for _ in 0..150 {
        engine
            .record_health(
                HealthKind::Error,
                HealthCategory::Http403,
                "1.0",
                "android",
                None,
            )
            .unwrap();
    }
    engine.sync_once(&Fake::default()).await.unwrap();
    let reports = mock.data.lock().unwrap().health.clone();
    assert_eq!(reports.len(), 150);
    assert_eq!(
        reports[0],
        json!({"kind":"error","category":"http_403","client_version":"1.0","platform":"android","at":reports[0]["at"]})
    );
    // Disabling a collection drops its queued writes.
    engine
        .enqueue(&[(
            "saved".into(),
            "Z".into(),
            Some(media("zzzzzzzzzzz")),
            engine.next_clock(),
        )])
        .unwrap();
    assert_eq!(engine.status().pending, 1);
    engine
        .set_settings(&BTreeMap::from([("saved".to_owned(), false)]), None)
        .unwrap();
    assert_eq!(engine.status().pending, 0);
    assert!(!engine.enabled("saved"));
    assert!(engine.enabled("playlist_entries"));
}

#[tokio::test(flavor = "multi_thread")]
async fn websocket_parses_events_reconnects_and_stops_on_401() {
    let mock = Mock::start().await;
    let alice = mock.add_user("alice", "pw");
    let dir = tempfile::tempdir().unwrap();
    let engine = sign_in(&mock, dir.path(), Arc::new(MemoryStore::default()), "alice").await;
    let session = engine.session().clone();
    let client = WsClient::connect(&session.server, &session.token, session.device_id).unwrap();
    let mut state = client.state();
    let mut events = client.subscribe();
    let wait_state = |state: &mut tokio::sync::watch::Receiver<WsState>, want: WsState| {
        let mut state = state.clone();
        async move {
            tokio::time::timeout(Duration::from_secs(10), state.wait_for(|s| *s == want))
                .await
                .expect("state")
                .unwrap();
        }
    };
    wait_state(&mut state, WsState::Connected).await;
    assert!(client.realtime());
    assert!(matches!(
        events.recv().await.unwrap(),
        ServerEvent::Hello { .. }
    ));
    // Request/reply through rid.
    let pong = client
        .request(&vidcue_sync::protocol::ClientMessage::Ping { t0: 7 })
        .await
        .unwrap();
    assert_eq!(pong["t0"], 7);
    assert!(pong["server_ms"].as_i64().unwrap() > 0);
    // Events from the hub arrive typed; unknown ones are kept.
    mock.hub
        .send((
            alice,
            json!({"type": "jam_ended", "room_id": Uuid::nil(), "reason": "ended"}),
        ))
        .unwrap();
    mock.hub
        .send((alice, json!({"type": "brand_new", "x": 1})))
        .unwrap();
    assert!(matches!(
        events.recv().await.unwrap(),
        ServerEvent::JamEnded { .. }
    ));
    assert!(matches!(
        events.recv().await.unwrap(),
        ServerEvent::Other(_)
    ));
    // The server drops the socket: the client reconnects after backoff (~1 s).
    mock.hub
        .send((alice, json!({"type": "close_all"})))
        .unwrap();
    wait_state(&mut state, WsState::Disconnected).await;
    wait_state(&mut state, WsState::Connected).await;
    assert!(matches!(
        events.recv().await.unwrap(),
        ServerEvent::Hello { .. }
    ));
    // Revoked token: 401 on the next upgrade means signed out, no reconnect loop.
    mock.revoke_all(alice);
    mock.hub
        .send((alice, json!({"type": "close_all"})))
        .unwrap();
    wait_state(&mut state, WsState::SignedOut).await;
    assert!(
        client
            .request(&vidcue_sync::protocol::ClientMessage::PresenceQuery)
            .await
            .is_err()
    );
}
