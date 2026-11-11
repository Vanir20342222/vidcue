//! The video stage and everything drawn over it.

use rune::{
    Align, Color, Cx, Drag, El, Phase, SurfaceFit, TextStyle, Transition, Tx, col, el, icon, row,
    spacer, text,
};

use crate::app::App;
use crate::icons as ic;
use crate::images::{Size, thumb_url};
use crate::player::mpv::{BAR_H, BAR_WATCH};
use crate::player::session::{self, PlayerMenu, Sleep};
use crate::theme::ty;
use crate::ui::widgets::overlay_button;

pub const STAGE_RADIUS: f32 = 18.0;

/// The video at `w`×`h`: ambient light behind, the picture, captions and controls on top.
pub fn stage(app: &App, cx: &mut Cx<App>, w: f32, h: f32) -> El {
    let t = &app.theme;
    let p = &app.player;
    let Some(now) = &p.now else { return el() };
    let scale = cx.env().scale;
    if let Some(m) = &p.mpv {
        m.set_target(w * scale, h * scale);
    }
    let radius = if p.fullscreen { 0.0 } else { STAGE_RADIUS };
    let has_frames = p.has_video();
    let mut video = el()
        .key("stage")
        .label("Video: press to play or pause, double-press for full screen")
        .w(w)
        .h(h)
        .radius(radius)
        .clip_rounded(radius)
        .fill(Color::BLACK);

    // Poster: the card's artwork flies here and holds until frames arrive.
    if !has_frames {
        let mut poster = el()
            .key("poster")
            .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .matched(format!("art-{}", now.media.url));
        if let Some(img) = app
            .images
            .get(&thumb_url(&now.media, Size::HERO), Size::HERO)
        {
            poster = poster.child(
                el().w_full()
                    .h_full()
                    .pixels_fit(img.pixels(), SurfaceFit::Fill)
                    .opacity(0.55)
                    .blur(if now.resolving { 6.0 } else { 0.0 }),
            );
        }
        video = video.child(poster);
    }
    video = video.child(
        el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .native_surface(&p.surface, SurfaceFit::Fit),
    );

    // Status in the middle: resolving, buffering, an error.
    if let Some(err) = &now.error {
        video = video.child(error_card(app, err));
    } else if now.resolving || (!p.snap.loaded && p.unavailable.is_none()) || p.snap.buffering {
        // (Headless renders have no player; they show the resting state.)
        let mut status = col()
            .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .center()
            .gap(14.0)
            .inert()
            .child(spinner(t.accent));
        if !now.resolving {
            status = status.child(
                text("Buffering…")
                    .font(ty::SMALL_STRONG)
                    .color(Color::WHITE.alpha(0.85)),
            );
        }
        video = video.child(status);
    } else if let Some(u) = p.unavailable.as_ref().filter(|_| !app.offline()) {
        video = video.child(error_card(app, u));
    }

    if let Some(sub) = &p.snap.sub_text {
        video = video.child(
            el().pin(
                Some(0.0),
                None,
                Some(0.0),
                Some(if p.snap.paused { 96.0 } else { 40.0 }),
            )
            .items_center()
            .inert()
            .motion(rune::tok::motion::GLIDE)
            .child(
                el().key("caption")
                    .max_w(w * 0.8)
                    .px(14.0)
                    .py(6.0)
                    .radius(10.0)
                    .fill(Color::BLACK.alpha(0.55))
                    .child(
                        text(sub)
                            .font(
                                TextStyle::new((h / 26.0).clamp(15.0, 30.0), 560.0)
                                    .lh((h / 26.0).clamp(15.0, 30.0) * 1.3),
                            )
                            .color(Color::WHITE)
                            .wrap()
                            .align(Align::Center),
                    ),
            ),
        );
    }
    if let Some((label, _)) = &p.flash {
        video = video.child(
            el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .center()
                .inert()
                .child(
                    el().key("flash")
                        .px(20.0)
                        .py(12.0)
                        .radius(14.0)
                        .fill(Color::BLACK.alpha(0.55))
                        .transition(Transition::scale(0.85).combined(Transition::opacity()))
                        .child(text(label).font(ty::H3).color(Color::WHITE)),
                ),
        );
    }
    video = video.child(controls(app, cx, w, h));
    video = video
        .hover_group()
        .on_press(|s: &mut App| {
            s.player.menu = None;
            s.player.toggle_pause();
        })
        .on_double_press_tx(|s: &mut App, tx: &mut Tx| {
            s.player.fullscreen = !s.player.fullscreen;
            tx.toggle_full_screen();
        });

    // Ambient mode: the frame itself, blurred and scaled, lights the page behind the player.
    let mut wrap = el().key("stage-wrap").w(w).h(h);
    if app.prefs.ambient && has_frames && !p.fullscreen {
        wrap = wrap.child(
            el().key("ambient")
                .pin(
                    Some(-w * 0.08),
                    Some(-h * 0.1),
                    Some(-w * 0.08),
                    Some(-h * 0.1),
                )
                .native_surface(&p.surface, SurfaceFit::Fill)
                .blur(56.0)
                .opacity(if t.dark { 0.55 } else { 0.3 })
                .inert()
                .transition(Transition::opacity()),
        );
    } else if app.prefs.ambient
        && !p.fullscreen
        && let Some(img) = app
            .images
            .get(&thumb_url(&now.media, Size::CARD), Size::CARD)
    {
        wrap = wrap.child(
            el().key("ambient")
                .pin(
                    Some(-w * 0.06),
                    Some(-h * 0.08),
                    Some(-w * 0.06),
                    Some(-h * 0.08),
                )
                .pixels_fit(img.pixels(), SurfaceFit::Fill)
                .blur(56.0)
                .opacity(0.35)
                .inert()
                .transition(Transition::opacity()),
        );
    }
    wrap.child(video.shadow(
        if p.fullscreen { 0.0 } else { 18.0 },
        50.0,
        Color::BLACK.alpha(0.45),
    ))
}

pub fn spinner(c: Color) -> El {
    spinner_sized(c, 36.0)
}

/// A ring with a turning arc, `d` points across.
pub fn spinner_sized(c: Color, d: f32) -> El {
    let w = (d / 12.0).clamp(2.0, 3.0);
    el().size(d, d)
        .radius(d / 2.0)
        .border(w, c.alpha(0.22))
        .child(
            el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .radius(d / 2.0)
                .border_sides([w, w, 0.0, 0.0], c)
                .looping(rune::LoopSpec::rotation(0.9)),
        )
}

fn error_card(app: &App, msg: &str) -> El {
    col()
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .center()
        .gap(12.0)
        .px(40.0)
        .child(
            el().size(56.0, 56.0)
                .radius(28.0)
                .fill(app.theme.error.alpha(0.2))
                .center()
                .child(icon(&ic::WARNING, 26.0).color(app.theme.error)),
        )
        .child(
            text("This can't play right now")
                .font(ty::H3)
                .color(Color::WHITE),
        )
        .child(
            text(msg)
                .font(ty::SMALL)
                .color(Color::WHITE.alpha(0.75))
                .max_w(520.0)
                .wrap()
                .align(Align::Center)
                .max_lines(4),
        )
        .child(
            crate::ui::widgets::button(
                &app.theme,
                "Try again",
                Some(&ic::REFRESH),
                crate::ui::widgets::Kind::Glass,
            )
            .on_press_tx(|s: &mut App, tx: &mut Tx| {
                if let Some(n) = &mut s.player.now {
                    n.error = None;
                    n.renewed = false;
                }
                session::retry(s, tx)
            }),
        )
}

fn controls(app: &App, cx: &mut Cx<App>, w: f32, h: f32) -> El {
    let p = &app.player;
    let Some(now) = &p.now else { return el() };
    let shown = p.snap.paused || p.menu.is_some() || p.seek_preview.is_some();
    let bar_w = w - 40.0;
    p.lives.bars[BAR_WATCH].set_width(
        bar_w,
        p.snap.duration.map_or(0.0, |d| (p.snap.time / d) as f32),
    );

    let play = overlay_button(
        if p.snap.paused || p.snap.eof {
            &ic::PLAY
        } else {
            &ic::PAUSE
        },
        if p.snap.paused {
            "Play (k)"
        } else {
            "Pause (k)"
        },
        44.0,
    )
    .key("ctl-play")
    .on_press(|s: &mut App| s.player.toggle_pause());
    let prev = overlay_button(&ic::PREV, "Previous (p)", 38.0)
        .on_press(|s: &mut App| session::previous(s));
    let next =
        overlay_button(&ic::NEXT, "Next (n)", 38.0).on_press(|s: &mut App| session::next(s, false));
    let vol_glyph = if p.snap.muted || p.volume == 0.0 {
        &ic::MUTE
    } else if p.volume < 50.0 {
        &ic::VOLUME_LOW
    } else {
        &ic::VOLUME
    };
    let volume = row()
        .key("ctl-volume")
        .gap(4.0)
        .center_y()
        .hover_group()
        .child(
            overlay_button(vol_glyph, "Mute (m)", 38.0)
                .on_press(|s: &mut App| s.player.toggle_mute()),
        )
        .child(
            el().w(96.0)
                .group_opacity(0.0, 1.0)
                .child(volume_slider(app, "vol-watch", 92.0)),
        );
    let time = if now.live {
        row()
            .gap(8.0)
            .center_y()
            .child(
                el().size(8.0, 8.0).radius(4.0).fill(app.theme.error).child(
                    crate::fx::pulse(0, 4, 1.6, 10.0)
                        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                        .radius(4.0),
                ),
            )
            .child(
                text("LIVE")
                    .font(TextStyle::new(12.0, 760.0).tracking(0.8))
                    .color(Color::WHITE),
            )
    } else {
        row().child(
            text("00:00:00 / 00:00:00")
                .live_text(&p.lives.time_of)
                .font(ty::NUM)
                .color(Color::WHITE.alpha(0.92)),
        )
    };
    let mut right = row().gap(2.0).center_y();
    if !now.captions.is_empty() {
        right = right.child(
            overlay_button(
                &ic::CAPTIONS,
                if now.caption.is_some() {
                    "Captions on (c)"
                } else {
                    "Captions off (c)"
                },
                38.0,
            )
            .opacity(if now.caption.is_some() { 1.0 } else { 0.6 })
            .on_press(|s: &mut App| toggle_captions(s)),
        );
    }
    right = right
        .child(
            overlay_button(&ic::SETTINGS, "Settings", 38.0)
                .key("ctl-settings")
                .on_press(|s: &mut App| {
                    s.player.menu = if s.player.menu.is_some() {
                        None
                    } else {
                        Some(PlayerMenu::Settings)
                    }
                }),
        )
        .child(
            overlay_button(&ic::THEATER, "Theater mode (t)", 38.0)
                .on_press(|s: &mut App| s.player.theater = !s.player.theater),
        )
        .child(
            overlay_button(
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
        );
    let bar = row()
        .w_full()
        .px(14.0)
        .h(52.0)
        .gap(4.0)
        .center_y()
        .child(prev)
        .child(play)
        .child(next)
        .child(volume)
        .child(el().w(10.0))
        .child(time)
        .child(spacer())
        .child(right);
    let scrim = el()
        .pin(Some(0.0), None, Some(0.0), Some(0.0))
        .h(h.min(180.0))
        .gradient(Color::TRANSPARENT, Color::BLACK.alpha(0.72))
        .inert();
    let mut layer = el()
        .key("controls")
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .group_opacity(if shown { 1.0 } else { 0.0 }, 1.0)
        .child(scrim)
        .child(
            col()
                .pin(Some(0.0), None, Some(0.0), Some(0.0))
                .pb(6.0)
                .child(if now.live {
                    el().h(8.0)
                } else {
                    seek_bar(app, bar_w)
                })
                .child(bar),
        );
    if let Some(menu) = p.menu {
        layer = layer.child(settings_panel(app, cx, menu, h));
    }
    layer
}

fn toggle_captions(s: &mut App) {
    let next = match s.player.now.as_ref().map(|n| (n.caption, n.captions.len())) {
        Some((Some(_), _)) => None,
        Some((None, n)) if n > 0 => Some(0),
        _ => None,
    };
    s.player.set_caption(next);
    s.edit_prefs(|p| p.captions = next.is_some());
}

pub fn volume_slider(app: &App, key: &str, width: f32) -> El {
    let v = if app.player.snap.muted {
        0.0
    } else {
        (app.player.volume / 100.0) as f32
    };
    crate::ui::widgets::slider(&app.theme, key, v.min(1.0), width, |s, v, done| {
        s.player.set_volume((v * 100.0) as f64);
        if done {
            let vol = s.player.volume;
            s.edit_prefs(|p| p.volume = vol);
        }
    })
}

/// The seek bar: played part (live), buffered part, SponsorBlock and chapter marks, a knob
/// and a time bubble while dragging.
fn seek_bar(app: &App, w: f32) -> El {
    let t = &app.theme;
    let p = &app.player;
    let dur = p.snap.duration.unwrap_or(0.0);
    let buffered = p
        .snap
        .cached_until
        .zip(p.snap.duration)
        .map_or(0.0, |(c, d)| (c / d).clamp(0.0, 1.0) as f32);
    let mut track = el()
        .key("seek-track")
        .w(w)
        .h(BAR_H)
        .radius(2.0)
        .fill(Color::WHITE.alpha(0.22))
        .child(
            el().absolute(0.0, 0.0)
                .w(w * buffered)
                .h(BAR_H)
                .radius(2.0)
                .fill(Color::WHITE.alpha(0.3)),
        );
    if dur > 0.0 {
        for s in &p.sponsor {
            let x = (s.start / dur) as f32 * w;
            let sw = (((s.end - s.start) / dur) as f32 * w).max(2.0);
            track = track.child(
                el().absolute(x, 0.0)
                    .w(sw)
                    .h(BAR_H)
                    .fill(sponsor_color(&s.category))
                    .help(format!("{} segment", s.category)),
            );
        }
        for c in chapter_times(app) {
            let x = (c / dur) as f32 * w;
            track = track.child(
                el().absolute(x - 1.0, 0.0)
                    .w(2.0)
                    .h(BAR_H)
                    .fill(Color::BLACK.alpha(0.6)),
            );
        }
    }
    let played = el()
        .absolute(0.0, 0.0)
        .h(BAR_H)
        .radius(2.0)
        .fill(t.accent)
        .live_rect(&p.lives.bars[BAR_WATCH].rect);
    track = track.child(played);
    let mut hit = el()
        .key("seek")
        .w(w)
        .h(18.0)
        .mx(20.0)
        .cursor(rune::Cursor::Pointer)
        .role("slider")
        .label("Seek")
        .hover_group()
        .child(el().absolute(0.0, 7.0).child(track.group_opacity(1.0, 1.0)));
    if let Some(f) = p.seek_preview {
        let x = f * w;
        hit = hit
            .child(
                el().absolute(x - 7.0, 2.0)
                    .size(14.0, 14.0)
                    .radius(7.0)
                    .fill(Color::WHITE)
                    .shadow(1.0, 6.0, Color::BLACK.alpha(0.4)),
            )
            .child(
                el().absolute((x - 34.0).clamp(0.0, w - 68.0), -34.0)
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
    hit.on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| {
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
    })
}

fn chapter_times(app: &App) -> Vec<f64> {
    let from_mpv: Vec<f64> = app
        .player
        .snap
        .chapters
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["time"].as_f64())
        .filter(|t| *t > 0.5)
        .collect();
    if !from_mpv.is_empty() {
        return from_mpv;
    }
    app.player
        .now
        .as_ref()
        .map(|n| {
            n.media
                .chapters
                .iter()
                .map(|c| c.start)
                .filter(|t| *t > 0.5)
                .collect()
        })
        .unwrap_or_default()
}

pub fn sponsor_color(category: &str) -> Color {
    match category {
        "sponsor" => Color::from_oklab([0.72, -0.17, 0.12, 1.0]),
        "selfpromo" => Color::from_oklab([0.85, -0.05, 0.16, 1.0]),
        "interaction" => Color::from_oklab([0.65, -0.05, -0.15, 1.0]),
        "intro" | "outro" => Color::from_oklab([0.7, 0.02, -0.12, 1.0]),
        _ => Color::from_oklab([0.75, 0.08, 0.05, 1.0]),
    }
}

const SPEEDS: [f64; 8] = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 3.0];
const SLEEPS: [u32; 6] = [5, 10, 15, 30, 45, 60];

/// The glass settings panel and its pages.
fn settings_panel(app: &App, _cx: &mut Cx<App>, menu: PlayerMenu, h: f32) -> El {
    let t = &app.theme;
    let p = &app.player;
    let now = p.now.as_ref();
    let mut list = col()
        .key("player-menu")
        .w(300.0)
        .max_h((h - 90.0).max(160.0))
        .padding(6.0)
        .gap(1.0)
        .radius(16.0)
        .fill(Color::from_oklab([0.16, 0.0, 0.0, 0.72]))
        .backdrop_blur(28.0)
        .border(1.0, Color::WHITE.alpha(0.12))
        .shadow(20.0, 50.0, Color::BLACK.alpha(0.5))
        .role("menu")
        .focus_group()
        .transition(Transition::scale(0.94).combined(Transition::opacity()))
        .scroll(rune::ScrollSpec {
            stick_bottom: false,
        });
    let item = |label: String, checked: bool| {
        row()
            .h(38.0)
            .px(12.0)
            .gap(10.0)
            .center_y()
            .radius(10.0)
            .hover(Color::WHITE.alpha(0.1))
            .role("menuitemradio")
            .checked(checked)
            .cursor(rune::Cursor::Pointer)
            .child(el().w(18.0).child(if checked {
                icon(&ic::CHECK, 15.0).color(t.accent)
            } else {
                el()
            }))
            .child(text(label).font(ty::BODY).color(Color::WHITE).grow(1.0))
    };
    let header = |title: &str| {
        row()
            .h(40.0)
            .px(6.0)
            .gap(6.0)
            .center_y()
            .cursor(rune::Cursor::Pointer)
            .on_press(|s: &mut App| s.player.menu = Some(PlayerMenu::Settings))
            .child(icon(&ic::BACK, 15.0).color(Color::WHITE.alpha(0.8)))
            .child(text(title).font(ty::BODY_STRONG).color(Color::WHITE))
    };
    match menu {
        PlayerMenu::Settings => {
            let quality = now
                .map(|n| session::quality_label(&n.quality))
                .unwrap_or_default();
            let speed = format!("{}×", trim(p.speed));
            let captions = now
                .and_then(|n| {
                    n.caption
                        .and_then(|i| n.captions.get(i))
                        .map(|c| c.0.clone())
                })
                .unwrap_or_else(|| "Off".into());
            let audio = now
                .and_then(|n| n.audio_language.clone())
                .unwrap_or_else(|| "Default".into());
            let sleep = match p.sleep {
                None => "Off".to_owned(),
                Some(Sleep::EndOfTrack) => "End of track".to_owned(),
                Some(Sleep::At(at)) => format!("{} min", ((at - app.clock) / 60.0).ceil()),
            };
            let row_ = |glyph, label: &str, value: String, target: PlayerMenu| {
                crate::ui::widgets::value_row(t, glyph, label, &value)
                    .on_press(move |s: &mut App| s.player.menu = Some(target))
            };
            if now.is_some_and(|n| !n.audio && n.source != session::Source::Radio) {
                list = list.child(row_(&ic::SIGNAL, "Quality", quality, PlayerMenu::Quality));
            }
            list = list.child(row_(&ic::GAUGE, "Speed", speed, PlayerMenu::Speed));
            if now.is_some_and(|n| !n.captions.is_empty()) {
                list = list.child(row_(
                    &ic::CAPTIONS,
                    "Captions",
                    captions,
                    PlayerMenu::Captions,
                ));
            }
            if now.is_some_and(|n| n.audio_tracks.len() > 1) {
                list = list.child(row_(&ic::GLOBE, "Audio track", audio, PlayerMenu::Audio));
            }
            list = list
                .child(row_(&ic::MOON, "Sleep timer", sleep, PlayerMenu::Sleep))
                .child(row_(
                    &ic::SPEAKER,
                    "Audio output",
                    output_label(app),
                    PlayerMenu::Output,
                ))
                .child(row_(
                    &ic::SLIDERS,
                    "Equalizer",
                    app.config.equalizer.clone(),
                    PlayerMenu::Equalizer,
                ));
            if now.is_some_and(|n| !n.audio) {
                list = list.child(row_(
                    &ic::PALETTE,
                    "Colour grade",
                    app.config.color_grade.clone(),
                    PlayerMenu::Grade,
                ));
            }
        }
        PlayerMenu::Quality => {
            list = list.child(header("Quality"));
            let current = now.map(|n| n.quality.clone()).unwrap_or_default();
            let mut options: Vec<String> = vec!["best".into()];
            let mut q: Vec<u32> = now.map(|n| n.qualities.clone()).unwrap_or_default();
            if q.is_empty() {
                q = vec![2160, 1440, 1080, 720, 480, 360];
            }
            q.sort_unstable_by(|a, b| b.cmp(a));
            q.dedup();
            options.extend(q.iter().map(|h| h.to_string()));
            for o in options {
                let label = session::quality_label(&o);
                let pick = o.clone();
                list = list.child(item(label, o == current).on_press(move |s: &mut App| {
                    session::change_quality(s, pick.clone());
                    s.player.menu = None;
                }));
            }
        }
        PlayerMenu::Speed => {
            list = list.child(header("Speed"));
            for sp in SPEEDS {
                list = list.child(
                    item(
                        if sp == 1.0 {
                            "Normal".into()
                        } else {
                            format!("{}×", trim(sp))
                        },
                        (p.speed - sp).abs() < 0.01,
                    )
                    .on_press(move |s: &mut App| {
                        s.player.set_speed(sp);
                        s.player.flash = Some((format!("Speed {}×", trim(sp)), s.clock + 1.2));
                        s.edit_prefs(|p| p.speed = sp);
                        s.player.menu = None;
                    }),
                );
            }
        }
        PlayerMenu::Captions => {
            list = list.child(header("Captions"));
            let current = now.and_then(|n| n.caption);
            list = list.child(
                item("Off".into(), current.is_none()).on_press(|s: &mut App| {
                    s.player.set_caption(None);
                    s.edit_prefs(|p| p.captions = false);
                    s.player.menu = None;
                }),
            );
            for (i, (label, _)) in now
                .map(|n| n.captions.clone())
                .unwrap_or_default()
                .into_iter()
                .enumerate()
            {
                list = list.child(
                    item(label, current == Some(i)).on_press(move |s: &mut App| {
                        s.player.set_caption(Some(i));
                        s.edit_prefs(|p| p.captions = true);
                        s.player.menu = None;
                    }),
                );
            }
        }
        PlayerMenu::Audio => {
            list = list.child(header("Audio track"));
            let current = now.and_then(|n| n.audio_language.clone());
            for (lang, label) in now.map(|n| n.audio_tracks.clone()).unwrap_or_default() {
                let pick = lang.clone();
                list = list.child(
                    item(label, current.as_deref() == Some(lang.as_str())).on_press(
                        move |s: &mut App| {
                            session::change_audio_language(s, pick.clone());
                            s.player.menu = None;
                        },
                    ),
                );
            }
        }
        PlayerMenu::Sleep => {
            list = list.child(header("Sleep timer"));
            list = list.child(
                item("Off".into(), p.sleep.is_none()).on_press(|s: &mut App| {
                    session::set_sleep(s, None);
                    s.player.menu = None;
                }),
            );
            for m in SLEEPS {
                list = list.child(item(format!("{m} minutes"), false).on_press(
                    move |s: &mut App| {
                        let at = s.clock + m as f64 * 60.0;
                        session::set_sleep(s, Some(Sleep::At(at)));
                        s.player.menu = None;
                    },
                ));
            }
            list = list.child(
                item(
                    "End of this track".into(),
                    p.sleep == Some(Sleep::EndOfTrack),
                )
                .on_press(|s: &mut App| {
                    session::set_sleep(s, Some(Sleep::EndOfTrack));
                    s.player.menu = None;
                }),
            );
        }
        PlayerMenu::Output => {
            list = list.child(header("Audio output"));
            let current = p.snap.audio_device.clone().unwrap_or_else(|| "auto".into());
            for d in p
                .snap
                .audio_devices
                .as_array()
                .into_iter()
                .flatten()
                .take(24)
            {
                let name = d["name"].as_str().unwrap_or_default().to_owned();
                let desc = d["description"].as_str().unwrap_or(&name).to_owned();
                list = list.child(
                    item(
                        if name == "auto" {
                            "System default".into()
                        } else {
                            desc
                        },
                        name == current,
                    )
                    .on_press(move |s: &mut App| {
                        s.player.set_audio_device(&name);
                        s.player.menu = None;
                    }),
                );
            }
        }
        PlayerMenu::Equalizer => {
            list = list.child(header("Equalizer"));
            for name in vidcue_core::equalizer::names(&app.config.equalizer_custom) {
                let pick = name.clone();
                list = list.child(item(nice(&name), app.config.equalizer == name).on_press(
                    move |s: &mut App| {
                        let n = pick.clone();
                        s.edit_config(|c| c.equalizer = n);
                    },
                ));
            }
        }
        PlayerMenu::Grade => {
            list = list.child(header("Colour grade"));
            for g in vidcue_core::COLOR_GRADES {
                list = list.child(item(nice(g), app.config.color_grade == *g).on_press(
                    move |s: &mut App| s.edit_config(|c| c.color_grade = (*g).to_owned()),
                ));
            }
        }
    }
    el().pin(None, None, Some(14.0), Some(64.0))
        .child(list)
        .role("dialog")
        .label("Player settings")
        .on_press(|_: &mut App| {})
}

fn output_label(app: &App) -> String {
    let current = app
        .player
        .snap
        .audio_device
        .clone()
        .unwrap_or_else(|| "auto".into());
    if current == "auto" {
        return "Default".into();
    }
    app.player
        .snap
        .audio_devices
        .as_array()
        .into_iter()
        .flatten()
        .find(|d| d["name"] == current.as_str())
        .and_then(|d| d["description"].as_str())
        .unwrap_or(&current)
        .chars()
        .take(22)
        .collect()
}

pub fn nice(s: &str) -> String {
    crate::themes::label(s)
}

fn trim(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}
