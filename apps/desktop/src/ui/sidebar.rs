//! The sidebar: destinations, library shortcuts, account and settings. The selection pill
//! flies between rows (shared-element motion) and the bar collapses to icons on a spring.

use rune::tok::motion;
use rune::{Color, Cx, El, Icon, col, el, icon, row, text};

use crate::app::{App, FeedRoute, LibraryTab, Route, SettingsSection, Tab};
use crate::icons as ic;
use crate::theme::{sz, ty};

struct Item {
    id: &'static str,
    label: &'static str,
    glyph: &'static Icon,
    route: Route,
}

fn items(app: &App) -> (Vec<Item>, Vec<Item>) {
    let mut main = vec![
        Item {
            id: "home",
            label: "Home",
            glyph: &ic::HOME,
            route: Route::Feed(FeedRoute::root(Tab::Home)),
        },
        Item {
            id: "music",
            label: "Music",
            glyph: &ic::MUSIC,
            route: Route::Feed(FeedRoute::root(Tab::Music)),
        },
        Item {
            id: "radio",
            label: "Radio",
            glyph: &ic::RADIO,
            route: Route::Feed(FeedRoute::root(Tab::Radio)),
        },
    ];
    if app.prefs.alpha_movies {
        main.push(Item {
            id: "movies",
            label: "Movies",
            glyph: &ic::FILM,
            route: Route::Movies,
        });
    }
    main.push(Item {
        id: "downloads",
        label: "Downloads",
        glyph: &ic::DOWNLOAD,
        route: Route::Downloads,
    });
    let library = vec![
        Item {
            id: "library-saved",
            label: "Saved",
            glyph: &ic::BOOKMARK,
            route: Route::Library(LibraryTab::Saved),
        },
        Item {
            id: "library-history",
            label: "History",
            glyph: &ic::HISTORY,
            route: Route::Library(LibraryTab::History),
        },
        Item {
            id: "library-playlists",
            label: "Playlists",
            glyph: &ic::LIBRARY,
            route: Route::Library(LibraryTab::Playlists),
        },
        Item {
            id: "library-queue",
            label: "Queue",
            glyph: &ic::QUEUE,
            route: Route::Library(LibraryTab::Queue),
        },
    ];
    (main, library)
}

fn selected_id(app: &App) -> String {
    match &app.route {
        Route::Library(t) => format!("library-{}", format!("{t:?}").to_lowercase()),
        Route::OfflinePlaylist(_) => "library-playlists".into(),
        Route::Feed(f)
            if !crate::feed::is_chip(f)
                && !matches!(
                    f.request,
                    vidcue_core::BrowseRequest::Cliamp | vidcue_core::BrowseRequest::Favorites
                ) =>
        {
            // Searches and collections keep their tab highlighted.
            match f.tab {
                Tab::Home => "home",
                Tab::Music => "music",
                Tab::Radio => "radio",
            }
            .into()
        }
        r => r.section().into(),
    }
}

fn nav_row(app: &App, item: &Item, selected: bool, compact: bool, badge: Option<String>) -> El {
    let t = &app.theme;
    let route = item.route.clone();
    let fg = if selected { t.text } else { t.muted };
    let mut r = row()
        .key(format!("nav-{}", item.id))
        .h(42.0)
        .w_full()
        .px(if compact { 0.0 } else { 14.0 })
        .gap(14.0)
        .center_y()
        .radius(sz::R_MD)
        .fill(if selected {
            t.accent_soft
        } else {
            Color::TRANSPARENT
        })
        .hover(if selected { t.accent_soft } else { t.hover })
        .role("tab")
        .label(item.label)
        .checked(selected)
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| s.go_root(route.clone()));
    if compact {
        r = r.justify_center().help(item.label);
    }
    r = r.child(icon(item.glyph, 19.0).color(if selected { t.accent } else { fg }));
    if !compact {
        r = r.child(text(item.label).font(ty::BODY_STRONG).color(fg).grow(1.0));
        if let Some(b) = badge {
            r = r.child(
                row()
                    .h(20.0)
                    .min_w(20.0)
                    .px(6.0)
                    .center()
                    .radius(10.0)
                    .fill(t.accent)
                    .child(text(b).font(ty::BADGE).color(t.on_accent)),
            );
        }
        if item.id == "movies" {
            r = r.child(alpha_badge_for(t));
        }
    }
    r
}

/// A small "Alpha" tag in the warning colour.
pub fn alpha_badge_for(t: &crate::theme::Theme) -> El {
    crate::ui::widgets::badge("Alpha", t.warning.alpha(0.16), t.warning)
}

/// The app mark, as on Android: an amber-to-pink arc around a play triangle.
pub fn logo(size: f32) -> El {
    icon(&ic::LOGO, size)
}

pub fn view(app: &App, _cx: &mut Cx<App>, window_w: f32) -> El {
    let t = &app.theme;
    let compact = app.prefs.sidebar_compact || window_w < 1100.0;
    let width = if compact {
        sz::SIDEBAR_COMPACT
    } else {
        sz::SIDEBAR
    };
    let selected = selected_id(app);
    let (main, library) = items(app);
    let active = app.downloads.active_count();

    let logo_mark = logo(32.0);
    let mut logo = row()
        .h(sz::TOPBAR)
        .px(if compact { 0.0 } else { 16.0 })
        .gap(10.0)
        .center_y()
        .window_drag()
        .child(logo_mark);
    if compact {
        logo = logo.justify_center();
    } else {
        logo = logo.child(text("vidcue").font(ty::BRAND).color(t.text));
    }

    let mut nav = col().gap(2.0).px(10.0).role("tablist").focus_group();
    for item in &main {
        let badge = (item.id == "downloads" && active > 0).then(|| active.to_string());
        nav = nav.child(nav_row(app, item, selected == item.id, compact, badge));
    }
    let mut lib = col().gap(2.0).px(10.0).pt(18.0);
    if !compact {
        lib = lib.child(
            text("Library")
                .font(ty::LABEL)
                .color(t.muted)
                .px(14.0)
                .pb(6.0),
        );
    } else {
        lib = lib.child(el().h(1.0).mx(14.0).mb(10.0).fill(t.hairline));
    }
    for item in &library {
        let badge = (item.id == "library-queue" && !app.player.queue.is_empty())
            .then(|| app.player.queue.len().to_string());
        lib = lib.child(nav_row(app, item, selected == item.id, compact, badge));
    }

    let account = Item {
        id: "account",
        label: if app.account.connected {
            "Account"
        } else {
            "Sign in"
        },
        glyph: &ic::USER,
        route: Route::Account,
    };
    let settings = Item {
        id: "settings",
        label: "Settings",
        glyph: &ic::SETTINGS,
        route: Route::Settings(SettingsSection::Appearance),
    };
    let foot = col()
        .gap(2.0)
        .px(10.0)
        .pb(14.0)
        .child(nav_row(app, &account, selected == "account", compact, None))
        .child(nav_row(
            app,
            &settings,
            selected == "settings",
            compact,
            None,
        ));

    col()
        .key("sidebar")
        .w(width)
        .h_full()
        .fill(t.chrome)
        .motion(motion::GLIDE)
        .hover_group()
        .child(logo)
        .child(
            col()
                .grow(1.0)
                .min_h(0.0)
                .scroll(rune::ScrollSpec {
                    stick_bottom: false,
                })
                .scroll_edge_fade(16.0)
                .child(col().w_full().pt(6.0).child(nav).child(lib)),
        )
        .child(foot)
        .child(
            el().pin(None, Some(0.0), Some(0.0), Some(0.0))
                .w(1.0)
                .fill(t.hairline),
        )
}
