//! Feed pages: Home, Music, Radio, searches, channels, playlists and albums.
//!
//! The page is one virtual list: row 0 is the hero (or collection header) with chips; then the
//! grid, one row of cards per list row so only visible rows exist; then a footer that loads the
//! next page as it scrolls into view.

use std::rc::Rc;

use rune::{Color, Cx, El, GrowthSpec, ScrollSpec, Tx, col, el, icon, row, spacer, text};
use vidcue_core::{BrowseRequest, Media, MediaKind};

use crate::app::{App, FeedRoute, Route, Tab};
use crate::feed::{self, FeedState};
use crate::icons as ic;
use crate::images::{Size, thumb_url};
use crate::theme::{sz, ty};
use crate::ui::widgets::{self, CardCtx, Kind, button, chip};

const GAP: f32 = 20.0;

#[derive(Clone, Copy, PartialEq)]
enum Layout {
    Grid { cols: usize, card_w: f32 },
    List,
}

/// `square_hint` shapes the loading grid before any items arrive (music and radio).
fn layout_for(items: &[Media], width: f32, dense: bool, square_hint: bool) -> Layout {
    let songs = items.iter().filter(|m| m.kind == MediaKind::Song).count();
    if !items.is_empty() && songs * 2 > items.len() {
        return Layout::List;
    }
    let square = items
        .iter()
        .filter(|m| {
            matches!(
                m.kind,
                MediaKind::Album | MediaKind::Artist | MediaKind::Station
            )
        })
        .count()
        * 2
        > items.len().max(1)
        || (items.is_empty() && square_hint);
    let min = match (square, dense) {
        (true, _) => 176.0,
        (false, true) => 240.0,
        (false, false) => 300.0,
    };
    let inner = (width - sz::PAGE_PAD * 2.0).max(min);
    let cols = ((inner + GAP) / (min + GAP)).floor().clamp(1.0, 8.0) as usize;
    let card_w = ((inner - GAP * (cols as f32 - 1.0)) / cols as f32).floor();
    Layout::Grid { cols, card_w }
}

fn rows_for(layout: Layout, n: usize) -> usize {
    match layout {
        Layout::Grid { cols, .. } => n.div_ceil(cols),
        Layout::List => n,
    }
}

pub fn view(app: &App, cx: &mut Cx<App>, route: &FeedRoute) -> El {
    let width = cx
        .layout_size("feed-scroll")
        .map(|s| s[0])
        .unwrap_or(1180.0);
    let empty = FeedState::default();
    let state = app.feeds.get(route).unwrap_or(&empty);
    let square_hint = route.tab != Tab::Home;
    let layout = layout_for(&state.items, width, app.prefs.grid_dense, square_hint);
    let loading_first =
        state.items.is_empty() && (state.loading || !state.loaded()) && state.error.is_none();
    let body_rows = if loading_first {
        3
    } else {
        rows_for(layout, state.items.len())
    };
    let total = body_rows + 2;
    let key = route.key();
    let r = route.clone();
    let r2 = route.clone();
    let est = match layout {
        Layout::Grid { card_w, .. } => card_w * 9.0 / 16.0 + 96.0,
        Layout::List => 68.0,
    };
    let items_rc = Rc::new(state.items.clone());
    let rows = col()
        .key("feed-rows")
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows(
            total,
            est,
            move |_s: &App, i| std::borrow::Cow::Owned(format!("row-{i}")),
            move |s: &App, i| {
                let empty = FeedState::default();
                let state = s.feeds.get(&r).unwrap_or(&empty);
                if i == 0 {
                    header(s, &r, state, width)
                } else if i == total - 1 {
                    footer(s, &r, state)
                } else if loading_first {
                    skeleton_row(s, layout, i - 1, square_hint)
                } else {
                    body_row(s, &r, state, layout, i - 1, &items_rc)
                }
            },
        )
        .virtual_rows_version((
            key.as_str(),
            state.items.len(),
            state.generation,
            layout == Layout::List,
            body_rows,
        ));
    col()
        .key("feed-scroll")
        .w_full()
        .h_full()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .scroll_anchor()
        .on_at_bottom(move |s: &mut App, at: bool| {
            if at {
                feed::more(s, &r2);
            }
        })
        .child(rows)
}

fn skeleton_row(app: &App, layout: Layout, i: usize, square: bool) -> El {
    let t = &app.theme;
    match layout {
        Layout::Grid { cols, card_w } => {
            row().w_full().px(sz::PAGE_PAD).pb(28.0).gap(GAP).children(
                (0..cols).map(|c| widgets::skeleton_card(t, card_w, i * cols + c, square)),
            )
        }
        Layout::List => row()
            .w_full()
            .px(sz::PAGE_PAD)
            .py(8.0)
            .gap(14.0)
            .child(el().size(52.0, 52.0).radius(sz::R_SM).fill(t.raised))
            .child(
                col()
                    .gap(8.0)
                    .child(el().w(320.0).h(12.0).radius(6.0).fill(t.raised))
                    .child(el().w(180.0).h(10.0).radius(5.0).fill(t.surface)),
            )
            .cycle(widgets::PULSE),
    }
}

fn body_row(
    app: &App,
    route: &FeedRoute,
    state: &FeedState,
    layout: Layout,
    i: usize,
    siblings: &Rc<Vec<Media>>,
) -> El {
    match layout {
        Layout::Grid { cols, card_w } => {
            let start = i * cols;
            let end = (start + cols).min(state.items.len());
            let mut r = row()
                .w_full()
                .px(sz::PAGE_PAD)
                .pb(30.0)
                .gap(GAP)
                .items_start();
            for (n, m) in state.items[start.min(end)..end].iter().enumerate() {
                let cx = CardCtx {
                    key: format!("card-{}-{}", start + n, short_hash(&m.url)),
                    width: card_w,
                    tab: route.tab,
                    siblings: siblings.clone(),
                };
                r = r.child(widgets::media_card(app, m, &cx));
            }
            r
        }
        Layout::List => {
            let Some(m) = state.items.get(i) else {
                return el();
            };
            let index = text(format!("{}", i + 1))
                .font(ty::NUM)
                .color(app.theme.faint)
                .w(28.0);
            let media = m.clone();
            let tab = route.tab;
            let sib = siblings.clone();
            el().w_full().px(sz::PAGE_PAD - 10.0).child(
                row().w_full().center_y().child(index).child(
                    widgets::media_row(app, m, &format!("row-{i}-{}", short_hash(&m.url)), None)
                        .on_press(move |s: &mut App| {
                            crate::player::actions::open_item(s, tab, &media, &sib)
                        })
                        .cursor(rune::Cursor::Pointer)
                        .grow(1.0),
                ),
            )
        }
    }
}

pub fn short_hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish() & 0xffff_ffff
}

fn footer(app: &App, route: &FeedRoute, state: &FeedState) -> El {
    let t = &app.theme;
    let more = state.continuation.is_some() || state.radio_more;
    let r = route.clone();
    let mut f = col().w_full().py(28.0).pb(sz::DOCK + 40.0).items_center();
    if state.loading_more {
        f = f.child(
            row()
                .gap(10.0)
                .center_y()
                .cycle(widgets::PULSE)
                .child(el().size(8.0, 8.0).radius(4.0).fill(t.accent))
                .child(text("Loading more").font(ty::SMALL).color(t.muted)),
        );
    } else if more {
        f = f.child(
            button(t, "Load more", None, Kind::Secondary)
                .on_press(move |s: &mut App| feed::more(s, &r)),
        );
    } else if !state.items.is_empty() {
        f = f.child(text("You're all caught up").font(ty::SMALL).color(t.faint));
    }
    f.on_visible(move |s: &mut App, visible: bool, _tx: &mut Tx| {
        if visible {
            feed::more(
                s,
                route_clone(&s.route)
                    .as_ref()
                    .unwrap_or(&FeedRoute::root(Tab::Home)),
            );
        }
    })
}

fn route_clone(r: &Route) -> Option<FeedRoute> {
    if let Route::Feed(f) = r {
        Some(f.clone())
    } else {
        None
    }
}

fn header(app: &App, route: &FeedRoute, state: &FeedState, width: f32) -> El {
    let t = &app.theme;
    let mut h = col().w_full().pt(sz::TOPBAR + 20.0).gap(18.0).pb(22.0);
    let root = feed::is_chip(route);
    let collection = matches!(
        route.request,
        BrowseRequest::Collection(_) | BrowseRequest::CliampTracks(_)
    );
    if root {
        h = h.child(el().px(sz::PAGE_PAD).child(widgets::page_title(
            t,
            title_for(route),
            subtitle_for(route),
        )));
        h = h.child(chips(app, route));
    } else if collection {
        h = h.child(collection_header(app, route, state, width));
    } else {
        let title = if state.title.is_empty()
            || matches!(
                route.request,
                BrowseRequest::Search(_) | BrowseRequest::RadioSearch(_)
            ) {
            route.title.clone()
        } else {
            state.title.clone()
        };
        let sub = match &route.request {
            BrowseRequest::Search(_) | BrowseRequest::RadioSearch(_) => Some("Search results"),
            _ => None,
        };
        h = h.child(
            el().px(sz::PAGE_PAD)
                .child(widgets::page_title(t, &title, sub)),
        );
    }
    if !state.categories.is_empty() {
        let mut cats = row().w_full().px(sz::PAGE_PAD).gap(8.0).flex_wrap();
        for c in state.categories.iter().take(24) {
            let req = c.request.clone();
            let title = c.title.clone();
            let tab = route.tab;
            cats = cats.child(
                chip(t, &c.title, false)
                    .on_press(move |s: &mut App| s.open_feed(tab, req.clone(), title.clone())),
            );
        }
        h = h.child(cats);
    }
    if let Some(n) = &state.notice
        && !n.is_empty()
    {
        h = h.child(
            row()
                .mx(sz::PAGE_PAD)
                .px(16.0)
                .py(12.0)
                .gap(12.0)
                .center_y()
                .radius(sz::R_MD)
                .fill(t.accent_soft)
                .child(icon(&ic::INFO, 16.0).color(t.accent))
                .child(text(n).font(ty::BODY).color(t.text).shrink().wrap()),
        );
    }
    if let Some(e) = &state.error {
        let r = route.clone();
        let mut err = widgets::error_state(
            t,
            if e.needs_account() {
                "Sign in to see this"
            } else {
                "This page didn't load"
            },
            &e.message,
        );
        let mut actions = row().gap(10.0).pt(6.0).child(
            button(t, "Try again", Some(&ic::REFRESH), Kind::Secondary)
                .on_press(move |s: &mut App| feed::ensure(s, &r, true)),
        );
        if e.needs_account() || e.code == "anonymous_home_empty" {
            actions = actions.child(
                button(t, "Sign in", Some(&ic::USER), Kind::Primary)
                    .on_press(|s: &mut App| s.go_root(Route::Account)),
            );
        }
        err = err.child(actions);
        h = h.child(err);
    } else if state.loaded() && state.items.is_empty() && !state.loading {
        h = h.child(widgets::empty_state(
            t,
            &ic::SEARCH,
            "Nothing here yet",
            empty_hint(route),
        ));
    }
    h
}

fn empty_hint(route: &FeedRoute) -> &'static str {
    match route.request {
        BrowseRequest::Favorites => "Save a station with its ⋮ menu and it will wait for you here.",
        BrowseRequest::SavedMedia { .. } => {
            "Save videos and songs from their ⋮ menu to keep them on this computer."
        }
        BrowseRequest::WatchLater
        | BrowseRequest::Liked
        | BrowseRequest::Subscriptions
        | BrowseRequest::Playlists
        | BrowseRequest::Library => {
            "This list comes from your YouTube account. Sign in from Account to see it."
        }
        _ => "Try another search or come back later.",
    }
}

fn title_for(route: &FeedRoute) -> &'static str {
    match (route.tab, &route.request) {
        (Tab::Radio, BrowseRequest::Favorites) => "Saved stations",
        (Tab::Radio, _) => "Radio",
        (Tab::Music, BrowseRequest::Home) => "Music",
        (_, BrowseRequest::Home) => "Home",
        (_, BrowseRequest::Subscriptions) => "Subscriptions",
        (_, BrowseRequest::WatchLater) => "Watch later",
        (_, BrowseRequest::Liked) => "Liked",
        (_, BrowseRequest::Playlists) => "My playlists",
        (_, BrowseRequest::Library) => "Library",
        (_, BrowseRequest::SavedMedia { .. }) => "Saved locally",
        _ => "",
    }
}

fn subtitle_for(route: &FeedRoute) -> Option<&'static str> {
    match (route.tab, &route.request) {
        (Tab::Radio, BrowseRequest::Cliamp) => {
            Some("Hand-picked channels from cliamp, and stations from around the world")
        }
        (Tab::Radio, BrowseRequest::Favorites) => Some("Stations you saved"),
        _ => None,
    }
}

fn chips(app: &App, route: &FeedRoute) -> El {
    let t = &app.theme;
    let mut r = row()
        .key("chips")
        .w_full()
        .px(sz::PAGE_PAD)
        .gap(8.0)
        .role("tablist")
        .focus_group();
    for (label, req) in feed::chips(route.tab) {
        let on = req == route.request;
        let tab = route.tab;
        let title = label.to_owned();
        r = r.child(chip(t, label, on).key(format!("chip-{label}")).on_press(
            move |s: &mut App| {
                let target = Route::Feed(FeedRoute {
                    tab,
                    request: req.clone(),
                    title: title.clone(),
                });
                // Chips swap in place rather than stacking history.
                s.route = target;
                if let Route::Feed(f) = s.route.clone() {
                    crate::feed::ensure(s, &f, false);
                }
            },
        ));
    }
    if route.tab == Tab::Radio {
        for (label, cc) in radio_countries(app) {
            let tab = route.tab;
            r = r.child(chip(t, &label, false).on_press(move |s: &mut App| {
                s.open_feed(tab, BrowseRequest::RadioCountry(cc.clone()), label.clone())
            }));
        }
    }
    let refresh_route = route.clone();
    r.child(spacer()).child(
        widgets::icon_button(t, &ic::REFRESH, "Refresh", 34.0)
            .on_press(move |s: &mut App| crate::feed::ensure(s, &refresh_route, true)),
    )
}

fn radio_countries(app: &App) -> Vec<(String, String)> {
    let mut list: Vec<String> = app.config.radio_pinned_countries.clone();
    if list.is_empty() {
        let lang = std::env::var("LANG").unwrap_or_default();
        if let Some(cc) = lang.split(['_', '.']).nth(1).filter(|c| c.len() == 2) {
            list.push(cc.to_uppercase());
        }
    }
    list.into_iter()
        .take(4)
        .map(|cc| (format!("Stations in {cc}"), cc))
        .collect()
}

/// The screen's one primary action.
pub fn play_button(app: &App) -> El {
    let t = &app.theme;
    // Same height as `widgets::button`, so it lines up with the buttons beside it.
    row()
        .h(38.0)
        .px(18.0)
        .gap(8.0)
        .center_y()
        .radius(sz::R_PILL)
        .fill(Color::WHITE)
        .hover(Color::WHITE.mix(t.accent, 0.12))
        .press_scale(0.95)
        .role("button")
        .label("Play")
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .shadow(6.0, 18.0, Color::BLACK.alpha(0.3))
        .child(icon(&ic::PLAY, 17.0).color(Color::BLACK))
        .child(text("Play").font(ty::BUTTON).color(Color::BLACK))
}

fn collection_header(app: &App, route: &FeedRoute, state: &FeedState, width: f32) -> El {
    let t = &app.theme;
    let first = state.items.first();
    let square = route.tab == Tab::Music || first.is_some_and(|m| m.kind == MediaKind::Song);
    let art_w = if square { 220.0 } else { 300.0 };
    let art_h = if square { 220.0 } else { 169.0 };
    let size = if square { Size::SQUARE } else { Size::CARD };
    let mut art = el()
        .w(art_w)
        .h(art_h)
        .radius(sz::R_PANEL)
        .clip_rounded(sz::R_PANEL)
        .fill(t.raised)
        .shadow(20.0, 50.0, t.shadow)
        .center();
    let img = first.and_then(|m| app.images.get(&thumb_url(m, size), size));
    if let Some(i) = &img {
        art = art.child(
            el().w_full()
                .h_full()
                .pixels_fit(i.pixels(), rune::SurfaceFit::Fill),
        );
    } else {
        art = art.child(icon(&ic::QUEUE, 40.0).color(t.faint));
    }
    let title = if state.title.is_empty() {
        route.title.clone()
    } else {
        state.title.clone()
    };
    let n = state.items.len();
    let total: u64 = state.items.iter().filter_map(|m| m.duration).sum();
    let mut meta = format!("{n} item{}", if n == 1 { "" } else { "s" });
    if total > 0 {
        meta.push_str(&format!(" · {}", human_duration(total)));
    }
    if state.continuation.is_some() {
        meta.push_str(" · more available");
    }
    let creator = first.map(|m| m.creator.clone()).unwrap_or_default();
    let (r1, r2, r3) = (route.clone(), route.clone(), route.clone());
    let items1 = state.items.clone();
    let items2 = state.items.clone();
    let words = col()
        .grow(1.0)
        .shrink()
        .gap(10.0)
        .justify_end()
        .child(text(&title).font(ty::DISPLAY).color(t.text).max_lines(2))
        .child(
            text({
                // The kind leads the meta line instead of sitting above the title as a label.
                let kind = if square { "Collection" } else { "Playlist" };
                if creator.is_empty() {
                    format!("{kind} · {meta}")
                } else {
                    format!("{kind} by {creator} · {meta}")
                }
            })
            .font(ty::BODY)
            .color(t.muted),
        )
        .child(
            row()
                .gap(10.0)
                .pt(8.0)
                .child(play_button(app).on_press(move |s: &mut App| {
                    crate::player::actions::play_all(s, r1.tab, r1.request.clone(), &items1, false)
                }))
                .child(
                    button(t, "Shuffle", Some(&ic::SHUFFLE), Kind::Secondary).on_press(
                        move |s: &mut App| {
                            crate::player::actions::play_all(
                                s,
                                r2.tab,
                                r2.request.clone(),
                                &items2,
                                true,
                            )
                        },
                    ),
                )
                .child(
                    button(t, "Download all", Some(&ic::DOWNLOAD), Kind::Ghost)
                        .on_press(move |s: &mut App| crate::downloads::add_collection(s, &r3)),
                )
                .child(
                    widgets::icon_button(t, &ic::QUEUE, "Add all to queue", 38.0).on_press({
                        let r = route.clone();
                        move |s: &mut App| crate::downloads::enqueue_collection(s, &r)
                    }),
                ),
        );
    // A wash of the artwork's colours behind the header.
    let mut wash = el()
        .pin(Some(0.0), Some(-sz::TOPBAR - 20.0), Some(0.0), Some(-40.0))
        .clip_rounded(0.0);
    if let Some(i) = &img {
        wash = wash.child(
            el().pin(Some(-60.0), Some(-60.0), Some(-60.0), Some(-60.0))
                .pixels_fit(i.pixels(), rune::SurfaceFit::Fill)
                .blur(64.0)
                .opacity(0.45),
        );
    }
    wash = wash.child(
        el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .gradient(Color::TRANSPARENT, t.bg),
    );
    el().w_full().child(wash).child(
        row()
            .w(width)
            .px(sz::PAGE_PAD)
            .gap(28.0)
            .items_end()
            .child(art.matched(format!(
                "art-{}",
                first.map(|m| m.url.as_str()).unwrap_or("")
            )))
            .child(words),
    )
}

fn human_duration(s: u64) -> String {
    if s >= 3600 {
        format!("{} h {} min", s / 3600, s / 60 % 60)
    } else {
        format!("{} min", (s / 60).max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(kind: MediaKind) -> Media {
        Media {
            kind,
            url: "u".into(),
            ..Default::default()
        }
    }

    #[test]
    fn songs_list_videos_grid_and_columns_fit_the_width() {
        let songs = vec![video(MediaKind::Song); 6];
        assert!(layout_for(&songs, 1200.0, false, false) == Layout::List);
        let vids = vec![video(MediaKind::Video); 6];
        match layout_for(&vids, 1280.0, false, false) {
            Layout::Grid { cols, card_w } => {
                assert!(cols >= 3 && card_w >= 300.0, "{cols} {card_w}");
                assert!(
                    cols as f32 * card_w + (cols as f32 - 1.0) * GAP
                        <= 1280.0 - sz::PAGE_PAD * 2.0 + 0.5
                );
            }
            Layout::List => panic!("videos should grid"),
        }
        assert_eq!(
            rows_for(
                Layout::Grid {
                    cols: 4,
                    card_w: 300.0
                },
                9
            ),
            3
        );
    }
}
