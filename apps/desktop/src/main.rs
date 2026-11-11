//! Vidcue for the desktop: a GPU-rendered Rune interface over the shared Vidcue engine.
// A GUI app on Windows: no console window opens behind it.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod backend;
mod downloads;
mod feed;
mod fx;
mod icons;
mod images;
mod keys;
mod links;
#[cfg(test)]
mod live_tests;
mod lyrics;
mod movies;
mod player;
mod scenes;
mod signin;
mod theme;
mod themes;
mod ui;
#[cfg(test)]
mod ui_tests;
mod updater;

use rune::AppSpec;

fn main() {
    // The shared download/extraction code runs the native extractor in a child process by
    // re-executing this binary with a hidden argument; serve that request without a window.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "__native-worker") {
        let Some(path) = args.get(2) else {
            eprintln!("missing worker request");
            std::process::exit(2);
        };
        if let Err(e) = vidcue_services::native::worker(std::path::Path::new(path)) {
            eprintln!("{}", vidcue_providers::diagnostic(&format!("{e:#}")));
            std::process::exit(1);
        }
        return;
    }
    // "Sign in with Google" runs its web view window in a child process of this binary.
    if args.get(1).is_some_and(|a| a == signin::ARG) {
        let Some(dir) = args.get(2) else {
            std::process::exit(2);
        };
        std::process::exit(signin::run(std::path::Path::new(dir)));
    }
    // Windows draws the title-bar, taskbar and Alt+Tab icon from the window, not the exe;
    // without this it shows the generic application icon. Linux uses the desktop entry's.
    #[cfg(windows)]
    if let Err(e) = rune::set_app_icon(include_bytes!("../assets/vidcue-256.png").as_slice()) {
        eprintln!("{e}");
    }
    rune::prewarm_gpu();
    let spec = AppSpec::<app::App> {
        min_size: Some((960.0, 620.0)),
        setup: Some(app::setup),
        poll: Some(app::poll),
        on_key: Some(keys::on_key),
        scenes: scenes::SCENES,
        ..AppSpec::new("Vidcue", (1440.0, 900.0), app::App::new, ui::view)
    };
    rune::App::new(spec).compositor(fx::Gfx::default()).run();
}
