//! Offline playlists: the card grid (also shown on the Downloads page) and a playlist's page.

use std::hash::{Hash, Hasher};

use rune::{Color, Cx, El, Tx, col, el, icon, row, text};
use serde_json::json;
use vidcue_core::offline::OfflinePlaylist;
use vidcue_core::{BrowseRequest, Media};

use super::{
    bottom_pad, create_playlist, mosaic_cells, mosaic_items, mostly_songs, move_target, moved,
    plural, presence, scroll_page, short_hash, update_playlist, virtual_page, without,
};
use crate::app::{App, Route, Tab, ToastKind};
use crate::backend::op;
use crate::icons as ic;
use crate::images::{Size, thumb_url};
use crate::theme::{Theme, sz, ty};
use crate::ui::widgets::{self, Kind, button, icon_button};

const CARD_W: f32 = 200.0;

/// A square of up to four thumbnails (one when there is a single item).
fn mosaic(app: &App, items: &[Media], side: f32, radius: f32) -> El {
    let t = &app.theme;
    let shown = mosaic_items(items);
    let cell = |m: Option<&Media>, w: f32| -> El {
        let mut c = el().w(w).h(w).fill(t.raised).clip_rounded(0.0).center();
        match m {
            Some(m) => {
                if let Some(img) = app
                    .images
                    .get(&thumb_url(m, Size::CARD), Size::CARD)
                    .filter(|_| app.config.thumbnails)
                {
                    c = c.child(
                        el().w_full()
                            .h_full()
                            .pixels_fit(img.pixels(), rune::SurfaceFit::Fill)
                            .transition(rune::Transition::opacity()),
                    );
                } else {
                    c = c.child(icon(&ic::MUSIC, (w / 4.0).clamp(14.0, 40.0)).color(t.faint));
                }
            }
            None if items.is_empty() => {
                c = c.child(icon(&ic::QUEUE, (w / 4.0).clamp(14.0, 40.0)).color(t.faint))
            }
            None => {}
        }
        c
    };
    let body = if mosaic_cells(items.len()) == 1 {
        col().child(cell(shown.first(), side))
    } else {
        let w = ((side - 2.0) / 2.0).floor();
        col()
            .gap(2.0)
            .child(
                row()
                    .gap(2.0)
                    .child(cell(shown.first(), w))
                    .child(cell(shown.get(1), w)),
            )
            .child(
                row()
                    .gap(2.0)
                    .child(cell(shown.get(2), w))
                    .child(cell(shown.get(3), w)),
            )
    };
    el().w(side)
        .h(side)
        .radius(radius)
        .clip_rounded(radius)
        .fill(t.raised)
        .child(body)
}

fn playlist_card(app: &App, p: &OfflinePlaylist) -> El {
    let t = &app.theme;
    let id = p.id.clone();
    let art = mosaic(app, &p.items, CARD_W, sz::R_CARD)
        .key(format!("pl-art-{}", p.id))
        .matched(format!("pl-{}", p.id));
    presence(
        col()
            .key(format!("pl-card-{}", p.id))
            .w(CARD_W)
            .gap(10.0)
            .hover_group()
            .role("listitem")
            .label(p.title.as_str())
            .focusable(true)
            .cursor(rune::Cursor::Pointer)
            .press_scale(0.975)
            .on_press(move |s: &mut App| s.go(Route::OfflinePlaylist(id.clone())))
            .child(
                el().w(CARD_W)
                    .h(CARD_W)
                    .radius(sz::R_CARD)
                    .shadow(0.0, 0.0, t.shadow)
                    .child(art),
            )
            .child(
                col()
                    .gap(3.0)
                    .child(text(&p.title).font(ty::TITLE).color(t.text).max_lines(2))
                    .child(
                        text(plural(p.items.len(), "item"))
                            .font(ty::SMALL)
                            .color(t.muted),
                    ),
            ),
    )
}

fn create_card(app: &App) -> El {
    let t = &app.theme;
    let open = app.library.creating;
    let tile = el()
        .w(CARD_W)
        .h(CARD_W)
        .radius(sz::R_CARD)
        .border(1.0, t.border)
        .fill(if open { t.surface } else { t.raised })
        .hover(t.overlay)
        .center()
        .child(
            col()
                .gap(8.0)
                .center()
                .child(icon(&ic::PLUS, 30.0).color(t.accent))
                .child(text("New playlist").font(ty::BODY_STRONG).color(t.text)),
        );
    let mut c = col().key("pl-create").w(CARD_W).gap(10.0);
    if open {
        c = c
            .child(
                widgets::field(t, &app.library.new_name, "Playlist name", |s: &mut App| {
                    &mut s.library.new_name
                })
                .key("pl-create-name")
                .label("Playlist name")
                .on_submit(|s: &mut App, tx: &mut Tx| {
                    submit_create(s);
                    tx.blur();
                }),
            )
            .child(
                row()
                    .gap(8.0)
                    .child(
                        button(t, "Create", Some(&ic::CHECK), Kind::Primary)
                            .on_press(submit_create),
                    )
                    .child(
                        button(t, "Cancel", None, Kind::Ghost)
                            .on_press(|s: &mut App| s.library.creating = false),
                    ),
            );
    } else {
        c = c
            .child(
                tile.role("button")
                    .label("Create playlist")
                    .focusable(true)
                    .cursor(rune::Cursor::Pointer)
                    .press_scale(0.975)
                    .on_press_tx(|s: &mut App, tx: &mut Tx| {
                        s.library.creating = true;
                        s.library.new_name.clear();
                        tx.focus("pl-create-name", false);
                    }),
            )
            .child(
                text("Offline playlists keep saved items together")
                    .font(ty::SMALL)
                    .color(t.faint)
                    .wrap(),
            );
    }
    c
}

fn submit_create(app: &mut App) {
    let typed = app.library.new_name.text.trim().to_owned();
    let name = if typed.is_empty() {
        "New playlist".to_owned()
    } else {
        typed
            .chars()
            .filter(|c| !c.is_control())
            .take(120)
            .collect()
    };
    app.library.creating = false;
    app.library.new_name.clear();
    create_playlist(app, name, Vec::new());
}

/// The create card and one card per playlist. No page padding: the caller supplies it.
pub fn playlists_grid(app: &App, _cx: &mut Cx<App>) -> El {
    let mut grid = row()
        .key("playlists-grid")
        .w_full()
        .gap(20.0)
        .flex_wrap()
        .items_start()
        .child(create_card(app));
    for (n, p) in app.library.playlists.iter().enumerate() {
        grid = grid.child(playlist_card(app, p).enter_delay(n as f32 * 0.03));
    }
    if app.library.playlists.is_empty() {
        return col().w_full().gap(8.0).child(grid).child(
            el().pt(16.0).child(
                text("Create a playlist here, or add saved items to one from their ⋮ menu.")
                    .font(ty::BODY)
                    .color(app.theme.muted),
            ),
        );
    }
    grid
}

fn signature(p: &OfflinePlaylist, app: &App) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    p.title.hash(&mut h);
    for m in &p.items {
        m.url.hash(&mut h);
    }
    (
        app.library.renaming.is_some(),
        app.library.confirm_delete,
        app.player.now.as_ref().map(|n| n.media.url.clone()),
    )
        .hash(&mut h);
    h.finish()
}

pub fn playlist_view(app: &App, _cx: &mut Cx<App>, id: &str) -> El {
    let t = &app.theme;
    let Some(p) = app.library.playlists.iter().find(|p| p.id == id) else {
        let back = button(t, "Back to playlists", Some(&ic::BACK), Kind::Secondary)
            .on_press(|s: &mut App| s.go_back());
        return scroll_page(app).pt(sz::TOPBAR + 24.0).child(
            widgets::empty_state(
                t,
                &ic::QUEUE,
                "Playlist not found",
                "It may have been deleted. Saved items stay in your library.",
            )
            .child(back),
        );
    };
    let n = p.items.len();
    let total = n + 2;
    let pid = id.to_owned();
    let version = (signature(p, app), id.to_owned());
    virtual_page("lib-scroll", total, 68.0, version, move |s: &App, i| {
        let Some(p) = s.library.playlists.iter().find(|p| p.id == pid) else {
            return el();
        };
        if i == 0 {
            return header(s, p);
        }
        if i == total - 1 {
            return bottom_pad();
        }
        item_row(s, p, i - 1)
    })
}

fn item_row(app: &App, p: &OfflinePlaylist, i: usize) -> El {
    let t = &app.theme;
    let Some(m) = p.items.get(i) else { return el() };
    let len = p.items.len();
    let key = format!("pli-{}-{}", p.id, short_hash(&m.url));
    let slot = |e: Option<El>| e.unwrap_or_else(|| el().size(30.0, 30.0));
    let up = move_target(i, len, -1).map(|to| {
        let (id, items) = (p.id.clone(), p.items.clone());
        icon_button(t, &ic::UP, "Move up", 30.0)
            .on_press(move |s: &mut App| update_playlist(s, &id, None, Some(moved(&items, i, to))))
    });
    let down = move_target(i, len, 1).map(|to| {
        let (id, items) = (p.id.clone(), p.items.clone());
        icon_button(t, &ic::DOWN, "Move down", 30.0)
            .on_press(move |s: &mut App| update_playlist(s, &id, None, Some(moved(&items, i, to))))
    });
    let remove = {
        let (id, items, url) = (p.id.clone(), p.items.clone(), m.url.clone());
        icon_button(t, &ic::CLOSE, "Remove from playlist", 30.0)
            .on_press(move |s: &mut App| update_playlist(s, &id, None, Some(without(&items, &url))))
    };
    let controls = row()
        .gap(2.0)
        .center_y()
        .child(slot(up))
        .child(slot(down))
        .child(remove);
    let (media, siblings) = (m.clone(), p.items.clone());
    let index = text(format!("{}", i + 1))
        .font(ty::NUM)
        .color(t.faint)
        .w(28.0);
    presence(
        el().key(format!("{key}-wrap"))
            .w_full()
            .px(sz::PAGE_PAD - 10.0)
            .child(
                row().w_full().center_y().child(index).child(
                    widgets::media_row(app, m, &key, Some(controls))
                        .on_press(move |s: &mut App| {
                            crate::player::actions::open_item(s, Tab::Home, &media, &siblings)
                        })
                        .cursor(rune::Cursor::Pointer)
                        .grow(1.0),
                ),
            ),
    )
}

fn header(app: &App, p: &OfflinePlaylist) -> El {
    let t = &app.theme;
    let side = 220.0;
    let art = mosaic(app, &p.items, side, 18.0)
        .key(format!("pl-art-{}", p.id))
        .shadow(20.0, 50.0, t.shadow)
        .matched(format!("pl-{}", p.id));
    let renaming = app.library.renaming.as_deref() == Some(p.id.as_str());
    let title_row = if renaming {
        row()
            .gap(8.0)
            .center_y()
            .child(
                widgets::field(t, &app.library.rename, "Playlist name", |s: &mut App| {
                    &mut s.library.rename
                })
                .key("pl-rename")
                .label("Playlist name")
                .w(360.0)
                .on_submit(|s: &mut App, tx: &mut Tx| {
                    commit_rename(s);
                    tx.blur();
                }),
            )
            .child(icon_button(t, &ic::CHECK, "Save name", 34.0).on_press(commit_rename))
            .child(
                icon_button(t, &ic::CLOSE, "Cancel rename", 34.0)
                    .on_press(|s: &mut App| s.library.renaming = None),
            )
    } else {
        let (id, title) = (p.id.clone(), p.title.clone());
        row()
            .gap(8.0)
            .center_y()
            .child(
                text(&p.title)
                    .font(ty::DISPLAY)
                    .color(t.text)
                    .max_lines(2)
                    .shrink(),
            )
            .child(
                icon_button(t, &ic::SETTINGS, "Rename playlist", 34.0).on_press_tx(
                    move |s: &mut App, tx: &mut Tx| {
                        s.library.renaming = Some(id.clone());
                        s.library.rename.replace_text(&title);
                        tx.focus("pl-rename", false);
                    },
                ),
            )
    };
    let items = p.items.clone();
    let (i1, i2, i3) = (items.clone(), items.clone(), items.clone());
    let audio = mostly_songs(&items);
    let mut actions = row().gap(10.0).pt(8.0).flex_wrap();
    if !items.is_empty() {
        actions = actions
            .child(
                button(t, "Play all", Some(&ic::PLAY), Kind::Primary).on_press(
                    move |s: &mut App| {
                        crate::player::actions::play_all(
                            s,
                            Tab::Home,
                            BrowseRequest::Home,
                            &i1,
                            false,
                        )
                    },
                ),
            )
            .child(
                button(t, "Shuffle", Some(&ic::SHUFFLE), Kind::Secondary).on_press(
                    move |s: &mut App| {
                        crate::player::actions::play_all(
                            s,
                            Tab::Home,
                            BrowseRequest::Home,
                            &i2,
                            true,
                        )
                    },
                ),
            )
            .child(
                button(t, "Download all", Some(&ic::DOWNLOAD), Kind::Ghost).on_press(
                    move |s: &mut App| {
                        for m in &i3 {
                            crate::downloads::add(s, m, audio);
                        }
                    },
                ),
            );
    }
    if app.library.confirm_delete {
        let id = p.id.clone();
        actions = actions
            .child(
                button(t, "Delete playlist", Some(&ic::TRASH), Kind::Danger)
                    .on_press(move |s: &mut App| delete_playlist(s, &id)),
            )
            .child(
                button(t, "Keep it", None, Kind::Ghost)
                    .on_press(|s: &mut App| s.library.confirm_delete = false),
            );
    } else {
        actions = actions.child(
            button(t, "Delete", Some(&ic::TRASH), Kind::Ghost)
                .on_press(|s: &mut App| s.library.confirm_delete = true),
        );
    }
    let words = col()
        .grow(1.0)
        .shrink()
        .gap(10.0)
        .justify_end()
        .child(title_row)
        .child(
            text(format!(
                "Offline playlist · {}",
                plural(p.items.len(), "item")
            ))
            .font(ty::BODY)
            .color(t.muted),
        )
        .child(actions);
    // A blurred wash of the first artwork behind the header, fading into the page.
    let mut wash = el()
        .pin(Some(0.0), Some(-sz::TOPBAR - 20.0), Some(0.0), Some(-40.0))
        .clip_rounded(0.0);
    if let Some(img) = p
        .items
        .first()
        .and_then(|m| app.images.get(&thumb_url(m, Size::CARD), Size::CARD))
    {
        wash = wash.child(
            el().pin(Some(-60.0), Some(-60.0), Some(-60.0), Some(-60.0))
                .pixels_fit(img.pixels(), rune::SurfaceFit::Fill)
                .blur(64.0)
                .opacity(0.45),
        );
    }
    wash = wash.child(
        el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .gradient(Color::TRANSPARENT, t.bg),
    );
    let back = row().px(sz::PAGE_PAD).pb(14.0).child(
        button(t, "Playlists", Some(&ic::BACK), Kind::Glass).on_press(|s: &mut App| s.go_back()),
    );
    col()
        .w_full()
        .pt(sz::TOPBAR + 20.0)
        .pb(22.0)
        .child(back_wrap(
            wash,
            back,
            row()
                .w_full()
                .px(sz::PAGE_PAD)
                .gap(28.0)
                .items_end()
                .child(art)
                .child(words),
            t,
        ))
}

fn back_wrap(wash: El, back: El, body: El, _t: &Theme) -> El {
    el().w_full()
        .child(wash)
        .child(col().w_full().child(back).child(body))
}

fn commit_rename(app: &mut App) {
    let Some(id) = app.library.renaming.take() else {
        return;
    };
    let title: String = app
        .library
        .rename
        .text
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect();
    if title.trim().is_empty()
        || app
            .library
            .playlists
            .iter()
            .find(|p| p.id == id)
            .is_some_and(|p| p.title == title.trim())
    {
        return;
    }
    update_playlist(app, &id, Some(title), None);
}

fn delete_playlist(app: &mut App, id: &str) {
    app.library.confirm_delete = false;
    let title = app
        .library
        .playlists
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.title.clone())
        .unwrap_or_default();
    app.library.playlists.retain(|p| p.id != id);
    app.be.call(
        op("offline_playlist_delete", json!({ "id": id })),
        move |app, r, _| {
            match r {
                Ok(_) => app.toast(ToastKind::Success, format!("Deleted “{title}”")),
                Err(e) => app.toast(ToastKind::Error, e.to_string()),
            }
            super::refresh_playlists(app);
        },
    );
    app.go_back();
}
