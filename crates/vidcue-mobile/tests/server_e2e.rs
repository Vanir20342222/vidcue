//! Opt-in end-to-end run against the real `vidcue-server` binary and a disposable
//! PostgreSQL database. Skipped unless both are given:
//!
//! ```sh
//! cargo build -p vidcue-server
//! VIDCUE_SERVER_BIN=target/debug/vidcue-server \
//! VIDCUE_TEST_DATABASE_URL=postgres://user:pass@127.0.0.1/db \
//!   cargo test -p vidcue-mobile --test server_e2e
//! ```
//!
//! Everything stays on 127.0.0.1 with synthetic accounts and cookies; no provider is
//! contacted. It proves that the values this client pushes pass the server's
//! validation (`rejected_count` stays 0) and that the flows work end to end.
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use std::{process::Stdio, sync::Arc, time::Duration};
use vidcue_mobile::Engine;

struct Server {
    origin: String,
    _child: tokio::process::Child,
    _dir: tempfile::TempDir,
    bin: String,
    env: Vec<(String, String)>,
}

impl Server {
    async fn start() -> Option<Self> {
        let (Ok(bin), Ok(database)) = (
            std::env::var("VIDCUE_SERVER_BIN"),
            std::env::var("VIDCUE_TEST_DATABASE_URL"),
        ) else {
            eprintln!("skipped: set VIDCUE_SERVER_BIN and VIDCUE_TEST_DATABASE_URL");
            return None;
        };
        let dir = tempfile::tempdir().unwrap();
        let key = std::process::Command::new(&bin)
            .arg("gen-key")
            .output()
            .unwrap();
        let key_file = dir.path().join("secret.key");
        std::fs::write(&key_file, &key.stdout).unwrap();
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let origin = format!("http://127.0.0.1:{port}");
        let env = vec![
            ("DATABASE_URL".to_owned(), database),
            ("VIDCUE_PUBLIC_URL".to_owned(), origin.clone()),
            (
                "VIDCUE_SECRET_KEY_FILE".to_owned(),
                key_file.display().to_string(),
            ),
            ("VIDCUE_RECS".to_owned(), "off".to_owned()),
            ("RUST_LOG".to_owned(), "warn".to_owned()),
        ];
        let child = tokio::process::Command::new(&bin)
            .args(["serve", "--bind", &format!("127.0.0.1:{port}")])
            .env_remove("CREDENTIALS_DIRECTORY")
            .envs(env.iter().cloned())
            .stdout(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let client = reqwest::Client::new();
        for _ in 0..300 {
            if client
                .get(format!("{origin}/healthz"))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                return Some(Self {
                    origin,
                    _child: child,
                    _dir: dir,
                    bin,
                    env,
                });
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("vidcue-server did not start");
    }

    /// Creates an account through the invite and signup API; returns its recovery codes.
    async fn account(&self, username: &str, password: &str) -> Vec<String> {
        let invite = std::process::Command::new(&self.bin)
            .args(["invite", "--admin"])
            .env_remove("CREDENTIALS_DIRECTORY")
            .envs(self.env.iter().cloned())
            .output()
            .unwrap();
        let link = String::from_utf8(invite.stdout).unwrap();
        let token = link.trim().rsplit_once('#').unwrap().1.to_owned();
        let client = reqwest::Client::new();
        let signup: Value = client
            .post(format!("{}/api/signup", self.origin))
            .json(&json!({"invite": token, "username": username, "password": password}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let secret = BASE32_NOPAD
            .decode(signup["totp_secret"].as_str().unwrap().as_bytes())
            .unwrap();
        let confirm: Value = client
            .post(format!("{}/api/signup/confirm", self.origin))
            .json(
                &json!({"invite": token, "signup_id": signup["signup_id"], "code": totp(&secret)}),
            )
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        serde_json::from_value(confirm["recovery_codes"].clone()).unwrap()
    }
}

fn totp(secret: &[u8]) -> String {
    let step = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 30;
    let mut mac = Hmac::<sha1::Sha1>::new_from_slice(secret).unwrap();
    mac.update(&step.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let bin = u32::from_be_bytes([
        digest[offset] & 0x7f,
        digest[offset + 1],
        digest[offset + 2],
        digest[offset + 3],
    ]);
    format!("{:06}", bin % 1_000_000)
}

struct Device {
    engine: Arc<Engine>,
    _root: tempfile::TempDir,
}

impl Device {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let engine =
            Arc::new(Engine::new(&root.path().join("files"), &root.path().join("cache")).unwrap());
        engine.start_sync();
        Self {
            engine,
            _root: root,
        }
    }
    async fn call(&self, request: Value) -> Value {
        self.engine
            .request(request.clone())
            .await
            .unwrap_or_else(|e| panic!("{request}: {e:#}"))
    }
}

fn media(id: &str) -> Value {
    json!({"id": id, "title": format!("Song {id}"), "creator": "Band", "url": format!("https://www.youtube.com/watch?v={id}"),
           "thumbnail": "https://i.ytimg.com/vi/x/hq.jpg", "thumbnails": [], "section": "", "duration": 200,
           "description": "long text", "kind": "song", "channel_id": "UCaaaaaaaaaaaaaaaaaaaaaa",
           "browse_id": null, "tracks_url": null, "live": false, "chapters": []})
}

async fn eventually(mut check: impl AsyncFnMut() -> bool) {
    for _ in 0..200 {
        if check().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("condition not reached");
}

fn clean(status: &Value) {
    assert_eq!(
        status["rejected_count"], 0,
        "server rejected a value: {status}"
    );
    assert!(status["notice"].is_null(), "{status}");
}

#[tokio::test(flavor = "multi_thread")]
async fn engine_against_the_real_server() {
    let Some(server) = Server::start().await else {
        return;
    };
    let user = format!("e2e{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
    let password = "synthetic password 42";
    let codes = server.account(&user, password).await;
    let sign_in = |code: &str| json!({"op": "sync_sign_in", "server": server.origin, "username": user, "password": password, "code": code, "device_name": "E2E"});
    let a = Device::new();
    a.call(sign_in(&codes[0])).await;
    let status = a.call(json!({"op": "sync_now"})).await;
    clean(&status);
    assert_eq!(status["key_state"], "ok");
    assert!(
        status["features"]
            .as_array()
            .unwrap()
            .contains(&json!("health"))
    );
    // Library writes of every collection pass the server's validation.
    let song = media("abcdefghijk");
    a.call(json!({"op": "save_media", "media": song})).await;
    a.call(json!({"op": "history_record", "media": song})).await;
    a.call(
        json!({"op": "position", "url": song["url"], "milliseconds": 90000, "duration_ms": 200000}),
    )
    .await;
    let playlist = a
        .call(json!({"op": "playlist_create", "title": "Mix", "media": media("bbbbbbbbbbb")}))
        .await;
    a.call(json!({"op": "follow_toggle", "channel_id": "UCbbbbbbbbbbbbbbbbbbbbbb", "title": "Band", "thumbnail": "https://yt3.ggpht.com/x"}))
        .await;
    a.call(json!({"op": "filter_add", "kind": "keyword", "value": "trailer"}))
        .await;
    a.call(json!({"op": "filter_add", "kind": "shorts"})).await;
    let mut config = a.call(json!({"op": "settings"})).await;
    config["theme"] = json!("vidcue-light");
    config["equalizer_custom"] =
        json!({"mine": [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0]});
    config["equalizer"] = json!("mine");
    a.call(json!({"op": "save_settings", "config": config}))
        .await;
    let mut item = song.clone();
    item["offline_id"] = json!("a".repeat(64));
    a.call(json!({"op": "offline_playlist_create", "title": "Trip", "items": [item]}))
        .await;
    a.call(json!({"op": "import_cookies", "header": "SAPISID=synthetic-e2e; SID=synthetic-only"}))
        .await;
    let status = a.call(json!({"op": "sync_now"})).await;
    clean(&status);
    assert_eq!(status["pending"], 0, "{status}");
    // A second device signed in with a password gets everything but the session.
    let b = Device::new();
    b.call(sign_in(&codes[1])).await;
    let status = b.call(json!({"op": "sync_now"})).await;
    clean(&status);
    assert_eq!(status["key_state"], "missing");
    assert_eq!(b.call(json!({"op": "saved"})).await[0]["url"], song["url"]);
    assert_eq!(b.call(json!({"op": "follows"})).await[0]["title"], "Band");
    assert_eq!(
        b.call(json!({"op": "filters"}))
            .await
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        b.call(json!({"op": "playlists"})).await[0]["id"],
        playlist["id"]
    );
    let settings = b.call(json!({"op": "settings"})).await;
    assert_eq!(settings["theme"], "vidcue-light");
    assert_eq!(settings["equalizer"], "mine");
    assert_eq!(
        b.call(json!({"op": "position", "url": song["url"]})).await["milliseconds"],
        90000
    );
    assert_eq!(
        b.call(json!({"op": "offline_playlists"})).await[0]["title"],
        "Trip"
    );
    assert_eq!(b.call(json!({"op": "account"})).await["connected"], false);
    // Key relay: fingerprints match on both sides, then the session unlocks.
    let request = b.call(json!({"op": "key_request_start"})).await;
    let requests = a.call(json!({"op": "key_requests"})).await;
    assert_eq!(
        requests["requests"][0]["fingerprint"],
        request["fingerprint"]
    );
    a.call(json!({"op": "key_grant", "id": requests["requests"][0]["id"]}))
        .await;
    eventually(async || b.call(json!({"op": "key_fingerprint"})).await["state"] == "ok").await;
    b.call(json!({"op": "sync_now"})).await;
    eventually(async || b.call(json!({"op": "account"})).await["connected"] == true).await;
    // QR pairing carries the key directly.
    let pairing = a
        .call(json!({"op": "sync_pair_start", "password": password}))
        .await;
    let c = Device::new();
    c.call(json!({"op": "sync_pair_claim", "uri": pairing["uri"], "device_name": "Paired"}))
        .await;
    let status = c.call(json!({"op": "sync_now"})).await;
    clean(&status);
    assert_eq!(status["key_state"], "ok");
    assert_eq!(c.call(json!({"op": "account"})).await["connected"], true);
    // Realtime: presence and handoff between A and B over the real WebSocket.
    for device in [&a, &b] {
        device
            .call(json!({"op": "sync_foreground", "visible": true}))
            .await;
        eventually(async || {
            let status = device.call(json!({"op": "sync_status"})).await;
            status["connected"] == true
                && status["features"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("realtime"))
        })
        .await;
    }
    let b_id = b.call(json!({"op": "sync_status"})).await["device_id"].clone();
    b.engine.take_sync_events();
    a.call(json!({"op": "handoff_send", "to_device": b_id, "media": song, "position_ms": 4000, "playing": true, "queue": [media("ccccccccccc")]}))
        .await;
    eventually(async || {
        b.engine
            .take_sync_events()
            .iter()
            .any(|(k, t)| k == "handoff_received" && t.contains("ccccccccccc"))
    })
    .await;
    a.call(json!({"op": "presence_update", "media": song, "playing": true, "position_ms": 1000, "duration_ms": 200000}))
        .await;
    eventually(async || {
        b.call(json!({"op": "presence_devices"})).await["devices"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["state"]["media"]["url"] == song["url"])
    })
    .await;
    // Jam clock against the server's room.
    let room = a.call(json!({"op": "jam_create", "title": "E2E"})).await;
    a.call(json!({"op": "jam_queue_add", "room_id": room["id"], "media": song}))
        .await;
    // The first queued item starts at once; the room state arrives as `jam_state`.
    eventually(async || {
        let target = a.call(json!({"op": "jam_target"})).await;
        target["in_room"] == true && target["media_key"] == "youtube:abcdefghijk"
    })
    .await;
    let paused = a
        .call(json!({"op": "jam_control", "room_id": room["id"], "action": "pause", "position_ms": 3000}))
        .await;
    assert_eq!(paused["state"], "paused");
    assert_eq!(a.call(json!({"op": "jam_target"})).await["target_ms"], 3000);
    a.call(json!({"op": "jam_end", "room_id": room["id"]}))
        .await;
    // Personal services and admin passthrough.
    b.call(json!({"op": "sync_settings", "health_reports": true}))
        .await;
    b.call(json!({"op": "health_record", "kind": "start", "duration_ms": 700}))
        .await;
    b.call(
        json!({"op": "health_record", "kind": "error", "error_text": "HTTP Error 403: Forbidden"}),
    )
    .await;
    clean(&b.call(json!({"op": "sync_now"})).await);
    let summary = b.call(json!({"op": "health_summary", "days": 1})).await;
    assert_eq!(summary["total"], 2, "{summary}");
    let now = vidcue_sync::now_ms();
    let stats = a
        .call(json!({"op": "stats", "from": now - 86_400_000, "to": now + 60_000, "tz_offset_min": 60}))
        .await;
    assert!(stats["plays"].is_number(), "{stats}");
    assert!(a.call(json!({"op": "friends"})).await["friends"].is_array());
    assert!(a.call(json!({"op": "admin_users"})).await["users"].is_array());
    let invite = a
        .call(json!({"op": "admin_invite", "password": password, "admin": false, "hours": 1}))
        .await;
    assert!(invite["qr"].is_array() && invite.get("qr_svg").is_none());
    // Revoking B from A signs B out with reason `revoked`; its library stays.
    a.call(json!({"op": "sync_revoke", "device_id": b_id}))
        .await;
    eventually(async || {
        let status = b.call(json!({"op": "sync_status"})).await;
        status["signed_in"] == false && status["reason"] == "revoked"
    })
    .await;
    assert_eq!(b.call(json!({"op": "saved"})).await[0]["url"], song["url"]);
}
