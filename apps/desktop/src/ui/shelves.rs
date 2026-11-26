//! Shelves: a feed whose items carry two or more provider sections ("Quick picks", "Listen
//! again") is shown as titled rows, as YouTube Music and the Android app show it. A shelf of
//! songs is a Quick picks grid (four rows, columns scrolling sideways); any other shelf is a
//! carousel of cards with ‹ › arrows. "Show all" opens a shelf as a grid on the same page.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rune::{Color, El, Live, ScrollPos, Tx, col, el, icon, row, spacer, text};
use vidcue_core::{BrowseRequest, Media, MediaKind};

use crate::app::{App, FeedRoute, Tab};
use crate::icons as ic;
use crate::theme::{sz, ty};
use crate::ui::widgets::{self, CardCtx};

const GAP: f32 = 20.0;
/// Rows in a Quick picks column.
const PICK_ROWS: usize = 4;

/// One titled shelf: its heading and the items under it, in feed order.
#[derive(Clone, Debug, PartialEq)]
pub struct Shelf {
    pub title: String,
    pub items: Vec<Media>,
}

impl Shelf {
    pub fn songs(&self) -> bool {
        self.items.iter().all(|m| m.kind == MediaKind::Song)
    }
    fn playable(&self) -> bool {
        self.items
            .iter()
            .any(|m| !m.collection() && !m.live && !m.url.is_empty())
    }
}

/// Groups a Home or Music feed into shelves by section, when it has at least two named ones.
/// Searches and single-section feeds stay a grid (`None`). Items without a section follow
/// under "More for you".
pub fn group(route: &FeedRoute, items: &[Media]) -> Option<Vec<Shelf>> {
    if route.tab == Tab::Music && matches!(route.request, BrowseRequest::Search(_)) {
        return search_groups(items);
    }
    if route.tab == Tab::Radio
        || crate::feed::search_query(&route.request).is_some()
        || matches!(route.request, BrowseRequest::RadioSearch(_))
    {
        return None;
    }
    let mut shelves: Vec<Shelf> = Vec::new();
    for m in items {
        let title = m.section.trim().to_owned();
        // The shelf heading names the section; cards don't repeat it.
        let mut item = m.clone();
        item.section.clear();
        match shelves.iter_mut().find(|s| s.title == title) {
            Some(s) => s.items.push(item),
            None => shelves.push(Shelf {
                title,
                items: vec![item],
            }),
        }
    }
    if shelves.iter().filter(|s| !s.title.is_empty()).count() < 2 {
        return None;
    }
    for s in &mut shelves {
        if s.title.is_empty() {
            s.title = "More for you".into();
        }
    }
    Some(shelves)
}

/// A music search's "All" results grouped as music apps group them: the top artist, then
/// songs, albums, artists, playlists and videos, each under its heading.
fn search_groups(items: &[Media]) -> Option<Vec<Shelf>> {
    let order = |k: MediaKind| match k {
        MediaKind::Artist | MediaKind::Channel => 0,
        MediaKind::Song => 1,
        MediaKind::Album => 2,
        MediaKind::Playlist => 4,
        MediaKind::Video | MediaKind::Station => 5,
    };
    let mut sorted: Vec<&Media> = items.iter().collect();
    sorted.sort_by_key(|m| order(m.kind));
    let mut shelves: Vec<Shelf> = Vec::new();
    for m in sorted {
        let title = match m.kind {
            MediaKind::Artist | MediaKind::Channel if shelves.is_empty() => "Top result",
            MediaKind::Artist | MediaKind::Channel => "Artists",
            MediaKind::Song => "Songs",
            MediaKind::Album => "Albums",
            MediaKind::Playlist => "Playlists",
            _ => "Videos",
        };
        let mut m = m.clone();
        m.section.clear();
        match shelves.last_mut().filter(|s| s.title == title) {
            Some(s) => s.items.push(m),
            None => shelves.push(Shelf {
                title: title.into(),
                items: vec![m],
            }),
        }
    }
    (shelves.len() >= 2).then_some(shelves)
}

/// The key under which a page remembers its opened shelf.
pub fn open_key(route: &FeedRoute) -> String {
    format!("shelf-open:{}", route.key())
}

/// Where each carousel is scrolled to, so the arrows can page it.
#[derive(Default)]
pub struct Offsets(RefCell<HashMap<String, Live<f32>>>);

impl Offsets {
    fn live(&self, key: &str) -> Live<f32> {
        self.0
            .borrow_mut()
            .entry(key.to_owned())
            .or_insert_with(|| Live::new(0.0))
            .clone()
    }
    fn get(&self, key: &str) -> f32 {
        self.0.borrow().get(key).map_or(0.0, |l| l.get())
    }
}

/// A shelf's heading: title, then Play all, Show all and the ‹ › arrows as icons.
fn heading(app: &App, route: &FeedRoute, shelf: &Shelf, scroll_key: &str, step: f32) -> El {
    let t = &app.theme;
    let mut r = row()
        .key(format!("shelf-head-{}", shelf.title))
        .w_full()
        .px(sz::PAGE_PAD)
        .h(40.0)
        .gap(6.0)
        .center_y()
        .child(
            text(&shelf.title)
                .font(ty::H2)
                .color(t.text)
                .truncate()
                .shrink(),
        )
        .child(spacer());
    if shelf.playable() {
        let (items, tab, req) = (shelf.items.clone(), route.tab, route.request.clone());
        r = r.child(
            widgets::icon_button(t, &ic::PLAY, "Play all", 34.0)
                .key(format!("shelf-play-{}", shelf.title))
                .on_press(move |s: &mut App| {
                    crate::player::actions::play_all(s, tab, req.clone(), &items, false)
                }),
        );
    }
    if !shelf.songs() && shelf.items.len() > 2 {
        let (k, title) = (open_key(route), shelf.title.clone());
        r = r.child(
            widgets::icon_button(t, &ic::GRID, "Show all", 34.0)
                .key(format!("shelf-all-{}", shelf.title))
                .on_press(move |s: &mut App| {
                    s.scratch.insert(k.clone(), title.clone());
                }),
        );
    }
    for (glyph, label, dir) in [
        (&ic::BACK, "Scroll back", -1.0f32),
        (&ic::FORWARD, "Scroll on", 1.0),
    ] {
        let key = scroll_key.to_owned();
        r = r.child(
            el().size(34.0, 34.0)
                .radius(17.0)
                .center()
                .border(1.0, t.border)
                .hover(t.hover)
                .press_scale(0.9)
                .role("button")
                .label(label)
                .cursor(rune::Cursor::Pointer)
                .on_press_tx(move |s: &mut App, tx: &mut Tx| {
                    let at = s.shelf_offsets.get(&key);
                    tx.scroll_to(key.clone(), (at + dir * step).max(0.0), 0.0);
                })
                .child(icon(glyph, 16.0).color(t.text)),
        );
    }
    r
}

/// One shelf as a page row: heading and a sideways-scrolling strip.
pub fn shelf_row(app: &App, route: &FeedRoute, shelf: &Shelf, index: usize, width: f32) -> El {
    let inner = (width - sz::PAGE_PAD * 2.0).max(320.0);
    let scroll_key = format!("shelf-{index}");
    let siblings = Rc::new(shelf.items.clone());
    let (strip, step) = if shelf.songs() {
        quick_picks(app, route, shelf, &siblings, inner)
    } else {
        carousel(app, route, shelf, index, &siblings, inner)
    };
    let live = app.shelf_offsets.live(&scroll_key);
    col()
        .w_full()
        .pb(30.0)
        .gap(14.0)
        .child(heading(app, route, shelf, &scroll_key, step))
        .child(
            el().key(scroll_key)
                .w_full()
                .scroll_x()
                .scroll_edge_fade(sz::PAGE_PAD)
                .on_scroll_offset(move |at: ScrollPos| {
                    live.set(at.offset[0]);
                })
                .child(strip),
        )
}

/// Card width for a carousel: YouTube Music fits five or six squares; videos keep YouTube's
/// roughly 300 pt cards.
fn card_width(tab: Tab, items: &[Media], inner: f32) -> f32 {
    let square = items.iter().any(|m| {
        matches!(
            m.kind,
            MediaKind::Album | MediaKind::Artist | MediaKind::Station | MediaKind::Song
        )
    }) || tab == Tab::Music;
    let min = if square { 180.0 } else { 290.0 };
    let n = ((inner + GAP) / (min + GAP)).floor().max(1.0);
    // A slice of the next card shows, so the row reads as one to scroll.
    ((inner - GAP * (n - 1.0)) / (n + 0.25)).floor()
}

fn carousel(
    app: &App,
    route: &FeedRoute,
    shelf: &Shelf,
    index: usize,
    siblings: &Rc<Vec<Media>>,
    inner: f32,
) -> (El, f32) {
    let w = card_width(route.tab, &shelf.items, inner);
    let mut strip = row().px(sz::PAGE_PAD).gap(GAP).items_start();
    for (i, m) in shelf.items.iter().enumerate() {
        let cx = CardCtx {
            key: format!("shelf-{index}-card-{i}"),
            width: w,
            tab: route.tab,
            siblings: siblings.clone(),
        };
        strip = strip.child(widgets::media_card(app, m, &cx));
    }
    let per_page = ((inner + GAP) / (w + GAP)).floor().max(1.0);
    (strip, per_page * (w + GAP))
}

fn quick_picks(
    app: &App,
    route: &FeedRoute,
    shelf: &Shelf,
    siblings: &Rc<Vec<Media>>,
    inner: f32,
) -> (El, f32) {
    let cols_visible = if inner >= 1100.0 {
        3.0
    } else if inner >= 700.0 {
        2.0
    } else {
        1.0
    };
    let col_w = ((inner - 24.0 * (cols_visible - 1.0)) / (cols_visible + 0.15)).floor();
    let mut strip = row().px(sz::PAGE_PAD).gap(24.0).items_start();
    for (c, chunk) in shelf.items.chunks(PICK_ROWS).enumerate() {
        let mut column = col().w(col_w).gap(2.0);
        for (r, m) in chunk.iter().enumerate() {
            let i = c * PICK_ROWS + r;
            let (media, tab, sib) = (m.clone(), route.tab, siblings.clone());
            column = column.child(
                widgets::media_row(
                    app,
                    m,
                    &format!("pick-{i}-{}", crate::ui::browse::short_hash(&m.url)),
                    None,
                )
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| {
                    crate::player::actions::open_item(s, tab, &media, &sib)
                }),
            );
        }
        strip = strip.child(column);
    }
    (strip, cols_visible * (col_w + 24.0))
}

/// The opened shelf's heading, with a way back to all shelves.
pub fn open_heading(app: &App, route: &FeedRoute, title: &str) -> El {
    let t = &app.theme;
    let k = open_key(route);
    row()
        .w_full()
        .px(sz::PAGE_PAD)
        .pb(16.0)
        .gap(10.0)
        .center_y()
        .child(
            widgets::icon_button(t, &ic::BACK, "All shelves", 36.0)
                .key("shelf-close")
                .on_press(move |s: &mut App| {
                    s.scratch.remove(&k);
                }),
        )
        .child(text(title).font(ty::H2).color(t.text).truncate())
        .fill(Color::TRANSPARENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(section: &str, kind: MediaKind, n: usize) -> Media {
        Media {
            kind,
            url: format!("https://x/{section}/{n}"),
            section: section.into(),
            ..Default::default()
        }
    }

    #[test]
    fn two_named_sections_make_shelves_and_searches_stay_a_grid() {
        let items = vec![
            m("Quick picks", MediaKind::Song, 1),
            m("Quick picks", MediaKind::Song, 2),
            m("Listen again", MediaKind::Album, 3),
            m("", MediaKind::Album, 4),
        ];
        let home = FeedRoute::root(Tab::Music);
        let shelves = group(&home, &items).expect("shelves");
        assert_eq!(
            shelves.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
            ["Quick picks", "Listen again", "More for you"]
        );
        assert!(shelves[0].songs() && !shelves[1].songs());
        // Video searches stay a grid (music searches group by kind; see below).
        let search = FeedRoute {
            tab: Tab::Home,
            request: BrowseRequest::Search("x".into()),
            title: "x".into(),
        };
        assert!(group(&search, &items).is_none());
        assert!(
            group(&home, &items[..2]).is_none(),
            "one section stays a grid"
        );
        assert!(group(&FeedRoute::root(Tab::Radio), &items).is_none());
    }

    #[test]
    fn music_search_groups_by_kind_with_a_top_result() {
        let items = vec![
            m("", MediaKind::Song, 1),
            m("", MediaKind::Artist, 2),
            m("", MediaKind::Album, 3),
            m("", MediaKind::Song, 4),
            m("", MediaKind::Artist, 5),
        ];
        let route = FeedRoute {
            tab: Tab::Music,
            request: BrowseRequest::Search("x".into()),
            title: "x".into(),
        };
        let shelves = group(&route, &items).expect("groups");
        let titles: Vec<&str> = shelves.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["Top result", "Artists", "Songs", "Albums"]);
        assert_eq!(shelves[2].items.len(), 2);
    }

    #[test]
    fn carousels_show_a_slice_of_the_next_card() {
        let w = card_width(Tab::Music, &[m("a", MediaKind::Album, 1)], 1100.0);
        let n = (1100.0 + GAP) / (w + GAP);
        assert!(n.fract() > 0.05, "{n}");
        assert!(w >= 150.0);
    }
}
