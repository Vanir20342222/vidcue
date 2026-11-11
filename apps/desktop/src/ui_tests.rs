//! Headless interaction tests: the real view, driven through Rune's `Runtime` with pointer and
//! key input, the app's own `poll` and `on_key`, and an isolated offline engine. No window, no
//! network, no user files.

use std::time::Duration;

use rune::{Env, Key, KeyEvent, Mods, Runtime};

use crate::app::{self, App, FeedRoute, LibraryTab, Route, SettingsSection, Tab};
use crate::player::session::PlayerMenu;

const SIZE: (f32, f32) = (1440.0, 900.0);

fn runtime(scene: Option<&str>) -> Runtime<App> {
    let mut a = App::offline_for_tests();
    app::setup(&mut a, std::sync::Arc::new(|| {}));
    if let Some(name) = scene {
        let (_, f) = crate::scenes::SCENES
            .iter()
            .find(|(n, _)| *n == name)
            .expect("scene exists");
        f(&mut a);
    }
    let mut rt = Runtime::new(a, crate::ui::view, SIZE, Env::new(true, 1.0));
    rt.poll = Some(app::poll);
    rt.on_key = Some(crate::keys::on_key);
    rt.frame(0.0);
    settle(&mut rt);
    rt
}

/// Runs frames until engine answers stop landing and springs come to rest.
fn settle(rt: &mut Runtime<App>) {
    for _ in 0..40 {
        rt.frame(1.0 / 30.0);
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn click(rt: &mut Runtime<App>, key: &str) {
    let (x, y) = rt
        .find(key)
        .unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn click_where(rt: &mut Runtime<App>, what: &str, pick: impl Fn(&rune::ElementInfo) -> bool) {
    let e = rt
        .elements()
        .into_iter()
        .filter(|e| e.opacity > 0.01 && e.rect[2] > 0.0)
        .find(|e| pick(e))
        .unwrap_or_else(|| panic!("no element: {what}"));
    let (x, y) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn press(rt: &mut Runtime<App>, key: Key) {
    rt.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    settle(rt);
}

fn text_present(rt: &Runtime<App>, s: &str) -> bool {
    rt.elements()
        .iter()
        .any(|e| e.text.as_deref() == Some(s) && e.opacity > 0.01)
}

#[test]
fn home_renders_title_chips_and_a_card_grid_without_a_featured_hero() {
    let rt = runtime(Some("home"));
    assert!(
        rt.find("hero").is_none(),
        "no featured recommendation above the feed"
    );
    assert!(text_present(&rt, "Home"));
    assert!(rt.find("chips").is_some(), "chips");
    let cards = rt
        .elements()
        .iter()
        .filter(|e| {
            e.key
                .as_deref()
                .is_some_and(|k| k.starts_with("card-") && k.matches('-').count() == 2)
        })
        .count();
    assert!(cards >= 3, "visible cards: {cards}");
    assert!(text_present(&rt, "Northern Lights Over Tromsø in 8K"));
}

#[test]
fn sidebar_navigates_and_back_returns() {
    let mut rt = runtime(Some("home"));
    click(&mut rt, "nav-music");
    assert!(matches!(&rt.state.route, Route::Feed(f) if f.tab == Tab::Music));
    click(&mut rt, "nav-library-queue");
    assert_eq!(rt.state.route, Route::Library(LibraryTab::Queue));
    click(&mut rt, "nav-back");
    assert!(
        matches!(&rt.state.route, Route::Feed(f) if f.tab == Tab::Music),
        "{:?}",
        rt.state.route
    );
    click(&mut rt, "nav-forward");
    assert_eq!(rt.state.route, Route::Library(LibraryTab::Queue));
}

#[test]
fn chips_swap_the_feed_in_place() {
    let mut rt = runtime(Some("home"));
    let depth = rt.state.back.len();
    click(&mut rt, "chip-Watch later");
    assert!(
        matches!(&rt.state.route, Route::Feed(f) if f.request == vidcue_core::BrowseRequest::WatchLater)
    );
    assert_eq!(rt.state.back.len(), depth, "chips do not stack history");
}

#[test]
fn typing_a_search_opens_results_and_remembers_it() {
    let mut rt = runtime(Some("home"));
    click(&mut rt, "search");
    for c in "lofi beats".chars() {
        rt.key(KeyEvent {
            key: Key::Text(c.to_string()),
            mods: Mods::default(),
        });
    }
    settle(&mut rt);
    assert_eq!(rt.state.search.text, "lofi beats");
    press(&mut rt, Key::Enter);
    assert!(
        matches!(&rt.state.route, Route::Feed(f) if f.request == vidcue_core::BrowseRequest::Search("lofi beats".into())),
        "{:?}",
        rt.state.route
    );
    assert_eq!(
        rt.state.prefs.recent_media.first().map(String::as_str),
        Some("lofi beats")
    );
}

#[test]
fn a_pasted_watch_link_opens_the_player() {
    let mut rt = runtime(Some("home"));
    rt.update(|s, _| s.search_submit("https://youtu.be/dQw4w9WgXcQ"));
    settle(&mut rt);
    assert_eq!(rt.state.route, Route::Watch);
    let now = rt.state.player.now.as_ref().expect("now playing");
    assert_eq!(now.media.id, "dQw4w9WgXcQ");
    // Offline, the resolve fails and the stage says so instead of spinning.
    assert!(now.error.is_some() && !now.resolving, "{:?}", now.error);
}

#[test]
fn player_settings_menu_walks_into_speed_and_sets_it() {
    let mut rt = runtime(Some("watch"));
    rt.update(|s, _| s.player.snap.paused = true);
    settle(&mut rt);
    click(&mut rt, "ctl-settings");
    assert_eq!(rt.state.player.menu, Some(PlayerMenu::Settings));
    click(&mut rt, "vr-Speed");
    assert_eq!(rt.state.player.menu, Some(PlayerMenu::Speed));
    click_where(&mut rt, "1.5× item", |e| e.text.as_deref() == Some("1.5×"));
    assert_eq!(rt.state.player.speed, 1.5);
    assert_eq!(rt.state.player.menu, None);
    assert_eq!(rt.state.prefs.speed, 1.5);
}

#[test]
fn keyboard_shortcuts_drive_the_player() {
    let mut rt = runtime(Some("watch"));
    press(&mut rt, Key::Text("t".into()));
    assert!(rt.state.player.theater);
    press(&mut rt, Key::Text("c".into()));
    assert_eq!(
        rt.state.player.now.as_ref().unwrap().caption,
        Some(0),
        "captions on"
    );
    press(&mut rt, Key::Text("c".into()));
    assert_eq!(
        rt.state.player.now.as_ref().unwrap().caption,
        None,
        "captions off"
    );
    rt.update(|s, _| s.player.menu = Some(PlayerMenu::Quality));
    press(&mut rt, Key::Escape);
    assert_eq!(rt.state.player.menu, None, "Escape closes the menu first");
    press(&mut rt, Key::Escape);
    assert_ne!(rt.state.route, Route::Watch, "then leaves the player");
}

#[test]
fn card_menu_saves_and_queues_through_the_engine() {
    let mut rt = runtime(Some("home"));
    let url = rt
        .state
        .feeds
        .get(&FeedRoute::root(Tab::Home))
        .unwrap()
        .items[1]
        .url
        .clone();
    let more = rt
        .elements()
        .into_iter()
        .find_map(|e| {
            e.key
                .filter(|k| k.starts_with("card-1-") && k.ends_with("-more"))
        })
        .expect("card menu button");
    click(&mut rt, &more);
    assert!(rt.state.menu.is_some(), "menu opened");
    click_where(&mut rt, "Save item", |e| {
        e.text.as_deref() == Some("Save") && e.path.contains("menu")
    });
    assert!(rt.state.saved_urls.contains(&url), "saved in the engine");
    assert!(
        rt.state.library.saved.iter().any(|m| m.url == url),
        "library refreshed from the engine"
    );
    click(&mut rt, &more);
    click_where(&mut rt, "Add to queue", |e| {
        e.text.as_deref() == Some("Add to queue") && e.path.contains("menu")
    });
    assert!(
        rt.state.player.queue.iter().any(|m| m.url == url),
        "queued: {:?}",
        rt.state.player.queue.len()
    );
}

#[test]
fn settings_switch_theme_and_persist_through_the_engine() {
    let mut rt = runtime(Some("home"));
    click(&mut rt, "nav-settings");
    assert_eq!(rt.state.route, Route::Settings(SettingsSection::Appearance));
    click(&mut rt, "st-theme-nord");
    assert_eq!(rt.state.config.theme, "nord");
    assert_eq!(
        rt.state.theme.palette.name,
        vidcue_core::theme::builtin("nord").unwrap().name
    );
    // The engine accepted it (a rejected save would have reverted the theme).
    let saved = rt
        .state
        .be
        .rt
        .block_on(
            rt.state
                .be
                .engine
                .request(serde_json::json!({"op":"settings"})),
        )
        .unwrap();
    assert_eq!(saved["theme"], "nord");
    click(&mut rt, "st-nav-Playback");
    assert_eq!(rt.state.route, Route::Settings(SettingsSection::Playback));
}

#[test]
fn onboarding_walks_every_step_and_finishes() {
    let mut rt = runtime(Some("onboarding"));
    for step in 1..crate::ui::onboarding::STEPS.len() {
        click(&mut rt, "ob-next");
        assert_eq!(rt.state.onboarding, Some(step));
    }
    click(&mut rt, "ob-finish");
    assert_eq!(rt.state.onboarding, None);
    assert_eq!(rt.state.prefs.onboarding_version, app::ONBOARDING_VERSION);
}

#[test]
fn downloads_pause_and_resume_jobs() {
    let mut rt = runtime(Some("downloads"));
    let first = rt.state.downloads.queue.jobs[0].id;
    click_where(&mut rt, "first Pause", |e| {
        e.label.as_deref() == Some("Pause")
    });
    assert_eq!(
        rt.state.downloads.queue.jobs[0].state,
        vidcue_core::JobState::Paused
    );
    let resume = rt
        .elements()
        .into_iter()
        .filter(|e| e.label.as_deref() == Some("Resume"))
        .count();
    assert!(resume >= 2, "paused jobs offer Resume");
    rt.update(|s, _| crate::downloads::resume_job(s, first));
    settle(&mut rt);
    assert_eq!(
        rt.state
            .downloads
            .queue
            .jobs
            .iter()
            .find(|j| j.id == first)
            .unwrap()
            .state,
        vidcue_core::JobState::Queued
    );
}

#[test]
fn music_styles_switch_from_the_stage() {
    let mut rt = runtime(Some("song"));
    for (label, style) in [
        ("Vinyl", "vinyl"),
        ("VU", "vu"),
        ("Orbit", "orbit"),
        ("Classic", "classic"),
    ] {
        click_where(&mut rt, label, |e| {
            e.text.as_deref() == Some(label) && e.rect[1] < 300.0
        });
        assert_eq!(rt.state.config.music_player_style, style);
    }
}

#[test]
fn every_scene_renders_without_panicking_and_icon_buttons_are_named() {
    let mut unnamed: Vec<String> = Vec::new();
    for (name, _) in crate::scenes::SCENES {
        let rt = runtime(Some(name));
        unnamed.extend(
            rt.elements()
                .into_iter()
                .filter(|e| {
                    e.role == "button"
                        && e.label.as_deref().is_none_or(str::is_empty)
                        && e.text.as_deref().is_none_or(str::is_empty)
                        && e.opacity > 0.01
                })
                .map(|e| format!("{name}: {}", e.path)),
        );
    }
    unnamed.sort();
    unnamed.dedup();
    assert!(
        unnamed.is_empty(),
        "unnamed buttons:\n{}",
        unnamed.join("\n")
    );
}

#[test]
fn resolving_a_stream_shows_only_the_spinner() {
    for scene in ["watch", "song"] {
        let mut rt = runtime(Some(scene));
        rt.update(|s, _| s.player.now.as_mut().expect("scene plays").resolving = true);
        settle(&mut rt);
        assert!(
            rt.state.player.now.as_ref().is_some_and(|n| n.resolving),
            "{scene}"
        );
        assert!(
            !text_present(&rt, "Finding the best stream…"),
            "{scene}: no status text while resolving"
        );
    }
}

fn offered_release() -> vidcue_services::update::Release {
    vidcue_services::update::Release {
        tag: "desktop-9.0.0".into(),
        version: "9.0.0".into(),
        name: "Vidcue 9.0.0".into(),
        url: "https://example.invalid/r".into(),
        notes: vec!["Faster search".into()],
        assets: Vec::new(),
    }
}

#[test]
fn the_startup_offer_skips_closes_and_shows_install_progress() {
    use crate::updater::{Method, State};
    let mut rt = runtime(Some("home"));
    rt.update(|a, _| {
        a.update.state = State::Available(offered_release());
        a.update.prompt = true;
    });
    settle(&mut rt);
    assert!(text_present(&rt, "Update available"));
    // This test build cannot replace itself, so the dialog offers the release page.
    assert!(rt.find("update-now").is_none());
    assert!(text_present(&rt, "Open release page"));
    click(&mut rt, "update-skip");
    assert!(!rt.state.update.prompt);
    assert!(rt.find("update-card").is_none());
    assert_eq!(
        rt.state.prefs.skipped_update.as_deref(),
        Some("desktop-9.0.0")
    );

    // An installable copy offers "Update now"; "Later" closes without skipping.
    rt.update(|a, _| {
        a.update.method = Method::WindowsInstaller;
        a.update.prompt = true;
    });
    settle(&mut rt);
    assert!(rt.find("update-now").is_some());
    click(&mut rt, "update-later");
    assert!(!rt.state.update.prompt);

    // Progress and the finished state render in the dialog and in Settings → About.
    rt.update(|a, _| {
        a.update.method = Method::Source {
            launcher: "/nonexistent/vidcue".into(),
        };
        a.update.state = State::Installing(offered_release());
        a.update.prompt = true;
    });
    settle(&mut rt);
    assert!(rt.find("update-hide").is_some());
    rt.update(|a, _| {
        a.update.state = State::Installed(offered_release());
        a.update.prompt = false;
        a.go_root(Route::Settings(SettingsSection::About));
    });
    settle(&mut rt);
    assert!(rt.find("update-restart").is_some());
}
