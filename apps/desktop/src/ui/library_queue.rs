//! The Queue tab: what is playing, the manual queue (reorder, remove) and the context tail.

use rune::{El, col, el, row, text};
use serde_json::json;
use vidcue_core::Media;

use super::{move_target, moved, plural, presence, section_title, short_hash};
use crate::app::{App, ToastKind};
use crate::backend::op;
use crate::icons as ic;
use crate::player::session::play_next_item;
use crate::theme::{sz, ty};
use crate::ui::widgets::{self, icon_button};

pub(super) fn body(app: &App) -> El {
    let t = &app.theme;
    let queue = &app.player.queue;
    let context = app.player.context_items();
    let mut page = col().w_full();
    if app.player.now.is_none() && queue.is_empty() && context.is_empty() {
        return page.child(widgets::empty_state(
            t,
            &ic::QUEUE,
            "The queue is empty",
            "Add videos and songs from their ⋮ menu, or just start playing something.",
        ));
    }
    if let Some(now) = &app.player.now {
        page = page.child(section_title(t, "Now playing", None)).child(
            el().w_full()
                .px(sz::PAGE_PAD - 10.0)
                .child(widgets::media_row(app, &now.media, "q-now", None)),
        );
    }
    page = page.child(section_title(
        t,
        "Next in queue",
        Some(plural(queue.len(), "item")),
    ));
    if queue.is_empty() {
        page = page.child(
            el().px(sz::PAGE_PAD).py(6.0).child(
                text("Nothing queued. Use “Add to queue” on any item.")
                    .font(ty::BODY)
                    .color(t.muted),
            ),
        );
    }
    let mut seen: Vec<&str> = Vec::new();
    for (i, m) in queue.iter().enumerate() {
        let n = seen.iter().filter(|u| **u == m.url).count();
        seen.push(&m.url);
        page = page.child(queue_row(app, m, i, queue.len(), n));
    }
    if !context.is_empty() {
        let related = app.player.context["related"].as_bool().unwrap_or(false);
        page = page.child(section_title(
            t,
            if related {
                "Up next · related"
            } else {
                "Up next from context"
            },
            Some(plural(context.len(), "item")),
        ));
        for (i, m) in context.iter().enumerate() {
            let media = m.clone();
            let key = format!("ctx-{i}-{}", short_hash(&m.url));
            page = page.child(presence(
                el().key(format!("{key}-wrap"))
                    .w_full()
                    .px(sz::PAGE_PAD - 10.0)
                    .child(
                        widgets::media_row(app, m, &key, None)
                            .on_press(move |s: &mut App| play_next_item(s, media.clone(), false))
                            .cursor(rune::Cursor::Pointer),
                    ),
            ));
        }
    }
    page
}

fn queue_row(app: &App, m: &Media, i: usize, len: usize, dup: usize) -> El {
    let t = &app.theme;
    let url = m.url.clone();
    let key = format!("q-{}-{dup}", short_hash(&m.url));
    let slot = |e: Option<El>| e.unwrap_or_else(|| el().size(30.0, 30.0));
    let up = move_target(i, len, -1).map(|to| {
        let u = url.clone();
        icon_button(t, &ic::UP, "Move up", 30.0)
            .on_press(move |s: &mut App| move_item(s, i, to, &u))
    });
    let down = move_target(i, len, 1).map(|to| {
        let u = url.clone();
        icon_button(t, &ic::DOWN, "Move down", 30.0)
            .on_press(move |s: &mut App| move_item(s, i, to, &u))
    });
    let remove = {
        let u = url.clone();
        icon_button(t, &ic::CLOSE, "Remove from queue", 30.0)
            .on_press(move |s: &mut App| remove_item(s, i, &u))
    };
    let controls = row()
        .gap(2.0)
        .center_y()
        .child(slot(up))
        .child(slot(down))
        .child(remove);
    let media = m.clone();
    presence(
        el().key(format!("{key}-wrap"))
            .w_full()
            .px(sz::PAGE_PAD - 10.0)
            .child(
                widgets::media_row(app, m, &key, Some(controls))
                    .on_press(move |s: &mut App| play_next_item(s, media.clone(), true))
                    .cursor(rune::Cursor::Pointer),
            ),
    )
}

fn move_item(app: &mut App, from: usize, to: usize, url: &str) {
    if app.player.queue.get(from).is_none_or(|m| m.url != url) {
        return;
    }
    app.player.queue = moved(&app.player.queue, from, to);
    let url = url.to_owned();
    app.be.call(
        op("queue_move", json!({ "from": from, "to": to, "url": url })),
        |app, r, _| {
            if let Err(e) = r {
                app.toast(ToastKind::Error, e.to_string());
            }
            app.player.refresh_up_next(&app.be);
        },
    );
}

fn remove_item(app: &mut App, index: usize, url: &str) {
    if app.player.queue.get(index).is_none_or(|m| m.url != url) {
        return;
    }
    app.player.queue.remove(index);
    let url = url.to_owned();
    app.be.call(
        op("queue_remove", json!({ "index": index, "url": url })),
        |app, r, _| {
            if let Err(e) = r {
                app.toast(ToastKind::Error, e.to_string());
            }
            app.player.refresh_up_next(&app.be);
        },
    );
}

/// Empties the manual queue, last item first, one engine call at a time so each index and
/// URL check still holds when it runs.
pub(super) fn clear(app: &mut App) {
    let pending: Vec<(usize, String)> = app
        .player
        .queue
        .iter()
        .enumerate()
        .map(|(i, m)| (i, m.url.clone()))
        .collect();
    app.player.queue.clear();
    clear_step(app, pending);
}

fn clear_step(app: &mut App, mut pending: Vec<(usize, String)>) {
    let Some((index, url)) = pending.pop() else {
        app.toast(ToastKind::Success, "Queue cleared");
        app.player.refresh_up_next(&app.be);
        return;
    };
    app.be.call(
        op("queue_remove", json!({ "index": index, "url": url })),
        move |app, r, _| match r {
            Ok(_) => clear_step(app, pending),
            Err(e) => {
                app.toast(ToastKind::Error, e.to_string());
                app.player.refresh_up_next(&app.be);
            }
        },
    );
}
