//! The now-playing dock: frosted, over the bottom of every page except Watch.

use rune::{Color, Cx, El, SurfaceFit, Transition, col, el, icon, row, spacer, text};

use crate::app::{App, LibraryTab, Route};
use crate::icons as ic;
use crate::images::{Size, thumb_url};
use crate::player::mpv::{BAR_DOCK, BAR_H};
use crate::player::session;
use crate::theme::{sz, ty};
use crate::ui::widgets::icon_button;

pub fn view(app: &App, cx: &mut Cx<App>) -> El {
    let t = &app.theme;
    let p = &app.player;
    let Some(now) = &p.now else { return el() };
    let w = cx.layout_size("dock").map(|s| s[0]).unwrap_or(1100.0);

    // Artwork, or the playing video itself, small.
    let video = !now.audio && p.has_video();
    let (aw, ah) = if video { (128.0, 72.0) } else { (60.0, 60.0) };
    let mut art = el()
        .key("dock-art")
        .label("Open the player")
        .w(aw)
        .h(ah)
        .radius(sz::R_SM)
        .clip_rounded(sz::R_SM)
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
                .child(icon(if now.audio { &ic::MUSIC } else { &ic::PLAY }, 22.0).color(t.faint));
        }
    }

    let mut words = col().gap(3.0).w(280.0).shrink();
    let title = p
        .snap
        .title
        .clone()
        .filter(|_| now.source == session::Source::Radio)
        .unwrap_or_else(|| now.media.title.clone());
    words = words.child(
        text(title)
            .font(ty::BODY_STRONG)
            .color(t.text)
            .truncate_fade(),
    );
    let sub = if now.source == session::Source::Radio {
        now.media.title.clone()
    } else {
        now.media.creator.clone()
    };
    words = words.child(text(sub).font(ty::SMALL).color(t.muted).truncate());

    let playing = !p.snap.paused && p.snap.loaded;
    let play = el()
        .key("dock-play")
        .size(44.0, 44.0)
        .radius(22.0)
        .center()
        .fill(t.text)
        .hover(t.text.mix(t.bg, 0.12))
        .press_scale(0.9)
        .role("button")
        .label(if playing { "Pause" } else { "Play" })
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut App| s.player.toggle_pause())
        .child(icon(if playing { &ic::PAUSE } else { &ic::PLAY }, 18.0).color(t.bg));
    let buttons = row()
        .gap(6.0)
        .center_y()
        .child(
            icon_button(t, &ic::PREV, "Previous", 38.0)
                .on_press(|s: &mut App| session::previous(s)),
        )
        .child(play)
        .child(
            icon_button(t, &ic::NEXT, "Next", 38.0).on_press(|s: &mut App| session::next(s, false)),
        );

    let bar_w = (w * 0.34).clamp(220.0, 520.0);
    p.lives.bars[BAR_DOCK].set_width(
        bar_w,
        p.snap.duration.map_or(0.0, |d| (p.snap.time / d) as f32),
    );
    let progress = if now.live {
        row()
            .gap(8.0)
            .center_y()
            .child(el().size(8.0, 8.0).radius(4.0).fill(t.error))
            .child(text("Live").font(ty::BADGE).color(t.error))
    } else {
        row()
            .gap(10.0)
            .center_y()
            .child(
                text("00:00:00")
                    .live_text(&p.lives.time)
                    .font(ty::NUM)
                    .color(t.muted)
                    .w(56.0)
                    .align(rune::Align::End),
            )
            .child(
                el().w(bar_w)
                    .h(BAR_H)
                    .radius(2.0)
                    .fill(t.overlay)
                    .cursor(rune::Cursor::Pointer)
                    .child(
                        el().absolute(0.0, 0.0)
                            .h(BAR_H)
                            .radius(2.0)
                            .fill(t.accent)
                            .live_rect(&p.lives.bars[BAR_DOCK].rect),
                    )
                    .on_drag(|s: &mut App, d: &mut rune::Drag, tx: &mut rune::Tx| {
                        tx.instant();
                        if d.phase == rune::Phase::Up
                            && let Some(dur) = s.player.snap.duration
                        {
                            s.player
                                .seek_to((d.x / d.w.max(1.0)).clamp(0.0, 1.0) as f64 * dur, true);
                        }
                    }),
            )
            .child(
                text(
                    p.snap
                        .duration
                        .map(crate::player::mpv::clock)
                        .unwrap_or_default(),
                )
                .font(ty::NUM)
                .color(t.muted)
                .w(46.0),
            )
    };
    let centre = col().gap(6.0).items_center().child(buttons).child(progress);

    let mut right = row().gap(4.0).center_y();
    if now.is_music() {
        right = right.child(
            icon_button(t, &ic::LYRICS, "Lyrics", 36.0).on_press(|s: &mut App| {
                s.edit_prefs(|p| p.show_lyrics = true);
                s.go(Route::Watch);
                crate::lyrics::load(s, false);
            }),
        );
    }
    right = right
        .child(
            icon_button(t, &ic::QUEUE, "Queue", 36.0)
                .on_press(|s: &mut App| s.go_root(Route::Library(LibraryTab::Queue))),
        )
        .child(icon(if p.snap.muted { &ic::MUTE } else { &ic::VOLUME }, 17.0).color(t.muted))
        .child(crate::ui::watch_controls::volume_slider(
            app, "vol-dock", 96.0,
        ))
        .child(
            icon_button(t, &ic::UP, "Open player", 36.0).on_press(|s: &mut App| s.go(Route::Watch)),
        )
        .child(icon_button(t, &ic::CLOSE, "Stop", 36.0).on_press(|s: &mut App| session::stop(s)));

    let body = row()
        .key("dock")
        .w_full()
        .h(sz::DOCK)
        .px(18.0)
        .gap(16.0)
        .center_y()
        .child(
            row()
                .gap(14.0)
                .center_y()
                .w(380.0)
                .shrink()
                .child(art)
                .child(words),
        )
        .child(spacer())
        .child(centre)
        .child(spacer())
        .child(right);
    el().key("dock-wrap")
        .pin(Some(12.0), None, Some(12.0), Some(12.0))
        .radius(20.0)
        .backdrop_blur(26.0)
        .fill(t.glass)
        .border(1.0, t.border)
        .shadow(16.0, 46.0, t.shadow)
        .transition(Transition::offset(0.0, 40.0).combined(Transition::opacity()))
        .child(body)
        .child(if now.error.is_some() {
            el().pin(Some(0.0), Some(0.0), Some(0.0), None)
                .h(2.0)
                .fill(t.error)
        } else {
            el()
        })
        .opacity(1.0)
        .fill(t.glass.mix(Color::TRANSPARENT, 0.0))
}
