//! The floating capsule player, as YouTube Music's and the Android mini player: centred over
//! the bottom of every page except Now Playing, frosted, with its progress running along its
//! lower edge. A press on its artwork or title opens Now Playing.

use rune::{Color, Cx, El, SurfaceFit, Transition, col, el, icon, row, spacer, text};

use crate::app::{App, LibraryTab, Route};
use crate::icons as ic;
use crate::images::{Size, thumb_url};
use crate::player::mpv::{BAR_DOCK, BAR_H};
use crate::player::session;
use crate::theme::{sz, ty};
use crate::ui::widgets::icon_button;

/// The capsule's width in a page `room_w` wide.
pub fn width(room_w: f32) -> f32 {
    (room_w - 48.0).clamp(420.0, 880.0)
}

pub fn view(app: &App, cx: &mut Cx<App>) -> El {
    let t = &app.theme;
    let p = &app.player;
    let Some(now) = &p.now else { return el() };
    let room = cx.layout_size("main").map(|s| s[0]).unwrap_or(1180.0);
    let w = width(room);
    let roomy = w >= 720.0;

    // Artwork, or the playing video itself, small.
    let video = !now.audio && p.has_video();
    let (aw, ah, ar) = if video {
        (78.0, 44.0, 12.0)
    } else {
        (44.0, 44.0, 22.0)
    };
    let mut art = el()
        .key("dock-art")
        .label("Open Now Playing")
        .w(aw)
        .h(ah)
        .radius(ar)
        .clip_rounded(ar)
        .fill(t.raised)
        .matched(format!("art-{}", now.media.url))
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut App| s.go(Route::Watch));
    if video {
        if let Some(m) = &p.mpv {
            let scale = cx.env().scale;
            m.set_target(aw * scale * 2.0, ah * scale * 2.0);
        }
        art = art.child(
            el().w_full()
                .h_full()
                .native_surface(&p.surface, SurfaceFit::Fill),
        );
    } else {
        let size = if now.audio { Size::SQUARE } else { Size::CARD };
        if let Some(img) = app.images.get(&thumb_url(&now.media, size), size) {
            let spin = !p.snap.paused
                && p.snap.loaded
                && now.media.kind == vidcue_core::MediaKind::Song
                && app.config.music_player_style.starts_with("vinyl");
            let pic = el()
                .w_full()
                .h_full()
                .pixels_fit(img.pixels(), SurfaceFit::Fill);
            art = art.child(if spin {
                pic.looping(rune::LoopSpec::rotation(6.0))
            } else {
                pic
            });
        } else {
            art = art
                .center()
                .child(icon(if now.audio { &ic::MUSIC } else { &ic::PLAY }, 18.0).color(t.faint));
        }
    }

    let title = p
        .snap
        .title
        .clone()
        .filter(|_| now.source == session::Source::Radio)
        .unwrap_or_else(|| now.media.title.clone());
    let sub = if now.source == session::Source::Radio {
        now.media.title.clone()
    } else {
        now.media.creator.clone()
    };
    let mut title_row = row().gap(8.0).center_y().child(
        text(title)
            .font(ty::BODY_STRONG)
            .color(if now.error.is_some() { t.error } else { t.text })
            .truncate_fade()
            .shrink(),
    );
    if now.live {
        title_row = title_row.child(crate::ui::widgets::badge(
            "Live",
            t.error.alpha(0.18),
            t.error,
        ));
    }
    let words = col()
        .key("dock-words")
        .gap(2.0)
        .w(if roomy { 240.0 } else { 160.0 })
        .shrink()
        .role("button")
        .label("Open Now Playing")
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut App| s.go(Route::Watch))
        .child(title_row)
        .child(text(sub).font(ty::SMALL).color(t.muted).truncate());

    let playing = !p.snap.paused && p.snap.loaded;
    let play = el()
        .key("dock-play")
        .size(40.0, 40.0)
        .radius(20.0)
        .center()
        .fill(t.text)
        .hover(t.text.mix(t.bg, 0.12))
        .press_scale(0.9)
        .role("button")
        .label(if playing { "Pause" } else { "Play" })
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut App| s.player.toggle_pause())
        .child(icon(if playing { &ic::PAUSE } else { &ic::PLAY }, 17.0).color(t.bg));
    let mut buttons = row().gap(6.0).center_y();
    if roomy {
        let shuffle = p.shuffle;
        buttons = buttons.child(
            icon_button(t, &ic::SHUFFLE, "Shuffle", 34.0)
                .key("dock-shuffle")
                .checked(shuffle)
                .on_press(move |s: &mut App| s.player.shuffle = !shuffle)
                .child(state_dot(t, shuffle)),
        );
    }
    buttons = buttons
        .child(
            icon_button(t, &ic::PREV, "Previous", 36.0)
                .key("dock-prev")
                .on_press(|s: &mut App| session::previous(s)),
        )
        .child(play)
        .child(
            icon_button(t, &ic::NEXT, "Next", 36.0)
                .key("dock-next")
                .on_press(|s: &mut App| session::next(s, false)),
        );
    if roomy {
        let (label, next, on) = match p.repeat {
            session::Repeat::Off => ("Repeat off", session::Repeat::All, false),
            session::Repeat::All => ("Repeat all", session::Repeat::One, true),
            session::Repeat::One => ("Repeat one", session::Repeat::Off, true),
        };
        buttons = buttons.child(
            icon_button(
                t,
                if p.repeat == session::Repeat::One {
                    &ic::REPEAT_ONE
                } else {
                    &ic::REPEAT
                },
                label,
                34.0,
            )
            .key("dock-repeat")
            .checked(on)
            .on_press(move |s: &mut App| s.player.repeat = next)
            .child(state_dot(t, on)),
        );
    }

    let mut right = row().gap(2.0).center_y();
    if now.is_music() {
        right = right.child(
            icon_button(t, &ic::LYRICS, "Lyrics", 34.0).on_press(|s: &mut App| {
                s.edit_prefs(|p| p.show_lyrics = true);
                s.go(Route::Watch);
                crate::lyrics::load(s, false);
            }),
        );
    }
    right = right.child(
        icon_button(t, &ic::QUEUE, "Queue", 34.0)
            .on_press(|s: &mut App| s.go_root(Route::Library(LibraryTab::Queue))),
    );
    if roomy {
        right = right
            .child(
                icon_button(
                    t,
                    if p.snap.muted { &ic::MUTE } else { &ic::VOLUME },
                    if p.snap.muted { "Unmute" } else { "Mute" },
                    34.0,
                )
                .on_press(|s: &mut App| s.player.toggle_mute()),
            )
            .child(crate::ui::watch_controls::volume_slider(
                app, "vol-dock", 72.0,
            ));
    }
    right = right
        .child(
            icon_button(t, &ic::UP, "Open Now Playing", 34.0)
                .key("dock-open")
                .on_press(|s: &mut App| s.go(Route::Watch)),
        )
        .child(
            icon_button(t, &ic::CLOSE, "Stop", 34.0)
                .key("dock-stop")
                .on_press(|s: &mut App| session::stop(s)),
        );

    let body = row()
        .key("dock")
        .w_full()
        .h(sz::DOCK)
        .pl(10.0)
        .pr(12.0)
        .gap(12.0)
        .center_y()
        .child(art)
        .child(words)
        .child(spacer())
        .child(buttons)
        .child(spacer())
        .child(right);

    // Progress along the lower edge, inside the capsule's curve. A taller strip takes drags.
    let bar_w = w - 64.0;
    p.lives.bars[BAR_DOCK].set_width(
        bar_w,
        p.snap.duration.map_or(0.0, |d| (p.snap.time / d) as f32),
    );
    let progress = if now.live || p.snap.duration.is_none() {
        el()
    } else {
        el().key("dock-seek")
            .pin(Some(32.0), None, Some(32.0), Some(0.0))
            .h(10.0)
            .cursor(rune::Cursor::Pointer)
            .label("Seek")
            .on_drag(|s: &mut App, d: &mut rune::Drag, tx: &mut rune::Tx| {
                tx.instant();
                if d.phase == rune::Phase::Up
                    && let Some(dur) = s.player.snap.duration
                {
                    s.player
                        .seek_to((d.x / d.w.max(1.0)).clamp(0.0, 1.0) as f64 * dur, true);
                }
            })
            .child(
                el().pin(Some(0.0), None, Some(0.0), Some(0.0))
                    .h(3.0)
                    .radius(1.5)
                    .clip_rounded(1.5)
                    .fill(t.text.alpha(0.12))
                    .child(
                        el().absolute(0.0, 0.0)
                            .h(BAR_H)
                            .fill(if now.error.is_some() {
                                t.error
                            } else {
                                t.accent
                            })
                            .live_rect(&p.lives.bars[BAR_DOCK].rect),
                    ),
            )
    };

    el().key("dock-wrap")
        .pin(Some(((room - w) / 2.0).max(12.0)), None, None, Some(16.0))
        .w(w)
        .h(sz::DOCK)
        .radius(sz::DOCK / 2.0)
        .clip_rounded(sz::DOCK / 2.0)
        .backdrop_blur(28.0)
        .fill(t.overlay.alpha(if t.dark { 0.78 } else { 0.86 }))
        .border(1.0, t.border)
        .shadow(14.0, 40.0, t.shadow)
        .transition(Transition::offset(0.0, 40.0).combined(Transition::opacity()))
        .child(body)
        .child(progress)
}

/// The small dot under a shuffle or repeat glyph that says it is on (pink is for state).
fn state_dot(t: &crate::theme::Theme, on: bool) -> El {
    el().pin(None, None, None, Some(3.0))
        .w_full()
        .items_center()
        .child(
            el().size(4.0, 4.0)
                .radius(2.0)
                .fill(if on { t.accent } else { Color::TRANSPARENT }),
        )
}
