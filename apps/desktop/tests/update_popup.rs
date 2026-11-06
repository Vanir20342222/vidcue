//! Offline checks for the "new version available" popup. No request reaches GitHub:
//! check results are injected as UI events.
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};
use ratatui_image::picker::Picker;
use tokio::sync::mpsc;
use vidcue::{
    app::{App, HitTarget, Modal, RuntimeHandles, UiEvent},
    forms::Form,
    ui,
};
use vidcue_core::Config;
use vidcue_services::{paths::Paths, store::Snapshot, update::Release};

fn fixture() -> (App, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let paths = Paths {
        config_override: None,
        xdg_state: root.join("xdg-state"),
        config: root.join("config"),
        state: root.join("state"),
        cache: root.join("cache"),
        legacy_data: root.join("legacy"),
        home: root.into(),
    };
    paths.create().unwrap();
    let mut config = Config {
        theme: "vidcue-dark".into(),
        auth_store: "file".into(),
        notifications: false,
        ..Default::default()
    };
    paths.fill_directories(&mut config);
    let (player, _) = mpsc::channel(32);
    let (jobs, _) = mpsc::channel(32);
    let app = App::new(
        config,
        paths,
        Snapshot::default(),
        None,
        RuntimeHandles {
            player,
            jobs,
            picker: Picker::from_fontsize((8, 16)),
        },
        true,
        false,
    )
    .unwrap();
    (app, temp)
}

fn release() -> Release {
    Release {
        tag: "v9.0.0".into(),
        version: "9.0.0".into(),
        name: "Vidcue 9".into(),
        url: "https://example.invalid/v9.0.0".into(),
        notes: vec!["Brighter themes".into(), "Update popup".into()],
    }
}

fn found(app: &mut App, manual: bool) {
    app.apply(UiEvent::Update {
        manual,
        result: Ok(Some(release())),
    });
}

fn screen(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal.draw(|f| ui::render(f, app)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn startup_check_opens_the_popup_and_update_now_starts_the_installer() {
    let (mut app, _temp) = fixture();
    found(&mut app, false);
    assert!(matches!(
        &app.modal,
        Some(Modal::Update { selected: 0, .. })
    ));
    let text = screen(&mut app);
    for expected in [
        "New version available",
        "9.0.0",
        "Brighter themes",
        "Update now",
        "Later",
        "Skip this version",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in\n{text}");
    }
    app.key(KeyCode::Enter.into());
    assert!(app.modal.is_none());
    assert_eq!(
        app.run_update.as_ref().map(|r| r.tag.as_str()),
        Some("v9.0.0")
    );
}

#[tokio::test]
async fn later_closes_without_installing_and_arrows_wrap_between_buttons() {
    let (mut app, _temp) = fixture();
    found(&mut app, false);
    app.key(KeyCode::Left.into());
    assert!(matches!(
        &app.modal,
        Some(Modal::Update { selected: 2, .. })
    ));
    app.key(KeyCode::Right.into());
    app.key(KeyCode::Right.into());
    assert!(matches!(
        &app.modal,
        Some(Modal::Update { selected: 1, .. })
    ));
    app.key(KeyCode::Enter.into());
    assert!(app.modal.is_none() && app.run_update.is_none());
    found(&mut app, false);
    app.key(KeyCode::Esc.into());
    assert!(app.modal.is_none() && app.run_update.is_none());
}

#[tokio::test]
async fn clicking_update_now_starts_the_installer() {
    let (mut app, _temp) = fixture();
    found(&mut app, false);
    screen(&mut app);
    let (rect, _) = *app
        .hit_targets
        .iter()
        .rev()
        .find(|(_, hit)| *hit == HitTarget::MenuItem(0))
        .expect("Update now button is clickable");
    app.mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column: rect.x + 1,
        row: rect.y + 1,
        modifiers: crossterm::event::KeyModifiers::NONE,
    });
    assert!(app.run_update.is_none(), "hovering must not install");
    app.mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x + 1,
        row: rect.y + 1,
        modifiers: crossterm::event::KeyModifiers::NONE,
    });
    assert_eq!(app.run_update.map(|r| r.version), Some("9.0.0".into()));
}

#[tokio::test]
async fn skipped_version_is_saved_and_only_a_manual_check_shows_it_again() {
    let (mut app, _temp) = fixture();
    app.demo = false; // demo mode applies settings without writing them
    found(&mut app, false);
    app.key(KeyCode::Char('s').into());
    assert!(app.modal.is_none() && app.run_update.is_none());
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.config.skipped_update.is_none() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "skip was not saved: {}",
            app.status
        );
        if let Ok(Some(event)) =
            tokio::time::timeout(std::time::Duration::from_millis(100), app.tasks.join_next()).await
        {
            app.apply(event.unwrap());
        }
    }
    assert_eq!(
        app.paths.load_config().unwrap().skipped_update.as_deref(),
        Some("9.0.0")
    );
    found(&mut app, false);
    assert!(
        app.modal.is_none(),
        "a skipped version stays quiet at startup"
    );
    found(&mut app, true);
    assert!(matches!(&app.modal, Some(Modal::Update { .. })));
    app.shutdown().await;
}

#[tokio::test]
async fn background_result_never_replaces_an_open_panel_and_manual_checks_report_status() {
    let (mut app, _temp) = fixture();
    app.modal = Some(Modal::Help(0));
    found(&mut app, false);
    assert!(matches!(app.modal, Some(Modal::Help(0))));
    assert!(app.status.contains("9.0.0 is available"), "{}", app.status);
    app.modal = None;
    app.apply(UiEvent::Update {
        manual: true,
        result: Ok(None),
    });
    assert!(app.status.contains("up to date"), "{}", app.status);
    app.status.clear();
    app.apply(UiEvent::Update {
        manual: false,
        result: Err(anyhow::anyhow!("offline")),
    });
    assert!(app.status.is_empty(), "startup errors stay quiet");
}

#[tokio::test]
async fn settings_toggle_turns_the_startup_check_off() {
    let (app, _temp) = fixture();
    assert!(app.config.update_check, "on by default");
    let mut form = Form::settings(app.config.clone());
    let index = form
        .fields
        .iter()
        .position(|f| f.label == "Check GitHub for new releases")
        .unwrap();
    assert!(vidcue::forms::SETTINGS_SECTIONS[0].1.contains(&index));
    form.selected = index;
    form.cycle_choice(1);
    assert!(!form.config_values().unwrap().update_check);
}
