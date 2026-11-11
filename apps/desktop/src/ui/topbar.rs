//! The frosted top bar over each page: back/forward, search (words or a pasted link), recent
//! searches, and status shortcuts. Pages scroll underneath it, blurred.

use rune::{Align, Cx, El, Side, Transition, Tx, col, el, icon, row, spacer, text, text_input};

use crate::app::{App, Route, SearchScope, Tab};
use crate::icons as ic;
use crate::theme::{sz, ty};
use crate::ui::widgets::icon_button;

fn radio(app: &App) -> bool {
    app.search_scope() == SearchScope::Catalog(Tab::Radio)
}

pub fn view(app: &App, _cx: &mut Cx<App>) -> El {
    let t = &app.theme;
    let back = icon_button(t, &ic::BACK, "Back", 36.0)
        .key("nav-back")
        .on_press(|s: &mut App| s.go_back())
        .dim(if app.back.is_empty() { 0.35 } else { 1.0 });
    let fwd = icon_button(t, &ic::FORWARD, "Forward", 36.0)
        .key("nav-forward")
        .on_press(|s: &mut App| s.go_forward())
        .dim(if app.forward.is_empty() { 0.35 } else { 1.0 });

    let placeholder = match app.search_scope() {
        SearchScope::Catalog(Tab::Radio) => "Search radio stations",
        SearchScope::Catalog(Tab::Music) => "Search songs, albums and artists",
        SearchScope::Catalog(Tab::Home) => "Search videos, or paste a YouTube link",
        SearchScope::Movies => "Search films and series",
    };
    let field = text_input(&app.search, ty::BODY)
        .single_line()
        .placeholder(placeholder, t.faint)
        .on_edit(|s: &mut App| &mut s.search)
        .on_submit(|s: &mut App, tx: &mut Tx| {
            let q = s.search.text.clone();
            s.search_submit(&q);
            tx.blur();
        })
        .color(t.text)
        .grow(1.0)
        .label("Search");
    let mut search = row()
        .key("search")
        .h(40.0)
        .w(560.0)
        .max_w(560.0)
        .shrink()
        .px(14.0)
        .gap(10.0)
        .center_y()
        .radius(sz::R_PILL)
        .fill(t.raised.alpha(0.85))
        .border(
            1.0,
            if app.search.focused {
                t.accent.alpha(0.6)
            } else {
                t.border
            },
        )
        .motion(rune::tok::motion::SNAP)
        .child(icon(&ic::SEARCH, 17.0).color(t.muted))
        .child(field);
    if !app.search.text.is_empty() {
        search = search.child(
            el().size(24.0, 24.0)
                .radius(12.0)
                .center()
                .hover(t.hover)
                .role("button")
                .label("Clear search")
                .cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut App| s.search.clear())
                .child(icon(&ic::CLOSE, 13.0).color(t.muted)),
        );
    }

    let mut bar = row()
        .key("topbar")
        .pin(Some(0.0), Some(0.0), Some(0.0), None)
        .h(sz::TOPBAR)
        .px(20.0)
        .gap(6.0)
        .center_y()
        .backdrop_blur(22.0)
        .fill(t.glass)
        .window_drag()
        .child(back)
        .child(fwd)
        .child(el().w(14.0))
        .child(search)
        .child(spacer());

    if app.downloads.active_count() > 0 {
        let n = app.downloads.active_count();
        let frac = app.downloads.overall_fraction();
        bar = bar.child(
            row()
                .key("dl-status")
                .label("Downloads in progress")
                .h(34.0)
                .px(12.0)
                .gap(8.0)
                .center_y()
                .radius(sz::R_PILL)
                .fill(t.accent_soft)
                .hover(t.hover)
                .press_scale(0.96)
                .cursor(rune::Cursor::Pointer)
                .help("Downloads")
                .on_press(|s: &mut App| s.go_root(Route::Downloads))
                .child(icon(&ic::DOWNLOAD, 15.0).color(t.accent))
                .child(
                    text(format!("{n} · {:.0}%", frac * 100.0))
                        .font(ty::NUM)
                        .color(t.accent),
                ),
        );
    }
    if let Some(sleep) = app.player.sleep {
        bar = bar.child(
            row()
                .h(34.0)
                .px(12.0)
                .gap(8.0)
                .center_y()
                .radius(sz::R_PILL)
                .fill(t.raised)
                .help("Sleep timer")
                .child(icon(&ic::MOON, 15.0).color(t.muted))
                .child(match sleep {
                    crate::player::session::Sleep::At(_) => text("00:00")
                        .live_text(&app.player.sleep_text)
                        .font(ty::NUM)
                        .color(t.text),
                    crate::player::session::Sleep::EndOfTrack => {
                        text("End of track").font(ty::SMALL).color(t.text)
                    }
                }),
        );
    }
    let account_glyph = el()
        .size(34.0, 34.0)
        .radius(17.0)
        .center()
        .fill(if app.account.connected {
            t.accent_soft
        } else {
            t.raised
        })
        .child(icon(&ic::USER, 16.0).color(if app.account.connected {
            t.accent
        } else {
            t.muted
        }));
    bar = bar.child(
        el().key("topbar-account")
            .radius(17.0)
            .hover(t.hover)
            .press_scale(0.94)
            .role("button")
            .label("Account")
            .help(if app.account.connected {
                "Signed in"
            } else {
                "Sign in"
            })
            .cursor(rune::Cursor::Pointer)
            .on_press(|s: &mut App| s.go_root(Route::Account))
            .child(account_glyph),
    );

    let mut layer = el()
        .key("topbar-layer")
        .pin(Some(0.0), Some(0.0), Some(0.0), None)
        .child(bar)
        .child(
            el().pin(Some(0.0), Some(sz::TOPBAR - 1.0), Some(0.0), None)
                .h(1.0)
                .fill(t.hairline),
        );
    if app.search.focused
        && let Some(recents) = recents(app)
    {
        layer = layer.child(recents);
    }
    layer
}

fn recents(app: &App) -> Option<El> {
    let t = &app.theme;
    let list = if radio(app) {
        &app.prefs.recent_radio
    } else {
        &app.prefs.recent_media
    };
    let q = app.search.text.trim().to_lowercase();
    let hits: Vec<&String> = list
        .iter()
        .filter(|r| q.is_empty() || r.to_lowercase().contains(&q))
        .take(8)
        .collect();
    if hits.is_empty() || !app.config.playback_history {
        return None;
    }
    let mut c = col()
        .key("recents")
        .w(560.0)
        .padding(6.0)
        .gap(1.0)
        .radius(16.0)
        .fill(t.overlay.alpha(0.94))
        .backdrop_blur(24.0)
        .border(1.0, t.border)
        .shadow(16.0, 40.0, t.shadow)
        .role("listbox")
        .child(
            text("Recent searches")
                .font(ty::CAPTION)
                .color(t.faint)
                .px(10.0)
                .py(6.0),
        );
    for (i, r) in hits.into_iter().enumerate() {
        let query = r.clone();
        c = c.child(
            row()
                .key(format!("recent-{i}"))
                .h(36.0)
                .px(10.0)
                .gap(12.0)
                .center_y()
                .radius(10.0)
                .hover(t.hover)
                .role("option")
                .cursor(rune::Cursor::Pointer)
                .on_press_tx(move |s: &mut App, tx: &mut Tx| {
                    s.search.replace_text(&query);
                    s.search_submit(&query.clone());
                    tx.blur();
                })
                .child(icon(&ic::HISTORY, 15.0).color(t.faint))
                .child(text(r).font(ty::BODY).color(t.text).truncate()),
        );
    }
    Some(
        c.anchored("search", Side::Bottom, Align::Start, 8.0)
            .transition(Transition::offset(0.0, -6.0).combined(Transition::opacity())),
    )
}
