//! Offline request-count and fallback checks for the opt-in mobile player path.
#[path = "helpers/site_proxy.rs"]
mod site_proxy;
use yt_dlp_rs::{HttpOptions, YoutubeDl, YoutubeOptions};

fn downloader(proxy: &site_proxy::SiteProxy, format: Option<&str>) -> YoutubeDl {
    YoutubeDl::with_options(
        YoutubeOptions {
            no_playlist: true,
            playback_format: format.map(str::to_owned),
            playback_visitor_data: Some("synthetic-visitor".into()),
            ..Default::default()
        },
        HttpOptions {
            proxy: Some(proxy.url.clone()),
            no_check_certificates: true,
            socket_timeout: Some(2.0),
            ..Default::default()
        },
    )
    .unwrap()
}
const VIDEO: &str = "https://www.youtube.com/watch?v=BaW_jenozKc";
const FORMAT: &str = "bestvideo[height<=1080]+bestaudio/best";

// Synthetic session and URLs only. Header routing ensures that recovery really
// changes clients; no media probes or external requests can leave this proxy.
fn recovery_proxy(
    initial_streams: serde_json::Value,
    embedded: serde_json::Value,
    safari: serde_json::Value,
    embedded_status: u16,
) -> (tempfile::TempDir, site_proxy::SiteProxy) {
    use serde_json::json;
    let directory = tempfile::tempdir().unwrap();
    let player = |streams| {
        json!({
            "playabilityStatus":{"status":"OK"},
            "videoDetails":{"videoId":"BaW_jenozKc","title":"Recovery fixture","lengthSeconds":"10","isLiveContent":false},
            "streamingData":streams,
        })
    };
    let initial = if initial_streams.get("playabilityStatus").is_some() {
        initial_streams
    } else {
        player(initial_streams)
    };
    let webpage = format!(
        "<script>ytcfg.set({});ytInitialPlayerResponse={initial};</script>",
        json!({"VISITOR_DATA":"synthetic-visitor","STS":20522,
            "PLAYER_JS_URL":"/s/player/12345678/player_ias.vflset/en_US/base.js"})
    );
    let embedded = if embedded.get("videoDetails").is_some() {
        embedded
    } else {
        player(embedded)
    };
    let routes = json!({"routes":[
        {"url":"https://www.youtube.com/youtubei/v1/player?prettyPrint=false",
            "request_headers":{"X-YouTube-Client-Name":"56","X-Goog-Visitor-Id":"synthetic-visitor"},
            "request_body":["WEB_EMBEDDED_PLAYER","signatureTimestamp"],
            "status":embedded_status,"headers":{"Retry-After":"30"},"body":embedded.to_string()},
        {"url":"https://www.youtube.com/youtubei/v1/player?prettyPrint=false",
            "request_headers":{"X-YouTube-Client-Name":"1","X-Goog-Visitor-Id":"synthetic-visitor"},
            "request_body":["Safari","signatureTimestamp"],"body":player(safari).to_string()},
        {"url":"https://www.youtube.com/youtubei/v1/player?prettyPrint=false","body":initial.to_string()},
        {"url":"https://www.youtube.com/watch?v=BaW_jenozKc&bpctr=9999999999&has_verified=1","body":webpage},
        {"url":"https://manifest.test/hls/master.m3u8","body":include_str!("fixtures/sites/youtube-direct-hls-only/master.m3u8")},
    ]});
    std::fs::write(directory.path().join("routes.json"), routes.to_string()).unwrap();
    let proxy = site_proxy::SiteProxy::start(directory.path().to_str().unwrap());
    (directory, proxy)
}

fn recovery_downloader(proxy: &site_proxy::SiteProxy, playback: Option<&str>) -> YoutubeDl {
    YoutubeDl::with_options(
        YoutubeOptions {
            no_playlist: true,
            playback_format: playback.map(str::to_owned),
            playback_visitor_data: Some("synthetic-visitor".into()),
            // Exercise Android's configuration without registering a fake solver.
            js_runtime: Some("ytui-android-js".into()),
            ..Default::default()
        },
        HttpOptions {
            proxy: Some(proxy.url.clone()),
            no_check_certificates: true,
            socket_timeout: Some(2.0),
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn empty_default_clients_recover_native_paired_video_without_downgrading_quality() {
    let full: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/sites/youtube-direct-indexed/player.json"
    ))
    .unwrap();
    let (_directory, proxy) = recovery_proxy(
        serde_json::json!({}),
        full["streamingData"].clone(),
        serde_json::json!({}),
        200,
    );
    let info = recovery_downloader(&proxy, Some(FORMAT))
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap();
    assert_eq!(info.height, Some(1080));
    assert_eq!(info.requested_formats.len(), 2);
    assert_eq!(info.title, "Recovery fixture");
    assert_eq!(info.extra["ytui_playback_route"], "full");
    assert!(
        info.requested_formats
            .iter()
            .all(|format| format.extra["ytui_init_range"]["end"] == "99")
    );
    assert_eq!(proxy.requests.lock().unwrap().len(), 5);
}

#[test]
fn sabr_only_defaults_can_recover_through_native_safari_hls() {
    let (_directory, proxy) = recovery_proxy(
        serde_json::json!({"serverAbrStreamingUrl":"https://sabr.test/unsupported"}),
        serde_json::json!({}),
        serde_json::json!({"hlsManifestUrl":"https://manifest.test/hls/master.m3u8"}),
        200,
    );
    let info = recovery_downloader(&proxy, Some(FORMAT))
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap();
    assert_eq!(info.height, Some(720));
    assert_eq!(info.extra["ytui_playback_route"], "full");
    let requests = proxy.requests.lock().unwrap();
    assert_eq!(requests.len(), 7, "{requests:?}");
    assert!(requests.last().unwrap().contains("master.m3u8"));
    assert!(info.fragments.is_empty());
}

#[test]
fn recovery_rate_limit_stops_before_the_next_client_and_retains_retry_after() {
    let (_directory, proxy) = recovery_proxy(
        serde_json::json!({}),
        serde_json::json!({}),
        serde_json::json!({}),
        429,
    );
    let error = recovery_downloader(&proxy, Some(FORMAT))
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap_err();
    let yt_dlp_rs::Error::ExtractorRequest { source, .. } = error else {
        panic!("{error}");
    };
    assert!(
        matches!(*source, yt_dlp_rs::Error::HttpRetryAfter {retry_after, ..} if retry_after.as_secs() == 30)
    );
    assert_eq!(proxy.requests.lock().unwrap().len(), 5);
}

#[test]
fn exhausted_recovery_reports_android_solver_failure_without_signed_url_details() {
    let (_directory, proxy) = recovery_proxy(
        serde_json::json!({"formats":[{"itag":18,"url":"https://media.test/video?n=private-challenge&sig=private-signature",
            "mimeType":"video/mp4; codecs=\"avc1.42001e, mp4a.40.2\"","width":640,"height":360}]}),
        serde_json::json!({}),
        serde_json::json!({}),
        200,
    );
    let error = recovery_downloader(&proxy, Some(FORMAT))
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("Android's isolated JavaScript solver is unavailable"),
        "{error}"
    );
    assert!(!error.contains("private-"), "{error}");
    assert!(!error.contains("require an explicit runtime"), "{error}");
    assert_eq!(proxy.requests.lock().unwrap().len(), 6);
}

#[test]
fn download_acquisition_does_not_opt_into_playback_client_recovery() {
    let full: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/sites/youtube-direct-indexed/player.json"
    ))
    .unwrap();
    let (_directory, proxy) = recovery_proxy(
        serde_json::json!({}),
        full["streamingData"].clone(),
        serde_json::json!({}),
        200,
    );
    assert!(
        recovery_downloader(&proxy, None)
            .extract_info_with_format(VIDEO, Some(FORMAT))
            .is_err()
    );
    assert_eq!(proxy.requests.lock().unwrap().len(), 3);
}

#[test]
fn music_recovers_audio_only_streams_with_index_and_language_metadata() {
    let mut full: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/sites/youtube-direct-indexed/player.json"
    ))
    .unwrap();
    let streams = &mut full["streamingData"];
    streams["formats"] = serde_json::json!([]);
    streams["adaptiveFormats"]
        .as_array_mut()
        .unwrap()
        .retain(|format| format["mimeType"].as_str().unwrap().starts_with("audio/"));
    for format in streams["adaptiveFormats"].as_array_mut().unwrap() {
        format["audioTrack"] =
            serde_json::json!({"id":"de.0","audioIsDefault":true,"displayName":"German original"});
    }
    let (_directory, proxy) = recovery_proxy(
        serde_json::json!({}),
        streams.clone(),
        serde_json::json!({}),
        200,
    );
    let info = recovery_downloader(&proxy, Some("bestaudio/best"))
        .extract_info_with_format(VIDEO, Some("bestaudio/best"))
        .unwrap();
    assert_eq!(info.vcodec.as_deref(), Some("none"));
    assert!(info.formats.iter().any(|format| {
        Some(&format.format_id) == info.format_id.as_ref()
            && format.language.as_deref() == Some("de")
    }));
    assert_eq!(info.extra["ytui_init_range"]["end"], "99");
    assert_eq!(proxy.requests.lock().unwrap().len(), 5);
}

#[test]
fn wrong_video_from_an_alternative_client_is_never_played() {
    let mut wrong: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/sites/youtube-direct-indexed/player.json"
    ))
    .unwrap();
    wrong["videoDetails"]["videoId"] = "WRONG_VIDEO".into();
    let (_directory, proxy) = recovery_proxy(
        serde_json::json!({}),
        wrong,
        serde_json::json!({"hlsManifestUrl":"https://manifest.test/hls/master.m3u8"}),
        200,
    );
    let info = recovery_downloader(&proxy, Some(FORMAT))
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap();
    assert_eq!(info.height, Some(720));
    assert_eq!(info.id, "BaW_jenozKc");
    assert_eq!(proxy.requests.lock().unwrap().len(), 7);
}

#[test]
fn access_restrictions_report_the_reason_instead_of_retrying_native_clients() {
    use serde_json::json;
    for with_details in [false, true] {
        for (reason, expected) in [
            (
                "Join this channel to get access to members-only content",
                "for channel members",
            ),
            (
                "This is a private video. Please sign in to verify access.",
                "video is private",
            ),
            ("Sign in to confirm your age", "age verification"),
            ("Sign in to confirm you're not a bot", "requires sign-in"),
        ] {
            let mut denied = json!({"playabilityStatus":{"status":"LOGIN_REQUIRED",
                "errorScreen":{"playerErrorMessageRenderer":{"reason":{"runs":[{"text":reason}]},
                    "subreason":{"simpleText":"https://account.test/?token=private-session"}}}}});
            if with_details {
                denied["videoDetails"] =
                    json!({"videoId":"BaW_jenozKc","title":"Restricted fixture"});
                // YouTube also sends membership denials as a plain reason.
                if expected == "for channel members" {
                    denied["playabilityStatus"] = json!({"status":"UNPLAYABLE","reason":reason});
                }
            }
            let (_directory, proxy) = recovery_proxy(denied, json!({}), json!({}), 200);
            let error = recovery_downloader(&proxy, Some(FORMAT))
                .extract_info_with_format(VIDEO, Some(FORMAT))
                .unwrap_err()
                .to_string();
            assert!(error.contains(expected), "{error}");
            assert!(!error.contains("no eligible media formats"), "{error}");
            assert!(!error.contains("likely being blocked"), "{error}");
            assert!(!error.contains("private-session"), "{error}");
            assert_eq!(proxy.requests.lock().unwrap().len(), 4);
        }
    }
}

#[test]
fn a_members_only_title_never_marks_a_playable_video_as_restricted() {
    let mut full: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/sites/youtube-direct-indexed/player.json"
    ))
    .unwrap();
    full["videoDetails"]["title"] = "Members only — public video about memberships".into();
    let (_directory, proxy) =
        recovery_proxy(full, serde_json::json!({}), serde_json::json!({}), 200);
    let info = recovery_downloader(&proxy, Some(FORMAT))
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap();
    assert_eq!(info.height, Some(1080));
    assert_eq!(proxy.requests.lock().unwrap().len(), 1);
}

#[test]
fn indexed_playback_retains_initialization_index_and_duration_without_media_probes() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct-indexed");
    let info = downloader(&proxy, Some(FORMAT))
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap();
    assert_eq!(info.height, Some(1080));
    assert_eq!(info.requested_formats.len(), 2);
    for format in &info.requested_formats {
        assert_eq!(format.extra["ytui_init_range"]["end"], "99");
        assert_eq!(format.extra["ytui_index_range"]["start"], "100");
        assert_eq!(format.extra["ytui_duration_ms"], "10000");
    }
    assert_eq!(proxy.requests.lock().unwrap().len(), 1);
}

#[test]
fn mobile_paired_video_uses_one_player_request_and_keeps_requested_quality() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct");
    let info = downloader(&proxy, Some(FORMAT))
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap();
    assert_eq!(info.height, Some(1080));
    assert_eq!(info.extra["ytui_playback_route"], "direct");
    assert_eq!(info.requested_formats.len(), 2);
    let requests = proxy.requests.lock().unwrap();
    assert_eq!(requests.len(), 1, "requests: {requests:?}");
    assert!(requests[0].contains("/youtubei/v1/player"));
}

#[test]
fn mobile_audio_selects_audio_without_fetching_watch_page_or_player_js() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct");
    let info = downloader(&proxy, Some("bestaudio/best"))
        .extract_info_with_format(VIDEO, Some("bestaudio/best"))
        .unwrap();
    assert_eq!(info.vcodec.as_deref(), Some("none"));
    assert_eq!(proxy.requests.lock().unwrap().len(), 1);
}

#[test]
fn rejected_audio_recovery_bypasses_direct_player_and_keeps_playback_metadata() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct-indexed");
    let dl = YoutubeDl::with_options(
        YoutubeOptions {
            no_playlist: true,
            playback_format: Some("bestaudio/best".into()),
            playback_skip_direct: true,
            playback_visitor_data: Some("synthetic-visitor".into()),
            ..Default::default()
        },
        HttpOptions {
            proxy: Some(proxy.url.clone()),
            no_check_certificates: true,
            socket_timeout: Some(2.0),
            ..Default::default()
        },
    )
    .unwrap();
    let info = dl
        .extract_info_with_format(VIDEO, Some("bestaudio/best"))
        .unwrap_or_else(|error| panic!("{error}; requests={:?}", proxy.requests.lock().unwrap()));
    assert_eq!(info.vcodec.as_deref(), Some("none"));
    assert_eq!(info.extra["ytui_playback_route"], "full");
    assert_eq!(info.extra["ytui_init_range"]["end"], "99");
    let requests = proxy.requests.lock().unwrap();
    assert!(requests[0].contains("/watch?"), "requests: {requests:?}");
    assert!(
        requests
            .iter()
            .all(|request| !request.contains("/visitor_id?"))
    );
}

#[test]
fn restricted_direct_response_falls_back_to_watch_acquisition() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct-fallback");
    let info = downloader(&proxy, Some(FORMAT))
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap();
    assert_eq!(info.height, Some(1080));
    assert_eq!(info.extra["ytui_playback_route"], "full");
    let requests = proxy.requests.lock().unwrap();
    assert_eq!(requests.len(), 3, "requests: {requests:?}");
    assert!(requests[0].contains("/youtubei/v1/player"));
    assert!(requests[1].contains("/watch?"));
}

#[test]
fn direct_rate_limit_does_not_hammer_other_clients_or_watch_page() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct-limited");
    let error = downloader(&proxy, Some(FORMAT))
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap_err();
    assert!(error.to_string().contains("429"), "{error}");
    assert_eq!(proxy.requests.lock().unwrap().len(), 1);
}

#[test]
fn default_download_acquisition_still_begins_with_watch_page() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct");
    let _ = downloader(&proxy, None).extract_info_with_format(VIDEO, Some(FORMAT));
    let requests = proxy.requests.lock().unwrap();
    assert!(requests[0].contains("/watch?"), "requests: {requests:?}");
}

#[test]
fn missing_browse_context_bootstraps_once_and_reuses_visitor_without_watch_requests() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct");
    let dl = YoutubeDl::with_options(
        YoutubeOptions {
            no_playlist: true,
            playback_format: Some(FORMAT.into()),
            ..Default::default()
        },
        HttpOptions {
            proxy: Some(proxy.url.clone()),
            no_check_certificates: true,
            socket_timeout: Some(2.0),
            ..Default::default()
        },
    )
    .unwrap();
    for _ in 0..2 {
        dl.extract_info_with_format(VIDEO, Some(FORMAT)).unwrap();
    }
    let requests = proxy.requests.lock().unwrap();
    assert_eq!(requests.len(), 3, "requests: {requests:?}");
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.contains("/visitor_id?"))
            .count(),
        1
    );
    assert!(requests.iter().all(|request| !request.contains("/watch?")));
}

#[test]
fn full_fallback_preserves_rate_limit_instead_of_querying_more_clients() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct-fallback-limited");
    let dl = YoutubeDl::with_options(
        YoutubeOptions {
            no_playlist: true,
            playback_format: Some(FORMAT.into()),
            playback_visitor_data: Some("synthetic-visitor".into()),
            js_runtime: Some("nonexistent-runtime-never-needed".into()),
            ..Default::default()
        },
        HttpOptions {
            proxy: Some(proxy.url.clone()),
            no_check_certificates: true,
            socket_timeout: Some(2.0),
            ..Default::default()
        },
    )
    .unwrap();
    let error = dl
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap_err();
    assert!(error.to_string().contains("429"), "{error}");
    let requests = proxy.requests.lock().unwrap();
    assert_eq!(requests.len(), 3, "requests: {requests:?}");
}

#[test]
fn direct_challenges_do_not_silently_downgrade_offered_1080p_to_a_muxed_360p_fallback() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct-quality-fallback");
    let info = downloader(&proxy, Some(FORMAT))
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap();
    assert_eq!(info.height, Some(1080));
    assert_eq!(info.extra["ytui_playback_route"], "full");
    assert_eq!(proxy.requests.lock().unwrap().len(), 3);
}

#[test]
fn usable_direct_streams_do_not_fetch_optional_manifests_for_video_or_audio() {
    for selector in [FORMAT, "bestaudio/best"] {
        let proxy = site_proxy::SiteProxy::start("youtube-direct-optional-manifests");
        let info = downloader(&proxy, Some(selector))
            .extract_info_with_format(VIDEO, Some(selector))
            .unwrap();
        assert_eq!(info.extra["ytui_playback_route"], "direct");
        if selector == FORMAT {
            assert_eq!(info.height, Some(1080));
        } else {
            assert_eq!(info.vcodec.as_deref(), Some("none"));
        }
        let requests = proxy.requests.lock().unwrap();
        assert_eq!(
            requests.len(),
            1,
            "Optional manifests must not be on the first-frame path: {requests:?}"
        );
    }
}

#[test]
fn manifest_only_direct_response_keeps_necessary_hls_without_watch_refetch() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct-hls-only");
    let info = downloader(&proxy, Some(FORMAT))
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap_or_else(|error| panic!("{error}; requests={:?}", proxy.requests.lock().unwrap()));
    assert_eq!(info.extra["ytui_playback_route"], "direct");
    assert_eq!(info.height, Some(720));
    let requests = proxy.requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        2,
        "Only the player API and master: {requests:?}"
    );
    assert!(requests.iter().all(|request| !request.contains("/watch?")));
    assert!(requests[1].contains("manifest.test/hls/master.m3u8"));
}

#[test]
fn optional_manifest_is_required_when_it_can_improve_a_lower_direct_resolution() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct-hls-quality");
    let info = downloader(&proxy, Some(FORMAT))
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap();
    assert_eq!(info.extra["ytui_playback_route"], "direct");
    assert_eq!(
        info.height,
        Some(720),
        "Do not start the direct 360p fallback while HLS offers 720p"
    );
    let requests = proxy.requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        2,
        "Child playlists belong to Media3: {requests:?}"
    );
    assert!(requests[1].contains("manifest.test/hls/master.m3u8"));
    assert!(info.fragments.is_empty());
}

#[test]
fn best_quality_uses_master_catalog_without_traversing_every_variant() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct-hls-quality");
    let selector = "bestvideo+bestaudio/best";
    let info = downloader(&proxy, Some(selector))
        .extract_info_with_format(VIDEO, Some(selector))
        .unwrap();
    assert_eq!(info.height, Some(720));
    assert_eq!(info.extra["ytui_playback_route"], "direct");
    assert!(info.extra["ytui_manifest_ms"].is_u64());
    assert_eq!(info.vcodec.as_deref(), Some("avc1.64001f"));
    assert_eq!(info.acodec.as_deref(), Some("mp4a.40.2"));
    assert_eq!(
        info.extra["manifest_url"],
        "https://manifest.test/hls/master.m3u8"
    );
    assert_eq!(proxy.requests.lock().unwrap().len(), 2);
    assert!(
        info.formats
            .iter()
            .all(|format| format.fragments.is_empty())
    );
}

#[test]
fn playback_keeps_external_audio_groups_without_reading_child_playlists() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct-hls-audio");
    let info = downloader(&proxy, Some(FORMAT))
        .extract_info_with_format(VIDEO, Some(FORMAT))
        .unwrap();
    assert_eq!(info.height, Some(720));
    assert_eq!(info.requested_formats.len(), 2);
    assert_eq!(info.requested_formats[1].vcodec.as_deref(), Some("none"));
    assert_eq!(info.requested_formats[1].language.as_deref(), Some("en"));
    assert!(
        info.requested_formats
            .iter()
            .all(|format| format.fragments.is_empty())
    );
    assert_eq!(proxy.requests.lock().unwrap().len(), 2);
}

#[test]
fn normal_hls_download_extraction_still_traverses_children_and_keeps_fragments() {
    let proxy = site_proxy::SiteProxy::start("youtube-direct-hls-quality");
    let info = downloader(&proxy, None)
        .extract_info_with_format("https://manifest.test/hls/master.m3u8", Some("best"))
        .unwrap();
    assert_eq!(info.height, Some(720));
    assert!(!info.fragments.is_empty());
    let requests = proxy.requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        4,
        "Normal acquisition including VOD check: {requests:?}"
    );
    assert!(requests.iter().any(|url| url.contains("/itag/95/")));
    assert!(requests.iter().any(|url| url.contains("/itag/96/")));
}
