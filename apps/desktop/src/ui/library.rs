//! Library: saved items, history, offline playlists and the queue.
//!
//! One page with four tabs. A fixed header (title, tabs and a per-tab toolbar) sits above a
//! scrolling body; Saved and History are virtual lists so a large library stays cheap.
//! Playlists live in `library_playlists.rs` and the queue in `library_queue.rs`.

use std::rc::Rc;

use rune::{Cx, El, GrowthSpec, ScrollSpec, Transition, col, el, icon, row, text};
use serde_json::{Value, json};
use vidcue_core::offline::OfflinePlaylist;
use vidcue_core::{Media, MediaKind};

use crate::app::{App, LibraryTab, Route, SettingsSection, Tab, ToastKind};
use crate::backend::op;
use crate::icons as ic;
use crate::theme::{sz, ty};
use crate::ui::widgets::{self, CardCtx, Kind, button};

#[path = "library_playlists.rs"]
mod playlists;
#[path = "library_queue.rs"]
mod queue;

pub use playlists::{playlist_view, playlists_grid};

const GAP: f32 = 20.0;
const TABS: [(LibraryTab, &str); 4] = [
    (LibraryTab::Saved, "Saved"),
    (LibraryTab::History, "History"),
    (LibraryTab::Playlists, "Playlists"),
    (LibraryTab::Queue, "Queue"),
];

#[derive(Default)]
pub struct LibraryUi {
    pub saved: Vec<Media>,
    pub history: Vec<Media>,
    pub playlists: Vec<OfflinePlaylist>,
    /// The Saved tab's search field.
    pub filter: rune::TextEdit,
    /// The "new playlist" card's name field, and whether it is open.
    pub new_name: rune::TextEdit,
    pub creating: bool,
    /// The playlist page's inline rename: the id being renamed, and its field.
    pub renaming: Option<String>,
    pub rename: rune::TextEdit,
    pub confirm_clear: bool,
    pub confirm_delete: bool,
}

pub fn enter(app: &mut App, tab: LibraryTab) {
    app.library.confirm_clear = false;
    app.library.confirm_delete = false;
    app.library.creating = false;
    app.library.renaming = None;
    refresh_playlists(app);
    match tab {
        LibraryTab::Saved => app.refresh_saved(),
        LibraryTab::History => refresh_history(app),
        LibraryTab::Playlists => {}
        LibraryTab::Queue => app.player.refresh_up_next(&app.be),
    }
}

fn refresh_history(app: &mut App) {
    app.be.call(op("history_list", json!({})), |app, r, _| {
        if let Ok(Value::Array(items)) = r {
            app.library.history = items
                .into_iter()
                .filter_map(|m| serde_json::from_value(m).ok())
                .collect();
        }
    });
}

pub fn refresh_playlists(app: &mut App) {
    app.be
        .call(op("offline_playlists", json!({})), |app, r, _| {
            if let Ok(Value::Array(items)) = r {
                app.library.playlists = items
                    .into_iter()
                    .filter_map(|p| serde_json::from_value(p).ok())
                    .collect();
            }
        });
}

/// Writes a playlist's items (and optionally title) through the engine; the local copy is
/// updated first so the page responds at once, and the engine's copy is re-read afterwards.
pub(super) fn update_playlist(
    app: &mut App,
    id: &str,
    title: Option<String>,
    items: Option<Vec<Media>>,
) {
    if let Some(p) = app.library.playlists.iter_mut().find(|p| p.id == id) {
        if let Some(t) = &title {
            p.title = t.trim().to_owned();
        }
        if let Some(i) = &items {
            p.items = i.clone();
        }
    }
    let mut req = json!({ "id": id });
    if let Some(t) = title {
        req["title"] = json!(t.trim());
    }
    if let Some(i) = items {
        req["items"] = json!(i);
    }
    app.be
        .call(op("offline_playlist_update", req), |app, r, _| {
            if let Err(e) = r {
                app.toast(ToastKind::Error, e.to_string());
            }
            refresh_playlists(app);
        });
}

pub fn add_to_playlist(app: &mut App, id: &str, media: Media) {
    let Some(p) = app.library.playlists.iter().find(|p| p.id == id) else {
        app.toast(ToastKind::Error, "That playlist is no longer available");
        return;
    };
    let title = p.title.clone();
    let Some(items) = with_added(&p.items, &media) else {
        app.toast(ToastKind::Info, format!("Already in “{title}”"));
        return;
    };
    let id = id.to_owned();
    app.be.call(
        op(
            "offline_playlist_update",
            json!({ "id": id, "items": items }),
        ),
        move |app, r, _| {
            match r {
                Ok(_) => app.toast(ToastKind::Success, format!("Added to “{title}”")),
                Err(e) => app.toast(ToastKind::Error, e.to_string()),
            }
            refresh_playlists(app);
        },
    );
}

pub fn new_playlist_with(app: &mut App, media: Media) {
    let existing: Vec<String> = app
        .library
        .playlists
        .iter()
        .map(|p| p.title.clone())
        .collect();
    let name = playlist_name_for(&media.title, &existing);
    create_playlist(app, name, vec![media]);
}

pub(super) fn create_playlist(app: &mut App, name: String, items: Vec<Media>) {
    app.be.call(
        op(
            "offline_playlist_create",
            json!({ "title": name, "items": items }),
        ),
        move |app, r, _| {
            match r {
                Ok(_) => app.toast(ToastKind::Success, format!("Created “{name}”")),
                Err(e) => app.toast(ToastKind::Error, e.to_string()),
            }
            refresh_playlists(app);
        },
    );
}

// ---------------------------------------------------------------------------------------------
// Pure helpers

/// The playlist after adding `media`, or `None` when it is already in there.
pub(super) fn with_added(items: &[Media], media: &Media) -> Option<Vec<Media>> {
    if items.iter().any(|m| m.url == media.url) {
        return None;
    }
    let mut next = items.to_vec();
    next.push(media.clone());
    Some(next)
}

pub(super) fn without(items: &[Media], url: &str) -> Vec<Media> {
    items.iter().filter(|m| m.url != url).cloned().collect()
}

/// Where item `i` of `len` lands when nudged by `delta`, if that is inside the list.
pub(super) fn move_target(i: usize, len: usize, delta: i32) -> Option<usize> {
    let to = i as i64 + delta as i64;
    (to >= 0 && (to as usize) < len && to as usize != i).then_some(to as usize)
}

pub(super) fn moved<T: Clone>(items: &[T], from: usize, to: usize) -> Vec<T> {
    let mut next = items.to_vec();
    if from < next.len() && to < next.len() {
        let item = next.remove(from);
        next.insert(to, item);
    }
    next
}

/// The artwork of a playlist's mosaic: its first four items.
pub(super) fn mosaic_items(items: &[Media]) -> &[Media] {
    &items[..items.len().min(4)]
}

/// One full-bleed picture for a single item, a 2×2 grid otherwise.
pub(super) fn mosaic_cells(n: usize) -> usize {
    if n <= 1 { 1 } else { 4 }
}

/// A name for a playlist started from `title`, unique among `existing`.
pub(super) fn playlist_name_for(title: &str, existing: &[String]) -> String {
    let clean: String = title
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(60)
        .collect();
    let base = if clean.is_empty() {
        "New playlist".to_owned()
    } else {
        clean.trim().to_owned()
    };
    if !existing.iter().any(|e| e.eq_ignore_ascii_case(&base)) {
        return base;
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|c| !existing.iter().any(|e| e.eq_ignore_ascii_case(c)))
        .unwrap_or(base)
}

/// Case-insensitive match on title and creator; an empty query keeps everything.
pub(super) fn filter_media(items: &[Media], query: &str) -> Vec<Media> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return items.to_vec();
    }
    items
        .iter()
        .filter(|m| m.title.to_lowercase().contains(&q) || m.creator.to_lowercase().contains(&q))
        .cloned()
        .collect()
}

/// Columns and card width that fill `width` with cards of at least `min`.
pub(super) fn grid_columns(width: f32, min: f32) -> (usize, f32) {
    let inner = (width - sz::PAGE_PAD * 2.0).max(min);
    let cols = ((inner + GAP) / (min + GAP)).floor().clamp(1.0, 8.0) as usize;
    (
        cols,
        ((inner - GAP * (cols as f32 - 1.0)) / cols as f32).floor(),
    )
}

pub(super) fn mostly_songs(items: &[Media]) -> bool {
    !items.is_empty()
        && items.iter().filter(|m| m.kind == MediaKind::Song).count() * 2 > items.len()
}

pub(super) fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

pub(super) fn short_hash(s: &str) -> u64 {
    crate::ui::browse::short_hash(s)
}

/// Rows enter and leave with a short slide and fade, so removals and reorders read as motion.
pub(super) fn presence(e: El) -> El {
    e.transition(Transition::offset(0.0, 10.0).combined(Transition::opacity()))
}

pub(super) fn section_title(t: &crate::theme::Theme, title: &str, detail: Option<String>) -> El {
    let mut r = row()
        .w_full()
        .px(sz::PAGE_PAD)
        .pt(18.0)
        .pb(6.0)
        .gap(10.0)
        .center_y()
        .child(text(title).font(ty::H2).color(t.text));
    if let Some(d) = detail {
        r = r.child(text(d).font(ty::SMALL).color(t.muted));
    }
    r
}

pub(super) fn bottom_pad() -> El {
    el().w_full().h(sz::DOCK + 48.0)
}

fn banner(t: &crate::theme::Theme, message: &str, action: Option<El>) -> El {
    let mut r = row()
        .mx(sz::PAGE_PAD)
        .px(16.0)
        .py(12.0)
        .gap(12.0)
        .center_y()
        .radius(sz::R_MD)
        .fill(t.accent_soft)
        .child(icon(&ic::INFO, 16.0).color(t.accent))
        .child(
            text(message)
                .font(ty::BODY)
                .color(t.text)
                .grow(1.0)
                .shrink()
                .wrap(),
        );
    if let Some(a) = action {
        r = r.child(a);
    }
    r
}

// ---------------------------------------------------------------------------------------------
// Page

fn pick_tab(s: &mut App, i: usize) {
    let tab = TABS[i.min(TABS.len() - 1)].0;
    if s.route != Route::Library(tab) {
        s.route = Route::Library(tab);
        s.menu = None;
        enter(s, tab);
    }
}

pub fn view(app: &App, cx: &mut Cx<App>, tab: LibraryTab) -> El {
    let t = &app.theme;
    let labels: Vec<&str> = TABS.iter().map(|(_, l)| *l).collect();
    let selected = TABS.iter().position(|(x, _)| *x == tab).unwrap_or(0);
    let header = col()
        .w_full()
        .px(sz::PAGE_PAD)
        .pt(sz::TOPBAR + 24.0)
        .pb(12.0)
        .gap(14.0)
        .child(widgets::page_title(
            t,
            "Library",
            Some("Everything you kept, played and queued"),
        ))
        .child(
            row()
                .gap(16.0)
                .center_y()
                .child(widgets::segmented(
                    t, "lib-tabs", &labels, selected, pick_tab,
                ))
                .child(toolbar(app, tab)),
        );
    let body = match tab {
        LibraryTab::Saved => saved_body(app, cx),
        LibraryTab::History => history_body(app, cx),
        LibraryTab::Playlists => scroll_page(app)
            .child(playlists::playlists_grid(app, cx).px(sz::PAGE_PAD).pt(8.0))
            .child(bottom_pad()),
        LibraryTab::Queue => scroll_page(app).child(queue::body(app)).child(bottom_pad()),
    };
    col().w_full().h_full().min_h(0.0).child(header).child(body)
}

fn scroll_page(_app: &App) -> El {
    col()
        .key("lib-scroll")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
}

fn toolbar(app: &App, tab: LibraryTab) -> El {
    let t = &app.theme;
    let mut bar = row().grow(1.0).gap(10.0).center_y().justify_end();
    match tab {
        LibraryTab::Saved => {
            bar = bar
                .child(
                    text(plural(app.library.saved.len(), "item"))
                        .font(ty::SMALL)
                        .color(t.muted),
                )
                .child(
                    widgets::field(t, &app.library.filter, "Search saved", |s: &mut App| {
                        &mut s.library.filter
                    })
                    .key("lib-filter")
                    .w(260.0)
                    .label("Search saved"),
                );
        }
        LibraryTab::History => {
            if app.library.confirm_clear {
                bar = bar
                    .child(
                        text("Clear all history?")
                            .font(ty::BODY_STRONG)
                            .color(t.text),
                    )
                    .child(
                        button(t, "Clear", Some(&ic::TRASH), Kind::Danger).on_press(clear_history),
                    )
                    .child(
                        button(t, "Cancel", None, Kind::Ghost)
                            .on_press(|s: &mut App| s.library.confirm_clear = false),
                    );
            } else if !app.library.history.is_empty() {
                bar = bar.child(
                    button(t, "Clear history", Some(&ic::TRASH), Kind::Secondary)
                        .on_press(|s: &mut App| s.library.confirm_clear = true),
                );
            }
        }
        LibraryTab::Queue => {
            if !app.player.queue.is_empty() {
                bar = bar.child(
                    button(t, "Clear queue", Some(&ic::TRASH), Kind::Secondary)
                        .on_press(queue::clear),
                );
            }
        }
        LibraryTab::Playlists => {}
    }
    bar
}

fn clear_history(app: &mut App) {
    app.library.confirm_clear = false;
    app.library.history.clear();
    app.be
        .call(op("history_clear", json!({})), |app, r, _| match r {
            Ok(_) => app.toast(ToastKind::Success, "History cleared"),
            Err(e) => {
                app.toast(ToastKind::Error, e.to_string());
                refresh_history(app);
            }
        });
}

/// A virtual list under the fixed header: `total` rows built by `row_at`.
fn virtual_page(
    width_key: &str,
    total: usize,
    est: f32,
    version: impl std::hash::Hash + 'static,
    row_at: impl Fn(&App, usize) -> El + 'static,
) -> El {
    let rows = col()
        .key(format!("{width_key}-rows"))
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows(
            total,
            est,
            |_s: &App, i| std::borrow::Cow::Owned(format!("row-{i}")),
            row_at,
        )
        .virtual_rows_version(version);
    col()
        .key(width_key.to_owned())
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .scroll_anchor()
        .child(rows)
}

fn saved_body(app: &App, cx: &mut Cx<App>) -> El {
    let t = &app.theme;
    if app.library.saved.is_empty() {
        return scroll_page(app).child(widgets::empty_state(
            t,
            &ic::BOOKMARK,
            "Nothing saved yet",
            "Save videos and songs from their ⋮ menu to keep them on this computer.",
        ));
    }
    let query = app.library.filter.text.clone();
    let items = Rc::new(filter_media(&app.library.saved, &query));
    if items.is_empty() {
        return scroll_page(app).child(widgets::empty_state(
            t,
            &ic::SEARCH,
            "No matches",
            "Nothing saved matches that search.",
        ));
    }
    let width = cx.layout_size("lib-scroll").map(|s| s[0]).unwrap_or(1180.0);
    let (cols, card_w) = grid_columns(width, 260.0);
    let rows = items.len().div_ceil(cols);
    let list = items.clone();
    virtual_page(
        "lib-scroll",
        rows + 1,
        card_w * 9.0 / 16.0 + 96.0,
        (
            query,
            items.len(),
            cols,
            short_hash(&items.first().map(|m| m.url.clone()).unwrap_or_default()),
        ),
        move |s: &App, i| {
            if i == rows {
                return bottom_pad();
            }
            let (start, end) = (i * cols, ((i + 1) * cols).min(list.len()));
            let mut r = row()
                .w_full()
                .px(sz::PAGE_PAD)
                .pb(28.0)
                .gap(GAP)
                .items_start();
            for (n, m) in list[start.min(end)..end].iter().enumerate() {
                let cx = CardCtx {
                    key: format!("saved-{}", short_hash(&m.url)),
                    width: card_w,
                    tab: Tab::Home,
                    siblings: list.clone(),
                };
                r = r.child(widgets::media_card(s, m, &cx).enter_delay(n as f32 * 0.03));
            }
            r
        },
    )
}

fn history_body(app: &App, _cx: &mut Cx<App>) -> El {
    let t = &app.theme;
    let mut notice = None;
    if !app.config.playback_history {
        notice = Some("Playback history is off, so new plays are not recorded. Turn it on in Settings, Privacy.".to_owned());
    }
    if app.library.history.is_empty() {
        let mut page = scroll_page(app);
        if let Some(n) = &notice {
            page = page.child(banner(t, n, Some(open_privacy(t))));
        }
        return page.child(widgets::empty_state(
            t,
            &ic::HISTORY,
            "No history yet",
            "Videos and songs you play will show up here.",
        ));
    }
    let n = app.library.history.len();
    let total = n + 3;
    virtual_page(
        "lib-scroll",
        total,
        68.0,
        (
            n,
            notice.is_some(),
            app.library.history.first().map(|m| short_hash(&m.url)),
        ),
        move |s: &App, i| {
            let t = &s.theme;
            match i {
                0 => match &notice {
                    Some(msg) => banner(t, msg, Some(open_privacy(t))).pb(8.0),
                    None => el().h(0.0),
                },
                1 => section_title(
                    t,
                    "Recently played",
                    Some(plural(s.library.history.len(), "item")),
                ),
                i if i == total - 1 => bottom_pad(),
                i => {
                    let Some(m) = s.library.history.get(i - 2) else {
                        return el();
                    };
                    let media = m.clone();
                    let key = format!("hist-{}-{}", i - 2, short_hash(&m.url));
                    el().w_full().px(sz::PAGE_PAD - 10.0).child(
                        widgets::media_row(s, m, &key, None)
                            .on_press(move |s: &mut App| {
                                crate::player::actions::open_item(
                                    s,
                                    Tab::Home,
                                    &media,
                                    std::slice::from_ref(&media),
                                )
                            })
                            .cursor(rune::Cursor::Pointer),
                    )
                }
            }
        },
    )
}

fn open_privacy(t: &crate::theme::Theme) -> El {
    button(t, "Open settings", None, Kind::Secondary)
        .on_press(|s: &mut App| s.go(Route::Settings(SettingsSection::Privacy)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(url: &str, title: &str) -> Media {
        Media {
            url: url.into(),
            title: title.into(),
            ..Default::default()
        }
    }

    #[test]
    fn move_target_stays_inside_the_list() {
        assert_eq!(move_target(0, 3, -1), None);
        assert_eq!(move_target(0, 3, 1), Some(1));
        assert_eq!(move_target(2, 3, 1), None);
        assert_eq!(move_target(2, 3, -1), Some(1));
        assert_eq!(move_target(0, 0, 1), None);
    }

    #[test]
    fn moved_reorders_without_losing_items() {
        assert_eq!(moved(&[1, 2, 3, 4], 0, 2), vec![2, 3, 1, 4]);
        assert_eq!(moved(&[1, 2, 3, 4], 3, 0), vec![4, 1, 2, 3]);
        assert_eq!(moved(&[1, 2], 5, 0), vec![1, 2]);
    }

    #[test]
    fn mosaic_takes_four_and_one_cell_for_a_single_item() {
        let items: Vec<Media> = (0..6).map(|i| m(&format!("u{i}"), "t")).collect();
        assert_eq!(mosaic_items(&items).len(), 4);
        assert_eq!(mosaic_items(&items[..2]).len(), 2);
        assert_eq!(mosaic_cells(0), 1);
        assert_eq!(mosaic_cells(1), 1);
        assert_eq!(mosaic_cells(2), 4);
    }

    #[test]
    fn adding_skips_duplicates_and_removing_drops_by_url() {
        let a = m("a", "A");
        let b = m("b", "B");
        assert!(with_added(std::slice::from_ref(&a), &a).is_none());
        let next = with_added(std::slice::from_ref(&a), &b).unwrap();
        assert_eq!(next.len(), 2);
        let left = without(&next, "a");
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].url, "b");
    }

    #[test]
    fn playlist_names_are_clean_and_unique() {
        assert_eq!(playlist_name_for("", &[]), "New playlist");
        assert_eq!(playlist_name_for("  Road\ntrip ", &[]), "Roadtrip");
        let taken = vec!["Mix".to_owned(), "mix 2".to_owned()];
        assert_eq!(playlist_name_for("Mix", &taken), "Mix 3");
        assert_eq!(playlist_name_for(&"x".repeat(200), &[]).chars().count(), 60);
    }

    #[test]
    fn saved_filter_matches_title_and_creator() {
        let mut a = m("a", "Lo-fi beats");
        a.creator = "Chill Channel".into();
        let b = m("b", "News");
        let all = [a, b];
        assert_eq!(filter_media(&all, "").len(), 2);
        assert_eq!(filter_media(&all, " LOFI").len(), 0);
        assert_eq!(filter_media(&all, "lo-fi").len(), 1);
        assert_eq!(filter_media(&all, "chill").len(), 1);
    }

    #[test]
    fn grid_columns_fill_the_width() {
        let (cols, w) = grid_columns(1180.0, 260.0);
        assert_eq!(cols, 4);
        assert!(w * 4.0 + GAP * 3.0 <= 1180.0 - sz::PAGE_PAD * 2.0 + 0.5);
        assert_eq!(grid_columns(200.0, 260.0).0, 1);
    }

    #[test]
    fn songs_make_a_download_audio() {
        let song = Media {
            kind: MediaKind::Song,
            ..Default::default()
        };
        assert!(mostly_songs(&[
            song.clone(),
            song.clone(),
            Media::default()
        ]));
        assert!(!mostly_songs(&[song, Media::default()]));
        assert!(!mostly_songs(&[]));
    }
}
