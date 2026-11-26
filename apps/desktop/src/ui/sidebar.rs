//! The sidebar: a floating rounded panel, as on the Android tablet. Destinations at the top, a
//! Library group, and the account and settings at its foot. Narrow windows get an icon rail of
//! the same panel. The selected row is a quiet neutral pill with a filled glyph: pink is for
//! state, never for navigation.

use rune::tok::motion;
use rune::{Color, Cx, El, Icon, col, el, icon, row, text};

use crate::app::{App, FeedRoute, LibraryTab, Route, SettingsSection, Tab};
use crate::icons as ic;
use crate::theme::{sz, ty};

struct Item {
    id: &'static str,
    label: &'static str,
    glyph: &'static Icon,
    /// The filled form for the selected row, where one exists.
    selected_glyph: Option<&'static Icon>,
    route: Route,
}

fn items(app: &App) -> (Vec<Item>, Vec<Item>) {
    let mut main = vec![
        Item {
            id: "home",
            label: "Home",
            glyph: &ic::HOME,
            selected_glyph: Some(&ic::HOME_FILLED),
            route: Route::Feed(FeedRoute::root(Tab::Home)),
        },
        Item {
            id: "music",
            label: "Music",
            glyph: &ic::MUSIC,
            selected_glyph: Some(&ic::MUSIC_FILLED),
            route: Route::Feed(FeedRoute::root(Tab::Music)),
        },
        Item {
            id: "radio",
            label: "Radio",
            glyph: &ic::RADIO,
            selected_glyph: Some(&ic::RADIO_FILLED),
            route: Route::Feed(FeedRoute::root(Tab::Radio)),
        },
    ];
    if app.prefs.alpha_movies {
        main.push(Item {
            id: "movies",
            label: "Movies",
            glyph: &ic::FILM,
            selected_glyph: Some(&ic::FILM_FILLED),
            route: Route::Movies,
        });
    }
    let library = vec![
        Item {
            id: "library-queue",
            label: "Queue",
            glyph: &ic::QUEUE,
            selected_glyph: None,
            route: Route::Library(LibraryTab::Queue),
        },
        Item {
            id: "library-history",
            label: "Recently played",
            glyph: &ic::HISTORY,
            selected_glyph: None,
            route: Route::Library(LibraryTab::History),
        },
        Item {
            id: "library-saved",
            label: "Saved",
            glyph: &ic::BOOKMARK,
            selected_glyph: Some(&ic::BOOKMARK_FILLED),
            route: Route::Library(LibraryTab::Saved),
        },
        Item {
            id: "library-playlists",
            label: "Playlists",
            glyph: &ic::LIBRARY,
            selected_glyph: None,
            route: Route::Library(LibraryTab::Playlists),
        },
        Item {
            id: "downloads",
            label: "Downloads",
            glyph: &ic::DOWNLOAD,
            selected_glyph: None,
            route: Route::Downloads,
        },
    ];
    (main, library)
}

fn selected_id(app: &App) -> String {
    match &app.route {
        Route::Library(t) => format!("library-{}", format!("{t:?}").to_lowercase()),
        Route::OfflinePlaylist(_) | Route::LocalPlaylist(_) => "library-playlists".into(),
        // Searches, collections and the chips keep their catalog highlighted.
        Route::Feed(f) => match f.tab {
            Tab::Home => "home",
            Tab::Music => "music",
            Tab::Radio => "radio",
        }
        .into(),
        r => r.section().into(),
    }
}

/// The panel's width, without its outer inset.
pub fn panel_width(app: &App, window_w: f32) -> f32 {
    if compact(app, window_w) {
        sz::SIDEBAR_COMPACT
    } else {
        sz::SIDEBAR
    }
}

fn compact(app: &App, window_w: f32) -> bool {
    app.prefs.sidebar_compact || window_w < 1100.0
}

fn nav_row(app: &App, item: &Item, selected: bool, compact: bool, badge: Option<String>) -> El {
    let t = &app.theme;
    let route = item.route.clone();
    let fg = if selected { t.text } else { t.muted };
    let glyph = if selected {
        item.selected_glyph.unwrap_or(item.glyph)
    } else {
        item.glyph
    };
    let mut r = row()
        .key(format!("nav-{}", item.id))
        .h(if compact { 44.0 } else { 40.0 })
        .w_full()
        .px(if compact { 0.0 } else { 12.0 })
        .gap(16.0)
        .center_y()
        .radius(sz::R_MD)
        .fill(if selected {
            t.hover.mix(t.text, 0.04)
        } else {
            Color::TRANSPARENT
        })
        .hover(t.hover)
        .press_scale(0.97)
        .role("tab")
        .label(item.label)
        .checked(selected)
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| s.go_root(route.clone()));
    if compact {
        r = r.justify_center().help(item.label);
    }
    let mut glyph_box = el()
        .size(22.0, 22.0)
        .center()
        .child(icon(glyph, 20.0).color(fg));
    if compact && let Some(b) = &badge {
        glyph_box = glyph_box.child(
            el().pin(None, Some(-4.0), Some(-6.0), None)
                .h(16.0)
                .min_w(16.0)
                .px(4.0)
                .center()
                .radius(8.0)
                .fill(t.accent)
                .child(text(b).font(ty::BADGE).color(t.on_accent)),
        );
    }
    r = r.child(glyph_box);
    if !compact {
        r = r.child(
            text(item.label)
                .font(if selected { ty::BODY_STRONG } else { ty::BODY })
                .color(fg)
                .grow(1.0)
                .truncate(),
        );
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
    let compact = compact(app, window_w);
    let width = panel_width(app, window_w);
    let selected = selected_id(app);
    let (main, library) = items(app);
    let active = app.downloads.active_count();

    let mut logo = row()
        .h(56.0)
        .px(if compact { 0.0 } else { 12.0 })
        .gap(10.0)
        .center_y()
        .window_drag()
        .child(logo(30.0));
    if compact {
        logo = logo.justify_center();
    } else {
        logo = logo.child(text("vidcue").font(ty::BRAND).color(t.text));
    }

    let mut nav = col().gap(2.0).role("tablist").focus_group();
    for item in &main {
        nav = nav.child(nav_row(app, item, selected == item.id, compact, None));
    }
    let mut lib = col().gap(2.0).pt(if compact { 10.0 } else { 18.0 });
    if compact {
        lib = lib.child(el().h(1.0).mx(12.0).mb(10.0).fill(t.hairline));
    } else {
        lib = lib.child(
            text("Library")
                .font(ty::LABEL)
                .color(t.faint)
                .px(12.0)
                .pb(6.0),
        );
    }
    for item in &library {
        let badge = match item.id {
            "library-queue" if !app.player.queue.is_empty() => {
                Some(app.player.queue.len().to_string())
            }
            "downloads" if active > 0 => Some(active.to_string()),
            _ => None,
        };
        lib = lib.child(nav_row(app, item, selected == item.id, compact, badge));
    }

    let foot = foot(app, &selected, compact);

    let panel = col()
        .key("sidebar")
        .w(width)
        .h_full()
        .px(10.0)
        .pb(10.0)
        .radius(sz::R_SIDEBAR)
        .fill(t.panel)
        .border(1.0, t.hairline)
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
                .child(col().w_full().pt(4.0).child(nav).child(lib)),
        )
        .child(foot);
    // The gap around the panel; the page keeps its own padding on the other side.
    el().key("sidebar-room")
        .h_full()
        .pl(sz::PANEL_INSET)
        .py(sz::PANEL_INSET)
        .motion(motion::GLIDE)
        .child(panel)
}

/// The account and settings, at the foot of the panel.
fn foot(app: &App, selected: &str, compact: bool) -> El {
    let t = &app.theme;
    let account_on = selected == "account";
    let settings_on = selected == "settings";
    let label = if app.account.connected {
        "Account"
    } else {
        "Sign in"
    };
    let avatar = el()
        .size(28.0, 28.0)
        .radius(14.0)
        .center()
        .fill(if app.account.connected {
            t.accent_soft
        } else {
            t.raised
        })
        .child(icon(&ic::USER, 15.0).color(if app.account.connected {
            t.accent
        } else {
            t.muted
        }));
    let mut account = row()
        .key("nav-account")
        .h(44.0)
        .gap(12.0)
        .center_y()
        .radius(sz::R_MD)
        .fill(if account_on {
            t.hover
        } else {
            Color::TRANSPARENT
        })
        .hover(t.hover)
        .press_scale(0.97)
        .role("tab")
        .label(label)
        .checked(account_on)
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut App| s.go_root(Route::Account))
        .child(avatar);
    let settings = el()
        .key("nav-settings")
        .size(44.0, 44.0)
        .center()
        .radius(sz::R_MD)
        .fill(if settings_on {
            t.hover
        } else {
            Color::TRANSPARENT
        })
        .hover(t.hover)
        .press_scale(0.94)
        .role("tab")
        .label("Settings")
        .help("Settings")
        .checked(settings_on)
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut App| s.go_root(Route::Settings(SettingsSection::Appearance)))
        .child(icon(&ic::SETTINGS, 20.0).color(if settings_on { t.text } else { t.muted }));
    if compact {
        account = account.w_full().justify_center().help(label);
        col().gap(2.0).items_center().child(account).child(settings)
    } else {
        account = account.grow(1.0).px(8.0).child(
            text(label)
                .font(ty::BODY)
                .color(if account_on { t.text } else { t.muted })
                .truncate(),
        );
        row()
            .w_full()
            .gap(4.0)
            .center_y()
            .child(account)
            .child(settings)
    }
}
