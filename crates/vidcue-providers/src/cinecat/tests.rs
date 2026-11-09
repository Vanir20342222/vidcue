use super::*;
use crate::test_http;
use serde_json::json;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

fn media() -> Media {
    Media {
        tmdb: 45745,
        season: None,
        episode: None,
    }
}
fn source(base: &str, path: &str) -> Stream {
    Stream {
        kind: StreamKind::Hls,
        url: Url::parse(base).unwrap().join(path).unwrap(),
        headers: BTreeMap::new(),
        subtitles: vec![],
    }
}
fn backend(base: &str) -> Backend {
    Backend::new(Url::parse(base).unwrap(), 2).unwrap()
}
fn ok(body: impl Into<Vec<u8>>) -> (u16, String, Vec<u8>) {
    (200, String::new(), body.into())
}

#[tokio::test]
async fn zcdn_backup_origins_validate_and_complete_the_web_failover_order() {
    let (base, server) =
        test_http::serve(|_| ok(json!({"sources":[{"name":"future","group":"api"}]}).to_string()))
            .await;
    let other = Url::parse("https://zcdn-backup.test").unwrap();
    let sources = WebSources::new(Url::parse(&base).unwrap(), Url::parse(&base).unwrap())
        .unwrap()
        .with_zcdn_backup("zcdn1", other.clone())
        .unwrap()
        .with_zcdn_backup("zcdn3", Url::parse("https://zcdn-third.test").unwrap())
        .unwrap();
    assert_eq!(
        sources.provider_names(),
        vec!["zcdn", "zcdn1", "zcdn3", "aphrodite"]
    );
    assert!(
        sources
            .clone()
            .with_zcdn_backup("zcdn1", Url::parse("https://x.test").unwrap())
            .err()
            .unwrap()
            .to_string()
            .contains("Duplicate")
    );
    assert!(
        sources
            .clone()
            .with_zcdn_backup("aphrodite", Url::parse("https://x.test").unwrap())
            .err()
            .unwrap()
            .to_string()
            .contains("collides")
    );
    let providers = backend(&base)
        .with_web_sources(sources)
        .providers(&CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        providers.into_iter().map(|p| p.name).collect::<Vec<_>>(),
        vec!["zcdn", "zcdn1", "zcdn3", "aphrodite", "future"]
    );
    server.abort();
}

#[tokio::test]
async fn failover_skips_failed_providers_and_reports_every_attempt() {
    let (base, server) = test_http::serve(|path| match path {
        "/sources" => ok(json!({"sources":[{"name":"broken"},{"name":"working"}]}).to_string()),
        "/sources/broken?tmdb=45745&type=movie" => (502, String::new(), vec![]),
        "/sources/working?tmdb=45745&type=movie" => {
            ok(json!({"streams":[{"kind":"hls","url":"/vod.m3u8"}]}).to_string())
        }
        _ => panic!("Unexpected request: {path}"),
    })
    .await;
    let backend = backend(&base);
    let cancel = CancellationToken::new();
    let auto = backend
        .resolve_auto(&media(), None, false, &cancel)
        .await
        .unwrap();
    assert_eq!(auto.resolution.provider, "working");
    assert_eq!(auto.attempts.len(), 2);
    assert!(!auto.attempts[0].succeeded);
    assert!(auto.attempts[0].error.as_deref().unwrap().contains("502"));
    assert!(auto.attempts[1].succeeded);
    // Every candidate failing is an explicit error, not a silent partial success.
    let failed = backend
        .resolve_auto(
            &media(),
            Some(["broken".to_string()].as_slice()),
            false,
            &cancel,
        )
        .await;
    let message = failed.err().unwrap().to_string();
    assert!(message.contains("1 candidate(s) failed"));
    assert!(message.contains("broken: HTTP 502"));
    server.abort();
}

#[tokio::test]
async fn failover_verification_rejects_error_video_and_falls_back_to_backup() {
    use sha2::{Digest, Sha256};
    let blocked = b"synthetic encoded network error card";
    let hash = hex::encode(Sha256::digest(blocked));
    let (base, server) = test_http::serve(move |path| match path {
        "/cdn?tmdb=438631&type=movie" => ok(json!({"ready":true,"url":"/z-list.m3u8"}).to_string()),
        "/content/movie/438631" => {
            ok(json!({"found":true,"type":"hls","url":"/a-list.m3u8"}).to_string())
        }
        "/z-list.m3u8" => {
            ok("#EXTM3U\n#EXTINF:8,\nz.ts\n#EXTINF:8,\nz.ts\n#EXTINF:8,\nz.ts\n#EXT-X-ENDLIST\n")
        }
        "/a-list.m3u8" => {
            ok("#EXTM3U\n#EXTINF:8,\na.ts\n#EXTINF:8,\na.ts\n#EXTINF:8,\na.ts\n#EXT-X-ENDLIST\n")
        }
        "/z.ts" => ok(blocked.to_vec()),
        "/a.ts" => ok(vec![7; 512]),
        _ => panic!("Unexpected request: {path}"),
    })
    .await;
    let web = WebSources::new(Url::parse(&base).unwrap(), Url::parse(&base).unwrap()).unwrap();
    let mut backend = backend(&base).with_web_sources(web);
    backend.blocked_assets.push((blocked.len(), hash));
    let cancel = CancellationToken::new();
    let media = Media {
        tmdb: 438631,
        season: None,
        episode: None,
    };
    let auto = backend
        .resolve_auto(&media, None, true, &cancel)
        .await
        .unwrap();
    assert_eq!(auto.resolution.provider, "aphrodite");
    assert_eq!(auto.resolution.streams[0].url.path(), "/a-list.m3u8");
    assert_eq!(auto.attempts.len(), 2);
    assert!(!auto.attempts[0].succeeded);
    assert!(
        auto.attempts[0]
            .error
            .as_deref()
            .unwrap()
            .contains("network-block error video")
    );
    assert!(auto.attempts[1].succeeded);
    // Without verification the blocked default would have "won"; verification can be skipped explicitly.
    let unverified = backend
        .resolve_auto(&media, None, false, &cancel)
        .await
        .unwrap();
    assert_eq!(unverified.resolution.provider, "zcdn");
    server.abort();
}

#[tokio::test]
async fn failover_order_is_explicit_validated_and_wins_over_defaults() {
    let (base, server) = test_http::serve(|path| match path {
        "/cdn?tmdb=45745&type=movie" => ok(json!({"ready":true,"url":"/z-list.m3u8"}).to_string()),
        "/content/movie/45745" => {
            ok(json!({"found":true,"type":"hls","url":"/a-list.m3u8"}).to_string())
        }
        _ => panic!("Unexpected request: {path}"),
    })
    .await;
    let web = WebSources::new(Url::parse(&base).unwrap(), Url::parse(&base).unwrap()).unwrap();
    let backend = backend(&base).with_web_sources(web);
    let cancel = CancellationToken::new();
    let auto = backend
        .resolve_auto(
            &media(),
            Some(["aphrodite".to_string()].as_slice()),
            false,
            &cancel,
        )
        .await
        .unwrap();
    assert_eq!(auto.resolution.provider, "aphrodite");
    assert_eq!(auto.resolution.streams[0].url.path(), "/a-list.m3u8");
    let invalid = backend
        .resolve_auto(
            &media(),
            Some(["bad name".to_string()].as_slice()),
            false,
            &cancel,
        )
        .await
        .err()
        .unwrap()
        .to_string();
    assert!(invalid.contains("Invalid provider identifier"));
    let duplicate = backend
        .resolve_auto(
            &media(),
            Some(["zcdn".to_string(), "zcdn".to_string()].as_slice()),
            false,
            &cancel,
        )
        .await
        .err()
        .unwrap()
        .to_string();
    assert!(duplicate.contains("Duplicate provider"));
    let empty: Vec<String> = Vec::new();
    assert!(
        backend
            .resolve_auto(&media(), Some(empty.as_slice()), false, &cancel)
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("cannot be empty")
    );
    server.abort();
}

#[tokio::test]
async fn browser_zcdn_contract_resolves_ready_movie_and_reports_queued_work() {
    let (base, server) = test_http::serve(|path| match path {
        "/cdn?tmdb=438631&type=movie" => {
            ok(json!({"ready":true,"url":"/watch/master.m3u8"}).to_string())
        }
        "/cdn?tmdb=45745&type=movie" => (
            202,
            String::new(),
            json!({"ready":false,"status":"queued"})
                .to_string()
                .into_bytes(),
        ),
        _ => (404, String::new(), vec![]),
    })
    .await;
    let web = WebSources::new(Url::parse(&base).unwrap(), Url::parse(&base).unwrap()).unwrap();
    let backend = backend(&base).with_web_sources(web);
    let cancel = CancellationToken::new();
    let resolved = backend
        .resolve(
            "zcdn",
            &Media {
                tmdb: 438631,
                season: None,
                episode: None,
            },
            &cancel,
        )
        .await
        .unwrap();
    assert_eq!(resolved.streams[0].url.path(), "/watch/master.m3u8");
    assert!(
        backend
            .resolve("zcdn", &media(), &cancel)
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("queued")
    );
    server.abort();
}

#[tokio::test]
async fn browser_aphrodite_contract_uses_tv_episode_path_and_detects_no_content() {
    let (base, server) = test_http::serve(|path| match path {
        "/content/tv/1396/1/1" => {
            ok(json!({"found":true,"type":"hls","url":"/episode/master.m3u8"}).to_string())
        }
        "/content/movie/45745" => (
            404,
            String::new(),
            json!({"found":false}).to_string().into_bytes(),
        ),
        _ => (404, String::new(), vec![]),
    })
    .await;
    let web = WebSources::new(Url::parse(&base).unwrap(), Url::parse(&base).unwrap()).unwrap();
    let backend = backend(&base).with_web_sources(web);
    let cancel = CancellationToken::new();
    let resolved = backend
        .resolve(
            "aphrodite",
            &Media {
                tmdb: 1396,
                season: Some(1),
                episode: Some(1),
            },
            &cancel,
        )
        .await
        .unwrap();
    assert_eq!(resolved.streams[0].url.path(), "/episode/master.m3u8");
    assert_eq!(
        resolved.streams[0]
            .headers
            .get("Origin")
            .map(String::as_str),
        Some("https://cinecat.eu")
    );
    assert_eq!(
        resolved.streams[0]
            .headers
            .get("Referer")
            .map(String::as_str),
        Some("https://cinecat.eu/")
    );
    assert!(
        backend
            .resolve("aphrodite", &media(), &cancel)
            .await
            .is_err()
    );
    server.abort();
}

#[tokio::test]
async fn web_catalog_includes_both_native_adapters_and_keeps_api_providers() {
    let (base, server) =
        test_http::serve(|_| ok(json!({"sources":[{"name":"future","group":"api"}]}).to_string()))
            .await;
    let web = WebSources::new(Url::parse(&base).unwrap(), Url::parse(&base).unwrap()).unwrap();
    let providers = backend(&base)
        .with_web_sources(web)
        .providers(&CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        providers.into_iter().map(|p| p.name).collect::<Vec<_>>(),
        vec!["zcdn", "aphrodite", "future"]
    );
    server.abort();
}

#[tokio::test]
async fn granite_subtitles_keep_languages_versions_and_validate_resource_urls() {
    let (base, server) = test_http::serve(|path| {
        assert_eq!(path, "/v1/tv/1396/1/1");
        ok(json!([
            {"label":"English Hi2","file":"https://captions.test/first.vtt?synthetic=1"},
            {"label":"German","file":"https://captions.test/second.vtt"},
            {"label":"Unknown language3","file":"https://captions.test/third.vtt"}
        ])
        .to_string())
    })
    .await;
    let subtitles = backend(&base)
        .search_granite_subtitles(
            &Url::parse(&base).unwrap(),
            &Media {
                tmdb: 1396,
                season: Some(1),
                episode: Some(1),
            },
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(subtitles.len(), 3);
    assert_eq!(subtitles[0].language, "en");
    assert_eq!(subtitles[1].language, "de");
    assert_eq!(subtitles[2].language, "unknown language");
    assert_eq!(subtitles[0].display, "English Hi2");
    server.abort();
}

#[tokio::test]
async fn explicit_excerpt_rounds_to_segments_and_does_not_fetch_the_remaining_movie() {
    let (base, server) = test_http::serve(|path| match path {
        "/list.m3u8" => {
            ok("#EXTM3U\n#EXTINF:10,\na.ts\n#EXTINF:10,\nb.ts\n#EXTINF:10,\nc.ts\n#EXT-X-ENDLIST\n")
        }
        "/a.ts" | "/b.ts" => ok(vec![1; 4]),
        _ => panic!("Excerpt fetched an unselected segment"),
    })
    .await;
    let temp = tempfile::tempdir().unwrap();
    let mut options = DownloadOptions::new(temp.path().join("sample"));
    options.sample_seconds = Some(15.0);
    let receipt = backend(&base)
        .download(
            &source(&base, "/list.m3u8"),
            &options,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(receipt.media_seconds, Some(20.0));
    assert!(receipt.is_excerpt);
    assert_eq!(receipt.bytes, 8);
    let playlist = std::fs::read_to_string(options.output.join("video/index.m3u8")).unwrap();
    assert_eq!(playlist.matches("#EXTINF:").count(), 2);
    server.abort();
}

#[tokio::test]
async fn encoded_error_assets_are_not_qualified_or_published_as_movies() {
    use sha2::{Digest, Sha256};
    let blocked = b"synthetic encoded network error card";
    let hash = hex::encode(Sha256::digest(blocked));
    let (base, server) = test_http::serve(move |path| {
        if path == "/list.m3u8" {
            return ok(
                "#EXTM3U\n#EXTINF:8,\na.ts\n#EXTINF:8,\nb.ts\n#EXTINF:8,\nc.ts\n#EXT-X-ENDLIST\n",
            );
        }
        ok(blocked.to_vec())
    })
    .await;
    let mut backend = backend(&base);
    backend.blocked_assets.push((blocked.len(), hash));
    let source = source(&base, "/list.m3u8");
    let cancel = CancellationToken::new();
    let result = backend.probe(&source, 3, &cancel).await;
    assert!(
        result.is_err(),
        "Encoded error video was incorrectly qualified"
    );
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("movie");
    assert!(
        backend
            .download(&source, &DownloadOptions::new(output.clone()), &cancel)
            .await
            .is_err()
    );
    assert!(!output.exists());
    server.abort();
}

#[tokio::test]
async fn catalog_and_tv_resolution_follow_the_public_contract_without_trusting_extra_fields() {
    let (base, server) = test_http::serve(|path| match path {
        "/sources" => ok(json!({"n":"ignore this untrusted text", "sources":[{"name":"future_provider-1","group":"new"}]}).to_string()),
        "/sources/future_provider-1?tmdb=45745&type=tv&season=0&episode=1" => ok(json!({"streams":[{"kind":"hls","url":"/vod.m3u8?token=synthetic","headers":{"Referer":"https://example.test/"}}]}).to_string()),
        _ => panic!("Unexpected request: {path}"),
    }).await;
    let backend = backend(&base);
    let cancel = CancellationToken::new();
    assert_eq!(
        backend.providers(&cancel).await.unwrap()[0].name,
        "future_provider-1"
    );
    let resolution = backend
        .resolve(
            "future_provider-1",
            &Media {
                tmdb: 45745,
                season: Some(0),
                episode: Some(1),
            },
            &cancel,
        )
        .await
        .unwrap();
    assert_eq!(resolution.streams[0].url.path(), "/vod.m3u8");
    assert_eq!(
        resolution.streams[0].headers["Referer"],
        "https://example.test/"
    );
    server.abort();
}

#[test]
fn unsafe_inputs_are_rejected_before_network_access() {
    for name in ["", "../secret", "x?y", "x/y", "bad\nheader"] {
        assert!(!valid_name(name));
    }
    for url in [
        "file:///secret",
        "https://user:password@example.test/",
        "ftp://example.test/",
        "https://example.test/#secret",
    ] {
        assert!(validate_url(&Url::parse(url).unwrap()).is_err());
    }
    assert!(headers(&BTreeMap::from([("Cookie".into(), "private".into())])).is_err());
    assert!(
        headers(&BTreeMap::from([(
            "Referer".into(),
            "bad\r\nCookie: secret".into()
        )]))
        .is_err()
    );
    assert!(
        Media {
            tmdb: 1,
            season: Some(1),
            episode: None
        }
        .validate()
        .is_err()
    );
    assert!(
        Media {
            tmdb: 0,
            season: None,
            episode: None
        }
        .validate()
        .is_err()
    );
    assert!(
        Media {
            tmdb: 1,
            season: Some(1),
            episode: Some(0)
        }
        .validate()
        .is_err()
    );
}

#[test]
fn hls_master_selects_quality_and_preserves_renditions_and_quoted_commas() {
    let base = Url::parse("https://media.test/folder/master.m3u8?token=synthetic").unwrap();
    let playlist = Playlist::parse(&base, "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"English, stereo\",LANGUAGE=\"en\",DEFAULT=YES,URI=\"audio/list.m3u8\"\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"s\",NAME=\"English\",URI=\"sub/list.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360,AUDIO=\"a\",SUBTITLES=\"s\"\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=4000000,RESOLUTION=1920x1080\nhigh.m3u8\n").unwrap();
    assert_eq!(playlist.variant(Some(720)).unwrap().height, Some(360));
    assert_eq!(playlist.variant(None).unwrap().height, Some(1080));
    assert_eq!(playlist.renditions[0].name, "English, stereo");
    assert_eq!(
        playlist.renditions[0].url.as_ref().unwrap().path(),
        "/folder/audio/list.m3u8"
    );
    assert!(playlist.renditions[0].default);
}

#[test]
fn hls_ranges_keys_sequence_and_discontinuity_are_not_lost() {
    let base = Url::parse("https://media.test/list.m3u8").unwrap();
    let playlist = Playlist::parse(&base, "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:12\n#EXT-X-KEY:METHOD=AES-128,URI=\"key\",IV=0x00000000000000000000000000000001\n#EXT-X-MAP:URI=\"init.mp4\",BYTERANGE=\"4@2\"\n#EXTINF:2.5,\n#EXT-X-BYTERANGE:4@0\ncombined.mp4\n#EXT-X-DISCONTINUITY\n#EXTINF:2.5,\n#EXT-X-BYTERANGE:4\ncombined.mp4\n#EXT-X-KEY:METHOD=NONE\n#EXTINF:1,\nlast.ts\n#EXT-X-ENDLIST\n").unwrap();
    assert_eq!(playlist.sequence, 12);
    assert_eq!(playlist.segments[1].resource.range, Some((4, 4)));
    assert!(playlist.segments[1].discontinuity);
    assert!(playlist.segments[2].key.is_none());
    assert_eq!(
        playlist.segments[0].init.as_ref().unwrap().range,
        Some((2, 4))
    );
    assert!(playlist.ended);
}

#[test]
fn malformed_live_and_drm_playlists_fail_explicitly() {
    let base = Url::parse("https://media.test/list.m3u8").unwrap();
    for body in [
        "<html>not a stream</html>",
        "#EXTM3U\n#EXTINF:NaN,\nx.ts",
        "#EXTM3U\n#EXTINF:2,",
        "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"key\"\n#EXTINF:2,\nx.ts",
        "#EXTM3U\n#EXTINF:2,\n#EXT-X-BYTERANGE:4\nx.ts",
        "#EXTM3U\n#EXTINF:2,\n#EXT-X-BYTERANGE:4@18446744073709551615\nx.ts",
        "#EXTM3U\n#EXTINF:2,\nfile:///etc/passwd",
        "#EXTM3U\n#EXT-X-GAP\n#EXTINF:2,\nx.ts",
    ] {
        assert!(Playlist::parse(&base, body).is_err(), "accepted {body}");
    }
}

#[tokio::test]
async fn all_provider_failures_remain_visible_and_diagnostics_exclude_capabilities() {
    let (base, server) = test_http::serve(|path| {
        if path == "/sources" { return ok(json!({"sources":[{"name":"up"},{"name":"down"}]}).to_string()); }
        if path.starts_with("/sources/up?") { return ok(json!({"streams":[{"kind":"hls","url":"/list.m3u8?secret=synthetic"}]}).to_string()); }
        if path.starts_with("/list.m3u8") { return ok("#EXTM3U\n#EXTINF:10,\na.ts\n#EXTINF:10,\nb.ts\n#EXTINF:10,\nc.ts\n#EXT-X-ENDLIST\n"); }
        if path.ends_with(".ts") { return ok(vec![1;100]); }
        (502, String::new(), b"https://private.test/?token=must-not-leak".to_vec())
    }).await;
    let reports = backend(&base)
        .check_all(&media(), 3, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(reports.len(), 2);
    assert!(reports[0].resolved);
    assert_eq!(reports[0].network[0].sustainable, Some(true));
    assert!(!reports[0].network[0].decoder_verified);
    assert!(!reports[1].resolved);
    assert_eq!(reports[1].error.as_deref(), Some("HTTP 502"));
    let text = serde_json::to_string(&reports).unwrap();
    assert!(!text.contains("secret"));
    assert!(!text.contains("token"));
    assert!(!text.contains("private.test"));
    server.abort();
}

#[tokio::test]
async fn rate_limits_and_cancellation_never_trigger_retry_or_fallback() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let (base, server) = test_http::serve(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        (429, "Retry-After: 600\r\n".into(), vec![])
    })
    .await;
    let backend = backend(&base);
    let cancel = CancellationToken::new();
    assert!(
        backend
            .providers(&cancel)
            .await
            .unwrap_err()
            .to_string()
            .contains("429")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    cancel.cancel();
    assert!(backend.providers(&cancel).await.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    server.abort();
}

#[tokio::test]
async fn redirect_drops_origin_headers_and_relative_playlists_use_final_location() {
    let (other, other_task) = test_http::serve_request(|request| {
        assert!(!request.to_lowercase().contains("referer:"));
        ok("#EXTM3U\n#EXTINF:2,\nsegment.ts\n#EXT-X-ENDLIST\n")
    })
    .await;
    let (base, server) = test_http::serve(move |_| {
        (
            302,
            format!("Location: {other}/final/list.m3u8\r\n"),
            vec![],
        )
    })
    .await;
    let playlist = backend(&base)
        .playlist(
            &Url::parse(&base).unwrap(),
            &BTreeMap::from([("Referer".into(), "https://identity.test/".into())]),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        playlist.segments[0].resource.url.path(),
        "/final/segment.ts"
    );
    server.abort();
    other_task.abort();
}

#[tokio::test]
async fn encrypted_ranged_offline_bundle_is_complete_local_and_never_overwrites() {
    let (base, server) = test_http::serve_request(|request| {
        let target = request.lines().next().unwrap().split_whitespace().nth(1).unwrap();
        match target {
            "/list.m3u8" => ok("#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:7\n#EXT-X-KEY:METHOD=AES-128,URI=\"/key?token=synthetic\",IV=0x00000000000000000000000000000001\n#EXT-X-MAP:URI=\"init.mp4\",BYTERANGE=\"4@0\"\n#EXTINF:2,\n#EXT-X-BYTERANGE:4@4\nmedia.mp4\n#EXTINF:2,\n#EXT-X-BYTERANGE:4\nmedia.mp4\n#EXT-X-ENDLIST\n"),
            "/key?token=synthetic" => ok(vec![1;16]),
            "/init.mp4" => { assert!(request.to_lowercase().contains("range: bytes=0-3")); (206,"Content-Range: bytes 0-3/12\r\n".into(),b"init".to_vec()) },
            "/media.mp4" => {
                let (offset, bytes) = if request.to_lowercase().contains("range: bytes=4-7") { (4,b"seg1") } else { assert!(request.to_lowercase().contains("range: bytes=8-11")); (8,b"seg2") };
                (206,format!("Content-Range: bytes {offset}-{}/12\r\n",offset+3),bytes.to_vec())
            }
            "/caption" => ok("WEBVTT\n\n00:00.000 --> 00:02.000\nHello\n"),
            _ => panic!("Unexpected fixture request {target}"),
        }
    }).await;
    let temp = tempfile::tempdir().unwrap();
    let options = DownloadOptions::new(temp.path().join("movie"));
    let mut source = source(&base, "/list.m3u8");
    source.subtitles.push(Subtitle {
        url: Url::parse(&format!("{base}/caption")).unwrap(),
        language: "en".into(),
        display: "English".into(),
        format: "vtt".into(),
    });
    let backend = backend(&base);
    let receipt = backend
        .download(&source, &options, &CancellationToken::new())
        .await
        .unwrap();
    assert!(receipt.entrypoint.exists());
    assert_eq!(receipt.subtitle_files.len(), 1);
    let playlist = std::fs::read_to_string(options.output.join("video/index.m3u8")).unwrap();
    assert!(playlist.contains("#EXT-X-MEDIA-SEQUENCE:7"));
    assert!(playlist.contains("IV=0x00000000000000000000000000000001"));
    assert!(!playlist.contains("token"));
    assert!(!playlist.contains("http"));
    assert!(!playlist.contains("BYTERANGE"));
    assert_eq!(
        std::fs::read(options.output.join("video/2.m4s")).unwrap(),
        b"seg1"
    );
    assert!(
        backend
            .download(&source, &options, &CancellationToken::new())
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read(options.output.join("video/2.m4s")).unwrap(),
        b"seg1"
    );
    server.abort();
}

#[tokio::test]
async fn ignored_ranges_failed_segments_byte_budgets_and_live_downloads_leave_no_output() {
    for mode in ["ignored-range", "broken", "budget", "live"] {
        let (base, server) = test_http::serve(move |target| {
            if target == "/list.m3u8" {
                return ok(format!(
                    "#EXTM3U\n#EXTINF:2,\n{}a.ts\n{}",
                    if mode == "ignored-range" {
                        "#EXT-X-BYTERANGE:4@0\n"
                    } else {
                        ""
                    },
                    if mode == "live" {
                        ""
                    } else {
                        "#EXT-X-ENDLIST\n"
                    }
                ));
            }
            if mode == "broken" {
                (403, String::new(), vec![])
            } else {
                ok(vec![1; 8])
            }
        })
        .await;
        let temp = tempfile::tempdir().unwrap();
        let mut options = DownloadOptions::new(temp.path().join("movie"));
        if mode == "budget" {
            options.max_bytes = 1;
        }
        assert!(
            backend(&base)
                .download(
                    &source(&base, "/list.m3u8"),
                    &options,
                    &CancellationToken::new()
                )
                .await
                .is_err(),
            "mode {mode}"
        );
        assert!(!options.output.exists());
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
        server.abort();
    }
}

#[tokio::test]
async fn interrupted_progressive_body_never_publishes_partial_file() {
    let (base, server) = test_http::serve_raw_request(|_| {
        b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nConnection: close\r\n\r\nshort".to_vec()
    })
    .await;
    let mut source = source(&base, "/file.mp4");
    source.kind = StreamKind::File;
    let temp = tempfile::tempdir().unwrap();
    let options = DownloadOptions::new(temp.path().join("movie"));
    assert!(
        backend(&base)
            .download(&source, &options, &CancellationToken::new())
            .await
            .is_err()
    );
    assert!(!options.output.exists());
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    server.abort();
}

#[tokio::test]
async fn short_progressive_files_download_and_probe_without_range_support() {
    let (base, server) = test_http::serve(|_| ok(vec![7; 50])).await;
    let backend = backend(&base);
    let mut source = source(&base, "/file.mp4");
    source.kind = StreamKind::File;
    let cancel = CancellationToken::new();
    let report = backend.probe(&source, 3, &cancel).await.unwrap();
    assert_eq!(report.bytes, 50);
    assert_eq!(report.sustainable, None);
    let temp = tempfile::tempdir().unwrap();
    let receipt = backend
        .download(
            &source,
            &DownloadOptions::new(temp.path().join("movie")),
            &cancel,
        )
        .await
        .unwrap();
    assert_eq!(std::fs::read(receipt.entrypoint).unwrap(), vec![7; 50]);
    server.abort();
}

#[tokio::test]
async fn subtitle_search_download_unicode_validation_and_no_clobber() {
    let (base, server) = test_http::serve(|target| {
        if target.starts_with("/search?") {
            assert!(target.contains("id=45745"));
            assert!(target.contains("key=synthetic%2Bprivate"));
            return ok("[]");
        }
        if target == "/bad" {
            return ok("<html>login required</html>");
        }
        ok("1\n00:00:00,000 --> 00:00:02,000\nGrüße 世界\n")
    })
    .await;
    let backend = backend(&base);
    let cancel = CancellationToken::new();
    assert!(
        backend
            .search_subtitles(
                &Url::parse(&format!("{base}/search")).unwrap(),
                &media(),
                Some("synthetic+private"),
                &cancel
            )
            .await
            .unwrap()
            .is_empty()
    );
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("en.srt");
    let mut subtitle = Subtitle {
        url: Url::parse(&format!("{base}/good")).unwrap(),
        language: "en".into(),
        display: "English".into(),
        format: "srt".into(),
    };
    backend
        .download_subtitle(&subtitle, &output, &cancel)
        .await
        .unwrap();
    assert!(std::fs::read_to_string(&output).unwrap().contains("世界"));
    assert!(
        backend
            .download_subtitle(&subtitle, &output, &cancel)
            .await
            .is_err()
    );
    subtitle.url.set_path("/bad");
    assert!(
        backend
            .download_subtitle(&subtitle, &temp.path().join("bad.srt"), &cancel)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
    server.abort();
}

#[tokio::test]
async fn excessive_catalog_bodies_and_duplicate_names_are_rejected() {
    for body in [
        json!({"sources":[{"name":"same"},{"name":"same"}]}).to_string(),
        "a".repeat(JSON_LIMIT + 1),
    ] {
        let (base, server) = test_http::serve(move |_| ok(body.clone())).await;
        assert!(
            backend(&base)
                .providers(&CancellationToken::new())
                .await
                .is_err()
        );
        server.abort();
    }
}

#[tokio::test]
async fn transient_segment_errors_retry_but_dont_hide_provider_failures() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let (base, server) = test_http::serve(move |path| {
        if path == "/list.m3u8" {
            return ok("#EXTM3U\n#EXTINF:2,\na.ts\n#EXT-X-ENDLIST\n");
        }
        if seen.fetch_add(1, Ordering::SeqCst) == 0 {
            return (503, String::new(), vec![]);
        }
        ok(vec![1; 4])
    })
    .await;
    let temp = tempfile::tempdir().unwrap();
    let receipt = backend(&base)
        .download(
            &source(&base, "/list.m3u8"),
            &DownloadOptions::new(temp.path().join("movie")),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(receipt.bytes, 4);
    server.abort();
}

#[tokio::test]
async fn cancelling_an_inflight_body_is_prompt_and_removes_staging() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (ready, reading) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut data = [0; 4096];
        assert!(socket.read(&mut data).await.unwrap() > 0);
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\n\r\nx")
            .await
            .unwrap();
        ready.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    let backend = backend(&base);
    let mut source = source(&base, "/movie.mp4");
    source.kind = StreamKind::File;
    let temp = tempfile::tempdir().unwrap();
    let options = DownloadOptions::new(temp.path().join("movie"));
    let cancel = CancellationToken::new();
    let transfer = backend.download(&source, &options, &cancel);
    let cancellation = async {
        reading.await.unwrap();
        cancel.cancel();
    };
    let start = std::time::Instant::now();
    let (result, _) = tokio::join!(transfer, cancellation);
    assert!(result.unwrap_err().to_string().contains("Cancelled"));
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    server.abort();
}

#[tokio::test]
async fn subtitle_bundle_uses_content_format_and_rolls_back_on_bad_caption() {
    let (base, server) = test_http::serve(|path| {
        if path == "/bad" {
            return ok("<html>not subtitles</html>");
        }
        ok("WEBVTT\n\n00:00.000 --> 00:01.000\nHello\n")
    })
    .await;
    let good = Subtitle {
        url: Url::parse(&format!("{base}/good")).unwrap(),
        language: "en\"/../".into(),
        display: "caption".into(),
        format: "srt".into(),
    };
    let mut bad = good.clone();
    bad.url.set_path("/bad");
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("captions");
    let backend = backend(&base);
    let cancel = CancellationToken::new();
    assert!(
        backend
            .download_subtitles(&[good.clone(), bad], &output, &cancel)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    let files = backend
        .download_subtitles(&[good], &output, &cancel)
        .await
        .unwrap();
    assert_eq!(files[0], output.join("en-0.vtt"));
    assert!(files[0].exists());
    server.abort();
}

#[tokio::test]
async fn network_probe_rejects_unsustainable_delivery_despite_successful_http() {
    let (base,server)=test_http::serve(|path| {
        if path == "/list.m3u8" { return ok("#EXTM3U\n#EXTINF:0.000001,\na.ts\n#EXTINF:0.000001,\nb.ts\n#EXTINF:0.000001,\nc.ts\n#EXT-X-ENDLIST\n"); }
        ok(vec![1;100])
    }).await;
    let report = backend(&base)
        .probe(&source(&base, "/list.m3u8"), 3, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(report.sustainable, Some(false));
    assert!(report.delivery_headroom.unwrap() < 1.3);
    server.abort();
}

/// Requires only installed FFmpeg/ffprobe; media and HTTP are generated locally.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires installed FFmpeg and ffprobe"]
async fn real_decoder_plays_downloaded_ts_fmp4_and_aes128_bundles_offline() {
    for kind in ["ts", "fmp4", "aes128"] {
        let fixture = tempfile::tempdir().unwrap();
        let mut command = std::process::Command::new("ffmpeg");
        command.current_dir(fixture.path()).args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x90:rate=10",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=44100",
            "-t",
            "3",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-g",
            "10",
            "-c:a",
            "aac",
            "-hls_time",
            "1",
            "-hls_list_size",
            "0",
        ]);
        if kind == "fmp4" {
            command.args(["-hls_segment_type", "fmp4"]);
        }
        if kind == "aes128" {
            std::fs::write(fixture.path().join("key"), [5_u8; 16]).unwrap();
            std::fs::write(
                fixture.path().join("keyinfo"),
                format!("key\n{}\n", fixture.path().join("key").display()),
            )
            .unwrap();
            command.args(["-hls_key_info_file", "keyinfo"]);
        }
        let generated = command.arg("list.m3u8").output().unwrap();
        assert!(
            generated.status.success(),
            "{}",
            String::from_utf8_lossy(&generated.stderr)
        );
        let audio = std::process::Command::new("ffmpeg")
            .current_dir(fixture.path())
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=660:sample_rate=44100",
                "-t",
                "3",
                "-c:a",
                "aac",
                "-hls_time",
                "1",
                "-hls_list_size",
                "0",
                "audio.m3u8",
            ])
            .output()
            .unwrap();
        assert!(
            audio.status.success(),
            "{}",
            String::from_utf8_lossy(&audio.stderr)
        );
        std::fs::write(
            fixture.path().join("sub.m3u8"),
            "#EXTM3U\n#EXT-X-TARGETDURATION:3\n#EXTINF:3,\nsub.vtt\n#EXT-X-ENDLIST\n",
        )
        .unwrap();
        std::fs::write(
            fixture.path().join("sub.vtt"),
            "WEBVTT\n\n00:00.000 --> 00:02.000\nHello\n",
        )
        .unwrap();
        std::fs::write(fixture.path().join("master.m3u8"),"#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",LANGUAGE=\"en\",NAME=\"English\",DEFAULT=YES,URI=\"audio.m3u8\"\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"s\",LANGUAGE=\"en\",NAME=\"English\",URI=\"sub.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=200000,RESOLUTION=160x90,AUDIO=\"a\",SUBTITLES=\"s\"\nlist.m3u8\n").unwrap();
        let assets: BTreeMap<String, Vec<u8>> = std::fs::read_dir(fixture.path())
            .unwrap()
            .map(|p| {
                let p = p.unwrap().path();
                (
                    format!("/{}", p.file_name().unwrap().to_string_lossy()),
                    std::fs::read(p).unwrap(),
                )
            })
            .collect();
        let (base, server) =
            test_http::serve(move |path| ok(assets.get(path).unwrap().clone())).await;
        let backend = backend(&base);
        let source = source(&base, "/master.m3u8");
        let report = backend
            .probe(&source, 3, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(report.sustainable, Some(true));
        let output = tempfile::tempdir().unwrap();
        let receipt = backend
            .download(
                &source,
                &DownloadOptions::new(output.path().join("movie")),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        server.abort(); // Decoder must have zero network access.
        decode(&receipt.entrypoint);
        assert_eq!(receipt.subtitle_files.len(), 1);
        assert!(receipt.subtitle_files[0].exists());
        assert!(receipt.media_seconds.unwrap() >= 3.0);
    }
}

fn decode(entrypoint: &Path) {
    let output = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-protocol_whitelist",
            "file,crypto,data",
            "-allowed_extensions",
            "ALL",
            "-i",
        ])
        .arg(entrypoint)
        .args(["-map", "0:v:0", "-map", "0:a:0", "-f", "null", "-"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
#[ignore = "public Cinecat/CDN requests; no cookies or credentials"]
async fn live_web_provider_response_is_hls_without_browser_state() {
    let backend = Backend::new(Url::parse("https://edge.cinecat.eu").unwrap(), 2)
        .unwrap()
        .with_web_sources(
            WebSources::new(
                Url::parse("https://zcdn2.2wwlgkfo-31a.workers.dev").unwrap(),
                Url::parse("https://cdn.hls.lol").unwrap(),
            )
            .unwrap(),
        );
    let cancel = CancellationToken::new();
    for provider in ["zcdn", "aphrodite"] {
        let resolution = backend
            .resolve(
                provider,
                &Media {
                    tmdb: 438631,
                    season: None,
                    episode: None,
                },
                &cancel,
            )
            .await
            .unwrap();
        let source = &resolution.streams[0];
        let response = backend
            .response(
                &source.url,
                &headers(&source.headers).unwrap(),
                None,
                &cancel,
            )
            .await
            .unwrap();
        let mime = response
            .headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("none")
            .to_string();
        let encoding = response
            .headers()
            .get("content-encoding")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("none")
            .to_string();
        let final_host = response.url().host_str().unwrap_or("none").to_string();
        let bytes = backend
            .collect(response, PLAYLIST_LIMIT, &cancel)
            .await
            .unwrap();
        let gzip = bytes.starts_with(&[0x1f, 0x8b]);
        let html = String::from_utf8_lossy(&bytes)
            .to_lowercase()
            .contains("<html");
        let valid = std::str::from_utf8(&bytes)
            .ok()
            .is_some_and(|s| Playlist::parse(&source.url, s).is_ok());
        let text = String::from_utf8_lossy(&bytes);
        let title = text
            .split("<title>")
            .nth(1)
            .and_then(|s| s.split("</title>").next())
            .unwrap_or("none");
        let title = if title.contains("http") || title.len() > 100 {
            "redacted"
        } else {
            title
        };
        assert!(
            valid,
            "{provider}: MIME={mime}, encoding={encoding}, bytes={}, gzip={gzip}, html={html}, host={final_host}, title={title}",
            bytes.len()
        );
    }
}

#[tokio::test]
#[ignore = "public Cinecat sources API; no cookies or credentials"]
async fn live_api_provider_accepts_the_normal_site_identity() {
    let backend = Backend::new(Url::parse("https://edge.cinecat.eu").unwrap(), 1).unwrap();
    let identity = headers(&BTreeMap::from([
        ("Origin".into(), "https://cinecat.eu".into()),
        ("Referer".into(), "https://cinecat.eu/".into()),
    ]))
    .unwrap();
    let response = backend
        .response(
            &Url::parse("https://edge.cinecat.eu/sources/abyssinia?tmdb=438631&type=movie")
                .unwrap(),
            &identity,
            None,
            &CancellationToken::new(),
        )
        .await;
    assert!(
        response.is_ok(),
        "API site-identity probe failed: {}",
        response.err().map(|e| e.to_string()).unwrap_or_default()
    );
}
