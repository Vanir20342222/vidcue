//! The watch page: the stage (video, or the music stage for audio), details, and up next.

use rune::{
    Color, Cx, El, Link, ScrollSpec, Span, Transition, Tx, col, el, icon, row, spacer, text,
};
use vidcue_core::{Media, MediaKind};

use crate::app::{App, Route, Tab};
use crate::icons as ic;
use crate::images::Size;
use crate::player::session::{self, Source};
use crate::theme::{sz, ty};
use crate::ui::watch_controls;
use crate::ui::widgets::{self, Kind, button};

/// The side panel beside a video, as YouTube's up-next column.
const VIDEO_SIDE_W: f32 = 400.0;

/// What the Now Playing side panel shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Panel {
    UpNext,
    Lyrics,
    Chapters,
}

const PANEL_KEY: &str = "np-panel";

fn panel(app: &App, now: &session::Now) -> Panel {
    if now.is_music() && app.prefs.show_lyrics {
        return Panel::Lyrics;
    }
    match app.scratch.get(PANEL_KEY).map(String::as_str) {
        Some("chapters") if !now.media.chapters.is_empty() => Panel::Chapters,
        _ => Panel::UpNext,
    }
}

fn set_panel(s: &mut App, to: Panel) {
    let lyrics = to == Panel::Lyrics;
    if s.prefs.show_lyrics != lyrics {
        s.edit_prefs(|p| p.show_lyrics = lyrics);
    }
    if lyrics {
        crate::lyrics::load(s, false);
    }
    if to == Panel::Chapters {
        s.scratch.insert(PANEL_KEY.into(), "chapters".into());
    } else {
        s.scratch.remove(PANEL_KEY);
    }
}

pub fn view(app: &App, cx: &mut Cx<App>) -> El {
    let t = &app.theme;
    let p = &app.player;
    let Some(now) = &p.now else {
        return col()
            .w_full()
            .h_full()
            .pt(sz::TOPBAR)
            .center()
            .child(widgets::empty_state(
                t,
                &ic::PLAY,
                "Nothing is playing",
                "Pick a video, song or station to start.",
            ));
    };
    let [win_w, win_h] = cx.layout_size("watch-page").unwrap_or([1200.0, 800.0]);

    if p.fullscreen {
        let (w, h) = fit_16_9(win_w, win_h);
        let stage = if now.is_music() {
            crate::ui::music::stage(app, cx, win_w, win_h)
        } else {
            watch_controls::stage(app, cx, w, h)
        };
        return el()
            .key("watch-page")
            .w_full()
            .h_full()
            .fill(Color::BLACK)
            .center()
            .child(stage);
    }

    let music = now.is_music();
    // Wide windows: the stage on the left, a floating tabbed panel on the right (YouTube
    // Music's Now Playing, YouTube's up-next column). Narrow windows stack them.
    let split = win_w >= 1000.0 && !p.theater;
    let inset = sz::PANEL_INSET;
    let side_w = if music {
        (win_w * 0.36).clamp(340.0, 480.0)
    } else {
        VIDEO_SIDE_W
    };
    let left_w = if split {
        win_w - side_w - inset - sz::PAGE_PAD - 24.0
    } else {
        win_w - sz::PAGE_PAD * 2.0
    };
    let room_h = win_h - sz::TOPBAR - inset;

    let mut left = col().w(left_w).gap(18.0);
    if music {
        let stage_h = if split {
            (room_h - 72.0).max(360.0)
        } else {
            (left_w * 0.62).clamp(380.0, 620.0)
        };
        left = left
            .child(crate::ui::music::stage(app, cx, left_w, stage_h))
            .child(music_actions(app, now));
    } else {
        let max_stage_h = if p.theater {
            win_h - sz::TOPBAR - 40.0
        } else {
            (win_h - sz::TOPBAR - 160.0).max(320.0)
        };
        let (sw, sh) = fit_16_9(left_w, max_stage_h);
        left = left
            .child(
                el().w_full()
                    .items_center()
                    .child(watch_controls::stage(app, cx, sw, sh)),
            )
            .child(details(app, now));
    }

    let which = panel(app, now);
    if split {
        let body_h = room_h - inset - 56.0;
        let side = side_panel(app, cx, now, which, side_w, room_h - inset, body_h);
        let left_scroll = col()
            .key("watch-left")
            .grow(1.0)
            .min_w(0.0)
            .h_full()
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .on_scroll_offset(crate::ui::topbar::track(app))
            .child(
                col()
                    .w_full()
                    .pl(sz::PAGE_PAD)
                    .pt(sz::TOPBAR + 4.0)
                    .pb(if music { 12.0 } else { 80.0 })
                    .child(left),
            );
        return el().key("watch-page").w_full().h_full().child(
            row().w_full().h_full().gap(24.0).child(left_scroll).child(
                el().key("watch-side")
                    .pt(sz::TOPBAR)
                    .pr(inset)
                    .pb(inset)
                    .child(side),
            ),
        );
    }
    let side = side_panel(app, cx, now, which, left_w, 640.0, 560.0);
    col()
        .key("watch-page")
        .w_full()
        .h_full()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .on_scroll_offset(crate::ui::topbar::track(app))
        .child(
            col()
                .w_full()
                .px(sz::PAGE_PAD)
                .pt(sz::TOPBAR + 4.0)
                .pb(80.0)
                .gap(28.0)
                .child(left)
                .child(side),
        )
}

/// The floating Now Playing panel: tabs, then the chosen list.
fn side_panel(
    app: &App,
    cx: &mut Cx<App>,
    now: &session::Now,
    which: Panel,
    w: f32,
    h: f32,
    body_h: f32,
) -> El {
    let t = &app.theme;
    let mut tabs = row()
        .key("np-tabs")
        .w_full()
        .h(56.0)
        .px(20.0)
        .gap(22.0)
        .center_y()
        .role("tablist");
    let mut options = vec![(Panel::UpNext, "Up next")];
    if now.is_music() {
        options.push((Panel::Lyrics, "Lyrics"));
    }
    if !now.media.chapters.is_empty() {
        options.push((Panel::Chapters, "Chapters"));
    }
    for (p, label) in options {
        let on = p == which;
        tabs = tabs.child(
            col()
                .key(format!("np-tab-{label}"))
                .h_full()
                .justify_center()
                .role("tab")
                .label(label)
                .checked(on)
                .focusable(true)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| set_panel(s, p))
                .child(
                    text(label)
                        .font(ty::BODY_STRONG)
                        .color(if on { t.text } else { t.muted }),
                )
                .child(
                    el().pin(Some(0.0), None, Some(0.0), Some(0.0))
                        .h(2.0)
                        .radius(1.0)
                        .fill(if on { t.text } else { Color::TRANSPARENT }),
                ),
        );
    }
    let inner_w = w - 24.0;
    let body: El = match which {
        Panel::Lyrics => el().w_full().grow(1.0).min_h(0.0).px(16.0).pb(12.0).child(
            crate::ui::lyrics_panel::view(app, cx, inner_w - 8.0, body_h - 12.0),
        ),
        Panel::Chapters => scroll_body(chapters(app, &now.media, inner_w)),
        Panel::UpNext => scroll_body(up_next(app, inner_w)),
    };
    col()
        .key("np-panel")
        .w(w)
        .h(h)
        .radius(sz::R_SIDEBAR)
        .clip_rounded(sz::R_SIDEBAR)
        .fill(t.panel)
        .border(1.0, t.hairline)
        .child(tabs)
        .child(el().h(1.0).w_full().fill(t.hairline))
        .child(body)
}

fn scroll_body(content: El) -> El {
    col()
        .key("np-scroll")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .scroll_edge_fade(18.0)
        .child(el().w_full().px(12.0).pt(10.0).pb(16.0).child(content))
}

/// Save, download, queue and share for a song or station, as icons under the stage.
fn music_actions(app: &App, now: &session::Now) -> El {
    let t = &app.theme;
    let m = &now.media;
    let saved = app.saved_urls.contains(&m.url);
    let (save_m, q_m, dl_m, link) = (m.clone(), m.clone(), m.clone(), m.url.clone());
    let mut r = row().w_full().gap(6.0).justify_center().center_y();
    let mut save = widgets::icon_button(
        t,
        if saved {
            &ic::BOOKMARK_FILLED
        } else {
            &ic::BOOKMARK
        },
        if saved { "Saved" } else { "Save" },
        40.0,
    )
    .key("watch-save")
    .on_press(move |s: &mut App| s.toggle_saved(&save_m));
    if let Some(t0) = crate::fx::burst_at(app, &format!("save-{}", m.url)) {
        save = save.child(
            crate::fx::sparkles(t0, 22)
                .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .radius(sz::R_PILL),
        );
    }
    r = r.child(save);
    if now.source != Source::Radio && !now.live && !matches!(now.source, Source::Movie(_)) {
        r = r.child(
            widgets::icon_button(t, &ic::DOWNLOAD, "Download", 40.0).on_press(
                move |s: &mut App| {
                    crate::downloads::add(s, &dl_m, s.player.now.as_ref().is_some_and(|n| n.audio))
                },
            ),
        );
    }
    r = r
        .child(
            widgets::icon_button(t, &ic::QUEUE, "Add to queue", 40.0)
                .on_press(move |s: &mut App| s.enqueue(&q_m)),
        )
        .child(
            widgets::icon_button(t, &ic::LINK, "Copy link", 40.0).on_press_tx(
                move |s: &mut App, tx: &mut Tx| {
                    tx.copy(link.clone());
                    s.toast(crate::app::ToastKind::Success, "Link copied");
                },
            ),
        );
    if !m.channel_id.is_empty() {
        let media = m.clone();
        r = r.child(
            widgets::icon_button(t, &ic::USER, "Go to artist", 40.0)
                .on_press(move |s: &mut App| s.open_channel(Tab::Music, &media)),
        );
    }
    r
}

fn chapters(app: &App, m: &Media, w: f32) -> El {
    let t = &app.theme;
    let mut ch = col().w(w).gap(2.0);
    let now_t = app.player.snap.time;
    let current = m.chapters.iter().rposition(|c| c.start <= now_t);
    for (i, chap) in m.chapters.iter().enumerate().take(200) {
        let at = chap.start;
        let on = current == Some(i);
        ch = ch.child(
            row()
                .key(format!("chap-{i}"))
                .label(format!("Chapter {}: {}", i + 1, chap.title))
                .w_full()
                .h(44.0)
                .px(10.0)
                .gap(12.0)
                .center_y()
                .radius(sz::R_MD)
                .fill(if on { t.hover } else { Color::TRANSPARENT })
                .hover(t.hover)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| s.player.seek_to(at, true))
                .child(
                    text(crate::player::mpv::clock(at))
                        .font(ty::NUM)
                        .color(if on { t.accent } else { t.muted })
                        .w(56.0),
                )
                .child(
                    text(&chap.title)
                        .font(if on { ty::BODY_STRONG } else { ty::BODY })
                        .color(t.text)
                        .truncate(),
                ),
        );
    }
    ch
}

fn fit_16_9(w: f32, h: f32) -> (f32, f32) {
    let (mut sw, mut sh) = (w, w * 9.0 / 16.0);
    if sh > h {
        sh = h;
        sw = h * 16.0 / 9.0;
    }
    (sw.floor(), sh.floor())
}

fn details(app: &App, now: &session::Now) -> El {
    let t = &app.theme;
    let m = &now.media;
    let mut c = col().gap(14.0).w_full();
    c = c.child(
        text(&m.title)
            .font(ty::H1)
            .color(t.text)
            .max_lines(3)
            .selectable(true),
    );

    // Channel and actions.
    let mut channel = row().gap(12.0).center_y();
    let avatar_url = m.channel_thumbnail.clone();
    let avatar = if let Some(img) = app.images.get(&avatar_url, Size::AVATAR) {
        el().size(42.0, 42.0)
            .radius(21.0)
            .clip_rounded(21.0)
            .pixels_fit(img.pixels(), rune::SurfaceFit::Fill)
    } else {
        el().size(42.0, 42.0)
            .radius(21.0)
            .fill(t.accent_soft)
            .center()
            .child(
                text(
                    m.creator
                        .chars()
                        .next()
                        .map(|c| c.to_uppercase().to_string())
                        .unwrap_or_default(),
                )
                .font(ty::TITLE)
                .color(t.accent),
            )
    };
    let subs = now.details["subscribers_text"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| {
            m.channel_follower_count
                .map(|n| format!("{} subscribers", widgets::count(n)))
        });
    let mut who = col().gap(2.0).child(
        text(if m.creator.is_empty() {
            "Unknown creator"
        } else {
            &m.creator
        })
        .font(ty::BODY_STRONG)
        .color(t.text),
    );
    if let Some(s) = subs {
        who = who.child(text(s).font(ty::SMALL).color(t.muted));
    }
    channel = channel.child(avatar).child(who);
    if !m.channel_id.is_empty() {
        let media = m.clone();
        channel = channel.child(button(t, "Channel", None, Kind::Secondary).on_press(
            move |s: &mut App| {
                let tab = if media.kind == MediaKind::Song {
                    Tab::Music
                } else {
                    Tab::Home
                };
                s.open_channel(tab, &media)
            },
        ));
    }

    let saved = app.saved_urls.contains(&m.url);
    let (save_m, q_m, dl_m, link) = (m.clone(), m.clone(), m.clone(), m.url.clone());
    let save_key = format!("save-{}", m.url);
    let mut save = button(
        t,
        if saved { "Saved" } else { "Save" },
        Some(if saved {
            &ic::BOOKMARK_FILLED
        } else {
            &ic::BOOKMARK
        }),
        Kind::Secondary,
    )
    .key("watch-save")
    .on_press(move |s: &mut App| s.toggle_saved(&save_m));
    if let Some(t0) = crate::fx::burst_at(app, &save_key) {
        save = save.child(
            crate::fx::sparkles(t0, 22)
                .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .radius(sz::R_PILL),
        );
    }
    let mut actions = row().gap(8.0).center_y().child(save);
    if now.source != Source::Radio && !now.live && !matches!(now.source, Source::Movie(_)) {
        actions = actions.child(
            button(t, "Download", Some(&ic::DOWNLOAD), Kind::Secondary).on_press(
                move |s: &mut App| {
                    crate::downloads::add(s, &dl_m, s.player.now.as_ref().is_some_and(|n| n.audio))
                },
            ),
        );
    }
    actions = actions.child(
        button(t, "Queue", Some(&ic::QUEUE), Kind::Ghost)
            .on_press(move |s: &mut App| s.enqueue(&q_m)),
    );
    if now.is_music() {
        actions = actions.child(
            button(
                t,
                "Lyrics",
                Some(&ic::LYRICS),
                if app.prefs.show_lyrics {
                    Kind::Secondary
                } else {
                    Kind::Ghost
                },
            )
            .on_press(|s: &mut App| {
                s.edit_prefs(|p| p.show_lyrics = !p.show_lyrics);
                crate::lyrics::load(s, false);
            }),
        );
    }
    actions = actions.child(
        widgets::icon_button(t, &ic::LINK, "Copy link", 38.0).on_press_tx(
            move |s: &mut App, tx: &mut Tx| {
                tx.copy(link.clone());
                s.toast(crate::app::ToastKind::Success, "Link copied");
            },
        ),
    );
    c = c.child(
        row()
            .w_full()
            .gap(16.0)
            .center_y()
            .flex_wrap()
            .child(channel)
            .child(spacer())
            .child(actions),
    );

    // Description card.
    let mut stats: Vec<String> = Vec::new();
    if !m.views_text.is_empty() {
        stats.push(m.views_text.clone());
    } else if let Some(v) = m.view_count {
        stats.push(format!("{} views", widgets::count(v)));
    }
    if let Some(p) = now.details["published_text"]
        .as_str()
        .or(Some(m.published_text.as_str()).filter(|s| !s.is_empty()))
    {
        stats.push(p.to_owned());
    }
    if let Some(l) = now.details["like_count"].as_u64().or(m.like_count) {
        stats.push(format!("{} likes", widgets::count(l)));
    }
    let mut card = col()
        .w_full()
        .padding(16.0)
        .gap(10.0)
        .radius(sz::R_PANEL)
        .fill(t.surface);
    if !stats.is_empty() {
        card = card.child(text(stats.join(" · ")).font(ty::BODY_STRONG).color(t.text));
    }
    if !now.provenance.is_empty() {
        for line in &now.provenance {
            card = card.child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(icon(&ic::INFO, 14.0).color(t.muted))
                    .child(text(line).font(ty::SMALL).color(t.muted)),
            );
        }
    }
    let open = app.scratch.contains_key("desc-open");
    if !m.description.is_empty() {
        card = card.child(description(app, &m.description, open));
        card = card.child(
            text(if open { "Show less" } else { "Show more" })
                .font(ty::SMALL_STRONG)
                .color(t.text)
                .cursor(rune::Cursor::Pointer)
                .role("button")
                .on_press(|s: &mut App| {
                    if s.scratch.remove("desc-open").is_none() {
                        s.scratch.insert("desc-open".into(), "1".into());
                    }
                }),
        );
    }
    if m.description.is_empty() && stats.is_empty() && now.provenance.is_empty() {
        return c;
    }
    c.child(card.transition(Transition::opacity()))
}

/// Description text: URLs open in the browser, timestamps seek.
fn description(app: &App, raw: &str, open: bool) -> El {
    let t = &app.theme;
    let mut spans: Vec<Span> = Vec::new();
    let mut links: Vec<Link> = Vec::new();
    let text_ = raw
        .chars()
        .take(if open { 8000 } else { 360 })
        .collect::<String>();
    let mut offset = 0usize;
    for word in text_.split_inclusive(|c: char| c.is_whitespace()) {
        let w = word.trim_end();
        let start = offset;
        offset += word.len();
        if w.starts_with("http://") || w.starts_with("https://") {
            spans.push(Span::new(start, start + w.len()).color(t.accent));
            links.push(Link::new(start..start + w.len(), w));
        } else if is_timestamp(w) {
            spans.push(Span::new(start, start + w.len()).color(t.accent));
            links.push(Link::new(
                start..start + w.len(),
                format!("vidcue-seek:{}", parse_timestamp(w)),
            ));
        }
    }
    let shown = if !open && raw.chars().count() > 360 {
        format!("{text_}…")
    } else {
        text_
    };
    let mut el_ = rune::rich(shown, spans);
    for l in links {
        el_ = el_.link(l);
    }
    el_.font(ty::BODY)
        .color(t.muted)
        .wrap()
        .selectable(true)
        .on_link(|s: &mut App, url: &str, tx: &mut Tx| {
            if let Some(sec) = url.strip_prefix("vidcue-seek:") {
                if let Ok(sec) = sec.parse::<f64>() {
                    s.player.seek_to(sec, true);
                }
            } else {
                let _ = tx.open_url(url);
            }
        })
}

fn is_timestamp(w: &str) -> bool {
    let parts: Vec<&str> = w.split(':').collect();
    (parts.len() == 2 || parts.len() == 3)
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 2 && p.chars().all(|c| c.is_ascii_digit()))
}

fn parse_timestamp(w: &str) -> f64 {
    w.split(':')
        .fold(0.0, |acc, p| acc * 60.0 + p.parse::<f64>().unwrap_or(0.0))
}

fn up_next(app: &App, w: f32) -> El {
    let t = &app.theme;
    let p = &app.player;
    let mut c = col().key("up-next").w(w).gap(4.0);
    let autoplay = app.config.autoplay_related;
    c = c.child(
        row()
            .w_full()
            .pb(8.0)
            .gap(10.0)
            .center_y()
            .child(text("Autoplay").font(ty::SMALL).color(t.muted))
            .child(spacer())
            .child(
                widgets::switch(t, "autoplay", autoplay)
                    .on_press(move |s: &mut App| s.edit_config(|c| c.autoplay_related = !autoplay)),
            ),
    );
    let mut controls = row().gap(6.0).pb(6.0);
    let shuffle = p.shuffle;
    controls = controls.child(
        widgets::chip(t, "Shuffle", shuffle)
            .on_press(move |s: &mut App| s.player.shuffle = !shuffle),
    );
    let (label, next) = match p.repeat {
        session::Repeat::Off => ("Repeat off", session::Repeat::All),
        session::Repeat::All => ("Repeat all", session::Repeat::One),
        session::Repeat::One => ("Repeat one", session::Repeat::Off),
    };
    controls = controls.child(
        widgets::chip(t, label, p.repeat != session::Repeat::Off)
            .on_press(move |s: &mut App| s.player.repeat = next),
    );
    c = c.child(controls);
    if !p.queue.is_empty() {
        c = c.child(
            text(format!("Your queue · {}", p.queue.len()))
                .font(ty::LABEL)
                .color(t.muted)
                .py(6.0),
        );
        for (i, m) in p.queue.iter().enumerate().take(30) {
            c = c.child(next_row(app, m, &format!("q-{i}"), true));
        }
    }
    let ctx = p.context_items();
    if !ctx.is_empty() {
        let label = if p.context["related"].as_bool().unwrap_or(false) {
            "Related"
        } else {
            "From this list"
        };
        c = c.child(text(label).font(ty::LABEL).color(t.muted).py(6.0));
        for (i, m) in ctx.iter().enumerate().take(40) {
            c = c.child(next_row(app, m, &format!("ctx-{i}"), false));
        }
    } else if p.queue.is_empty() {
        c = c.child(
            text(if autoplay {
                "Related videos will appear here."
            } else {
                "Turn on Autoplay, or add items to your queue."
            })
            .font(ty::SMALL)
            .color(t.muted)
            .py(10.0),
        );
    }
    c.child(
        button(t, "Open queue", Some(&ic::QUEUE), Kind::Ghost)
            .on_press(|s: &mut App| s.go_root(Route::Library(crate::app::LibraryTab::Queue))),
    )
}

fn next_row(app: &App, m: &Media, key: &str, queued: bool) -> El {
    let media = m.clone();
    let row_ = widgets::media_row(app, m, key, None).cursor(rune::Cursor::Pointer);
    row_.on_press(move |s: &mut App| session::play_next_item(s, media.clone(), queued))
        .transition(Transition::offset(12.0, 0.0).combined(Transition::opacity()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_in_descriptions_seek() {
        assert!(is_timestamp("1:23"));
        assert!(is_timestamp("01:02:03"));
        assert!(!is_timestamp("12:3456"));
        assert!(!is_timestamp("hello"));
        assert_eq!(parse_timestamp("1:02:03"), 3723.0);
    }

    #[test]
    fn stage_keeps_16_by_9_inside_the_room() {
        assert_eq!(fit_16_9(1600.0, 2000.0), (1600.0, 900.0));
        let (w, h) = fit_16_9(1600.0, 450.0);
        assert!((w / h - 16.0 / 9.0).abs() < 0.01 && h <= 450.0);
    }
}
