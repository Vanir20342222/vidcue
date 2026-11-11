//! The music stage: what the Watch page shows instead of video for songs, radio stations and
//! "play as audio". Four styles (classic, vinyl, VU, orbit, each with a full-bleed variant)
//! over a blurred-artwork backdrop, with the same transport under all of them.

use rune::{
    Color, Cx, Drag, El, Phase, SurfaceFit, TextStyle, Tx, col, el, icon, row, spacer, text,
};

use crate::app::App;
use crate::icons as ic;
use crate::images::{Size, thumb_url};
use crate::player::mpv::{BAR_H, BAR_MUSIC};
use crate::player::session::{self, Now, Repeat, Source};
use crate::theme::ty;
use crate::ui::widgets::{self, Kind};

#[path = "music_visuals.rs"]
mod visuals;
use visuals::{Style, parse_style, visual_width};

const RADIUS: f32 = 18.0;

pub fn stage(app: &App, cx: &mut Cx<App>, w: f32, h: f32) -> El {
    let _ = cx;
    let p = &app.player;
    let Some(now) = &p.now else { return el() };
    let (style, full_cfg) = parse_style(&app.config.music_player_style);
    let full = full_cfg || p.fullscreen;
    let radius = if full { 0.0 } else { RADIUS };
    let playing = p.playing() && now.error.is_none() && !now.resolving;
    let fill_all = |e: El| e.pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0));

    let mut root = el()
        .key("music-stage")
        .w(w)
        .h(h)
        .radius(radius)
        .clip_rounded(radius)
        .fill(Color::BLACK)
        .hover_group();
    root = root.child(fill_all(backdrop(app, now, w, h)));

    // Layout: the visual beside the controls on wide stages, above them on tall ones.
    let pad = if full { 44.0 } else { 28.0 };
    let top = 64.0;
    let side_by_side = w >= h * 1.25;
    let (s, info_w) = if side_by_side {
        let s = (h - top - pad).min(w * 0.4).max(140.0);
        (
            s,
            (w - visual_width(style, s) - pad * 3.0).clamp(260.0, 520.0),
        )
    } else {
        let s = (w - pad * 2.0).min((h - top) * 0.42).max(140.0);
        (s, (w - pad * 2.0).clamp(260.0, 560.0))
    };
    let visual = match style {
        Style::Classic => visuals::classic(app, now, s),
        Style::Vinyl => visuals::vinyl(app, now, s, playing),
        Style::Vu => visuals::vu(app, now, s, app.theme.dot_matrix),
        Style::Orbit => visuals::orbit(app, now, s, playing),
    };
    let visual = el().key("music-visual").child(visual);
    let info = info_block(app, now, info_w, full);
    let content = if side_by_side {
        row().center_y().gap(pad * 1.6).child(visual).child(info)
    } else {
        col()
            .items_center()
            .gap(pad * 0.8)
            .child(visual)
            .child(info)
    };
    root = root.child(
        el().pin(Some(0.0), Some(top * 0.6), Some(0.0), Some(0.0))
            .center()
            .child(content),
    );

    // Style switcher and full screen.
    let mut chips = row().gap(6.0).center_y();
    for st in Style::ALL {
        let v = st.config_value(full_cfg);
        chips = chips.child(
            widgets::chip(&app.theme, st.label(), st == style)
                .role("button")
                .label(format!("{} player", st.label()))
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| {
                    let v = v.clone();
                    s.edit_config(|c| c.music_player_style = v);
                }),
        );
    }
    if style != Style::Classic {
        let next = style.config_value(!full_cfg);
        chips = chips.child(
            widgets::chip(&app.theme, "Full bleed", full_cfg)
                .role("button")
                .label("Full bleed")
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| {
                    let v = next.clone();
                    s.edit_config(|c| c.music_player_style = v);
                }),
        );
    }
    let bar = el()
        .key("music-styles")
        .pin(Some(0.0), Some(14.0), Some(0.0), None)
        .items_center()
        .group_opacity(if p.fullscreen { 0.0 } else { 1.0 }, 1.0)
        .child(
            el().px(8.0)
                .py(6.0)
                .radius(999.0)
                .fill(Color::BLACK.alpha(0.35))
                .child(chips),
        );
    root = root.child(bar);
    root = root.child(
        el().key("music-full")
            .pin(None, Some(14.0), Some(14.0), None)
            .group_opacity(if p.fullscreen { 0.0 } else { 1.0 }, 1.0)
            .child(
                widgets::overlay_button(
                    if p.fullscreen {
                        &ic::FULLSCREEN_EXIT
                    } else {
                        &ic::FULLSCREEN
                    },
                    "Full screen (f)",
                    38.0,
                )
                .on_press_tx(|s: &mut App, tx: &mut Tx| {
                    s.player.fullscreen = !s.player.fullscreen;
                    tx.toggle_full_screen();
                }),
            ),
    );
    el().key("music-stage-wrap").w(w).h(h).child(root.shadow(
        if full { 0.0 } else { 18.0 },
        50.0,
        Color::BLACK.alpha(0.45),
    ))
}

/// Huge blurred artwork, palette mesh and grain, under a scrim that keeps white text legible.
fn backdrop(app: &App, now: &Now, w: f32, h: f32) -> El {
    let mut b = el()
        .w(w)
        .h(h)
        .fill(Color::BLACK.mix(app.theme.accent, 0.12));
    let fill_all = |e: El| e.pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0));
    if let Some(img) = app
        .images
        .get(&thumb_url(&now.media, Size::SQUARE), Size::SQUARE)
    {
        b = b.child(
            el().pin(
                Some(-w * 0.2),
                Some(-h * 0.2),
                Some(-w * 0.2),
                Some(-h * 0.2),
            )
            .pixels_fit(img.pixels(), SurfaceFit::Fill)
            .blur(60.0)
            .opacity(0.75)
            .inert(),
        );
    }
    if app.prefs.effects {
        b = b.child(fill_all(crate::fx::mesh(7, 0.12)).opacity(0.28).inert());
    }
    b = b.child(
        fill_all(el())
            .gradient(Color::BLACK.alpha(0.25), Color::BLACK.alpha(0.68))
            .inert(),
    );
    if app.prefs.effects {
        b = b.child(fill_all(crate::fx::grain(0.05)).inert());
    }
    b
}

/// Title, status, seek bar, transport and volume.
fn info_block(app: &App, now: &Now, w: f32, full: bool) -> El {
    let t = &app.theme;
    let p = &app.player;
    let radio = matches!(now.source, Source::Radio);
    let live = radio || now.live;
    let white = Color::WHITE;
    let title_font = if full { ty::DISPLAY } else { ty::H1 };
    let subtitle = if radio {
        p.snap
            .title
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| now.media.creator.clone())
    } else {
        now.media.creator.clone()
    };
    let mut c = col().key("music-info").w(w).gap(16.0);
    let mut head = col().gap(4.0).child(
        text(&now.media.title)
            .font(title_font)
            .color(white)
            .max_lines(2)
            .selectable(true),
    );
    if !subtitle.is_empty() {
        head = head.child(
            text(subtitle)
                .font(ty::H3)
                .color(white.alpha(0.72))
                .max_lines(1),
        );
    }
    if live {
        head = head.child(
            row()
                .gap(8.0)
                .center_y()
                .pt(4.0)
                .child(
                    el().size(8.0, 8.0).radius(4.0).fill(t.error).child(
                        crate::fx::pulse(0, 4, 1.6, 10.0)
                            .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                            .radius(4.0),
                    ),
                )
                .child(
                    text("LIVE")
                        .font(TextStyle::new(12.0, 760.0).tracking(0.8))
                        .color(white),
                ),
        );
    }
    c = c.child(head);

    if let Some(err) = &now.error {
        return c.child(
            col()
                .gap(10.0)
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(icon(&ic::WARNING, 18.0).color(t.error))
                        .child(
                            text("This can't play right now")
                                .font(ty::BODY_STRONG)
                                .color(white),
                        ),
                )
                .child(
                    text(err)
                        .font(ty::SMALL)
                        .color(white.alpha(0.75))
                        .wrap()
                        .max_lines(4),
                )
                .child(
                    widgets::button(t, "Try again", Some(&ic::REFRESH), Kind::Glass).on_press_tx(
                        |s: &mut App, tx: &mut Tx| {
                            if let Some(n) = &mut s.player.now {
                                n.error = None;
                                n.renewed = false;
                            }
                            session::retry(s, tx)
                        },
                    ),
                ),
        );
    }
    let busy = now.resolving || (!p.snap.loaded && p.unavailable.is_none()) || p.snap.buffering;
    if busy {
        let mut status = row()
            .gap(12.0)
            .center_y()
            .child(super::watch_controls::spinner(t.accent));
        if !now.resolving {
            status = status.child(
                text("Buffering…")
                    .font(ty::SMALL_STRONG)
                    .color(white.alpha(0.85)),
            );
        }
        c = c.child(status);
    } else if let Some(u) = p.unavailable.as_ref().filter(|_| !app.offline()) {
        c = c.child(text(u).font(ty::SMALL).color(white.alpha(0.75)).wrap());
    }
    if !live {
        c = c.child(seek_bar(app, w));
    }

    // Transport.
    let flag = |glyph: &'static rune::Icon, label: &str, on: bool| {
        widgets::overlay_button(glyph, label, 40.0)
            .role("switch")
            .opacity(if on { 1.0 } else { 0.55 })
            .fill(if on {
                white.alpha(0.16)
            } else {
                Color::TRANSPARENT
            })
    };
    let (rglyph, rlabel) = match p.repeat {
        Repeat::One => (&ic::REPEAT_ONE, "Repeat one"),
        Repeat::All => (&ic::REPEAT, "Repeat all"),
        Repeat::Off => (&ic::REPEAT, "Repeat off"),
    };
    let play = el()
        .key("music-play")
        .size(68.0, 68.0)
        .radius(34.0)
        .center()
        .fill(white)
        .press_scale(0.92)
        .hover(Color::WHITE.mix(t.accent, 0.12))
        .cursor(rune::Cursor::Pointer)
        .role("button")
        .label(if p.snap.paused {
            "Play (k)"
        } else {
            "Pause (k)"
        })
        .shadow(6.0, 18.0, rune::Color::BLACK.alpha(0.35))
        .child(
            icon(
                if p.snap.paused || p.snap.eof {
                    &ic::PLAY
                } else {
                    &ic::PAUSE
                },
                30.0,
            )
            .color(Color::BLACK),
        )
        .on_press(|s: &mut App| s.player.toggle_pause());
    let mut transport = row().gap(14.0).center_y().w_full();
    if !live {
        transport = transport.child(
            flag(&ic::SHUFFLE, "Shuffle", p.shuffle)
                .on_press(|s: &mut App| s.player.shuffle = !s.player.shuffle),
        );
    }
    transport = transport
        .child(spacer())
        .child(
            widgets::overlay_button(&ic::PREV, "Previous (p)", 46.0)
                .on_press(|s: &mut App| session::previous(s)),
        )
        .child(play)
        .child(
            widgets::overlay_button(&ic::NEXT, "Next (n)", 46.0)
                .on_press(|s: &mut App| session::next(s, false)),
        )
        .child(spacer());
    if !live {
        transport = transport.child(flag(rglyph, rlabel, p.repeat != Repeat::Off).on_press(
            |s: &mut App| {
                s.player.repeat = match s.player.repeat {
                    Repeat::Off => Repeat::All,
                    Repeat::All => Repeat::One,
                    Repeat::One => Repeat::Off,
                }
            },
        ));
    }
    c = c.child(transport);

    let vol_glyph = if p.snap.muted || p.volume == 0.0 {
        &ic::MUTE
    } else if p.volume < 50.0 {
        &ic::VOLUME_LOW
    } else {
        &ic::VOLUME
    };
    let lyrics_on = app.prefs.show_lyrics;
    c.child(
        row()
            .gap(10.0)
            .center_y()
            .w_full()
            .child(
                widgets::overlay_button(vol_glyph, "Mute (m)", 36.0)
                    .on_press(|s: &mut App| s.player.toggle_mute()),
            )
            .child(el().w((w - 130.0).clamp(80.0, 220.0)).child(
                super::watch_controls::volume_slider(
                    app,
                    "vol-music",
                    (w - 130.0).clamp(80.0, 220.0),
                ),
            ))
            .child(spacer())
            .child(
                widgets::overlay_button(
                    &ic::LYRICS,
                    if lyrics_on {
                        "Hide lyrics"
                    } else {
                        "Show lyrics"
                    },
                    38.0,
                )
                .opacity(if lyrics_on { 1.0 } else { 0.6 })
                .on_press(|s: &mut App| s.edit_prefs(|p| p.show_lyrics = !p.show_lyrics)),
            ),
    )
}

/// The seek bar: the played part follows playback live; drag to seek.
fn seek_bar(app: &App, w: f32) -> El {
    let t = &app.theme;
    let p = &app.player;
    let dur = p.snap.duration.unwrap_or(0.0);
    p.lives.bars[BAR_MUSIC].set_width(w, p.snap.duration.map_or(0.0, |d| (p.snap.time / d) as f32));
    let buffered = p
        .snap
        .cached_until
        .zip(p.snap.duration)
        .map_or(0.0, |(c, d)| (c / d).clamp(0.0, 1.0) as f32);
    let track = el()
        .key("music-seek-track")
        .w(w)
        .h(BAR_H)
        .radius(2.0)
        .fill(Color::WHITE.alpha(0.22))
        .child(
            el().absolute(0.0, 0.0)
                .w(w * buffered)
                .h(BAR_H)
                .radius(2.0)
                .fill(Color::WHITE.alpha(0.28)),
        )
        .child(
            el().absolute(0.0, 0.0)
                .h(BAR_H)
                .radius(2.0)
                .fill(t.accent)
                .live_rect(&p.lives.bars[BAR_MUSIC].rect),
        );
    let mut hit = el()
        .key("music-seek")
        .w(w)
        .h(20.0)
        .cursor(rune::Cursor::Pointer)
        .role("slider")
        .label("Seek")
        .hover_group()
        .child(el().absolute(0.0, 8.0).child(track));
    if let Some(f) = p.seek_preview {
        let x = f * w;
        hit = hit
            .child(
                el().absolute(x - 7.0, 3.0)
                    .size(14.0, 14.0)
                    .radius(7.0)
                    .fill(Color::WHITE)
                    .shadow(1.0, 6.0, Color::BLACK.alpha(0.4)),
            )
            .child(
                el().absolute((x - 34.0).clamp(0.0, (w - 68.0).max(0.0)), -32.0)
                    .w(68.0)
                    .h(26.0)
                    .radius(8.0)
                    .center()
                    .fill(Color::BLACK.alpha(0.75))
                    .child(
                        text(crate::player::mpv::clock(f as f64 * dur))
                            .font(ty::NUM)
                            .color(Color::WHITE),
                    ),
            );
    }
    let hit = hit.on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| {
        tx.instant();
        let f = (d.x / d.w.max(1.0)).clamp(0.0, 1.0);
        if d.phase == Phase::Up {
            s.player.seek_preview = None;
            if let Some(dur) = s.player.snap.duration {
                s.player.seek_to(f as f64 * dur, true);
            }
        } else {
            s.player.seek_preview = Some(f);
        }
    });
    col().gap(2.0).w(w).child(hit).child(
        row()
            .w_full()
            .child(
                text("00:00:00 / 00:00:00")
                    .live_text(&p.lives.time_of)
                    .font(ty::NUM)
                    .color(Color::WHITE.alpha(0.7)),
            )
            .child(spacer()),
    )
}
