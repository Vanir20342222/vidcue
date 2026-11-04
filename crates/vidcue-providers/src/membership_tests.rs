//! Membership filtering through the shared parser and real loopback browse API.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn video(id: &str) -> Value {
    json!({"videoRenderer":{"videoId":id,"title":{"simpleText":"Fixture video"}}})
}

fn member(id: &str) -> Value {
    let mut row = video(id);
    row["videoRenderer"]["badges"] = json!([
        {"metadataBadgeRenderer":{"style":"BADGE_STYLE_TYPE_MEMBERS_ONLY"}}
    ]);
    row
}

fn lockup(id: &str) -> Value {
    json!({"lockupViewModel":{"contentId":id,"contentType":"LOCKUP_CONTENT_TYPE_VIDEO",
        "metadata":{"lockupMetadataViewModel":{"title":{"content":"Fixture lockup"},
            "metadata":{"contentMetadataViewModel":{"metadataRows":[]}}}}}})
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
}

#[test]
fn membership_badges_hide_classic_video_and_music_rows_without_hiding_public_items() {
    for renderer in [
        "videoRenderer",
        "gridVideoRenderer",
        "compactVideoRenderer",
        "playlistVideoRenderer",
        "playlistPanelVideoRenderer",
        "musicResponsiveListItemRenderer",
        "musicTwoRowItemRenderer",
        "musicMultiRowListItemRenderer",
    ] {
        for badge in [
            json!({"style":"BADGE_STYLE_TYPE_MEMBERS_ONLY","label":"Localized badge"}),
            json!({"style":"BADGE_STYLE_TYPE_SIMPLE","label":"Members only"}),
            json!({"tooltip":"Members-only"}),
            json!({"iconTooltip":"MEMBERS\u{2011}ONLY"}),
            json!({"accessibilityData":{"label":"Members only"}}),
            json!({"label":"Nur für Mitglieder"}),
        ] {
            let hidden = json!({renderer:{"videoId":"hidden00001","title":{"simpleText":"Exclusive"},
                "badges":[{"metadataBadgeRenderer":badge}]}});
            for music in [false, true] {
                let page = parse_page(&json!({"contents":[hidden, video("public00001")]}), music);
                assert_eq!(page.items.len(), 1, "{renderer}: {badge}");
                assert_eq!(page.items[0].id, "public00001");
                assert!(page.notice.is_none());
            }
        }
    }
}

#[test]
fn modern_home_membership_badges_and_thumbnail_overlays_are_hidden() {
    for (path, badges) in [
        (
            "/metadata/lockupMetadataViewModel/metadata/contentMetadataViewModel/metadataRows",
            json!([{"badges":[{"badgeViewModel":{"badgeStyle":"BADGE_MEMBERS_ONLY"}}]}]),
        ),
        (
            "/contentImage",
            json!({"thumbnailViewModel":{"overlays":[
                {"thumbnailBottomOverlayViewModel":{"badges":[
                    {"thumbnailBadgeViewModel":{"text":"Members only"}}]}}
            ]}}),
        ),
        (
            "/contentImage",
            json!({"thumbnailViewModel":{"overlays":[
                {"thumbnailOverlayBadgeViewModel":{"thumbnailBadges":[
                    {"thumbnailBadgeViewModel":{"badgeStyle":"THUMBNAIL_OVERLAY_BADGE_STYLE_MEMBERS_ONLY"}}]}}
            ]}}),
        ),
    ] {
        let mut hidden = lockup("hidden00001");
        if path == "/contentImage" {
            hidden["lockupViewModel"]["contentImage"] = badges;
        } else {
            *hidden["lockupViewModel"].pointer_mut(path).unwrap() = badges;
        }
        let page = parse_page(&json!({"contents":[hidden, lockup("public00001")]}), false);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, "public00001");
    }
}

#[test]
fn titles_channels_premium_verification_and_menu_actions_do_not_imply_membership() {
    let mut public = video("public00001");
    public["videoRenderer"]["title"] = json!({"simpleText":"Members-only videos explained"});
    public["videoRenderer"]["ownerText"] = json!({"simpleText":"Members Only Club"});
    public["videoRenderer"]["descriptionSnippet"] = json!({"simpleText":"Join this channel"});
    public["videoRenderer"]["badges"] = json!([
        {"metadataBadgeRenderer":{"style":"BADGE_STYLE_TYPE_PREMIUM","label":"Premium"}},
        {"metadataBadgeRenderer":{"style":"BADGE_STYLE_TYPE_VERIFIED","label":"Verified"}}
    ]);
    public["videoRenderer"]["menu"] = member("menu0000001");
    public["videoRenderer"]["ownerBadges"] = json!([
        {"metadataBadgeRenderer":{"label":"Members only"}}
    ]);
    let mut modern = lockup("public00002");
    modern["lockupViewModel"]["metadata"]["lockupMetadataViewModel"]["title"]["content"] =
        "Members only — public discussion".into();
    let page = parse_page(
        &json!({"contents":[public, modern,
            {"playlistRenderer":{"playlistId":"PLfixture","title":{"simpleText":"Members only"}}},
            {"channelRenderer":{"channelId":"UCfixture","title":{"simpleText":"Members Only Club"}}},
            {"playlistPanelVideoWrapperRenderer":{"primaryRenderer":video("public00003"),
                "counterpart":[{"counterpartRenderer":member("other000001")}]}}
        ]}),
        false,
    );
    assert_eq!(
        page.items.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        [
            "public00001",
            "public00002",
            "PLfixture",
            "UCfixture",
            "public00003"
        ]
    );
}

#[test]
fn membership_marker_wins_over_unmarked_duplicates_even_when_the_locked_card_is_unplayable() {
    for members_first in [false, true] {
        for unplayable in [false, true] {
            let mut hidden = member("hidden00001");
            if unplayable {
                hidden["videoRenderer"]["isPlayable"] = false.into();
                hidden["videoRenderer"]
                    .as_object_mut()
                    .unwrap()
                    .remove("title");
            }
            let plain = lockup("hidden00001");
            let items = if members_first {
                vec![hidden, plain]
            } else {
                vec![plain, hidden]
            };
            let page = parse_page(
                &json!({"contents":items,
                "continuationCommand":{"token":"next-public-page"}}),
                false,
            );
            assert!(page.items.is_empty());
            assert_eq!(page.continuation.as_deref(), Some("next-public-page"));
            assert_eq!(page.notice.as_deref(), Some(MEMBERS_ONLY_NOTICE));
        }
    }
}

#[test]
fn filtering_preserves_categories_shelves_continuations_and_service_notices() {
    let data = json!({"contents":[
        {"chipCloudChipRenderer":{"text":{"simpleText":"Music"},
            "navigationEndpoint":{"browseEndpoint":{"browseId":"FEfixture"}}}},
        {"shelfRenderer":{"title":{"simpleText":"Recommended"},
            "content":[member("hidden00001"), video("public00001")]}}
    ],"continuationCommand":{"token":"next"}});
    let page = parse_page(&data, false);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].section, "Recommended");
    assert_eq!(page.categories[0].title, "Music");
    assert_eq!(page.continuation.as_deref(), Some("next"));
    let page = parse_page(
        &json!({"contents":[member("hidden00001")],
        "messageRenderer":{"text":{"simpleText":"Service notice"}}}),
        false,
    );
    assert_eq!(page.notice.as_deref(), Some("Service notice"));
}

#[tokio::test]
async fn members_only_initial_home_follows_its_continuation_without_refetching_bootstrap() {
    for continuation in [false, true] {
        let gets = Arc::new(AtomicUsize::new(0));
        let posts = Arc::new(AtomicUsize::new(0));
        let (get_count, post_count) = (gets.clone(), posts.clone());
        let (base, task) = crate::test_http::serve_request(move |request| {
            let body = if request.starts_with("GET") {
                get_count.fetch_add(1, Ordering::SeqCst);
                let mut data = json!({"contents":[member("hidden00001")]});
                if continuation {
                    data["continuationCommand"] = json!({"token":"public-page"});
                }
                format!(
                    "ytcfg.set({{\"INNERTUBE_CLIENT_VERSION\":\"fixture\"}});ytInitialData={data};"
                )
            } else {
                post_count.fetch_add(1, Ordering::SeqCst);
                assert!(request.contains("\"continuation\":\"public-page\""));
                json!({"contents":[video("public00001")]}).to_string()
            };
            (200, String::new(), body.into_bytes())
        })
        .await;
        let mut browser = Browser::new(client(), None, 0);
        browser.origins = [base.clone(), base];
        let page = browser.fetch(false, &BrowseRequest::Home).await.unwrap();
        task.abort();
        assert_eq!(gets.load(Ordering::SeqCst), 1);
        assert_eq!(posts.load(Ordering::SeqCst), usize::from(continuation));
        if continuation {
            assert_eq!(page.items[0].id, "public00001");
            assert!(page.notice.is_none());
        } else {
            assert!(page.items.is_empty());
            assert_eq!(page.notice.as_deref(), Some(MEMBERS_ONLY_NOTICE));
        }
    }
}

#[tokio::test]
async fn filtered_continuation_pages_reach_public_results_and_cache_only_visible_items() {
    let posts = Arc::new(AtomicUsize::new(0));
    let count = posts.clone();
    let (base, task) = crate::test_http::serve_request(move |request| {
        let body = if request.starts_with("GET") {
            r#"ytcfg.set({"INNERTUBE_CLIENT_VERSION":"fixture"});"#.to_owned()
        } else {
            let n = count.fetch_add(1, Ordering::SeqCst);
            let token = format!("hidden-{n}");
            assert!(request.contains(&format!("\"continuation\":\"{token}\"")));
            if n < 2 {
                json!({"contents":[member("hidden00001")],
                    "continuationCommand":{"token":format!("hidden-{}", n + 1)}})
                .to_string()
            } else {
                json!({"contents":[member("hidden00001"), video("public00001")]}).to_string()
            }
        };
        (200, String::new(), body.into_bytes())
    })
    .await;
    let mut browser = Browser::new(client(), None, 0);
    browser.origins = [base.clone(), base];
    let request = BrowseRequest::Continue("browse:hidden-0".into());
    for _ in 0..2 {
        let page = browser.fetch(false, &request).await.unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, "public00001");
    }
    task.abort();
    assert_eq!(posts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn filtering_through_a_run_of_member_pages_is_bounded_and_keeps_the_next_token() {
    let posts = Arc::new(AtomicUsize::new(0));
    let count = posts.clone();
    let (base, task) = crate::test_http::serve_request(move |request| {
        let body = if request.starts_with("GET") {
            r#"ytcfg.set({"INNERTUBE_CLIENT_VERSION":"fixture"});"#.to_owned()
        } else {
            let n = count.fetch_add(1, Ordering::SeqCst);
            json!({"contents":[member("hidden00001")],
                "continuationCommand":{"token":format!("hidden-{}", n + 1)}})
            .to_string()
        };
        (200, String::new(), body.into_bytes())
    })
    .await;
    let mut browser = Browser::new(client(), None, 0);
    browser.origins = [base.clone(), base];
    let page = browser
        .fetch(false, &BrowseRequest::Continue("browse:hidden-0".into()))
        .await
        .unwrap();
    task.abort();
    assert!(page.items.is_empty());
    assert_eq!(page.notice.as_deref(), Some(MEMBERS_ONLY_NOTICE));
    assert_eq!(page.continuation.as_deref(), Some("browse:hidden-3"));
    assert_eq!(posts.load(Ordering::SeqCst), 3);
}
