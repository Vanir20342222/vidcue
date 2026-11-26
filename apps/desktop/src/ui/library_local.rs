//! The engine's own library, as the Android app has it: playlists (synced across devices once
//! a Vidcue account is signed in), followed channels and feed filters. They live in the shared
//! engine (`playlists`, `follows`, `filters` ops), so whatever Android shows here appears on
//! the desktop after a sync, and the other way round.

use std::collections::HashMap;

use rune::{Color, Cx, El, TextEdit, Tx, col, el, icon, row, text};
use serde::Deserialize;
use serde_json::{Value, json};
use vidcue_core::{BrowseRequest, Media};

use crate::app::{App, Route, Tab, ToastKind};
use crate::backend::op;
use crate::icons as ic;
use crate::images::{Size, thumb_url};
use crate::theme::{sz, ty};
use crate::ui::widgets::{self, Kind, button, icon_button};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Playlist {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
    pub shared: bool,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub cover: Option<Media>,
}

impl Playlist {
    /// Shared playlists joined from someone else: only the owner renames or deletes them.
    pub fn member(&self) -> bool {
        self.role.as_deref() == Some("member")
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Follow {
    pub channel_id: String,
    pub title: String,
    #[serde(default)]
    pub thumbnail: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FeedFilter {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub value: Option<String>,
}

/// The switch-like filters, as on Android: hide Shorts, watched videos and live streams.
pub const SWITCH_FILTERS: [(&str, &str); 3] = [
    ("shorts", "Hide Shorts"),
    ("watched", "Hide watched videos"),
    ("live", "Hide live streams"),
];

#[derive(Default)]
pub struct Local {
    pub playlists: Vec<Playlist>,
    /// Entries by playlist id: (entry id, media), in order.
    pub entries: HashMap<String, Vec<(String, Media)>>,
    pub follows: Vec<Follow>,
    pub filters: Vec<FeedFilter>,
    pub new_name: TextEdit,
    pub creating: bool,
    pub renaming: Option<String>,
    pub rename: TextEdit,
    pub confirm_delete: Option<String>,
    /// Settings: a keyword or channel to hide.
    pub hide: TextEdit,
    /// A reorder is on its way to the engine; further moves wait for it.
    pub moving: bool,
}

impl Local {
    pub fn following(&self, channel_id: &str) -> bool {
        !channel_id.is_empty() && self.follows.iter().any(|f| f.channel_id == channel_id)
    }
    pub fn switched(&self, kind: &str) -> Option<&FeedFilter> {
        self.filters.iter().find(|f| f.kind == kind)
    }
}

fn list<T: for<'de> Deserialize<'de>>(v: Value) -> Vec<T> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| serde_json::from_value(x.clone()).ok())
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Engine calls

pub fn refresh(app: &mut App) {
    app.be.call(op("playlists", json!({})), |app, r, _| {
        if let Ok(v) = r {
            app.library.local.playlists = list(v);
        }
    });
    app.be.call(op("follows", json!({})), |app, r, _| {
        if let Ok(v) = r {
            app.library.local.follows = list(v);
        }
    });
    refresh_filters(app);
}

pub fn refresh_filters(app: &mut App) {
    app.be.call(op("filters", json!({})), |app, r, _| {
        if let Ok(v) = r {
            app.library.local.filters = list(v);
        }
    });
}

pub fn refresh_entries(app: &mut App, id: &str) {
    let key = id.to_owned();
    app.be.call(
        op("playlist_entries", json!({ "id": id })),
        move |app, r, _| match r {
            Ok(v) => {
                let entries = v
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| {
                        Some((
                            e["entry_id"].as_str()?.to_owned(),
                            serde_json::from_value::<Media>(e["media"].clone()).ok()?,
                        ))
                    })
                    .collect();
                app.library.local.entries.insert(key, entries);
                app.library.local.moving = false;
            }
            Err(e) => {
                app.library.local.moving = false;
                app.toast(ToastKind::Error, e.to_string())
            }
        },
    );
}

fn after_write(app: &mut App, id: Option<String>) {
    refresh(app);
    if let Some(id) = id {
        refresh_entries(app, &id);
    }
}

pub fn create(app: &mut App, title: String, media: Option<Media>) {
    let name = title.clone();
    app.be.call(
        op("playlist_create", json!({ "title": title, "media": media })),
        move |app, r, _| {
            match r {
                Ok(_) => app.toast(ToastKind::Success, format!("Created “{name}”")),
                Err(e) => app.toast(ToastKind::Error, e.to_string()),
            }
            after_write(app, None);
        },
    );
}

pub fn add(app: &mut App, id: &str, media: Media) {
    let title = app
        .library
        .local
        .playlists
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.title.clone())
        .unwrap_or_default();
    if app
        .library
        .local
        .entries
        .get(id)
        .is_some_and(|e| e.iter().any(|(_, m)| m.url == media.url))
    {
        app.toast(ToastKind::Info, format!("Already in “{title}”"));
        return;
    }
    let pid = id.to_owned();
    app.be.call(
        op("playlist_add", json!({ "id": id, "media": media })),
        move |app, r, _| {
            match r {
                Ok(_) => app.toast(ToastKind::Success, format!("Added to “{title}”")),
                Err(e) => app.toast(ToastKind::Error, e.to_string()),
            }
            after_write(app, Some(pid));
        },
    );
}

fn remove(app: &mut App, id: &str, entry: &str) {
    if let Some(list) = app.library.local.entries.get_mut(id) {
        list.retain(|(e, _)| e != entry);
    }
    let pid = id.to_owned();
    app.be.call(
        op("playlist_remove", json!({ "id": id, "entry_id": entry })),
        move |app, r, _| {
            if let Err(e) = r {
                app.toast(ToastKind::Error, e.to_string());
            }
            after_write(app, Some(pid));
        },
    );
}

/// Moves entry `from` to index `to` (the engine takes the entry it should follow).
fn move_entry(app: &mut App, id: &str, from: usize, to: usize) {
    // Ops run concurrently; a second move before the first lands could apply out of order.
    if app.library.local.moving {
        return;
    }
    let Some(list) = app.library.local.entries.get_mut(id) else {
        return;
    };
    if from >= list.len() || to >= list.len() || from == to {
        return;
    }
    let entry = list.remove(from);
    list.insert(to, entry.clone());
    let after = to.checked_sub(1).map(|i| list[i].0.clone());
    app.library.local.moving = true;
    let pid = id.to_owned();
    app.be.call(
        op(
            "playlist_move",
            json!({ "id": id, "entry_id": entry.0, "after_entry_id": after }),
        ),
        move |app, r, _| {
            if let Err(e) = r {
                app.toast(ToastKind::Error, e.to_string());
            }
            refresh_entries(app, &pid);
        },
    );
}

fn rename(app: &mut App) {
    let Some(id) = app.library.local.renaming.take() else {
        return;
    };
    let title = app.library.local.rename.text.trim().to_owned();
    if title.is_empty() {
        return;
    }
    app.be.call(
        op("playlist_rename", json!({ "id": id, "title": title })),
        |app, r, _| {
            if let Err(e) = r {
                app.toast(ToastKind::Error, e.to_string());
            }
            after_write(app, None);
        },
    );
}

fn delete(app: &mut App, id: &str) {
    app.library.local.confirm_delete = None;
    let title = app
        .library
        .local
        .playlists
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.title.clone())
        .unwrap_or_default();
    app.be.call(
        op("playlist_delete", json!({ "id": id })),
        move |app, r, _| {
            match r {
                Ok(_) => {
                    app.toast(ToastKind::Success, format!("Deleted “{title}”"));
                    if matches!(app.route, Route::LocalPlaylist(_)) {
                        app.go_back();
                    }
                }
                Err(e) => app.toast(ToastKind::Error, e.to_string()),
            }
            after_write(app, None);
        },
    );
}

pub fn toggle_follow(app: &mut App, channel_id: &str, title: &str, thumbnail: &str) {
    let name = title.to_owned();
    app.be.call(
        op(
            "follow_toggle",
            json!({ "channel_id": channel_id, "title": title, "thumbnail": thumbnail }),
        ),
        move |app, r, _| {
            match r {
                Ok(v) => app.toast(
                    ToastKind::Success,
                    if v["following"].as_bool().unwrap_or(false) {
                        format!("Following {name}")
                    } else {
                        format!("Unfollowed {name}")
                    },
                ),
                Err(e) => app.toast(ToastKind::Error, e.to_string()),
            }
            refresh(app);
        },
    );
}

pub fn add_filter(app: &mut App, kind: &str, value: Option<String>) {
    let label = value.clone();
    app.be.call(
        op("filter_add", json!({ "kind": kind, "value": value })),
        move |app, r, _| {
            match r {
                Ok(_) => {
                    if let Some(v) = label {
                        app.toast(ToastKind::Success, format!("Hidden: {v}"));
                    }
                    // Feeds already loaded still show what is now hidden; reload them.
                    app.feeds.pages.clear();
                    if let Route::Feed(f) = app.route.clone() {
                        crate::feed::ensure(app, &f, true);
                    }
                }
                Err(e) => app.toast(ToastKind::Error, e.to_string()),
            }
            refresh_filters(app);
        },
    );
}

pub fn remove_filter(app: &mut App, id: &str) {
    app.library.local.filters.retain(|f| f.id != id);
    app.be
        .call(op("filter_remove", json!({ "id": id })), |app, r, _| {
            if let Err(e) = r {
                app.toast(ToastKind::Error, e.to_string());
            }
            app.feeds.pages.clear();
            refresh_filters(app);
        });
}

/// "Hide channel" from a card's menu: by channel id when known, else by name (as Android).
pub fn hide_channel(app: &mut App, m: &Media) {
    let value = Some(m.channel_id.clone())
        .filter(|c| !c.is_empty())
        .or_else(|| Some(m.creator.trim().to_owned()).filter(|c| !c.is_empty()));
    if value.is_some() {
        add_filter(app, "channel", value);
    }
}

/// Queues `media` ahead of everything else already queued (Android's "Play next").
pub fn play_next(app: &mut App, media: &Media) {
    let url = media.url.clone();
    app.be.call(
        op("enqueue", json!({ "media": media })),
        move |app, r, _| match r {
            Ok(v) => {
                let count = v["count"].as_u64().unwrap_or(0) as usize;
                if count > 1 {
                    app.be.call(
                        op(
                            "queue_move",
                            json!({ "from": count - 1, "to": 0, "url": url }),
                        ),
                        |app, _, _| app.player.refresh_up_next(&app.be),
                    );
                } else {
                    app.player.refresh_up_next(&app.be);
                }
                app.toast(ToastKind::Success, "Plays next");
            }
            Err(e) => app.toast(ToastKind::Error, e.to_string()),
        },
    );
}

// ---------------------------------------------------------------------------------------------
// Views

const CARD_W: f32 = 184.0;

fn cover(app: &App, m: Option<&Media>, side: f32, radius: f32) -> El {
    let t = &app.theme;
    let mut c = el()
        .w(side)
        .h(side)
        .radius(radius)
        .clip_rounded(radius)
        .fill(t.raised)
        .center();
    match m.and_then(|m| app.images.get(&thumb_url(m, Size::SQUARE), Size::SQUARE)) {
        Some(img) => {
            c = c.child(
                el().w_full()
                    .h_full()
                    .pixels_fit(img.pixels(), rune::SurfaceFit::Fill),
            )
        }
        None => c = c.child(icon(&ic::LIBRARY, (side / 5.0).clamp(16.0, 44.0)).color(t.faint)),
    }
    c
}

fn playlist_card(app: &App, p: &Playlist) -> El {
    let t = &app.theme;
    let id = p.id.clone();
    let mut meta = format!("{} item{}", p.count, if p.count == 1 { "" } else { "s" });
    if p.shared {
        meta.push_str(" · Shared");
    }
    col()
        .key(format!("lp-card-{}", p.id))
        .w(CARD_W)
        .gap(10.0)
        .role("listitem")
        .label(p.title.as_str())
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .press_scale(0.975)
        .on_press(move |s: &mut App| s.go(Route::LocalPlaylist(id.clone())))
        .child(cover(app, p.cover.as_ref(), CARD_W, sz::R_CARD).matched(format!("lp-{}", p.id)))
        .child(
            col()
                .gap(3.0)
                .child(text(&p.title).font(ty::TITLE).color(t.text).max_lines(2))
                .child(text(meta).font(ty::SMALL).color(t.muted)),
        )
}

fn create_card(app: &App) -> El {
    let t = &app.theme;
    let l = &app.library.local;
    let mut c = col().key("lp-create").w(CARD_W).gap(10.0);
    if l.creating {
        c = c
            .child(
                widgets::field(t, &l.new_name, "Playlist name", |s: &mut App| {
                    &mut s.library.local.new_name
                })
                .key("lp-create-name")
                .label("Playlist name")
                .on_submit(|s: &mut App, tx: &mut Tx| {
                    submit_create(s);
                    tx.blur();
                }),
            )
            .child(
                row()
                    .gap(8.0)
                    .child(icon_button(t, &ic::CHECK, "Create", 34.0).on_press(submit_create))
                    .child(
                        icon_button(t, &ic::CLOSE, "Cancel", 34.0)
                            .on_press(|s: &mut App| s.library.local.creating = false),
                    ),
            );
    } else {
        c = c.child(
            el().w(CARD_W)
                .h(CARD_W)
                .radius(sz::R_CARD)
                .border(1.0, t.border)
                .fill(t.raised)
                .hover(t.overlay)
                .center()
                .role("button")
                .label("New playlist")
                .help("New playlist")
                .focusable(true)
                .cursor(rune::Cursor::Pointer)
                .press_scale(0.975)
                .on_press_tx(|s: &mut App, tx: &mut Tx| {
                    s.library.local.creating = true;
                    s.library.local.new_name.clear();
                    tx.focus("lp-create-name", false);
                })
                .child(icon(&ic::PLUS, 30.0).color(t.text)),
        );
    }
    c
}

fn submit_create(app: &mut App) {
    let typed: String = app
        .library
        .local
        .new_name
        .text
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect();
    let name = if typed.trim().is_empty() {
        "New playlist".to_owned()
    } else {
        typed.trim().to_owned()
    };
    app.library.local.creating = false;
    app.library.local.new_name.clear();
    create(app, name, None);
}

/// Shortcuts to the account's YouTube lists, as Android's Library tiles.
fn account_tiles(app: &App) -> El {
    let t = &app.theme;
    let tiles: [(&str, &'static rune::Icon, BrowseRequest); 4] = [
        ("Liked", &ic::CHECK, BrowseRequest::Liked),
        ("Watch later", &ic::HISTORY, BrowseRequest::WatchLater),
        ("YouTube playlists", &ic::LIBRARY, BrowseRequest::Playlists),
        ("YouTube library", &ic::FOLDER, BrowseRequest::Library),
    ];
    let mut r = row().w_full().gap(12.0).flex_wrap();
    for (label, glyph, req) in tiles {
        r = r.child(
            row()
                .key(format!("lib-tile-{label}"))
                .h(56.0)
                .w(220.0)
                .px(16.0)
                .gap(14.0)
                .center_y()
                .radius(sz::R_MD)
                .fill(t.raised)
                .hover(t.overlay)
                .press_scale(0.97)
                .role("button")
                .label(label)
                .focusable(true)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| s.open_feed(Tab::Home, req.clone(), label))
                .child(icon(glyph, 18.0).color(t.text))
                .child(text(label).font(ty::BODY_STRONG).color(t.text)),
        );
    }
    r
}

fn following_row(app: &App) -> El {
    let t = &app.theme;
    let mut r = row().w_full().gap(20.0).flex_wrap();
    for f in &app.library.local.follows {
        let (id, title) = (f.channel_id.clone(), f.title.clone());
        let avatar = match app.images.get(&f.thumbnail, Size::AVATAR) {
            Some(img) => el()
                .size(72.0, 72.0)
                .radius(36.0)
                .clip_rounded(36.0)
                .pixels_fit(img.pixels(), rune::SurfaceFit::Fill),
            None => el()
                .size(72.0, 72.0)
                .radius(36.0)
                .fill(t.accent_soft)
                .center()
                .child(
                    text(
                        f.title
                            .chars()
                            .next()
                            .map(|c| c.to_uppercase().to_string())
                            .unwrap_or_default(),
                    )
                    .font(ty::H2)
                    .color(t.accent),
                ),
        };
        r = r.child(
            col()
                .key(format!("follow-{}", f.channel_id))
                .w(96.0)
                .gap(8.0)
                .items_center()
                .role("button")
                .label(f.title.as_str())
                .cursor(rune::Cursor::Pointer)
                .press_scale(0.95)
                .on_press(move |s: &mut App| {
                    s.open_feed(
                        Tab::Home,
                        BrowseRequest::Collection(id.clone()),
                        title.clone(),
                    )
                })
                .child(avatar)
                .child(
                    text(&f.title)
                        .font(ty::SMALL)
                        .color(t.text)
                        .max_lines(2)
                        .align(rune::Align::Center),
                ),
        );
    }
    r
}

fn heading(t: &crate::theme::Theme, title: &str) -> El {
    text(title).font(ty::H2).color(t.text).pt(10.0)
}

/// The top of the Playlists tab: account lists, this library's playlists and followed channels.
pub fn overview(app: &App) -> El {
    let t = &app.theme;
    let l = &app.library.local;
    let mut grid = row()
        .key("local-playlists")
        .w_full()
        .gap(20.0)
        .flex_wrap()
        .items_start()
        .child(create_card(app));
    for p in &l.playlists {
        grid = grid.child(playlist_card(app, p));
    }
    let mut c = col().w_full().gap(16.0);
    if app.account.connected {
        c = c.child(account_tiles(app));
    }
    c = c.child(heading(t, "Your playlists")).child(grid);
    if !l.follows.is_empty() {
        c = c.child(heading(t, "Following")).child(following_row(app));
    }
    c.child(heading(t, "Offline playlists"))
}

pub fn enter_playlist(app: &mut App, id: &str) {
    app.library.local.renaming = None;
    app.library.local.confirm_delete = None;
    refresh(app);
    refresh_entries(app, id);
}

/// A playlist's own page: cover, title, Play/Shuffle, and its entries.
pub fn playlist_view(app: &App, _cx: &mut Cx<App>, id: &str) -> El {
    let t = &app.theme;
    let l = &app.library.local;
    let Some(p) = l.playlists.iter().find(|p| p.id == id) else {
        return col()
            .w_full()
            .h_full()
            .pt(sz::TOPBAR + 24.0)
            .child(widgets::empty_state(
                t,
                &ic::LIBRARY,
                "Playlist not found",
                "It may have been deleted on another device.",
            ));
    };
    let entries = l.entries.get(id).cloned().unwrap_or_default();
    let items: Vec<Media> = entries.iter().map(|(_, m)| m.clone()).collect();

    let title: El = if l.renaming.as_deref() == Some(id) {
        row()
            .gap(8.0)
            .center_y()
            .child(
                widgets::field(t, &l.rename, "Playlist name", |s: &mut App| {
                    &mut s.library.local.rename
                })
                .key("lp-rename")
                .label("Playlist name")
                .w(360.0)
                .on_submit(|s: &mut App, tx: &mut Tx| {
                    rename(s);
                    tx.blur();
                }),
            )
            .child(icon_button(t, &ic::CHECK, "Save name", 34.0).on_press(rename))
            .child(
                icon_button(t, &ic::CLOSE, "Cancel rename", 34.0)
                    .on_press(|s: &mut App| s.library.local.renaming = None),
            )
    } else {
        text(&p.title).font(ty::DISPLAY).color(t.text).max_lines(2)
    };
    let mut meta = format!(
        "{} item{}",
        items.len(),
        if items.len() == 1 { "" } else { "s" }
    );
    if p.shared {
        meta.push_str(" · Shared");
    }
    let (i1, i2) = (items.clone(), items.clone());
    let mut actions = row().gap(10.0).pt(8.0).center_y().flex_wrap();
    if !items.is_empty() {
        actions = actions
            .child(
                crate::ui::browse::play_button(app).on_press(move |s: &mut App| {
                    crate::player::actions::play_all(s, Tab::Home, BrowseRequest::Home, &i1, false)
                }),
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
            );
    }
    if !p.member() {
        let (rid, rtitle, did) = (p.id.clone(), p.title.clone(), p.id.clone());
        let deletable = !p.shared;
        actions = actions.child(
            icon_button(t, &ic::SETTINGS, "Rename playlist", 38.0).on_press_tx(
                move |s: &mut App, tx: &mut Tx| {
                    s.library.local.renaming = Some(rid.clone());
                    s.library.local.rename.replace_text(&rtitle);
                    tx.focus("lp-rename", false);
                },
            ),
        );
        if !deletable {
            // The engine refuses to delete a shared playlist until it is unshared.
        } else if l.confirm_delete.as_deref() == Some(id) {
            actions = actions
                .child(
                    button(t, "Delete playlist", Some(&ic::TRASH), Kind::Danger)
                        .on_press(move |s: &mut App| delete(s, &did)),
                )
                .child(
                    button(t, "Keep", None, Kind::Ghost)
                        .on_press(|s: &mut App| s.library.local.confirm_delete = None),
                );
        } else {
            let id = p.id.clone();
            actions = actions.child(
                icon_button(t, &ic::TRASH, "Delete playlist", 38.0)
                    .on_press(move |s: &mut App| s.library.local.confirm_delete = Some(id.clone())),
            );
        }
    }
    let header = row()
        .w_full()
        .px(sz::PAGE_PAD)
        .pt(sz::TOPBAR + 20.0)
        .pb(24.0)
        .gap(28.0)
        .items_end()
        .child(
            cover(app, p.cover.as_ref(), 200.0, sz::R_PANEL)
                .matched(format!("lp-{}", p.id))
                .shadow(20.0, 50.0, t.shadow),
        )
        .child(
            col()
                .grow(1.0)
                .shrink()
                .gap(10.0)
                .child(title)
                .child(text(meta).font(ty::BODY).color(t.muted))
                .child(actions),
        );
    let mut list = col().w_full().px(sz::PAGE_PAD - 10.0).gap(2.0);
    let len = entries.len();
    for (i, (entry, m)) in entries.iter().enumerate() {
        let (pid, eid) = (id.to_owned(), entry.clone());
        let (up_id, down_id) = (id.to_owned(), id.to_owned());
        let mut controls = row().gap(2.0).center_y();
        if !p.member() || p.shared {
            if i > 0 {
                controls = controls.child(
                    icon_button(t, &ic::UP, "Move up", 30.0)
                        .on_press(move |s: &mut App| move_entry(s, &up_id, i, i - 1)),
                );
            }
            if i + 1 < len {
                controls = controls.child(
                    icon_button(t, &ic::DOWN, "Move down", 30.0)
                        .on_press(move |s: &mut App| move_entry(s, &down_id, i, i + 1)),
                );
            }
            controls = controls.child(
                icon_button(t, &ic::CLOSE, "Remove from playlist", 30.0)
                    .on_press(move |s: &mut App| remove(s, &pid, &eid)),
            );
        }
        let (media, sib) = (m.clone(), items.clone());
        list = list.child(
            widgets::media_row(
                app,
                m,
                &format!("lpe-{entry}"),
                Some(controls.group_opacity(0.0, 1.0)),
            )
            .cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| {
                crate::player::actions::open_item(s, Tab::Home, &media, &sib)
            }),
        );
    }
    if entries.is_empty() {
        list = list.child(
            text("Add videos and songs here from their ⋮ menu.")
                .font(ty::BODY)
                .color(t.muted)
                .px(10.0)
                .py(12.0),
        );
    }
    col()
        .key("lp-page")
        .w_full()
        .h_full()
        .scroll(rune::ScrollSpec {
            stick_bottom: false,
        })
        .on_scroll_offset(crate::ui::topbar::track(app))
        .child(
            col()
                .w_full()
                .pb(sz::DOCK_ROOM + 24.0)
                .child(header)
                .child(list),
        )
        .fill(Color::TRANSPARENT)
}

/// The Feed filters settings group: three switches and the hidden channels and keywords.
pub fn filters_settings(app: &App) -> El {
    let t = &app.theme;
    let l = &app.library.local;
    let mut c = col().w_full().gap(4.0);
    for (kind, label) in SWITCH_FILTERS {
        let existing = l.switched(kind).map(|f| f.id.clone());
        let on = existing.is_some();
        c = c.child(widgets::setting_row(
            t,
            label,
            "",
            widgets::switch(t, &format!("filter-{kind}"), on).on_press(move |s: &mut App| {
                match &existing {
                    Some(id) => remove_filter(s, id),
                    None => add_filter(s, kind, None),
                }
            }),
        ));
    }
    let field = widgets::field(t, &l.hide, "A keyword to hide", |s: &mut App| {
        &mut s.library.local.hide
    })
    .key("filter-keyword")
    .label("Keyword to hide")
    .grow(1.0)
    .on_submit(|s: &mut App, tx: &mut Tx| {
        submit_keyword(s);
        tx.blur();
    });
    c = c.child(
        row()
            .w_full()
            .pt(8.0)
            .gap(8.0)
            .center_y()
            .child(field)
            .child(icon_button(t, &ic::PLUS, "Hide keyword", 36.0).on_press(submit_keyword)),
    );
    for f in l
        .filters
        .iter()
        .filter(|f| matches!(f.kind.as_str(), "channel" | "keyword"))
    {
        let id = f.id.clone();
        let value = f.value.clone().unwrap_or_default();
        let shown = if f.kind == "channel" {
            l.follows
                .iter()
                .find(|x| x.channel_id == value)
                .map(|x| x.title.clone())
                .unwrap_or(value)
        } else {
            value
        };
        c = c.child(
            row()
                .key(format!("filter-{}", f.id))
                .w_full()
                .h(40.0)
                .px(8.0)
                .gap(12.0)
                .center_y()
                .radius(sz::R_SM)
                .hover(t.hover)
                .child(
                    icon(
                        if f.kind == "channel" {
                            &ic::USER
                        } else {
                            &ic::SEARCH
                        },
                        15.0,
                    )
                    .color(t.muted),
                )
                .child(
                    text(shown)
                        .font(ty::BODY)
                        .color(t.text)
                        .grow(1.0)
                        .truncate(),
                )
                .child(
                    icon_button(t, &ic::CLOSE, "Show again", 30.0)
                        .on_press(move |s: &mut App| remove_filter(s, &id)),
                ),
        );
    }
    c
}

fn submit_keyword(app: &mut App) {
    let v = app.library.local.hide.text.trim().to_owned();
    if v.is_empty() {
        return;
    }
    app.library.local.hide.clear();
    add_filter(app, "keyword", Some(v));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_shapes_parse() {
        let p: Vec<Playlist> = list(json!([{
            "id": "6a8f3c1e-0000-4000-8000-000000000001", "title": "Road trip", "count": 2,
            "shared": true, "role": "member", "cover": null
        }]));
        assert_eq!(p.len(), 1);
        assert!(p[0].member() && p[0].shared && p[0].count == 2);
        let f: Vec<FeedFilter> = list(
            json!([{"id": "x", "kind": "shorts"}, {"id": "y", "kind": "keyword", "value": "asmr"}]),
        );
        let l = Local {
            filters: f,
            ..Default::default()
        };
        assert!(l.switched("shorts").is_some() && l.switched("live").is_none());
        let follows: Vec<Follow> = list(json!([{"channel_id": "UC1", "title": "A"}]));
        assert_eq!(follows[0].thumbnail, "");
    }
}
