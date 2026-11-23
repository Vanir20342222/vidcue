//! End-to-end API flows against a real PostgreSQL database.
//!
//! Set `VIDCUE_TEST_DATABASE_URL` to a disposable database; the tests drop and
//! recreate its `public` schema. Without it they are skipped so `cargo test` stays offline.

use crate::{api, auth, db, secrets::ServerKey};
use axum::body::Body;
use axum::extract::connect_info::ConnectInfo;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::net::SocketAddr;
use tower::ServiceExt;

#[derive(Clone)]
struct Client {
    app: axum::Router,
    /// Simulated client address forwarded by the trusted proxy (X-Real-IP).
    ip: Option<String>,
    /// Simulated TCP peer; `None` behaves like loopback.
    peer: Option<SocketAddr>,
}

impl Client {
    fn from(&self, ip: &str) -> Self {
        Self {
            ip: Some(ip.into()),
            ..self.clone()
        }
    }
    async fn call(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            req = req.header("authorization", format!("Bearer {token}"));
        }
        if let Some(ip) = &self.ip {
            req = req.header("x-real-ip", ip);
        }
        let mut req = match body {
            Some(b) => req
                .header("content-type", "application/json")
                .body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
        }
        .unwrap();
        if let Some(peer) = self.peer {
            req.extensions_mut().insert(ConnectInfo(peer));
        }
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, value)
    }
    async fn post(&self, path: &str, token: Option<&str>, body: Value) -> (StatusCode, Value) {
        self.call(Method::POST, path, token, Some(body)).await
    }
    async fn get(&self, path: &str, token: Option<&str>) -> (StatusCode, Value) {
        self.call(Method::GET, path, token, None).await
    }
}

fn code_for(secret_b32: &str, offset_steps: i64) -> String {
    let secret = data_encoding::BASE32_NOPAD
        .decode(secret_b32.as_bytes())
        .unwrap();
    let step = (api::now_ms() / 1000 / auth::TOTP_STEP_SECONDS as i64 + offset_steps) as u64;
    format!("{:06}", auth::totp_at_step(&secret, step))
}

fn position(video: &str, seconds: f64) -> Value {
    json!({
        "media": {"id": video, "title": "Song", "url": format!("https://www.youtube.com/watch?v={video}")},
        "seconds": seconds, "duration": 600.0, "updated_at": 1, "finished": false
    })
}

/// All database tests reset the same schema; they must not overlap.
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn fresh_db() -> Option<(
    deadpool_postgres::Pool,
    tokio::sync::MutexGuard<'static, ()>,
)> {
    let Ok(url) = std::env::var("VIDCUE_TEST_DATABASE_URL") else {
        eprintln!("skipped: VIDCUE_TEST_DATABASE_URL is not set");
        return None;
    };
    let guard = DB_LOCK.lock().await;
    let pool = db::connect(&url).unwrap();
    pool.get()
        .await
        .unwrap()
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await
        .unwrap();
    db::migrate(&pool).await.unwrap();
    db::migrate(&pool).await.unwrap(); // idempotent
    Some((pool, guard))
}

fn client(pool: &deadpool_postgres::Pool) -> Client {
    let state = api::AppState::new(
        pool.clone(),
        api::Settings {
            public_url: "https://vidcue.test".into(),
            key: ServerKey::from_master([9; 32]),
            trusted_proxies: vec!["127.0.0.1".parse().unwrap()],
            proxies_only: true,
        },
    );
    Client {
        app: api::router(state),
        ip: None,
        peer: None,
    }
}

async fn clear_throttles(pool: &deadpool_postgres::Pool) {
    pool.get()
        .await
        .unwrap()
        .execute("DELETE FROM auth_failures", &[])
        .await
        .unwrap();
}

fn fragment(link: &str) -> String {
    link.split_once('#').unwrap().1.to_owned()
}

struct Account {
    token: String,
    secret: String,
    recovery: Vec<String>,
}

async fn register(c: &Client, pool: &deadpool_postgres::Pool, name: &str, admin: bool) -> Account {
    let (link, _) = api::create_invite(pool, "https://vidcue.test", None, admin, 1, api::now_ms())
        .await
        .unwrap();
    let invite = fragment(&link);
    let (status, s) = c
        .post(
            "/api/signup",
            None,
            json!({"invite": invite, "username": name, "password": "first password!"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    let secret = s["totp_secret"].as_str().unwrap().to_owned();
    let (status, d) = c
        .post(
            "/api/signup/confirm",
            None,
            json!({"invite": invite, "signup_id": s["signup_id"], "code": code_for(&secret, -1), "device_name": "first"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{d}");
    Account {
        token: d["token"].as_str().unwrap().to_owned(),
        secret,
        recovery: serde_json::from_value(d["recovery_codes"].clone()).unwrap(),
    }
}

#[tokio::test]
async fn invite_signup_login_pairing_and_sync() {
    let Some((pool, _guard)) = fresh_db().await else {
        return;
    };
    let now = api::now_ms();
    let (link, _) = api::create_invite(&pool, "https://vidcue.test", None, true, 1, now)
        .await
        .unwrap();
    assert!(
        link.starts_with("https://vidcue.test/join#vci_"),
        "token in fragment: {link}"
    );
    let invite = fragment(&link);
    let c = client(&pool);

    // The join page carries no token; the script reads it from the fragment.
    let (status, page) = c.get("/join", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.as_str().unwrap().contains("location.hash"));

    // Signup validates input, then requires a valid authenticator code.
    let pw = "long enough pw";
    assert_eq!(
        c.post(
            "/api/signup",
            None,
            json!({"invite": invite, "username": "x", "password": pw})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        c.post(
            "/api/signup",
            None,
            json!({"invite": "vci_bogus", "username": "alice", "password": pw})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, signup) = c
        .post(
            "/api/signup",
            None,
            json!({"invite": invite, "username": "Alice", "password": pw}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{signup}");
    assert!(signup["totp_qr_svg"].as_str().unwrap().contains("<svg"));
    let secret = signup["totp_secret"].as_str().unwrap().to_owned();
    let id = signup["signup_id"].clone();
    assert_eq!(
        c.post(
            "/api/signup/confirm",
            None,
            json!({"invite": invite, "signup_id": id, "code": "000000"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, done) = c
        .post(
            "/api/signup/confirm",
            None,
            json!({"invite": invite, "signup_id": id, "code": code_for(&secret, -1), "device_name": "Tab S9"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    let tablet = done["token"].as_str().unwrap().to_owned();

    // The authenticator seed is stored encrypted, bound to the user.
    let stored: Vec<u8> = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT totp_secret FROM users WHERE username = 'alice'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert!(ServerKey::is_sealed(&stored));
    assert_ne!(
        stored,
        data_encoding::BASE32_NOPAD
            .decode(secret.as_bytes())
            .unwrap()
    );

    // The invite is spent once the account is active.
    assert_eq!(
        c.post(
            "/api/signup",
            None,
            json!({"invite": invite, "username": "mallory", "password": pw})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );

    // Login needs password and a fresh code; the code used at signup is a replay.
    let login = |password: &str, code: String, name: &str| json!({"username": "alice", "password": password, "code": code, "device_name": name});
    assert_eq!(
        c.post(
            "/api/login",
            None,
            login("wrong password", code_for(&secret, 0), "x")
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        c.post("/api/login", None, login(pw, code_for(&secret, -1), "x"))
            .await
            .0,
        StatusCode::UNAUTHORIZED,
        "replayed code"
    );
    let (status, phone) = c
        .post("/api/login", None, login(pw, code_for(&secret, 0), "Phone"))
        .await;
    assert_eq!(status, StatusCode::OK, "{phone}");
    let phone = phone["token"].as_str().unwrap().to_owned();

    let (status, me) = c.get("/api/me", Some(&phone)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["username"], "alice");
    assert_eq!(me["is_admin"], true);
    assert_eq!(
        c.get("/api/me", Some("vcd_nope")).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(c.get("/api/me", None).await.0, StatusCode::UNAUTHORIZED);

    // QR pairing needs the password, is single use, and records the parent device.
    assert_eq!(
        c.post(
            "/api/pairing",
            Some(&phone),
            json!({"password": "wrong password"})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, pairing) = c
        .post("/api/pairing", Some(&phone), json!({"password": pw}))
        .await;
    assert_eq!(status, StatusCode::OK, "{pairing}");
    assert!(
        pairing["uri"]
            .as_str()
            .unwrap()
            .starts_with("vidcue://pair?server=https%3A%2F%2Fvidcue.test&code=")
    );
    let code = pairing["code"].as_str().unwrap().to_lowercase();
    let (status, claimed) = c
        .post(
            "/api/pairing/claim",
            None,
            json!({"code": code, "device_name": "TV"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{claimed}");
    let tv = claimed["token"].as_str().unwrap().to_owned();
    assert_eq!(
        c.post("/api/pairing/claim", None, json!({"code": code}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (_, devices) = c.get("/api/devices", Some(&tv)).await;
    assert_eq!(devices["devices"].as_array().unwrap().len(), 3);

    // Offline-first sync: the later clock wins regardless of arrival order.
    let key = "youtube:abcdefghijk";
    let early = uuid::Uuid::new_v4();
    let (status, pushed) = c
        .post(
            "/api/sync/push",
            Some(&phone),
            json!({"ops": [
                {"id": uuid::Uuid::new_v4(), "collection": "positions", "key": key, "value": position("abcdefghijk", 120.0), "clock": now + 2000},
                {"id": uuid::Uuid::new_v4(), "collection": "positions", "key": "/tmp/x", "value": position("abcdefghijk", 1.0), "clock": now},
                {"id": uuid::Uuid::new_v4(), "collection": "history", "key": "evt-1", "value": {"url": "https://youtu.be/abcdefghijk", "at": now}, "clock": now},
                {"id": uuid::Uuid::new_v4(), "collection": "history", "key": "evt-2", "value": {"url": "https://youtu.be/x", "cookie": "SID=1"}, "clock": now},
                {"id": uuid::Uuid::new_v4(), "collection": "history", "key": "evt\u{0}3", "value": {"url": "https://youtu.be/x"}, "clock": now},
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{pushed}");
    let statuses: Vec<&str> = pushed["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["status"].as_str().unwrap())
        .collect();
    assert_eq!(
        statuses,
        ["applied", "rejected", "applied", "rejected", "rejected"]
    );
    // The tablet was offline and changed the same position earlier.
    let stale = json!({"ops": [{"id": early, "collection": "positions", "key": key, "value": position("abcdefghijk", 30.0), "clock": now + 1000}]});
    let (_, pushed) = c.post("/api/sync/push", Some(&tablet), stale.clone()).await;
    assert_eq!(pushed["results"][0]["status"], "superseded");
    let (_, pushed) = c.post("/api/sync/push", Some(&tablet), stale).await;
    assert_eq!(pushed["results"][0]["status"], "duplicate");

    let (status, pulled) = c.get("/api/sync/pull?since=0", Some(&tv)).await;
    assert_eq!(status, StatusCode::OK);
    let changes = pulled["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 2);
    let pos = changes
        .iter()
        .find(|c| c["collection"] == "positions")
        .unwrap();
    assert_eq!(pos["value"]["seconds"], 120.0);
    let next = pulled["next"].as_i64().unwrap();
    let (_, empty) = c
        .get(&format!("/api/sync/pull?since={next}"), Some(&tv))
        .await;
    assert!(empty["changes"].as_array().unwrap().is_empty());

    // Delete is a tombstone that other devices receive.
    let (_, pushed) = c
        .post(
            "/api/sync/push",
            Some(&tablet),
            json!({"ops": [{"id": uuid::Uuid::new_v4(), "collection": "positions", "key": key, "clock": now + 3000}]}),
        )
        .await;
    assert_eq!(pushed["results"][0]["status"], "applied");
    let (_, pulled) = c
        .get(&format!("/api/sync/pull?since={next}"), Some(&tv))
        .await;
    assert_eq!(pulled["changes"][0]["value"], Value::Null);

    // Pagination.
    let ops: Vec<Value> = (0..5)
        .map(|i| json!({"id": uuid::Uuid::new_v4(), "collection": "history", "key": format!("p{i}"), "value": {"url": "https://youtu.be/x"}, "clock": now}))
        .collect();
    c.post("/api/sync/push", Some(&phone), json!({ "ops": ops }))
        .await;
    let (_, page) = c.get("/api/sync/pull?since=0&limit=3", Some(&tv)).await;
    assert_eq!(page["changes"].as_array().unwrap().len(), 3);
    assert_eq!(page["more"], true);

    // Admin invites need the password; users never see each other's records.
    assert_eq!(
        c.post("/api/admin/invites", Some(&phone), json!({"hours": 1}))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, inv) = c
        .post(
            "/api/admin/invites",
            Some(&phone),
            json!({"hours": 1, "password": pw}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{inv}");
    let bob_invite = fragment(inv["url"].as_str().unwrap());
    let (_, s) = c
        .post(
            "/api/signup",
            None,
            json!({"invite": bob_invite, "username": "bob", "password": "bobs long password"}),
        )
        .await;
    let (_, d) = c
        .post(
            "/api/signup/confirm",
            None,
            json!({"invite": bob_invite, "signup_id": s["signup_id"], "code": code_for(s["totp_secret"].as_str().unwrap(), 0), "device_name": "Bob phone"}),
        )
        .await;
    let bob = d["token"].as_str().unwrap().to_owned();
    let (_, pulled) = c.get("/api/sync/pull?since=0", Some(&bob)).await;
    assert!(pulled["changes"].as_array().unwrap().is_empty());
    assert_eq!(
        c.post(
            "/api/admin/invites",
            Some(&bob),
            json!({"password": "bobs long password"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (_, users) = c.get("/api/admin/users", Some(&phone)).await;
    assert_eq!(users["users"].as_array().unwrap().len(), 2);

    // Revoking a device also revokes the devices it paired.
    let phone_id = c.get("/api/me", Some(&phone)).await.1["device_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        c.call(
            Method::DELETE,
            &format!("/api/devices/{phone_id}"),
            Some(&bob),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, revoked) = c
        .call(
            Method::DELETE,
            &format!("/api/devices/{phone_id}"),
            Some(&tablet),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        revoked["revoked"].as_array().unwrap().len(),
        2,
        "phone and the TV it paired"
    );
    assert_eq!(
        c.get("/api/me", Some(&phone)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        c.get("/api/me", Some(&tv)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(c.get("/api/me", Some(&tablet)).await.0, StatusCode::OK);
}

#[tokio::test]
async fn throttles_are_race_free_per_address_and_proxy_only() {
    let Some((pool, _guard)) = fresh_db().await else {
        return;
    };
    let c = client(&pool);
    let bob = register(&c, &pool, "bob", false).await;
    let bad = json!({"username": "bob", "password": "nope nope nope", "code": "123456"});

    // 20 parallel wrong logins from one address: at most 5 reach the password check.
    let attacker = c.from("203.0.113.7");
    let tasks: Vec<_> = (0..20)
        .map(|_| {
            let (a, b) = (attacker.clone(), bad.clone());
            tokio::spawn(async move { a.post("/api/login", None, b).await.0 })
        })
        .collect();
    let mut checked = 0;
    for t in tasks {
        match t.await.unwrap() {
            StatusCode::UNAUTHORIZED => checked += 1,
            StatusCode::TOO_MANY_REQUESTS => {}
            other => panic!("unexpected {other}"),
        }
    }
    assert_eq!(checked, 5, "the limit holds under concurrency");

    // The same user still signs in from another address.
    let (status, ok) = c
        .from("198.51.100.4")
        .post(
            "/api/login",
            None,
            json!({"username": "bob", "password": "first password!", "code": code_for(&bob.secret, 0)}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{ok}");

    // Throttling survives a restart (a new AppState on the same database).
    assert_eq!(
        client(&pool)
            .from("203.0.113.7")
            .post("/api/login", None, bad.clone())
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS
    );

    // Pairing-code guessing from one address does not block other addresses.
    for _ in 0..30 {
        attacker
            .post("/api/pairing/claim", None, json!({"code": "AAAAAAAAAA"}))
            .await;
    }
    assert_eq!(
        attacker
            .post("/api/pairing/claim", None, json!({"code": "AAAAAAAAAA"}))
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        c.from("198.51.100.4")
            .post("/api/pairing/claim", None, json!({"code": "AAAAAAAAAA"}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );

    // Direct connections from anything but the proxy are refused.
    let direct = Client {
        peer: Some("192.168.2.77:5555".parse().unwrap()),
        ..c.clone()
    };
    assert_eq!(
        direct.get("/api/me", Some(&bob.token)).await.0,
        StatusCode::FORBIDDEN
    );
    let via_proxy = Client {
        peer: Some("127.0.0.1:5555".parse().unwrap()),
        ..c.clone()
    };
    assert_eq!(
        via_proxy.get("/api/me", Some(&bob.token)).await.0,
        StatusCode::OK
    );

    // A stolen token cannot plant a download hook or folder on other devices.
    let now = api::now_ms();
    let (_, pushed) = c
        .post(
            "/api/sync/push",
            Some(&bob.token),
            json!({"ops": [
                {"id": uuid::Uuid::new_v4(), "collection": "settings", "key": "download", "value": {"post_download_hook": "/tmp/x.sh"}, "clock": now},
                {"id": uuid::Uuid::new_v4(), "collection": "settings", "key": "download", "value": {"directory": "/tmp"}, "clock": now},
                {"id": uuid::Uuid::new_v4(), "collection": "settings", "key": "radio_servers", "value": ["https://evil.example"], "clock": now},
                {"id": uuid::Uuid::new_v4(), "collection": "settings", "key": "download", "value": {"quality": "720", "audio": true}, "clock": now},
            ]}),
        )
        .await;
    let statuses: Vec<&str> = pushed["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["status"].as_str().unwrap())
        .collect();
    assert_eq!(
        statuses,
        ["rejected", "rejected", "rejected", "applied"],
        "{pushed}"
    );
    let (_, pulled) = c.get("/api/sync/pull?since=0", Some(&bob.token)).await;
    assert_eq!(
        pulled["changes"][0]["value"],
        json!({"audio": true, "quality": "720"})
    );

    // Recovery codes are keyed HMACs, not plain hashes of the code.
    let stored: Vec<Vec<u8>> = pool
        .get()
        .await
        .unwrap()
        .query("SELECT code_hash FROM recovery_codes", &[])
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    let plain = auth::token_hash(&auth::normalize_recovery_code(&bob.recovery[0]));
    assert_eq!(stored.len(), 10);
    assert!(!stored.contains(&plain));

    // Maintenance runs cleanly.
    api::maintenance(&pool, api::now_ms()).await.unwrap();
}

#[tokio::test]
async fn recovery_reset_key_relay_and_library_sync() {
    let Some((pool, _guard)) = fresh_db().await else {
        return;
    };
    let c = client(&pool);
    let alice = register(&c, &pool, "alice", true).await;
    assert_eq!(alice.recovery.len(), crate::account::RECOVERY_CODE_COUNT);
    let login = |password: &str, code: String| json!({"username": "alice", "password": password, "code": code, "device_name": "other"});

    // Recovery codes replace the authenticator once each.
    let (_, status) = c
        .get("/api/account/recovery-codes", Some(&alice.token))
        .await;
    assert_eq!(status["remaining"], 10);
    let rc = alice.recovery[0].to_lowercase();
    let (status, second) = c
        .post("/api/login", None, login("first password!", rc.clone()))
        .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    let second = second["token"].as_str().unwrap().to_owned();
    assert_eq!(
        c.post("/api/login", None, login("first password!", rc))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (_, status) = c
        .get("/api/account/recovery-codes", Some(&alice.token))
        .await;
    assert_eq!(status["remaining"], 9);
    clear_throttles(&pool).await;

    // Password change needs password and a current code, and can sign out others.
    let change = |current: &str, code: String, new: &str| json!({"current_password": current, "code": code, "new_password": new, "sign_out_others": true});
    assert_eq!(
        c.post(
            "/api/account/password",
            Some(&alice.token),
            change(
                "nope nope nope",
                code_for(&alice.secret, 0),
                "second password!"
            )
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        c.post(
            "/api/account/password",
            Some(&alice.token),
            change("first password!", "000000".into(), "second password!")
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED,
        "password alone is not enough"
    );
    assert_eq!(
        c.post(
            "/api/account/password",
            Some(&alice.token),
            change("first password!", code_for(&alice.secret, 0), "short")
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, changed) = c
        .post(
            "/api/account/password",
            Some(&alice.token),
            change(
                "first password!",
                alice.recovery[1].clone(),
                "second password!",
            ),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert_eq!(changed["revoked_devices"], 1);
    assert_eq!(
        c.get("/api/me", Some(&second)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(c.get("/api/me", Some(&alice.token)).await.0, StatusCode::OK);
    clear_throttles(&pool).await;

    // Authenticator replacement needs password, current code, and a code from the new seed.
    assert_eq!(
        c.post(
            "/api/account/totp/confirm",
            Some(&alice.token),
            json!({"code": "123456"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        c.post(
            "/api/account/totp",
            Some(&alice.token),
            json!({"password": "second password!", "code": "000000"})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, fresh) = c
        .post(
            "/api/account/totp",
            Some(&alice.token),
            json!({"password": "second password!", "code": alice.recovery[2]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{fresh}");
    let secret2 = fresh["totp_secret"].as_str().unwrap().to_owned();
    assert_ne!(secret2, alice.secret);
    assert_eq!(
        c.post(
            "/api/account/totp/confirm",
            Some(&alice.token),
            json!({"code": code_for(&secret2, 0)})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        c.post(
            "/api/login",
            None,
            login("second password!", code_for(&alice.secret, 1))
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, b) = c
        .post(
            "/api/login",
            None,
            login("second password!", code_for(&secret2, 1)),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{b}");
    let device_b = b["token"].as_str().unwrap().to_owned();
    clear_throttles(&pool).await;

    // E2E key relay: device B asks, device A (trusted) seals the key for B's public key.
    use data_encoding::BASE64;
    assert_eq!(
        c.post(
            "/api/keys/requests",
            Some(&device_b),
            json!({"public_key": "short"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, req) = c
        .post(
            "/api/keys/requests",
            Some(&device_b),
            json!({"public_key": BASE64.encode(&[5; 32])}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let id = req["id"].as_str().unwrap().to_owned();
    let (_, own) = c.get("/api/keys/requests", Some(&device_b)).await;
    assert!(
        own["requests"].as_array().unwrap().is_empty(),
        "a device never grants itself"
    );
    let (_, open) = c.get("/api/keys/requests", Some(&alice.token)).await;
    assert_eq!(open["requests"][0]["id"], id.as_str());
    let (_, pending) = c
        .get(&format!("/api/keys/requests/{id}"), Some(&device_b))
        .await;
    assert_eq!(pending["sealed_key"], Value::Null);
    let sealed = BASE64.encode(&[8; 80]);
    assert_eq!(
        c.post(
            &format!("/api/keys/requests/{id}/grant"),
            Some(&device_b),
            json!({"sealed_key": sealed})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        c.post(
            &format!("/api/keys/requests/{id}/grant"),
            Some(&alice.token),
            json!({"sealed_key": sealed})
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, got) = c
        .get(&format!("/api/keys/requests/{id}"), Some(&device_b))
        .await;
    assert_eq!(got["sealed_key"], sealed.as_str());
    assert_eq!(
        c.get(&format!("/api/keys/requests/{id}"), Some(&device_b))
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    // Library, settings and encrypted secrets sync between devices.
    let playlist = uuid::Uuid::new_v4();
    let now = api::now_ms();
    let media = json!({"id": "abcdefghijk", "title": "Song", "url": "https://www.youtube.com/watch?v=abcdefghijk"});
    let ops = json!({"ops": [
        {"id": uuid::Uuid::new_v4(), "collection": "settings", "key": "theme", "value": "dracula", "clock": now},
        {"id": uuid::Uuid::new_v4(), "collection": "settings", "key": "proxy", "value": "socks5://x:1", "clock": now},
        {"id": uuid::Uuid::new_v4(), "collection": "eq_presets", "key": "warm", "value": {"gains": [1,1,1,0,0,0,0,0,-1,-1]}, "clock": now},
        {"id": uuid::Uuid::new_v4(), "collection": "secrets", "key": "youtube",
         "value": {"v": 1, "alg": "xchacha20poly1305", "nonce": BASE64.encode(&[1; 24]), "ciphertext": BASE64.encode(&[2; 64])}, "clock": now},
        {"id": uuid::Uuid::new_v4(), "collection": "playlists", "key": playlist, "value": {"title": "Mix", "extra": "dropped"}, "clock": now},
        {"id": uuid::Uuid::new_v4(), "collection": "playlist_entries", "key": format!("{playlist}/{}", uuid::Uuid::new_v4()),
         "value": {"position": "m", "media": media}, "clock": now},
        {"id": uuid::Uuid::new_v4(), "collection": "saved", "key": "youtube:abcdefghijk", "value": media, "clock": now},
    ]});
    let (status, pushed) = c.post("/api/sync/push", Some(&alice.token), ops).await;
    assert_eq!(status, StatusCode::OK);
    let statuses: Vec<&str> = pushed["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["status"].as_str().unwrap())
        .collect();
    assert_eq!(
        statuses,
        [
            "applied", "rejected", "applied", "applied", "applied", "applied", "applied"
        ]
    );
    let (_, pulled) = c.get("/api/sync/pull?since=0", Some(&device_b)).await;
    let changes = pulled["changes"].as_array().unwrap();
    let collections: Vec<&str> = changes
        .iter()
        .map(|c| c["collection"].as_str().unwrap())
        .collect();
    assert_eq!(
        collections,
        [
            "settings",
            "eq_presets",
            "secrets",
            "playlists",
            "playlist_entries",
            "saved"
        ]
    );
    assert_eq!(
        changes[3]["value"],
        json!({"title": "Mix"}),
        "unknown fields are not stored"
    );

    // Admin reset link: needs the admin's password and code; then new password and
    // authenticator, every device signed out.
    let bob = register(&c, &pool, "bob", false).await;
    let alice_id = c.get("/api/me", Some(&alice.token)).await.1["user_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let proof = json!({"password": "second password!", "code": alice.recovery[3]});
    assert_eq!(
        c.post(
            &format!("/api/admin/users/{alice_id}/reset"),
            Some(&bob.token),
            proof.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, link) = c
        .post(
            &format!("/api/admin/users/{alice_id}/reset"),
            Some(&alice.token),
            proof,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{link}");
    let link = link["url"].as_str().unwrap();
    assert!(link.starts_with("https://vidcue.test/reset#vcr_"));
    let token = fragment(link);
    let (status, page) = c.get("/reset", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.as_str().unwrap().contains("data-mode=\"reset\""));
    assert_eq!(
        c.post(
            "/api/reset",
            None,
            json!({"token": token, "password": "short"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        c.post(
            "/api/reset",
            None,
            json!({"token": "vcr_bogus", "password": "third password!"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, reset) = c
        .post(
            "/api/reset",
            None,
            json!({"token": token, "password": "third password!"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{reset}");
    assert_eq!(reset["username"], "alice");
    let secret3 = reset["totp_secret"].as_str().unwrap().to_owned();
    assert_eq!(
        c.post(
            "/api/reset/confirm",
            None,
            json!({"token": token, "code": "000000"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, done) = c
        .post(
            "/api/reset/confirm",
            None,
            json!({"token": token, "code": code_for(&secret3, 0)}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["revoked_devices"], 2);
    assert_eq!(done["recovery_codes"].as_array().unwrap().len(), 10);
    assert_eq!(
        c.get("/api/me", Some(&alice.token)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        c.get("/api/me", Some(&device_b)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        c.get("/api/me", Some(&bob.token)).await.0,
        StatusCode::OK,
        "other users are untouched"
    );
    // Old recovery codes died with the reset.
    assert_eq!(
        c.post(
            "/api/login",
            None,
            login("third password!", alice.recovery[4].clone())
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        c.post(
            "/api/login",
            None,
            login("third password!", code_for(&secret3, 1))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        c.post(
            "/api/reset/confirm",
            None,
            json!({"token": token, "code": code_for(&secret3, 1)})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    // Synced data survives the reset.
    let new_code = done["recovery_codes"][0].as_str().unwrap().to_owned();
    let (status, fresh) = c
        .post("/api/login", None, login("third password!", new_code))
        .await;
    assert_eq!(status, StatusCode::OK);
    let token = fresh["token"].as_str().unwrap().to_owned();
    let (_, pulled) = c.get("/api/sync/pull?since=0", Some(&token)).await;
    assert_eq!(pulled["changes"].as_array().unwrap().len(), 6);
}
