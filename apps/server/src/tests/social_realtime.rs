//! B4 flows: friends, activity, inbox, shared playlists, Jam and the realtime WebSocket.
//!
//! Same rules as the parent module: a disposable `VIDCUE_TEST_DATABASE_URL`, skipped
//! without one. WebSocket tests serve the router on `127.0.0.1:0` and connect with
//! tokio-tungstenite; nothing leaves the machine.

use super::{Client, DB_LOCK, fresh_db, register};
use crate::api;
use crate::secrets::ServerKey;
use axum::http::{Method, StatusCode};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use uuid::Uuid;

/// Test clock offset (ms); only these tests change it, and `DB_LOCK` serialises them.
static OFFSET: AtomicI64 = AtomicI64::new(0);

fn test_clock() -> i64 {
    api::now_ms() + OFFSET.load(Ordering::SeqCst)
}

fn advance(ms: i64) {
    OFFSET.fetch_add(ms, Ordering::SeqCst);
}

fn app(pool: &deadpool_postgres::Pool) -> (api::AppState, Client) {
    let _ = &DB_LOCK; // held by the caller through `fresh_db`
    OFFSET.store(0, Ordering::SeqCst);
    let mut state = api::AppState::new(
        pool.clone(),
        api::Settings {
            public_url: "https://vidcue.test".into(),
            key: ServerKey::from_master([9; 32]),
            trusted_proxies: vec!["127.0.0.1".parse().unwrap()],
            proxies_only: true,
        },
    );
    Arc::get_mut(&mut state.0).unwrap().clock = test_clock;
    let client = Client {
        app: api::router(state.clone()),
        ip: None,
        peer: None,
    };
    (state, client)
}

struct User {
    token: String,
    id: String,
    device: String,
}

async fn user(c: &Client, pool: &deadpool_postgres::Pool, name: &str) -> User {
    let account = register(c, pool, name, false).await;
    let (_, me) = c.get("/api/me", Some(&account.token)).await;
    User {
        token: account.token,
        id: me["user_id"].as_str().unwrap().to_owned(),
        device: me["device_id"].as_str().unwrap().to_owned(),
    }
}

/// A second device of the same account, through QR pairing.
async fn pair(c: &Client, of: &User, name: &str) -> User {
    let (status, p) = c
        .post(
            "/api/pairing",
            Some(&of.token),
            json!({"password": "first password!"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    let (status, d) = c
        .post(
            "/api/pairing/claim",
            None,
            json!({"code": p["code"], "device_name": name}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{d}");
    User {
        token: d["token"].as_str().unwrap().to_owned(),
        id: of.id.clone(),
        device: d["device_id"].as_str().unwrap().to_owned(),
    }
}

/// Active account rows without the signup ceremony (for capacity tests).
async fn bare_user(pool: &deadpool_postgres::Pool, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    pool.get()
        .await
        .unwrap()
        .execute(
            "INSERT INTO users (id, username, password_hash, totp_secret, active, created_at)
             VALUES ($1, $2, 'x', '\\x00', TRUE, 1)",
            &[&id, &name],
        )
        .await
        .unwrap();
    id
}

async fn befriend(c: &Client, a: &User, a_name: &str, b: &User, b_name: &str) {
    let (s, r) = c
        .post(
            "/api/friends/requests",
            Some(&a.token),
            json!({"username": b_name}),
        )
        .await;
    assert_eq!(
        (s, r["status"].as_str()),
        (StatusCode::OK, Some("requested"))
    );
    let (s, r) = c
        .post(
            "/api/friends/requests",
            Some(&b.token),
            json!({"username": a_name.to_uppercase()}),
        )
        .await;
    assert_eq!(
        (s, r["status"].as_str()),
        (StatusCode::OK, Some("accepted"))
    );
}

fn media(video: &str, duration: Option<u64>) -> Value {
    json!({"id": video, "title": format!("Song {video}"), "creator": "Band",
           "url": format!("https://www.youtube.com/watch?v={video}"),
           "description": "a long description", "duration": duration, "kind": "song"})
}

async fn delete(c: &Client, path: &str, token: &str) -> (StatusCode, Value) {
    c.call(Method::DELETE, path, Some(token), None).await
}

// ---------------------------------------------------------------- WebSocket helpers

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn serve(state: &api::AppState) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = api::router(state.clone());
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    addr
}

async fn connect(addr: SocketAddr, token: &str) -> (Ws, Value) {
    let mut req = format!("ws://{addr}/api/ws").into_client_request().unwrap();
    req.headers_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    let hello = next(&mut ws).await;
    assert_eq!(hello["type"], "hello");
    (ws, hello)
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(WsMessage::Text(v.to_string().into()))
        .await
        .unwrap();
}

/// The next JSON message (pings are skipped).
async fn next(ws: &mut Ws) -> Value {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("message within 5 s")
            .expect("socket open")
            .expect("frame");
        match msg {
            WsMessage::Text(t) => return serde_json::from_str(&t).unwrap(),
            WsMessage::Ping(_) | WsMessage::Pong(_) => {}
            other => panic!("unexpected frame {other:?}"),
        }
    }
}

/// The next message of `kind`, skipping others.
async fn expect(ws: &mut Ws, kind: &str) -> Value {
    loop {
        let v = next(ws).await;
        if v["type"] == kind {
            return v;
        }
    }
}

/// Everything that arrives within `ms`.
async fn drain(ws: &mut Ws, ms: u64) -> Vec<Value> {
    let mut out = vec![];
    let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
    while let Ok(Some(Ok(msg))) = tokio::time::timeout_at(deadline, ws.next()).await {
        if let WsMessage::Text(t) = msg {
            out.push(serde_json::from_str(&t).unwrap());
        }
    }
    out
}

fn types(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .map(|e| e["type"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// Reads until the server closes; returns the close code.
async fn close_code(ws: &mut Ws) -> u16 {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("close within 5 s")
        {
            Some(Ok(WsMessage::Close(frame))) => return frame.map_or(0, |f| f.code.into()),
            Some(Ok(_)) => {}
            other => panic!("socket ended without a close frame: {other:?}"),
        }
    }
}

// ---------------------------------------------------------------- tests

#[tokio::test]
async fn friends_activity_and_inbox() {
    let Some((pool, _guard)) = fresh_db().await else {
        return;
    };
    let (_, c) = app(&pool);
    let alice = user(&c, &pool, "alice").await;
    let bob = user(&c, &pool, "bob").await;
    let carol = user(&c, &pool, "carol").await;

    // Unknown, malformed and self requests look the same.
    for name in ["nobody", "x", "alice"] {
        let (s, e) = c
            .post(
                "/api/friends/requests",
                Some(&alice.token),
                json!({"username": name}),
            )
            .await;
        assert_eq!(
            (s, e["error"].as_str()),
            (StatusCode::NOT_FOUND, Some("No such user"))
        );
    }
    // Request, list on both sides, accept.
    let (s, r) = c
        .post(
            "/api/friends/requests",
            Some(&alice.token),
            json!({"username": "Bob"}),
        )
        .await;
    assert_eq!(
        (s, r["status"].as_str()),
        (StatusCode::OK, Some("requested"))
    );
    let (_, again) = c
        .post(
            "/api/friends/requests",
            Some(&alice.token),
            json!({"username": "bob"}),
        )
        .await;
    assert_eq!(again["status"], "requested", "idempotent");
    let (_, list) = c.get("/api/friends", Some(&alice.token)).await;
    assert_eq!(list["outgoing"][0]["username"], "bob");
    let (_, list) = c.get("/api/friends", Some(&bob.token)).await;
    assert_eq!(list["incoming"][0]["user_id"], alice.id.as_str());
    // Only the asked side accepts.
    let accept_alice = format!("/api/friends/{}/accept", bob.id);
    assert_eq!(
        c.post(&accept_alice, Some(&alice.token), json!({})).await.0,
        StatusCode::NOT_FOUND
    );
    let (s, r) = c
        .post(
            &format!("/api/friends/{}/accept", alice.id),
            Some(&bob.token),
            json!({}),
        )
        .await;
    assert_eq!(
        (s, r["status"].as_str()),
        (StatusCode::OK, Some("accepted"))
    );
    let (_, list) = c.get("/api/friends", Some(&alice.token)).await;
    assert_eq!(list["friends"][0]["username"], "bob");
    assert!(list["friends"][0]["since"].as_i64().unwrap() > 0);
    assert_eq!(list["outgoing"], json!([]));
    let (_, r) = c
        .post(
            "/api/friends/requests",
            Some(&bob.token),
            json!({"username": "alice"}),
        )
        .await;
    assert_eq!(r["status"], "accepted", "already friends");

    // Activity is opt-in: off by default, listed once shared.
    let (_, s0) = c.get("/api/social/settings", Some(&bob.token)).await;
    assert_eq!(s0["share_activity"], "off");
    let (_, act) = c.get("/api/friends/activity", Some(&alice.token)).await;
    assert_eq!(act["activity"], json!([]));
    let bad = c
        .call(
            Method::PUT,
            "/api/social/settings",
            Some(&bob.token),
            Some(json!({"share_activity": "all"})),
        )
        .await;
    assert_eq!(bad.0, StatusCode::BAD_REQUEST);
    // Bob's history (one recent, one too old, one fresh local-path thumbnail).
    let now = api::now_ms();
    let ops = json!({"ops": [
        {"id": Uuid::new_v4(), "collection": "history", "key": "e1", "clock": now,
         "value": {"url": "https://youtu.be/aaaaaaaaaaa", "title": "New", "creator": "C", "kind": "song",
                   "thumbnail": "/sdcard/x.jpg", "at": now - 1000, "seconds": 40.0}},
        {"id": Uuid::new_v4(), "collection": "history", "key": "e2", "clock": now,
         "value": {"url": "https://youtu.be/bbbbbbbbbbb", "title": "Old", "at": now - 8 * 86_400_000}},
        {"id": Uuid::new_v4(), "collection": "history", "key": "e3", "clock": now,
         "value": {"url": "https://youtu.be/ccccccccccc", "title": "Older", "at": now - 2000}},
    ]});
    let (s, _) = c.post("/api/sync/push", Some(&bob.token), ops).await;
    assert_eq!(s, StatusCode::OK);
    let put = |level: &'static str| {
        let c = c.clone();
        let token = bob.token.clone();
        async move {
            c.call(
                Method::PUT,
                "/api/social/settings",
                Some(&token),
                Some(json!({"share_activity": level})),
            )
            .await
        }
    };
    assert_eq!(put("now_playing").await.1["share_activity"], "now_playing");
    let (_, act) = c.get("/api/friends/activity", Some(&alice.token)).await;
    assert_eq!(act["activity"][0]["username"], "bob");
    assert_eq!(act["activity"][0]["now_playing"], Value::Null);
    assert!(
        act["activity"][0].get("recent").is_none(),
        "history not shared"
    );
    put("history").await;
    let (_, act) = c.get("/api/friends/activity", Some(&alice.token)).await;
    let recent = act["activity"][0]["recent"].as_array().unwrap();
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0]["title"], "New");
    assert_eq!(recent[0]["thumbnail"], "");
    assert_eq!(recent[1]["title"], "Older");
    assert!(recent[0].get("seconds").is_none());
    // Strangers see nothing.
    let (_, act) = c.get("/api/friends/activity", Some(&carol.token)).await;
    assert_eq!(act["activity"], json!([]));

    // Inbox: friends only, slim online media, short printable notes.
    let send = |from: &User, to: &str, m: Value, note: &str| {
        let c = c.clone();
        let token = from.token.clone();
        let body = json!({"to": to, "media": m, "note": note});
        async move { c.post("/api/inbox", Some(&token), body).await }
    };
    let (s, e) = send(&carol, &bob.id, media("aaaaaaaaaaa", None), "").await;
    assert_eq!(
        (s, e["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("No such friend"))
    );
    assert_eq!(
        send(&alice, &bob.id, json!({"url": "/sdcard/a.mp3"}), "")
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(&alice, &bob.id, media("aaaaaaaaaaa", None), "two\nlines")
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(
            &alice,
            &bob.id,
            media("aaaaaaaaaaa", None),
            &"x".repeat(201)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (s, sent) = send(&alice, &bob.id, media("aaaaaaaaaaa", Some(200)), " listen ").await;
    assert_eq!(s, StatusCode::CREATED, "{sent}");
    let (_, inbox) = c.get("/api/inbox", Some(&bob.token)).await;
    assert_eq!(inbox["unread"], 1);
    let item = &inbox["items"][0];
    assert_eq!(
        (item["note"].as_str(), item["read"].as_bool()),
        (Some("listen"), Some(false))
    );
    assert_eq!(item["from"]["username"], "alice");
    assert_eq!(item["media"]["description"], "");
    let item_path = format!("/api/inbox/{}", sent["id"].as_str().unwrap());
    // Only the recipient can touch it.
    assert_eq!(
        c.post(&format!("{item_path}/read"), Some(&alice.token), json!({}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        delete(&c, &item_path, &alice.token).await.0,
        StatusCode::NOT_FOUND
    );
    for _ in 0..2 {
        let (s, r) = c
            .post(&format!("{item_path}/read"), Some(&bob.token), json!({}))
            .await;
        assert_eq!((s, r["read"].as_bool()), (StatusCode::OK, Some(true)));
    }
    let (_, unread) = c.get("/api/inbox?unread_only=true", Some(&bob.token)).await;
    assert_eq!(
        (
            unread["items"].as_array().unwrap().len(),
            unread["unread"].as_i64()
        ),
        (0, Some(0))
    );

    // Cap: read items make room, 200 unread items refuse.
    let db = pool.get().await.unwrap();
    db.execute(
        "INSERT INTO inbox (id, to_user, from_user, media, note, created_at, read_at)
         SELECT gen_random_uuid(), $1, $2, '{}'::jsonb, '', g, CASE WHEN g <= 5 THEN g END
         FROM generate_series(1, 199) g",
        &[
            &Uuid::parse_str(&bob.id).unwrap(),
            &Uuid::parse_str(&alice.id).unwrap(),
        ],
    )
    .await
    .unwrap();
    let (s, _) = send(&alice, &bob.id, media("bbbbbbbbbbb", None), "").await;
    assert_eq!(s, StatusCode::CREATED, "oldest read item dropped");
    let count = |db: deadpool_postgres::Object, bob: String| async move {
        db.query_one(
            "SELECT COUNT(*) FROM inbox WHERE to_user = $1",
            &[&Uuid::parse_str(&bob).unwrap()],
        )
        .await
        .unwrap()
        .get::<_, i64>(0)
    };
    assert_eq!(count(pool.get().await.unwrap(), bob.id.clone()).await, 200);
    db.execute("UPDATE inbox SET read_at = NULL", &[])
        .await
        .unwrap();
    let (s, e) = send(&alice, &bob.id, media("ccccccccccc", None), "").await;
    assert_eq!(
        (s, e["error"].as_str()),
        (StatusCode::CONFLICT, Some("Their inbox is full"))
    );
    let (_, inbox) = c.get("/api/inbox", Some(&bob.token)).await;
    assert_eq!(inbox["items"].as_array().unwrap().len(), 200);

    // Retention.
    api::maintenance(&pool, api::now_ms() + 91 * 86_400_000)
        .await
        .unwrap();
    assert_eq!(count(pool.get().await.unwrap(), bob.id.clone()).await, 0);

    // Removing a friend ends sharing both ways.
    let (s, r) = delete(&c, &format!("/api/friends/{}", bob.id), &alice.token).await;
    assert_eq!((s, r["removed"].as_bool()), (StatusCode::OK, Some(true)));
    assert_eq!(
        delete(&c, &format!("/api/friends/{}", bob.id), &alice.token)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, act) = c.get("/api/friends/activity", Some(&alice.token)).await;
    assert_eq!(act["activity"], json!([]));
    assert_eq!(
        send(&alice, &bob.id, media("aaaaaaaaaaa", None), "")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    // A pending request can be declined by either side.
    c.post(
        "/api/friends/requests",
        Some(&carol.token),
        json!({"username": "alice"}),
    )
    .await;
    assert_eq!(
        delete(&c, &format!("/api/friends/{}", carol.id), &alice.token)
            .await
            .0,
        StatusCode::OK
    );
    let (_, list) = c.get("/api/friends", Some(&carol.token)).await;
    assert_eq!(list["outgoing"], json!([]));

    // Friend requests are throttled per user, counted on every call.
    super::clear_throttles(&pool).await;
    for i in 0..20 {
        c.post(
            "/api/friends/requests",
            Some(&carol.token),
            json!({"username": format!("ghost{i}")}),
        )
        .await;
    }
    let (s, _) = c
        .post(
            "/api/friends/requests",
            Some(&carol.token),
            json!({"username": "bob"}),
        )
        .await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);

    // Inbox sends are throttled per sender.
    befriend(&c, &alice, "alice", &bob, "bob").await;
    db.execute("DELETE FROM inbox", &[]).await.unwrap();
    for _ in 0..60 {
        assert_eq!(
            send(&alice, &bob.id, media("aaaaaaaaaaa", None), "")
                .await
                .0,
            StatusCode::CREATED
        );
    }
    assert_eq!(
        send(&alice, &bob.id, media("aaaaaaaaaaa", None), "")
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn friend_caps() {
    let Some((pool, _guard)) = fresh_db().await else {
        return;
    };
    let (_, c) = app(&pool);
    let alice = user(&c, &pool, "alice").await;
    let a = Uuid::parse_str(&alice.id).unwrap();
    let db = pool.get().await.unwrap();
    // 50 open outgoing requests.
    for i in 0..50 {
        let other = bare_user(&pool, &format!("pending{i}")).await;
        let (x, y) = crate::social::pair(a, other);
        db.execute(
            "INSERT INTO friendships (user_a, user_b, requested_by, created_at) VALUES ($1, $2, $3, 1)",
            &[&x, &y, &a],
        )
        .await
        .unwrap();
    }
    bare_user(&pool, "dave").await;
    let (s, e) = c
        .post(
            "/api/friends/requests",
            Some(&alice.token),
            json!({"username": "dave"}),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
    // 100 friends.
    db.execute("DELETE FROM friendships", &[]).await.unwrap();
    for i in 0..100 {
        let other = bare_user(&pool, &format!("friend{i}")).await;
        let (x, y) = crate::social::pair(a, other);
        db.execute(
            "INSERT INTO friendships (user_a, user_b, requested_by, created_at, accepted_at) VALUES ($1, $2, $3, 1, 1)",
            &[&x, &y, &a],
        )
        .await
        .unwrap();
    }
    let (s, _) = c
        .post(
            "/api/friends/requests",
            Some(&alice.token),
            json!({"username": "dave"}),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
}

#[tokio::test]
async fn shared_playlists() {
    let Some((pool, _guard)) = fresh_db().await else {
        return;
    };
    let (_, c) = app(&pool);
    let alice = user(&c, &pool, "alice").await;
    let bob = user(&c, &pool, "bob").await;
    let carol = user(&c, &pool, "carol").await;
    befriend(&c, &alice, "alice", &bob, "bob").await;

    let id = Uuid::new_v4();
    let base = format!("/api/shared-playlists/{id}");
    assert_eq!(
        c.post(
            "/api/shared-playlists",
            Some(&alice.token),
            json!({"id": id, "title": " "})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (s, p) = c
        .post(
            "/api/shared-playlists",
            Some(&alice.token),
            json!({"id": id, "title": "Road trip"}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    assert_eq!(
        (p["role"].as_str(), p["entry_count"].as_i64()),
        (Some("owner"), Some(0))
    );
    assert_eq!(p["owner"]["username"], "alice");
    // Idempotent create; another user cannot take the id.
    let (s, again) = c
        .post(
            "/api/shared-playlists",
            Some(&alice.token),
            json!({"id": id, "title": "Other"}),
        )
        .await;
    assert_eq!(
        (s, again["title"].as_str()),
        (StatusCode::OK, Some("Road trip"))
    );
    assert_eq!(
        c.post(
            "/api/shared-playlists",
            Some(&bob.token),
            json!({"id": id, "title": "Mine"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );

    // Non-members get 404 everywhere, never 403.
    let entry = Uuid::new_v4();
    let now = api::now_ms();
    let op = |id: Uuid, value: Value, clock: i64| json!({"id": id, "entry_id": entry, "value": value, "clock": clock});
    let push = |token: &str, ops: Vec<Value>| {
        let c = c.clone();
        let (token, path) = (token.to_owned(), format!("{base}/push"));
        async move { c.post(&path, Some(&token), json!({"ops": ops})).await }
    };
    let value = json!({"position": "m", "media": media("aaaaaaaaaaa", None)});
    assert_eq!(
        push(&bob.token, vec![op(Uuid::new_v4(), value.clone(), now)])
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        c.get(&format!("{base}/pull"), Some(&bob.token)).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        c.call(
            Method::PATCH,
            &base,
            Some(&bob.token),
            Some(json!({"title": "x"}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    // Members: only friends of the owner.
    let members = format!("{base}/members");
    assert_eq!(
        c.post(&members, Some(&alice.token), json!({"user_id": carol.id}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (s, p) = c
        .post(&members, Some(&alice.token), json!({"user_id": bob.id}))
        .await;
    assert_eq!(s, StatusCode::OK, "{p}");
    assert_eq!(p["members"].as_array().unwrap().len(), 2);
    assert_eq!(p["members"][0]["username"], "alice", "owner first");
    let (_, listed) = c.get("/api/shared-playlists", Some(&bob.token)).await;
    assert_eq!(listed["playlists"][0]["role"], "member");
    // A member cannot rename, delete or add.
    assert_eq!(
        c.call(
            Method::PATCH,
            &base,
            Some(&bob.token),
            Some(json!({"title": "x"}))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(delete(&c, &base, &bob.token).await.0, StatusCode::FORBIDDEN);
    assert_eq!(
        delete(&c, &format!("{members}/{}", alice.id), &alice.token)
            .await
            .0,
        StatusCode::CONFLICT,
        "the owner cannot leave"
    );

    // Push: register per entry, later clock wins, ops idempotent.
    let first = Uuid::new_v4();
    let (s, r) = push(&bob.token, vec![op(first, value.clone(), now)]).await;
    assert_eq!(s, StatusCode::OK, "{r}");
    assert_eq!(r["results"][0]["status"], "applied");
    let seq1 = r["seq"].as_i64().unwrap();
    let (_, r) = push(
        &alice.token,
        vec![
            op(first, value.clone(), now),
            op(
                Uuid::new_v4(),
                json!({"position": "a", "media": media("bbbbbbbbbbb", None)}),
                now - 1000,
            ),
            op(
                Uuid::new_v4(),
                json!({"position": "a b", "media": media("bbbbbbbbbbb", None)}),
                now,
            ),
            op(
                Uuid::new_v4(),
                json!({"position": "a", "media": {"url": "/sdcard/x.mp3"}}),
                now,
            ),
        ],
    )
    .await;
    let statuses: Vec<&str> = r["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["status"].as_str().unwrap())
        .collect();
    assert_eq!(
        statuses,
        ["duplicate", "superseded", "rejected", "rejected"]
    );
    assert_eq!(r["seq"], Value::Null);
    let newer = json!({"position": "n", "media": media("ccccccccccc", None)});
    let (_, r) = push(&alice.token, vec![op(Uuid::new_v4(), newer, now + 1000)]).await;
    assert_eq!(r["results"][0]["status"], "applied");
    let (s, pulled) = c
        .get(&format!("{base}/pull?since={seq1}"), Some(&bob.token))
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(pulled["changes"].as_array().unwrap().len(), 1);
    let change = &pulled["changes"][0];
    assert_eq!(change["value"]["position"], "n");
    assert_eq!(change["value"]["media"]["description"], "");
    assert_eq!(change["user_id"], alice.id.as_str());
    assert_eq!(pulled["playlist"]["entry_count"], 1);
    assert_eq!(pulled["more"], false);
    assert!(pulled["next"].as_i64().unwrap() >= pulled["playlist"]["seq"].as_i64().unwrap());
    // Paging.
    let (_, page) = c
        .get(&format!("{base}/pull?since=0&limit=1"), Some(&bob.token))
        .await;
    assert_eq!(
        (
            page["changes"].as_array().unwrap().len(),
            page["more"].as_bool()
        ),
        (1, Some(false))
    );

    // Rename bumps seq; a complete pull moves the cursor past it.
    let (_, before) = c
        .get(&format!("{base}/pull?since=0"), Some(&bob.token))
        .await;
    let (s, renamed) = c
        .call(
            Method::PATCH,
            &base,
            Some(&alice.token),
            Some(json!({"title": "Night drive"})),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert!(renamed["seq"].as_i64() > before["next"].as_i64());
    let (_, after) = c
        .get(
            &format!("{base}/pull?since={}", before["next"]),
            Some(&bob.token),
        )
        .await;
    assert_eq!(after["changes"], json!([]));
    assert_eq!(after["next"], renamed["seq"]);
    assert_eq!(after["playlist"]["title"], "Night drive");

    // Full playlist: new live entries are refused per op, tombstones still apply.
    let db = pool.get().await.unwrap();
    db.execute(
        "INSERT INTO shared_playlist_entries (playlist_id, entry_id, value, clock, device_id, user_id, seq)
         SELECT $1, gen_random_uuid(), '{\"position\":\"z\"}'::jsonb, 1, gen_random_uuid(), gen_random_uuid(), nextval('shared_seq')
         FROM generate_series(1, 1999)",
        &[&id],
    )
    .await
    .unwrap();
    let extra =
        json!({"id": Uuid::new_v4(), "entry_id": Uuid::new_v4(), "value": value, "clock": now});
    let tomb = json!({"id": Uuid::new_v4(), "entry_id": entry, "value": null, "clock": now + 5000});
    let (_, r) = push(&alice.token, vec![extra.clone(), tomb]).await;
    assert_eq!(r["results"][0]["status"], "rejected");
    assert_eq!(r["results"][0]["reason"], "playlist is full");
    assert_eq!(r["results"][1]["status"], "applied");
    let (_, r) = push(&alice.token, vec![extra]).await;
    assert_eq!(
        r["results"][0]["status"], "applied",
        "room after the tombstone"
    );
    let too_many: Vec<Value> = (0..201)
        .map(|_| op(Uuid::new_v4(), Value::Null, now))
        .collect();
    assert_eq!(
        push(&alice.token, too_many).await.0,
        StatusCode::BAD_REQUEST
    );

    // Leaving, removing and deleting.
    let (s, _) = delete(&c, &format!("{members}/{}", bob.id), &bob.token).await;
    assert_eq!(s, StatusCode::OK, "a member leaves");
    assert_eq!(
        c.get(&format!("{base}/pull"), Some(&bob.token)).await.0,
        StatusCode::NOT_FOUND
    );
    c.post(&members, Some(&alice.token), json!({"user_id": bob.id}))
        .await;
    assert_eq!(
        delete(&c, &format!("{members}/{}", bob.id), &alice.token)
            .await
            .0,
        StatusCode::OK
    );
    c.post(&members, Some(&alice.token), json!({"user_id": bob.id}))
        .await;
    // Unfriending removes each from the other's playlists.
    delete(&c, &format!("/api/friends/{}", alice.id), &bob.token).await;
    let (_, listed) = c.get("/api/shared-playlists", Some(&bob.token)).await;
    assert_eq!(listed["playlists"], json!([]));
    let (s, r) = delete(&c, &base, &alice.token).await;
    assert_eq!((s, r["deleted"].as_bool()), (StatusCode::OK, Some(true)));
    assert_eq!(
        c.get(&format!("{base}/pull"), Some(&alice.token)).await.0,
        StatusCode::NOT_FOUND
    );

    // At most 50 memberships per user.
    for _ in 0..50 {
        let (s, _) = c
            .post(
                "/api/shared-playlists",
                Some(&carol.token),
                json!({"id": Uuid::new_v4(), "title": "x"}),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED);
    }
    assert_eq!(
        c.post(
            "/api/shared-playlists",
            Some(&carol.token),
            json!({"id": Uuid::new_v4(), "title": "x"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn jam_rooms_queue_votes_and_permissions() {
    let Some((pool, _guard)) = fresh_db().await else {
        return;
    };
    let (_, c) = app(&pool);
    let alice = user(&c, &pool, "alice").await;
    let bob = user(&c, &pool, "bob").await;
    let carol = user(&c, &pool, "carol").await;
    befriend(&c, &alice, "alice", &bob, "bob").await;

    let (s, room) = c.post("/api/jam", Some(&alice.token), json!({})).await;
    assert_eq!(s, StatusCode::CREATED, "{room}");
    assert_eq!(room["title"], "alice's Jam");
    assert_eq!(
        (room["role"].as_str(), room["playback"]["state"].as_str()),
        (Some("host"), Some("stopped"))
    );
    assert_eq!(
        c.post(
            "/api/jam",
            Some(&alice.token),
            json!({"title": "x".repeat(81)})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let id = room["id"].as_str().unwrap().to_owned();
    let base = format!("/api/jam/{id}");

    // Only friends are invited; others are silently skipped.
    let (s, r) = c
        .post(
            &format!("{base}/invite"),
            Some(&alice.token),
            json!({"user_ids": [bob.id, carol.id, Uuid::new_v4()]}),
        )
        .await;
    assert_eq!((s, r["invited"].clone()), (StatusCode::OK, json!([bob.id])));
    let (_, mine) = c.get("/api/jam", Some(&bob.token)).await;
    assert_eq!(mine["current"], Value::Null);
    assert_eq!(mine["invites"][0]["room_id"], id.as_str());
    assert_eq!(mine["invites"][0]["from"]["username"], "alice");
    // Strangers cannot see or join.
    assert_eq!(
        c.get(&base, Some(&carol.token)).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        c.post(&format!("{base}/join"), Some(&carol.token), json!({}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (s, joined) = c
        .post(&format!("{base}/join"), Some(&bob.token), json!({}))
        .await;
    assert_eq!(s, StatusCode::OK, "{joined}");
    assert_eq!(joined["role"], "guest");
    assert_eq!(joined["members"].as_array().unwrap().len(), 2);
    assert_eq!(joined["invited"], json!([]));

    // The first queued item starts at once; later ones queue.
    let add = |token: &str, m: Value| {
        let (c, token, path) = (c.clone(), token.to_owned(), format!("{base}/queue"));
        async move { c.post(&path, Some(&token), json!({"media": m})).await }
    };
    assert_eq!(
        add(&bob.token, json!({"url": "/sdcard/a.mp3"})).await.0,
        StatusCode::BAD_REQUEST
    );
    let (s, first) = add(&bob.token, media("aaaaaaaaaaa", Some(200))).await;
    assert_eq!(s, StatusCode::CREATED, "{first}");
    let (_, b) = add(&bob.token, media("bbbbbbbbbbb", Some(200))).await;
    let (_, cc) = add(&alice.token, media("ccccccccccc", Some(200))).await;
    let (_, view) = c.get(&base, Some(&bob.token)).await;
    assert_eq!(view["playback"]["state"], "playing");
    assert_eq!(view["playback"]["item"]["media_key"], "youtube:aaaaaaaaaaa");
    assert_eq!(view["playback"]["item"]["item_id"], first["item_id"]);
    let keys: Vec<&str> = view["queue"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| q["media_key"].as_str().unwrap())
        .collect();
    assert_eq!(
        keys,
        ["youtube:bbbbbbbbbbb", "youtube:ccccccccccc"],
        "oldest first"
    );

    // Votes reorder.
    let vote = |token: &str, item: &Value, up: bool| {
        let c = c.clone();
        let (token, path) = (
            token.to_owned(),
            format!("{base}/queue/{}/vote", item["item_id"].as_str().unwrap()),
        );
        async move { c.post(&path, Some(&token), json!({"up": up})).await }
    };
    let (_, v) = vote(&alice.token, &cc, true).await;
    assert_eq!(
        (v["votes"].as_i64(), v["voted"].as_bool()),
        (Some(1), Some(true))
    );
    vote(&alice.token, &cc, true).await;
    let (_, v) = vote(&bob.token, &cc, true).await;
    assert_eq!(v["votes"], 2);
    let (_, view) = c.get(&base, Some(&bob.token)).await;
    assert_eq!(view["queue"][0]["item_id"], cc["item_id"]);
    assert_eq!(
        (
            view["queue"][0]["votes"].as_i64(),
            view["queue"][0]["voted"].as_bool()
        ),
        (Some(2), Some(true))
    );
    let (_, v) = vote(&bob.token, &cc, false).await;
    assert_eq!(
        (v["votes"].as_i64(), v["voted"].as_bool()),
        (Some(1), Some(false))
    );
    assert_eq!(vote(&carol.token, &cc, true).await.0, StatusCode::NOT_FOUND);

    // Guests control playback only when allowed; `ended` is always allowed.
    let control = |token: &str, body: Value| {
        let (c, token, path) = (c.clone(), token.to_owned(), format!("{base}/playback"));
        async move { c.post(&path, Some(&token), body).await }
    };
    let (s, e) = control(&bob.token, json!({"action": "pause"})).await;
    assert_eq!(
        (s, e["error"].as_str()),
        (
            StatusCode::FORBIDDEN,
            Some("Only the host can control playback")
        )
    );
    let (s, same) = control(
        &bob.token,
        json!({"action": "ended", "item_id": b["item_id"]}),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        same["item"]["item_id"], first["item_id"],
        "not current: no change"
    );
    let version = same["version"].as_i64().unwrap();
    let (s, room) = c
        .call(
            Method::PATCH,
            &base,
            Some(&bob.token),
            Some(json!({"guests_can_control": true})),
        )
        .await;
    assert_eq!(
        (s, room["guests_can_control"].as_bool()),
        (StatusCode::FORBIDDEN, None)
    );
    let (s, room) = c
        .call(
            Method::PATCH,
            &base,
            Some(&alice.token),
            Some(json!({"guests_can_control": true, "title": "Friday"})),
        )
        .await;
    assert_eq!(
        (s, room["title"].as_str()),
        (StatusCode::OK, Some("Friday"))
    );
    let (s, paused) = control(
        &bob.token,
        json!({"action": "pause", "position_ms": 30_000}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{paused}");
    assert_eq!(
        (paused["state"].as_str(), paused["position_ms"].as_i64()),
        (Some("paused"), Some(30_000))
    );
    assert!(paused["version"].as_i64().unwrap() > version);
    assert_eq!(
        control(
            &bob.token,
            json!({"action": "seek", "position_ms": 200_001})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        control(&bob.token, json!({"action": "rate", "rate": 3.0}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        control(&bob.token, json!({"action": "fly"})).await.0,
        StatusCode::BAD_REQUEST
    );
    let (_, rated) = control(&alice.token, json!({"action": "rate", "rate": 1.5})).await;
    assert_eq!(
        (rated["rate"].as_f64(), rated["position_ms"].as_i64()),
        (Some(1.5), Some(30_000))
    );
    let (_, playing) = control(&alice.token, json!({"action": "play"})).await;
    assert_eq!(playing["state"], "playing");

    // `ended` advances once to the most voted item, every repeat is a no-op.
    let ended = json!({"action": "ended", "item_id": first["item_id"]});
    let (_, next) = control(&bob.token, ended.clone()).await;
    assert_eq!(next["item"]["item_id"], cc["item_id"]);
    assert_eq!(
        (next["position_ms"].as_i64(), next["state"].as_str()),
        (Some(0), Some("playing"))
    );
    let (_, again) = control(&alice.token, ended).await;
    assert_eq!(again, next);
    let (_, picked) = control(
        &alice.token,
        json!({"action": "play_item", "item_id": b["item_id"]}),
    )
    .await;
    assert_eq!(picked["item"]["item_id"], b["item_id"]);
    assert_eq!(
        control(
            &alice.token,
            json!({"action": "play_item", "item_id": b["item_id"]})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, stopped) = control(&alice.token, json!({"action": "next"})).await;
    assert_eq!(
        (stopped["state"].as_str(), stopped["item"].clone()),
        (Some("stopped"), Value::Null)
    );
    assert_eq!(
        control(&alice.token, json!({"action": "pause"})).await.0,
        StatusCode::CONFLICT
    );

    // Queue removal: host or adder.
    let (_, d) = add(&alice.token, media("ddddddddddd", None)).await; // becomes current
    let (_, e) = add(&alice.token, media("eeeeeeeeeee", None)).await;
    assert_ne!(d["item_id"], e["item_id"]);
    let remove = format!("{base}/queue/{}", e["item_id"].as_str().unwrap());
    assert_eq!(
        delete(&c, &remove, &bob.token).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(delete(&c, &remove, &alice.token).await.0, StatusCode::OK);
    assert_eq!(
        delete(&c, &remove, &alice.token).await.0,
        StatusCode::NOT_FOUND
    );

    // Full room: 10 members including the host.
    let db = pool.get().await.unwrap();
    let room_id = Uuid::parse_str(&id).unwrap();
    for i in 0..8 {
        let u = bare_user(&pool, &format!("fan{i}")).await;
        db.execute(
            "INSERT INTO jam_members (room_id, user_id, role, joined_at) VALUES ($1, $2, 'guest', 1)",
            &[&room_id, &u],
        )
        .await
        .unwrap();
    }
    befriend(&c, &alice, "alice", &carol, "carol").await;
    c.post(
        &format!("{base}/invite"),
        Some(&alice.token),
        json!({"user_ids": [carol.id]}),
    )
    .await;
    let (s, e) = c
        .post(&format!("{base}/join"), Some(&carol.token), json!({}))
        .await;
    assert_eq!(
        (s, e["error"].as_str()),
        (StatusCode::CONFLICT, Some("This Jam is full"))
    );
    db.execute(
        "DELETE FROM jam_members WHERE role = 'guest' AND user_id <> $1",
        &[&Uuid::parse_str(&bob.id).unwrap()],
    )
    .await
    .unwrap();
    c.post(
        &format!("{base}/invite"),
        Some(&alice.token),
        json!({"user_ids": [carol.id]}),
    )
    .await;
    let (s, r) = c
        .post(&format!("{base}/decline"), Some(&carol.token), json!({}))
        .await;
    assert_eq!((s, r["declined"].as_bool()), (StatusCode::OK, Some(true)));
    assert_eq!(
        c.post(&format!("{base}/join"), Some(&carol.token), json!({}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    // Kick, leave, host ending.
    assert_eq!(
        delete(&c, &format!("{base}/members/{}", alice.id), &bob.token)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        delete(&c, &format!("{base}/members/{}", bob.id), &alice.token)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        c.get(&base, Some(&bob.token)).await.0,
        StatusCode::NOT_FOUND
    );
    c.post(
        &format!("{base}/invite"),
        Some(&alice.token),
        json!({"user_ids": [bob.id]}),
    )
    .await;
    c.post(&format!("{base}/join"), Some(&bob.token), json!({}))
        .await;
    let (s, r) = c
        .post(&format!("{base}/leave"), Some(&bob.token), json!({}))
        .await;
    assert_eq!((s, r["left"].as_bool()), (StatusCode::OK, Some(true)));
    // Joining another room leaves the current one; hosting a new room ends the old one.
    c.post(
        &format!("{base}/invite"),
        Some(&alice.token),
        json!({"user_ids": [bob.id]}),
    )
    .await;
    c.post(&format!("{base}/join"), Some(&bob.token), json!({}))
        .await;
    let (_, bobs) = c
        .post("/api/jam", Some(&bob.token), json!({"title": "Bob's"}))
        .await;
    let (_, view) = c.get(&base, Some(&alice.token)).await;
    assert_eq!(
        view["members"].as_array().unwrap().len(),
        1,
        "bob left alice's room"
    );
    let (_, alices2) = c.post("/api/jam", Some(&alice.token), json!({})).await;
    assert_eq!(
        c.get(&base, Some(&alice.token)).await.0,
        StatusCode::NOT_FOUND,
        "old room ended"
    );
    let (_, current) = c.get("/api/jam", Some(&alice.token)).await;
    assert_eq!(current["current"]["id"], alices2["id"]);
    let bobs_base = format!("/api/jam/{}", bobs["id"].as_str().unwrap());
    assert_eq!(
        delete(&c, &bobs_base, &alice.token).await.0,
        StatusCode::NOT_FOUND
    );
    let (s, r) = delete(&c, &bobs_base, &bob.token).await;
    assert_eq!((s, r["ended"].as_bool()), (StatusCode::OK, Some(true)));
    assert_eq!(
        c.get(&bobs_base, Some(&bob.token)).await.0,
        StatusCode::NOT_FOUND
    );

    // Unfriending removes the guest from the host's room.
    let second = format!("/api/jam/{}", alices2["id"].as_str().unwrap());
    c.post(
        &format!("{second}/invite"),
        Some(&alice.token),
        json!({"user_ids": [bob.id]}),
    )
    .await;
    c.post(&format!("{second}/join"), Some(&bob.token), json!({}))
        .await;
    assert_eq!(c.get(&second, Some(&bob.token)).await.0, StatusCode::OK);
    delete(&c, &format!("/api/friends/{}", bob.id), &alice.token).await;
    assert_eq!(
        c.get(&second, Some(&bob.token)).await.0,
        StatusCode::NOT_FOUND
    );

    // Lifetime: 30 idle minutes end the room; ended rooms are deleted after a day.
    advance(31 * 60_000);
    assert_eq!(
        c.get(&second, Some(&alice.token)).await.0,
        StatusCode::NOT_FOUND
    );
    let rows = |db: deadpool_postgres::Object| async move {
        db.query_one("SELECT COUNT(*) FROM jam_rooms", &[])
            .await
            .unwrap()
            .get::<_, i64>(0)
    };
    assert!(rows(pool.get().await.unwrap()).await > 0);
    api::maintenance(&pool, test_clock() + 25 * 3_600_000)
        .await
        .unwrap();
    assert_eq!(rows(pool.get().await.unwrap()).await, 0);
}

#[tokio::test]
async fn jam_rooms_expire_after_twelve_hours_and_count_actions() {
    let Some((pool, _guard)) = fresh_db().await else {
        return;
    };
    let (_, c) = app(&pool);
    let alice = user(&c, &pool, "alice").await;
    let (_, room) = c.post("/api/jam", Some(&alice.token), json!({})).await;
    let base = format!("/api/jam/{}", room["id"].as_str().unwrap());
    // Activity keeps it alive, but not past 12 hours.
    for _ in 0..25 {
        advance(29 * 60_000);
        let (s, _) = c
            .call(
                Method::PATCH,
                &base,
                Some(&alice.token),
                Some(json!({"title": "Still here"})),
            )
            .await;
        if s == StatusCode::NOT_FOUND {
            assert!(OFFSET.load(Ordering::SeqCst) >= 12 * 3_600_000);
            break;
        }
        assert_eq!(s, StatusCode::OK);
    }
    assert_eq!(
        c.get(&base, Some(&alice.token)).await.0,
        StatusCode::NOT_FOUND
    );
    // Queue additions are throttled per user.
    OFFSET.store(0, Ordering::SeqCst);
    super::clear_throttles(&pool).await;
    let (_, room) = c.post("/api/jam", Some(&alice.token), json!({})).await;
    let queue = format!("/api/jam/{}/queue", room["id"].as_str().unwrap());
    for i in 0..30 {
        let (s, _) = c
            .post(
                &queue,
                Some(&alice.token),
                json!({"media": media(&format!("{i:0>11}"), None)}),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED);
    }
    let (s, _) = c
        .post(
            &queue,
            Some(&alice.token),
            json!({"media": media("xxxxxxxxxxx", None)}),
        )
        .await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn websocket_presence_handoff_remote_and_jam() {
    let Some((pool, _guard)) = fresh_db().await else {
        return;
    };
    let (state, c) = app(&pool);
    let alice = user(&c, &pool, "alice").await;
    let tablet = pair(&c, &alice, "Tablet").await;
    let bob = user(&c, &pool, "bob").await;
    let carol = user(&c, &pool, "carol").await;
    befriend(&c, &alice, "alice", &bob, "bob").await;
    let addr = serve(&state).await;

    let (mut phone_ws, hello) = connect(addr, &alice.token).await;
    assert_eq!(hello["features"], json!(["realtime", "social", "jam"]));
    assert!(hello["server_ms"].as_i64().unwrap() > 0);
    let (mut tablet_ws, _) = connect(addr, &tablet.token).await;
    let (mut bob_ws, _) = connect(addr, &bob.token).await;
    let (mut carol_ws, _) = connect(addr, &carol.token).await;

    // Clock ping.
    send(&mut phone_ws, json!({"type": "ping", "t0": 42, "rid": 1})).await;
    let pong = next(&mut phone_ws).await;
    assert_eq!(
        (
            pong["type"].as_str(),
            pong["t0"].as_i64(),
            pong["rid"].as_i64()
        ),
        (Some("pong"), Some(42), Some(1))
    );
    assert!(pong["server_ms"].as_i64().unwrap() > 0);

    // Bob shares what he plays; alice's devices hear it.
    c.call(
        Method::PUT,
        "/api/social/settings",
        Some(&bob.token),
        Some(json!({"share_activity": "now_playing"})),
    )
    .await;
    expect(&mut phone_ws, "friend_activity").await;
    let presence = json!({"type": "presence", "rid": 2, "state": {
        "media": media("aaaaaaaaaaa", Some(215)), "playing": true, "position_ms": 51000, "duration_ms": 215000,
        "rate": 1.0, "volume": 80, "queue_len": 4, "at": 1_763_900_000_000i64, "can": ["remote", "handoff"]}});
    send(&mut bob_ws, presence.clone()).await;
    assert_eq!(next(&mut bob_ws).await, json!({"type": "ack", "rid": 2}));
    let activity = expect(&mut phone_ws, "friend_activity").await;
    assert_eq!(activity["user_id"], bob.id.as_str());
    assert_eq!(activity["now_playing"]["media"]["description"], "");
    assert_eq!(activity["now_playing"]["playing"], true);
    let (_, act) = c.get("/api/friends/activity", Some(&alice.token)).await;
    assert_eq!(act["activity"][0]["now_playing"]["position_ms"], 51000);
    // Carol is not a friend: nothing for her.
    assert!(!types(&drain(&mut carol_ws, 300).await).contains(&"friend_activity".to_owned()));

    // Presence fans out to the user's other devices only.
    send(
        &mut phone_ws,
        json!({"type": "presence", "state": {"media": null, "playing": false}}),
    )
    .await;
    let seen = expect(&mut tablet_ws, "presence").await;
    assert_eq!(
        (seen["device_id"].as_str(), seen["device_name"].as_str()),
        (Some(alice.device.as_str()), Some("first"))
    );
    assert!(!types(&drain(&mut phone_ws, 300).await).contains(&"presence".to_owned()));
    // Rate limit: one presence per 2 s.
    send(
        &mut phone_ws,
        json!({"type": "presence", "rid": 3, "state": {"media": null}}),
    )
    .await;
    let limited = next(&mut phone_ws).await;
    assert_eq!(
        (limited["code"].as_str(), limited["rid"].as_i64()),
        (Some("rate_limited"), Some(3))
    );
    send(&mut tablet_ws, json!({"type": "presence_query", "rid": 4})).await;
    let snap = expect(&mut tablet_ws, "presence_snapshot").await;
    assert_eq!(snap["rid"], 4);
    assert_eq!(snap["devices"][0]["device_id"], alice.device.as_str());
    // Local media is never announced.
    send(
        &mut tablet_ws,
        json!({"type": "presence", "rid": 5, "state": {"media": {"url": "/sdcard/a.mp3"}}}),
    )
    .await;
    let invalid = expect(&mut tablet_ws, "error").await;
    assert_eq!(
        (invalid["code"].as_str(), invalid["rid"].as_i64()),
        (Some("invalid"), Some(5))
    );

    // Handoff and remote reach only the target device of the same user.
    send(&mut tablet_ws, json!({"type": "handoff", "rid": 6, "to_device": alice.device, "media": media("bbbbbbbbbbb", None),
                                "position_ms": 9000, "playing": true, "queue": [media("ccccccccccc", None)]})).await;
    assert_eq!(expect(&mut tablet_ws, "ack").await["rid"], 6);
    let handoff = expect(&mut phone_ws, "handoff").await;
    assert_eq!(
        (
            handoff["from_device"].as_str(),
            handoff["from_name"].as_str()
        ),
        (Some(tablet.device.as_str()), Some("Tablet"))
    );
    assert_eq!(handoff["queue"][0]["description"], "");
    assert!(!types(&drain(&mut tablet_ws, 300).await).contains(&"handoff".to_owned()));
    assert!(!types(&drain(&mut bob_ws, 100).await).contains(&"handoff".to_owned()));
    send(&mut tablet_ws, json!({"type": "remote", "rid": 7, "to_device": alice.device, "command": "seek", "position_ms": 1000})).await;
    assert_eq!(expect(&mut tablet_ws, "ack").await["rid"], 7);
    let remote = expect(&mut phone_ws, "remote").await;
    assert_eq!(
        (remote["command"].as_str(), remote["position_ms"].as_i64()),
        (Some("seek"), Some(1000))
    );
    // Another user's device is not found, as is the sender itself.
    for to in [&bob.device, &tablet.device] {
        send(
            &mut tablet_ws,
            json!({"type": "remote", "rid": 8, "to_device": to, "command": "play"}),
        )
        .await;
        let e = expect(&mut tablet_ws, "error").await;
        assert_eq!(
            (e["code"].as_str(), e["rid"].as_i64()),
            (Some("not_found"), Some(8))
        );
    }
    assert!(!types(&drain(&mut bob_ws, 300).await).contains(&"remote".to_owned()));
    // Without rid a failure is silent; an unknown type with rid is answered.
    send(
        &mut tablet_ws,
        json!({"type": "remote", "to_device": bob.device, "command": "play"}),
    )
    .await;
    send(&mut tablet_ws, json!({"type": "teleport", "rid": 9})).await;
    let e = expect(&mut tablet_ws, "error").await;
    assert_eq!(
        (e["code"].as_str(), e["rid"].as_i64()),
        (Some("unknown_type"), Some(9))
    );

    // Jam: room events reach members only; jam_ping is acknowledged for members.
    let (_, room) = c.post("/api/jam", Some(&alice.token), json!({})).await;
    let room_id = room["id"].as_str().unwrap().to_owned();
    c.post(
        &format!("/api/jam/{room_id}/invite"),
        Some(&alice.token),
        json!({"user_ids": [bob.id]}),
    )
    .await;
    let invite = expect(&mut bob_ws, "jam_invite").await;
    assert_eq!(invite["from"]["username"], "alice");
    c.post(
        &format!("/api/jam/{room_id}/join"),
        Some(&bob.token),
        json!({}),
    )
    .await;
    drain(&mut carol_ws, 100).await;
    c.post(
        &format!("/api/jam/{room_id}/queue"),
        Some(&bob.token),
        json!({"media": media("aaaaaaaaaaa", Some(200))}),
    )
    .await;
    let jam_state = expect(&mut phone_ws, "jam_state").await;
    assert_eq!(
        jam_state["playback"]["item"]["media_key"],
        "youtube:aaaaaaaaaaa"
    );
    assert_eq!(
        expect(&mut bob_ws, "jam_state").await["room_id"],
        room_id.as_str()
    );
    let carol_saw = types(&drain(&mut carol_ws, 300).await);
    assert!(
        carol_saw.iter().all(|t| !t.starts_with("jam_")),
        "{carol_saw:?}"
    );
    send(
        &mut bob_ws,
        json!({"type": "jam_ping", "room_id": room_id, "rid": 10}),
    )
    .await;
    assert_eq!(expect(&mut bob_ws, "ack").await["rid"], 10);
    send(
        &mut carol_ws,
        json!({"type": "jam_ping", "room_id": room_id, "rid": 11}),
    )
    .await;
    assert_eq!(expect(&mut carol_ws, "error").await["code"], "not_found");
    // Kicked members get jam_ended.
    delete(
        &c,
        &format!("/api/jam/{room_id}/members/{}", bob.id),
        &alice.token,
    )
    .await;
    let ended = expect(&mut bob_ws, "jam_ended").await;
    assert_eq!(ended["reason"], "removed");
    // Expiry is announced when a member next touches the room.
    advance(31 * 60_000);
    send(
        &mut phone_ws,
        json!({"type": "jam_ping", "room_id": room_id, "rid": 12}),
    )
    .await;
    let (mut expired, mut refused) = (false, false);
    while !(expired && refused) {
        let v = next(&mut phone_ws).await;
        expired |= v["type"] == "jam_ended" && v["reason"] == "expired";
        refused |= v["type"] == "error" && v["code"] == "not_found" && v["rid"] == 12;
    }

    // Disconnecting the last socket of a device announces presence_gone.
    drop(phone_ws);
    let gone = expect(&mut tablet_ws, "presence_gone").await;
    assert_eq!(gone["device_id"], alice.device.as_str());
    // The handoff target is gone now.
    send(
        &mut tablet_ws,
        json!({"type": "remote", "rid": 13, "to_device": alice.device, "command": "play"}),
    )
    .await;
    assert_eq!(expect(&mut tablet_ws, "error").await["code"], "not_found");

    // Three malformed messages close the socket with 1008; binary frames with 1003.
    for _ in 0..3 {
        carol_ws
            .send(WsMessage::Text("not json".into()))
            .await
            .unwrap();
    }
    for _ in 0..3 {
        assert_eq!(expect(&mut carol_ws, "error").await["code"], "bad_message");
    }
    assert_eq!(close_code(&mut carol_ws).await, 1008);
    bob_ws
        .send(WsMessage::Binary(vec![1, 2, 3].into()))
        .await
        .unwrap();
    assert_eq!(close_code(&mut bob_ws).await, 1003);

    // Other messages: 20 per 10 s.
    let (mut burst, _) = connect(addr, &carol.token).await;
    for i in 0..21 {
        send(&mut burst, json!({"type": "ping", "t0": i, "rid": i + 1})).await;
    }
    let mut codes = vec![];
    for _ in 0..21 {
        let v = next(&mut burst).await;
        codes.push(v["type"].as_str().unwrap().to_owned());
    }
    assert_eq!(codes.iter().filter(|t| *t == "pong").count(), 20);
    assert_eq!(codes.last().map(String::as_str), Some("error"));
}
