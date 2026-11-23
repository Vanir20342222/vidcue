//! B5 database tests: health log, stats, recommendations and administration.

use super::*;
use crate::recs::fakes::{FakeMedia, FakeMusic, config, media};
use crate::recs::{Recs, playable_key};
use crate::stats::{DAY_MS, days_from_civil};
use axum::http::HeaderMap;
use std::sync::Arc;
use uuid::Uuid;
use vidcue_core::MediaKind;

const PW: &str = "first password!";

/// A raw request; returns status, headers and the body as text.
async fn raw(
    c: &Client,
    method: Method,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<(&str, String)>,
) -> (StatusCode, HeaderMap, String) {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(ip) = &c.ip {
        req = req.header("x-real-ip", ip);
    }
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let req = match body {
        Some((kind, body)) => req.header("content-type", kind).body(Body::from(body)),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let res = c.app.clone().oneshot(req).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        headers,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

fn form(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(k, v)| format!("{k}={}", crate::web::query_escape(v)))
        .collect::<Vec<_>>()
        .join("&")
}

async fn post_form(
    c: &Client,
    path: &str,
    cookie: Option<&str>,
    extra: &[(&str, &str)],
    fields: &[(&str, &str)],
) -> (StatusCode, HeaderMap, String) {
    let cookie = cookie.map(|t| format!("vidcue_admin={t}"));
    let mut headers: Vec<(&str, &str)> = extra.to_vec();
    if let Some(cookie) = &cookie {
        headers.push(("cookie", cookie));
    }
    raw(
        c,
        Method::POST,
        path,
        &headers,
        Some(("application/x-www-form-urlencoded", form(fields))),
    )
    .await
}

async fn put(c: &Client, path: &str, token: &str, body: Value) -> (StatusCode, Value) {
    c.call(Method::PUT, path, Some(token), Some(body)).await
}

async fn push(c: &Client, token: &str, ops: Vec<(&str, String, Value)>) {
    let ops: Vec<Value> = ops
        .into_iter()
        .map(|(collection, key, value)| {
            json!({"id": Uuid::new_v4(), "collection": collection, "key": key, "value": value, "clock": api::now_ms()})
        })
        .collect();
    let (status, res) = c
        .post("/api/sync/push", Some(token), json!({ "ops": ops }))
        .await;
    assert_eq!(status, StatusCode::OK, "{res}");
    for r in res["results"].as_array().unwrap() {
        assert_eq!(r["status"], "applied", "{res}");
    }
}

async fn user_id(pool: &deadpool_postgres::Pool, name: &str) -> Uuid {
    pool.get()
        .await
        .unwrap()
        .query_one("SELECT id FROM users WHERE username = $1", &[&name])
        .await
        .unwrap()
        .get(0)
}

fn report(kind: &str, category: &str, at: i64, duration: Option<i64>) -> Value {
    json!({"kind": kind, "category": category, "client_version": "0.2.0-beta.3",
           "platform": "android", "at": at, "duration_ms": duration})
}

#[tokio::test]
async fn health_reports_summary_admin_cap_and_retention() {
    let Some((pool, _guard)) = fresh_db().await else {
        return;
    };
    let c = client(&pool);
    let root = register(&c, &pool, "root", true).await;
    let bob = register(&c, &pool, "bob", false).await;
    let now = api::now_ms();

    let (status, res) = c
        .post(
            "/api/health/reports",
            Some(&bob.token),
            json!({"reports": [
                report("start", "ok", now - 1000, Some(840)),
                report("start", "http_403", now - 500, None),
                report("resolve", "ok", now - 400, None),
                {"kind": "error", "category": "unknown", "client_version": "1", "platform": "android",
                 "at": now, "message": "https://example.test/?sig=secret"},
                report("stall", "network_timeout", now - 40 * DAY_MS, None),
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res, json!({"accepted": 3, "rejected": 2}));
    // Nothing of the rejected report was stored.
    let stored: i64 = pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT COUNT(*) FROM health_reports", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(stored, 3);

    let many: Vec<Value> = (0..101).map(|_| report("start", "ok", now, None)).collect();
    let (status, _) = c
        .post(
            "/api/health/reports",
            Some(&bob.token),
            json!({ "reports": many }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        c.post("/api/health/reports", None, json!({"reports": []}))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    let (status, summary) = c.get("/api/health/summary?days=7", Some(&bob.token)).await;
    assert_eq!(status, StatusCode::OK, "{summary}");
    assert_eq!(summary["total"], 3);
    assert_eq!(
        summary["by_kind"]["start"],
        json!({"total": 2, "ok": 1, "ok_rate": 0.5})
    );
    assert_eq!(summary["by_kind"]["resolve"]["ok_rate"], 1.0);
    assert_eq!(summary["by_kind"]["stall"], json!({"total": 0}));
    assert_eq!(summary["by_kind"]["download"]["ok_rate"], Value::Null);
    assert_eq!(
        summary["top_categories"],
        json!([{"kind": "start", "category": "http_403", "count": 1}])
    );
    assert_eq!(summary["start_ms"], json!({"p50": 840, "p90": 840}));
    assert_eq!(summary["by_day"].as_array().unwrap().len(), 1);
    let (_, own) = c.get("/api/health/summary", Some(&root.token)).await;
    assert_eq!(own["total"], 0, "summaries are per user");
    for bad in ["days=0", "days=91", "days=x"] {
        let (status, body) = c
            .get(&format!("/api/health/summary?{bad}"), Some(&bob.token))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        assert!(body["error"].is_string());
    }

    assert_eq!(
        c.get("/api/admin/health?days=7", Some(&bob.token)).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, all) = c.get("/api/admin/health?days=7", Some(&root.token)).await;
    assert_eq!(status, StatusCode::OK, "{all}");
    assert_eq!(all["users_reporting"], 1);
    assert_eq!(
        all["by_version"],
        json!([{"client_version": "0.2.0-beta.3", "total": 3, "ok_rate": 0.667}])
    );

    // Daily cap: 2000 accepted reports per user per rolling 24 hours.
    let bob_id = user_id(&pool, "bob").await;
    pool.get()
        .await
        .unwrap()
        .execute(
            "INSERT INTO health_reports (user_id, device_id, kind, category, client_version, platform, at, received_at)
             SELECT $1, $1, 'start', 'ok', '1', 'android', $2, $2 FROM generate_series(1, 1995)",
            &[&bob_id, &now],
        )
        .await
        .unwrap();
    let five: Vec<Value> = (0..5).map(|_| report("start", "ok", now, None)).collect();
    let (_, res) = c
        .post(
            "/api/health/reports",
            Some(&bob.token),
            json!({ "reports": five }),
        )
        .await;
    assert_eq!(res, json!({"accepted": 2, "rejected": 3}));

    // 30 uploads per device and hour; bob's device used 2 of them (the oversized one never counted).
    let mut last = StatusCode::OK;
    for _ in 0..28 {
        last = c
            .post(
                "/api/health/reports",
                Some(&bob.token),
                json!({"reports": []}),
            )
            .await
            .0;
    }
    assert_eq!(last, StatusCode::OK);
    assert_eq!(
        c.post(
            "/api/health/reports",
            Some(&bob.token),
            json!({"reports": []})
        )
        .await
        .0,
        StatusCode::TOO_MANY_REQUESTS
    );

    // Retention: 90 days.
    pool.get()
        .await
        .unwrap()
        .execute(
            "INSERT INTO health_reports (user_id, device_id, kind, category, client_version, platform, at, received_at)
             VALUES ($1, $1, 'start', 'ok', '1', 'android', $2, $2)",
            &[&bob_id, &(now - 91 * DAY_MS)],
        )
        .await
        .unwrap();
    api::maintenance(&pool, now).await.unwrap();
    let (old, total): (i64, i64) = {
        let row = pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT COUNT(*) FILTER (WHERE at < $1), COUNT(*) FROM health_reports",
                &[&(now - 90 * DAY_MS)],
            )
            .await
            .unwrap();
        (row.get(0), row.get(1))
    };
    assert_eq!((old, total), (0, 2000));
}

fn play(
    id: &str,
    creator: &str,
    kind: &str,
    at: i64,
    seconds: f64,
) -> (&'static str, String, Value) {
    (
        "history",
        Uuid::new_v4().to_string(),
        json!({"url": format!("https://www.youtube.com/watch?v={id}"), "title": format!("Title {id}"),
               "creator": creator, "kind": kind, "thumbnail": format!("https://i.ytimg.com/vi/{id}/hq.jpg"),
               "at": at, "seconds": seconds, "duration": 300.0, "finished": seconds >= 295.0}),
    )
}

#[tokio::test]
async fn stats_period_and_wrapped_from_synced_history() {
    let Some((pool, _guard)) = fresh_db().await else {
        return;
    };
    let c = client(&pool);
    let bob = register(&c, &pool, "bob", false).await;
    let eve = register(&c, &pool, "eve", false).await;
    let day = |y, m, d| days_from_civil(y, m, d) * DAY_MS;
    // 2026-11-22 23:30 UTC is 2026-11-23 00:30 at UTC+1.
    let late = day(2026, 11, 22) + 23 * 3_600_000 + 30 * 60_000;
    push(
        &c,
        &bob.token,
        vec![
            play("aaaaaaaaaaa", "Ann", "song", late, 200.0),
            play("aaaaaaaaaaa", "Ann", "song", late + 600_000, 400.0), // clamped to 300
            play(
                "bbbbbbbbbbb",
                "Bob Channel",
                "video",
                late + 3_600_000,
                120.0,
            ),
            play("ccccccccccc", "Ann", "song", late + 7_200_000, 4.0), // accidental start
            play("ddddddddddd", "Old", "song", day(2025, 6, 1), 100.0),
            play("eeeeeeeeeee", "Old", "song", day(2026, 2, 1), 100.0),
            play("eeeeeeeeeee", "Old", "song", day(2026, 2, 2), 100.0),
        ],
    )
    .await;
    push(
        &c,
        &eve.token,
        vec![play("zzzzzzzzzzz", "Zed", "song", late, 100.0)],
    )
    .await;

    let (from, to) = (day(2026, 11, 1), day(2026, 12, 1));
    let (status, stats) = c
        .get(
            &format!("/api/stats?from={from}&to={to}&tz_offset_min=60"),
            Some(&bob.token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{stats}");
    assert_eq!(stats["plays"], 3);
    assert_eq!(stats["seconds"], 620.0);
    assert_eq!(stats["distinct_items"], 2);
    assert_eq!(stats["finished"], 1);
    assert_eq!(
        stats["by_kind"]["song"],
        json!({"plays": 2, "seconds": 500.0})
    );
    assert_eq!(
        stats["by_day"],
        json!([{"day": "2026-11-23", "seconds": 620.0, "plays": 3}])
    );
    assert_eq!(stats["by_hour"][0], 500.0);
    assert_eq!(stats["by_hour"][1], 120.0);
    assert_eq!(stats["top_creators"][0]["creator"], "Ann");
    assert_eq!(
        stats["top_creators"][0]["thumbnail"],
        "https://i.ytimg.com/vi/aaaaaaaaaaa/hq.jpg"
    );
    assert_eq!(stats["top_items"][0]["plays"], 2);
    let (_, utc) = c
        .get(&format!("/api/stats?from={from}&to={to}"), Some(&bob.token))
        .await;
    assert_eq!(utc["by_day"].as_array().unwrap().len(), 2, "{utc}");

    for bad in [
        format!("from={to}&to={from}"),
        format!("from={from}&to={}", from + 367 * DAY_MS),
        format!("from={from}"),
        format!("from={from}&to={to}&tz_offset_min=900"),
        "from=x&to=y".into(),
    ] {
        let (status, body) = c.get(&format!("/api/stats?{bad}"), Some(&bob.token)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        assert!(body["error"].is_string(), "{bad}: {body}");
    }

    let (status, w) = c
        .get(
            "/api/stats/wrapped?year=2026&tz_offset_min=60",
            Some(&bob.token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    assert_eq!(w["plays"], 5);
    assert_eq!(
        w["new_creators"], 2,
        "Ann and Bob Channel; Old started in 2025"
    );
    assert_eq!(w["by_month"][1]["seconds"], 200.0);
    assert_eq!(w["longest_streak_days"], 2);
    assert_eq!(w["first_play"]["creator"], "Old");
    assert_eq!(w["top_videos"][0]["creator"], "Bob Channel");
    // Cached: the same computation comes back.
    let (_, again) = c
        .get(
            "/api/stats/wrapped?year=2026&tz_offset_min=60",
            Some(&bob.token),
        )
        .await;
    assert_eq!(again["computed_at"], w["computed_at"]);
    let (_, utc) = c
        .get("/api/stats/wrapped?year=2026", Some(&bob.token))
        .await;
    assert_eq!(utc["plays"], 5);
    let rows: i64 = pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT COUNT(*) FROM stats_wrapped", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(rows, 1, "one cached review per user and year");

    let (status, empty) = c
        .get("/api/stats/wrapped?year=2021", Some(&bob.token))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty["plays"], 0);
    assert_eq!(empty["seconds"], 0.0);
    assert!(empty["top_items"].as_array().unwrap().is_empty());
    for bad in ["year=2019", "year=9999", "", "year=2026&tz_offset_min=-841"] {
        assert_eq!(
            c.get(&format!("/api/stats/wrapped?{bad}"), Some(&bob.token))
                .await
                .0,
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }
}

fn recs_client(pool: &deadpool_postgres::Pool, recs: Arc<Recs>) -> (Client, api::AppState) {
    let state = api::AppState::new(
        pool.clone(),
        api::Settings {
            public_url: "https://vidcue.test".into(),
            key: ServerKey::from_master([9; 32]),
            trusted_proxies: vec![],
            proxies_only: false,
        },
    );
    let app = crate::recs::routes_with(recs).with_state(state.clone());
    (
        Client {
            app,
            ip: None,
            peer: None,
        },
        state,
    )
}

const CHANNEL_A: &str = "UCaaaaaaaaaaaaaaaaaaaaaa";
const CHANNEL_B: &str = "UCbbbbbbbbbbbbbbbbbbbbbb";
const MBID: &str = "a74b1b7f-71a5-4011-9441-d0b5e4122711";
const SIMILAR_MBID: &str = "8ed2e0b3-aa4c-4e13-bec3-dc7393ed4d6b";

fn fakes() -> (Arc<FakeMedia>, Arc<FakeMusic>) {
    let v = |id: &str, title: &str| media(id, title, "Chan A", MediaKind::Video);
    let mut live = v("lllllllllll", "Live now");
    live.live = true;
    let mut fake = FakeMedia::default();
    fake.uploads.insert(
        CHANNEL_A.into(),
        vec![
            v("saved000001", "Saved already"),
            v("trailer0001", "Official TRAILER"),
            v("upload00001", "Upload 1"),
            v("upload00002", "Upload 2"),
            v("upload00003", "Upload 3"),
            v("upload00004", "Upload 4"),
        ],
    );
    fake.related.insert(
        "songseed001".into(),
        vec![
            media("songseed001", "Seed itself", "Radiohead", MediaKind::Song),
            media("related0001", "Related 1", "X", MediaKind::Song),
            media("related0001", "Related 1 again", "X", MediaKind::Song),
            live,
            media("PLplaylist1", "A playlist", "X", MediaKind::Playlist),
            media(
                "related0002",
                "Related 2",
                "Blocked Artist",
                MediaKind::Song,
            ),
            media("related0003", "Related 3", "Y", MediaKind::Song),
        ],
    );
    fake.songs.insert(
        "Thom Yorke".into(),
        vec![media(
            "similar0001",
            "Similar song",
            "Thom Yorke",
            MediaKind::Song,
        )],
    );
    let mut music = FakeMusic::default();
    music.mbids.insert("Radiohead".into(), MBID.into());
    music.similar.insert(
        MBID.into(),
        vec![
            ("radiohead".into(), MBID.into()),
            ("Thom Yorke".into(), SIMILAR_MBID.into()),
        ],
    );
    (Arc::new(fake), Arc::new(music))
}

#[tokio::test]
async fn recommendations_build_offline_with_fakes() {
    let Some((pool, _guard)) = fresh_db().await else {
        return;
    };
    let main = client(&pool);
    let bob = register(&main, &pool, "bob", false).await;
    let bob_id = user_id(&pool, "bob").await;
    let now = api::now_ms();
    let saved = media("saved000001", "Saved already", "Chan A", MediaKind::Video);
    push(
        &main,
        &bob.token,
        vec![
            (
                "subscriptions",
                CHANNEL_A.into(),
                json!({"channel_id": CHANNEL_A, "title": "Chan A"}),
            ),
            (
                "subscriptions",
                CHANNEL_B.into(),
                json!({"channel_id": CHANNEL_B, "title": "Chan B"}),
            ),
            play("songseed001", "Radiohead", "song", now - DAY_MS, 300.0),
            (
                "saved",
                playable_key(&saved).unwrap(),
                serde_json::to_value(&saved).unwrap(),
            ),
            (
                "filters",
                Uuid::new_v4().to_string(),
                json!({"kind": "keyword", "value": "trailer"}),
            ),
            (
                "filters",
                Uuid::new_v4().to_string(),
                json!({"kind": "channel", "value": "blocked artist"}),
            ),
        ],
    )
    .await;

    let (fake, music) = fakes();
    let recs = Arc::new(Recs::new(config(true, 100), fake.clone(), music.clone()));
    let (c, state) = recs_client(&pool, recs.clone());

    let (status, features) = c.get("/api/features", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        features,
        json!({"features": ["health", "stats", "recs", "admin_web"], "recs_available": true})
    );
    // Off by default; nothing is built.
    assert_eq!(
        c.get("/api/recs/settings", Some(&bob.token)).await.1,
        json!({"enabled": false, "music": true})
    );
    assert_eq!(
        c.get("/api/recs", Some(&bob.token)).await.1,
        json!({"enabled": false, "items": []})
    );
    assert_eq!(
        c.post("/api/recs/refresh", Some(&bob.token), json!({}))
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        put(
            &c,
            "/api/recs/settings",
            &bob.token,
            json!({"enabled": true, "bogus": 1})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, settings) = put(
        &c,
        "/api/recs/settings",
        &bob.token,
        json!({"enabled": true, "music": true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{settings}");

    let count = crate::recs::build(&pool, &recs, bob_id, now).await.unwrap();
    let (status, feed) = c.get("/api/recs", Some(&bob.token)).await;
    assert_eq!(status, StatusCode::OK, "{feed}");
    assert_eq!(feed["enabled"], true);
    assert_eq!(feed["built_at"], now);
    assert_eq!(feed["building"], false, "a fresh feed is not rebuilt");
    let items = feed["items"].as_array().unwrap();
    assert_eq!(items.len(), count);
    let ids: Vec<&str> = items
        .iter()
        .map(|i| i["media"]["id"].as_str().unwrap())
        .collect();
    let reasons: Vec<&str> = items
        .iter()
        .map(|i| i["reason"].as_str().unwrap())
        .collect();
    // Round robin; saved, filtered, live, playlist, duplicate and played items dropped.
    assert_eq!(
        ids,
        [
            "upload00001",
            "related0001",
            "similar0001",
            "upload00002",
            "related0003",
            "upload00003"
        ]
    );
    assert_eq!(
        reasons,
        [
            "subscription",
            "related",
            "similar_artist",
            "subscription",
            "related",
            "subscription"
        ]
    );
    assert_eq!(items[0]["source"], "Chan A");
    assert_eq!(items[1]["source"], "Title songseed001");
    assert_eq!(items[2]["source"], "Thom Yorke");
    assert_eq!(items[0]["media"]["description"], "", "slim media");
    let calls = fake.calls.lock().unwrap().clone();
    assert!(
        calls.contains(&format!("uploads:{CHANNEL_B}")),
        "failing channel is skipped"
    );
    assert!(calls.contains(&"related:songseed001:true".to_owned()));
    assert_eq!(
        music.calls.lock().unwrap().clone(),
        vec!["mbid:Radiohead".to_owned(), format!("similar:{MBID}")]
    );

    // A second build is served from the shared cache (the failed channel is retried).
    let before = (
        fake.calls.lock().unwrap().len(),
        music.calls.lock().unwrap().len(),
    );
    crate::recs::build(&pool, &recs, bob_id, now + 1)
        .await
        .unwrap();
    let after = (
        fake.calls.lock().unwrap().len(),
        music.calls.lock().unwrap().len(),
    );
    assert_eq!((after.0 - before.0, after.1 - before.1), (1, 0));
    let keys: Vec<String> = pool
        .get()
        .await
        .unwrap()
        .query("SELECT key FROM recs_cache ORDER BY key", &[])
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert!(keys.contains(&format!("yt:uploads:{CHANNEL_A}")));
    assert!(keys.contains(&"mb:artist:radiohead".to_owned()));
    assert!(keys.contains(&format!("lb:similar:{MBID}")));
    assert!(keys.contains(&"yt:songs:thom yorke".to_owned()));
    assert!(keys.contains(&format!("rotation:{bob_id}")));

    // Budget exhaustion: one upstream request per hour stores what it has.
    pool.get()
        .await
        .unwrap()
        .execute(
            "DELETE FROM recs_cache WHERE key NOT LIKE 'rotation:%'",
            &[],
        )
        .await
        .unwrap();
    let (fake2, music2) = fakes();
    let tight = Recs::new(config(true, 1), fake2.clone(), music2.clone());
    crate::recs::build(&pool, &tight, bob_id, now + 2)
        .await
        .unwrap();
    assert_eq!(
        fake2.calls.lock().unwrap().len() + music2.calls.lock().unwrap().len(),
        1
    );
    let (_, feed) = c.get("/api/recs", Some(&bob.token)).await;
    assert!(
        feed["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["reason"] == "subscription"),
        "{feed}"
    );

    // Background refresh: 202, then at most one build per 30 minutes.
    let (status, res) = c
        .post("/api/recs/refresh", Some(&bob.token), json!({}))
        .await;
    assert_eq!(
        (status, res),
        (StatusCode::ACCEPTED, json!({"building": true}))
    );
    assert_eq!(
        c.post("/api/recs/refresh", Some(&bob.token), json!({}))
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while recs.is_building(bob_id) {
        assert!(std::time::Instant::now() < deadline, "build did not finish");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let (_, feed) = c.get("/api/recs", Some(&bob.token)).await;
    assert!(feed["built_at"].as_i64().unwrap() >= now, "{feed}");
    assert_eq!(feed["items"].as_array().unwrap().len(), 6);
    drop(state);

    // Switching off deletes the feed and the rotation memory.
    put(
        &c,
        "/api/recs/settings",
        &bob.token,
        json!({"enabled": false}),
    )
    .await;
    let left: i64 = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT (SELECT COUNT(*) FROM recs_feed) + (SELECT COUNT(*) FROM recs_cache WHERE key LIKE 'rotation:%')",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(left, 0);
    assert_eq!(
        crate::recs::build(&pool, &recs, bob_id, now).await.unwrap(),
        0
    );

    // Maintenance drops expired cache rows.
    api::maintenance(&pool, now + 400 * DAY_MS).await.unwrap();
    let cached: i64 = pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT COUNT(*) FROM recs_cache", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(cached, 0);

    // VIDCUE_RECS=off: every endpoint is 404 and the feature is reported unavailable.
    let (fake3, music3) = fakes();
    let (off, _) = recs_client(
        &pool,
        Arc::new(Recs::new(config(false, 100), fake3, music3)),
    );
    assert_eq!(
        off.get("/api/features", None).await.1["recs_available"],
        false
    );
    for path in ["/api/recs", "/api/recs/settings"] {
        let (status, body) = off.get(path, Some(&bob.token)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(body["error"].is_string());
    }
    assert_eq!(
        off.post("/api/recs/refresh", Some(&bob.token), json!({}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    let set = headers.get("set-cookie")?.to_str().ok()?;
    let value = set.strip_prefix("vidcue_admin=")?.split(';').next()?;
    (!value.is_empty()).then(|| value.to_owned())
}

fn csrf_of(page: &str) -> String {
    let marker = "name=\"csrf\" value=\"";
    let start = page.find(marker).unwrap() + marker.len();
    page[start..][..page[start..].find('"').unwrap()].to_owned()
}

async fn admin_login(c: &Client, user: &str, code: &str) -> (StatusCode, HeaderMap, String) {
    post_form(
        c,
        "/admin/login",
        None,
        &[
            ("origin", "https://vidcue.test"),
            ("sec-fetch-site", "same-origin"),
        ],
        &[("username", user), ("password", PW), ("code", code)],
    )
    .await
}

#[tokio::test]
async fn admin_web_login_sessions_and_csrf() {
    let Some((pool, _guard)) = fresh_db().await else {
        return;
    };
    let c = client(&pool);
    let root = register(&c, &pool, "root", true).await;
    let bob = register(&c, &pool, "bob", false).await;

    let (status, headers, page) = raw(&c, Method::GET, "/admin", &[], None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("action=\"/admin/login\"") && !page.contains("<script"));
    let csp = headers["content-security-policy"].to_str().unwrap();
    assert!(csp.contains(crate::admin_web::style_hash()));
    assert_eq!(headers["cache-control"], "no-store");

    // A non-admin with correct credentials fails exactly like a wrong password.
    let (s1, _, p1) = admin_login(&c, "bob", &bob.recovery[0]).await;
    let (s2, _, p2) = post_form(
        &c,
        "/admin/login",
        None,
        &[],
        &[
            ("username", "root"),
            ("password", "wrong password"),
            ("code", "123456"),
        ],
    )
    .await;
    assert_eq!(
        (s1, s2),
        (StatusCode::UNAUTHORIZED, StatusCode::UNAUTHORIZED)
    );
    assert_eq!(p1, p2);
    // Cross-site posts are refused before anything is checked.
    let (status, _, _) = post_form(
        &c,
        "/admin/login",
        None,
        &[("origin", "https://evil.example")],
        &[
            ("username", "root"),
            ("password", PW),
            ("code", &root.recovery[0]),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = post_form(
        &c,
        "/admin/login",
        None,
        &[("sec-fetch-site", "cross-site")],
        &[
            ("username", "root"),
            ("password", PW),
            ("code", &root.recovery[0]),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, headers, _) = admin_login(&c, "root", &root.recovery[0]).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers["location"], "/admin");
    let set = headers["set-cookie"].to_str().unwrap();
    for part in [
        "HttpOnly",
        "Secure",
        "SameSite=Strict",
        "Path=/admin",
        "Max-Age=1800",
    ] {
        assert!(set.contains(part), "{set}");
    }
    let token = cookie_token(&headers).unwrap();
    assert!(token.starts_with("vca_"));
    // Stored only as a hash.
    let stored: Vec<u8> = pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT token_hash FROM admin_sessions", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(stored, auth::token_hash(&token));

    let cookie = format!("vidcue_admin={token}");
    let (status, _, page) = raw(&c, Method::GET, "/admin", &[("cookie", &cookie)], None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains(">bob<") && page.contains("Playback health"));
    assert!(
        !page.contains("vcd_") && !page.contains(&root.secret) && !page.contains(&root.recovery[1])
    );
    let csrf = csrf_of(&page);

    // CSRF and origin checks on actions.
    let (status, _, _) =
        post_form(&c, "/admin/invites", Some(&token), &[], &[("password", PW)]).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "missing csrf");
    let (status, _, _) = post_form(
        &c,
        "/admin/invites",
        Some(&token),
        &[],
        &[("csrf", "wrong"), ("password", PW)],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = post_form(
        &c,
        "/admin/invites",
        Some(&token),
        &[("sec-fetch-site", "same-site")],
        &[("csrf", &csrf), ("password", PW)],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = post_form(
        &c,
        "/admin/invites",
        None,
        &[],
        &[("csrf", &csrf), ("password", PW)],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no session");
    let invites: i64 = pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT COUNT(*) FROM invites", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(invites, 2, "refused requests changed nothing");
    let (status, _, page) = post_form(
        &c,
        "/admin/invites",
        Some(&token),
        &[
            ("origin", "https://vidcue.test"),
            ("sec-fetch-site", "same-origin"),
        ],
        &[
            ("csrf", &csrf),
            ("password", PW),
            ("admin", "on"),
            ("hours", "5"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains("https://vidcue.test/join#vci_") && page.contains("<svg"));
    let (status, _, page) = post_form(
        &c,
        "/admin/invites",
        Some(&token),
        &[],
        &[("csrf", &csrf), ("password", "wrong password")],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(page.contains("Wrong password"));

    // Reset link for a user (password + code), shown once.
    let bob_id = user_id(&pool, "bob").await;
    let (status, _, page) = post_form(
        &c,
        &format!("/admin/users/{bob_id}/reset"),
        Some(&token),
        &[],
        &[
            ("csrf", &csrf),
            ("password", PW),
            ("code", &root.recovery[1]),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains("https://vidcue.test/reset#vcr_"));

    // At most three sessions per admin.
    for code in &root.recovery[2..5] {
        let (status, _, _) = admin_login(&c, "root", code).await;
        assert_eq!(status, StatusCode::SEE_OTHER);
    }
    let sessions: i64 = pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT COUNT(*) FROM admin_sessions", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(sessions, 3);
    let (_, _, page) = raw(&c, Method::GET, "/admin", &[("cookie", &cookie)], None).await;
    assert!(
        page.contains("action=\"/admin/login\""),
        "oldest session was dropped"
    );

    // Idle timeout of 30 minutes.
    let (_, headers, _) = admin_login(&c, "root", &root.recovery[5]).await;
    let token = cookie_token(&headers).unwrap();
    let cookie = format!("vidcue_admin={token}");
    let (_, _, page) = raw(&c, Method::GET, "/admin", &[("cookie", &cookie)], None).await;
    let csrf = csrf_of(&page);
    pool.get()
        .await
        .unwrap()
        .execute(
            "UPDATE admin_sessions SET last_used = last_used - 31 * 60000 WHERE token_hash = $1",
            &[&auth::token_hash(&token)],
        )
        .await
        .unwrap();
    let (_, headers, page) = raw(&c, Method::GET, "/admin", &[("cookie", &cookie)], None).await;
    assert!(page.contains("action=\"/admin/login\""));
    assert!(
        headers["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    api::maintenance(&pool, api::now_ms()).await.unwrap();
    let left: i64 = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT COUNT(*) FROM admin_sessions WHERE token_hash = $1",
            &[&auth::token_hash(&token)],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(left, 0, "maintenance removes idle sessions");
    let _ = csrf;

    // Logout deletes the session.
    let (_, headers, _) = admin_login(&c, "root", &root.recovery[6]).await;
    let token = cookie_token(&headers).unwrap();
    let cookie = format!("vidcue_admin={token}");
    let (_, _, page) = raw(&c, Method::GET, "/admin", &[("cookie", &cookie)], None).await;
    let csrf = csrf_of(&page);
    let (status, headers, _) =
        post_form(&c, "/admin/logout", Some(&token), &[], &[("csrf", &csrf)]).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert!(
        headers["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    let (_, _, page) = raw(&c, Method::GET, "/admin", &[("cookie", &cookie)], None).await;
    assert!(page.contains("action=\"/admin/login\""));

    // Login throttle shares the /api/login limits under admin- keys.
    let attacker = c.from("203.0.113.9");
    let mut last = StatusCode::OK;
    for _ in 0..6 {
        last = post_form(
            &attacker,
            "/admin/login",
            None,
            &[],
            &[
                ("username", "root"),
                ("password", "wrong password"),
                ("code", "000000"),
            ],
        )
        .await
        .0;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
    let keys: Vec<String> = pool
        .get()
        .await
        .unwrap()
        .query("SELECT DISTINCT key FROM auth_failures ORDER BY key", &[])
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert!(
        keys.iter().any(|k| k == "admin-login:root:203.0.113.9"),
        "{keys:?}"
    );
    assert!(keys.iter().all(|k| !k.starts_with("login")), "{keys:?}");
    // The app login is unaffected.
    let (status, _) = attacker
        .post(
            "/api/login",
            None,
            json!({"username": "root", "password": PW, "code": root.recovery[7]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn admin_disable_enable_and_device_endpoints() {
    let Some((pool, _guard)) = fresh_db().await else {
        return;
    };
    let c = client(&pool);
    let root = register(&c, &pool, "root", true).await;
    let ada = register(&c, &pool, "ada", true).await;
    let bob = register(&c, &pool, "bob", false).await;
    let (root_id, ada_id, bob_id) = (
        user_id(&pool, "root").await,
        user_id(&pool, "ada").await,
        user_id(&pool, "bob").await,
    );
    // A second device of bob, paired from the first.
    let (_, pairing) = c
        .post("/api/pairing", Some(&bob.token), json!({"password": PW}))
        .await;
    let (_, tablet) = c
        .post(
            "/api/pairing/claim",
            None,
            json!({"code": pairing["code"], "device_name": "Tab"}),
        )
        .await;
    let tablet = tablet["token"].as_str().unwrap().to_owned();

    assert_eq!(
        c.get(
            &format!("/api/admin/users/{bob_id}/devices"),
            Some(&bob.token)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, devices) = c
        .get(
            &format!("/api/admin/users/{bob_id}/devices"),
            Some(&root.token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{devices}");
    let list = devices["devices"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert!(list[1]["paired_by"] == list[0]["id"]);
    assert!(list[0].get("token").is_none() && list[0].get("token_hash").is_none());
    assert_eq!(
        c.get(
            &format!("/api/admin/users/{}/devices", Uuid::new_v4()),
            Some(&root.token)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    // Revoking one device revokes the devices it paired.
    let revoke = format!("/api/admin/users/{bob_id}/devices/revoke");
    assert_eq!(
        c.post(
            &revoke,
            Some(&root.token),
            json!({"password": "wrong password", "device_id": null})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, res) = c
        .post(
            &revoke,
            Some(&root.token),
            json!({"password": PW, "device_id": list[0]["id"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["revoked"].as_array().unwrap().len(), 2);
    for token in [&bob.token, &tablet] {
        assert_eq!(
            c.get("/api/me", Some(token)).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    let (status, bob2) = c
        .post(
            "/api/login",
            None,
            json!({"username": "bob", "password": PW, "code": bob.recovery[0]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let bob2 = bob2["token"].as_str().unwrap().to_owned();
    let (status, res) = c
        .post(
            &revoke,
            Some(&root.token),
            json!({"password": PW, "device_id": null}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(res["revoked"].as_array().unwrap().len(), 1);
    assert_eq!(
        c.get("/api/me", Some(&bob2)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        c.post(
            &revoke,
            Some(&root.token),
            json!({"password": PW, "device_id": Uuid::new_v4()})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    // Disable: not oneself; signs out and refuses login, tokens and resets.
    assert_eq!(
        c.post(
            &format!("/api/admin/users/{root_id}/disable"),
            Some(&root.token),
            json!({"password": PW})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        c.post(
            &format!("/api/admin/users/{ada_id}/disable"),
            Some(&bob2),
            json!({"password": PW})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (_, pending) = c
        .post("/api/pairing", Some(&ada.token), json!({"password": PW}))
        .await;
    let (status, res) = c
        .post(
            &format!("/api/admin/users/{ada_id}/disable"),
            Some(&root.token),
            json!({"password": PW}),
        )
        .await;
    assert_eq!((status, res), (StatusCode::OK, json!({"disabled": true})));
    assert_eq!(
        c.get("/api/me", Some(&ada.token)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        c.post("/api/pairing/claim", None, json!({"code": pending["code"]}))
            .await
            .0,
        StatusCode::BAD_REQUEST,
        "open pairings are dropped"
    );
    let ada_login = |code: &str| json!({"username": "ada", "password": PW, "code": code});
    assert_eq!(
        c.post("/api/login", None, ada_login(&ada.recovery[0]))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert!(
        crate::account::user_id(&pool, "ada")
            .await
            .unwrap()
            .is_none()
    );
    let (status, body) = c
        .post(
            &format!("/api/admin/users/{ada_id}/reset"),
            Some(&root.token),
            json!({"password": PW, "code": root.recovery[0]}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "Enable the user first");
    let (_, users) = c.get("/api/admin/users", Some(&root.token)).await;
    let flags: Vec<(String, bool)> = users["users"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| {
            (
                u["username"].as_str().unwrap().to_owned(),
                u["disabled"].as_bool().unwrap(),
            )
        })
        .collect();
    assert!(flags.contains(&("ada".into(), true)) && flags.contains(&("bob".into(), false)));
    // Disabled admins cannot use the web page either.
    assert_eq!(
        admin_login(&c, "ada", &ada.recovery[1]).await.0,
        StatusCode::UNAUTHORIZED
    );
    // The hourly cleanup keeps disabled accounts.
    api::maintenance(&pool, api::now_ms() + 2 * DAY_MS)
        .await
        .unwrap();
    assert_eq!(user_id(&pool, "ada").await, ada_id);

    // Enable: signing in works again.
    let (status, res) = c
        .post(
            &format!("/api/admin/users/{ada_id}/enable"),
            Some(&root.token),
            json!({"password": PW}),
        )
        .await;
    assert_eq!((status, res), (StatusCode::OK, json!({"disabled": false})));
    let (status, ada2) = c
        .post("/api/login", None, ada_login(&ada.recovery[2]))
        .await;
    assert_eq!(status, StatusCode::OK, "{ada2}");
    let ada2 = ada2["token"].as_str().unwrap().to_owned();

    // With two enabled admins, either may disable the other, never the last one.
    let (status, _) = c
        .post(
            &format!("/api/admin/users/{root_id}/disable"),
            Some(&ada2),
            json!({"password": PW}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let client_db = pool.get().await.unwrap();
    let enabled_admins: i64 = client_db
        .query_one(
            "SELECT COUNT(*) FROM users WHERE is_admin AND disabled_at IS NULL",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(enabled_admins, 1);
    drop(client_db);
    let state = api::AppState::new(
        pool.clone(),
        api::Settings {
            public_url: "https://vidcue.test".into(),
            key: ServerKey::from_master([9; 32]),
            trusted_proxies: vec![],
            proxies_only: false,
        },
    );
    // Even a forged call cannot disable the only enabled admin.
    let err = crate::admin_web::disable_user(&state, root_id, ada_id)
        .await
        .err()
        .unwrap();
    assert_eq!(err.0, StatusCode::FORBIDDEN, "a disabled admin is no actor");
    let (status, _) = c
        .post(
            &format!("/api/admin/users/{root_id}/enable"),
            Some(&ada2),
            json!({"password": PW}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let _ = bob_id;
}
