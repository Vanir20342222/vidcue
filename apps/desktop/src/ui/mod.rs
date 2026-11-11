//! The window: sidebar, top bar, the routed page, the now-playing dock and the overlay layer.

pub mod account;
pub mod browse;
pub mod dock;
pub mod library;
pub mod lyrics_panel;
pub mod movies;
pub mod music;
pub mod onboarding;
pub mod settings;
pub mod sidebar;
pub mod topbar;
pub mod watch;
pub mod watch_controls;
pub mod widgets;

use rune::{Cx, El, Transition, el, row};

use crate::app::{App, Route};
use crate::theme::sz;

pub fn view(app: &App, cx: &mut Cx<App>) -> El {
    cx.reduced_motion(app.config.reduced_motion || cx.system_reduced_motion());
    crate::themes::observe_system_dark(app, cx.system_dark());
    let t = &app.theme;
    let width = cx.layout_size("window").map(|s| s[0]).unwrap_or(1440.0);
    let fullscreen_watch = app.route == Route::Watch && app.player.fullscreen;

    let page = match &app.route {
        Route::Feed(f) => browse::view(app, cx, f),
        Route::Watch => watch::view(app, cx),
        Route::Downloads => crate::downloads::view(app, cx),
        Route::Library(tab) => library::view(app, cx, *tab),
        Route::OfflinePlaylist(id) => library::playlist_view(app, cx, id),
        Route::Movies => movies::view(app, cx),
        Route::Settings(section) => settings::view(app, cx, *section),
        Route::Account => account::view(app, cx),
    };
    let route_key = route_key(&app.route);
    let page = el()
        .key(format!("page-{route_key}"))
        .w_full()
        .h_full()
        .min_h(0.0)
        .transition(Transition::blur_replace())
        .child(page);

    let show_dock = app.player.now.is_some() && app.route != Route::Watch;
    let main = el()
        .key("main")
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .fill(t.bg)
        .child(page)
        .child(topbar::view(app, cx))
        .child(if show_dock {
            dock::view(app, cx)
        } else {
            el().key("dock-none")
        });

    let mut root = el()
        .key("window")
        .w_full()
        .h_full()
        .fill(t.bg)
        .hover_motion_within(rune::tok::motion::SNAP, rune::tok::motion::SETTLE);
    if fullscreen_watch {
        root = root.child(watch::view(app, cx));
    } else {
        root = root.child(
            row()
                .w_full()
                .h_full()
                .child(sidebar::view(app, cx, width))
                .child(main),
        );
    }
    let bottom = if show_dock { sz::DOCK + 20.0 } else { 28.0 };
    root = root.child(widgets::toasts(app, bottom));
    if let Some(m) = widgets::menu_layer(app) {
        root = root.child(m);
    }
    if let Some(step) = app.onboarding {
        root = root.child(onboarding::view(app, cx, step));
    }
    root
}

pub fn route_key(r: &Route) -> String {
    match r {
        Route::Feed(f) => format!("feed-{}", f.key().len() as u64 ^ hash(&f.key())),
        Route::Watch => "watch".into(),
        Route::Downloads => "downloads".into(),
        Route::Library(t) => format!("library-{t:?}"),
        Route::OfflinePlaylist(id) => format!("offline-{id}"),
        Route::Movies => "movies".into(),
        Route::Settings(_) => "settings".into(),
        Route::Account => "account".into(),
    }
}

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}
