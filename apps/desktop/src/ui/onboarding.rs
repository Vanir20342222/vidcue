//! First-run onboarding: five skippable steps (Welcome, Look and feel, Playback, Account,
//! Ready) in a centred glass card over a blurred scrim, like the Android app's welcome flow.
//! Finishing or skipping records `ONBOARDING_VERSION` so it is not shown again.

use std::sync::atomic::{AtomicI32, Ordering};

use rune::tok::motion;
use rune::{Color, Cx, El, Stagger, Transition, col, el, icon, row, text};

use crate::app::{App, ONBOARDING_VERSION, Route};
use crate::icons as ic;
use crate::theme::{Theme, sz, ty};
use crate::ui::widgets::{Kind, button, chip, setting_row, switch};

pub const STEPS: [&str; 5] = ["Welcome", "Look and feel", "Playback", "Account", "Ready"];
const QUALITIES: [(&str, &str); 6] = [
    ("best", "Best"),
    ("2160", "4K"),
    ("1440", "1440p"),
    ("1080", "1080p"),
    ("720", "720p"),
    ("480", "480p"),
];
const SPONSOR: [(&str, &str); 3] = [("skip", "Skip"), ("mark", "Mark only"), ("off", "Off")];

/// Which way the last step change went (+1 forward, -1 back); steers the slide direction.
static DIRECTION: AtomicI32 = AtomicI32::new(1);

/// The step after `i`, or None at the end.
pub fn next_step(i: usize) -> Option<usize> {
    (i + 1 < STEPS.len()).then_some(i + 1)
}

/// The step before `i`, or None at the start.
pub fn prev_step(i: usize) -> Option<usize> {
    i.checked_sub(1)
}

fn go(s: &mut App, to: Option<usize>, dir: i32) {
    if let Some(to) = to {
        DIRECTION.store(dir, Ordering::Relaxed);
        s.onboarding = Some(to);
    }
}

fn finish(s: &mut App) {
    s.onboarding = None;
    s.edit_prefs(|p| p.onboarding_version = ONBOARDING_VERSION);
}

pub fn view(app: &App, _cx: &mut Cx<App>, step: usize) -> El {
    let t = &app.theme;
    let step = step.min(STEPS.len() - 1);
    let dir = DIRECTION.load(Ordering::Relaxed).signum().max(-1) as f32;
    let r = 28.0;

    let body = match step {
        0 => welcome(t),
        1 => look(app),
        2 => playback(app),
        3 => account(t),
        _ => ready(t),
    };
    // Content is keyed per step: the old step slides out, the new one slides in.
    let content = el()
        .key(format!("ob-step-{step}"))
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .transition(Transition::offset(40.0 * dir, 0.0).combined(Transition::opacity()))
        .child(body);

    let mut card = col()
        .key("ob-card")
        .w(760.0)
        .h(520.0)
        .radius(r)
        .clip_rounded(r)
        .fill(t.surface.alpha(0.94))
        .backdrop_blur(30.0)
        .border(1.0, t.border)
        .shadow(24.0, 80.0, t.shadow)
        .transition(Transition::scale(0.95).combined(Transition::opacity()))
        .role("dialog")
        .label("Welcome to Vidcue");
    if step == 0 {
        card = card.child(
            el().key("ob-aurora")
                .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .transition(Transition::opacity())
                .child(
                    crate::fx::mesh(4, 0.3)
                        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                        .opacity(0.85),
                )
                .child(crate::fx::aurora(0.6, 0.4).pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))),
        );
    }
    card = card
        .child(content)
        .child(el().w_full().h(1.0).fill(t.hairline))
        .child(footer(t, step));

    el().key("onboarding")
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .center()
        .fill(t.scrim)
        .transition(Transition::opacity())
        .child(card)
        .modal(false, |s: &mut App| finish(s))
}

fn footer(t: &Theme, step: usize) -> El {
    let last = step == STEPS.len() - 1;
    // Progress dots: the current one is a wider accent pill that springs between slots.
    let mut dots = row().gap(6.0).center_y();
    for (i, _) in STEPS.iter().enumerate() {
        let on = i == step;
        dots = dots.child(
            el().key(format!("ob-dot-{i}"))
                .w(if on { 26.0 } else { 8.0 })
                .h(8.0)
                .radius(4.0)
                .fill(if on { t.accent } else { t.overlay })
                .motion(motion::POP)
                .cursor(rune::Cursor::Pointer)
                .role("button")
                .label(format!("Step {} of {}: {}", i + 1, STEPS.len(), STEPS[i]))
                .on_press(move |s: &mut App| go(s, Some(i), if i >= step { 1 } else { -1 })),
        );
    }
    let mut r = row()
        .key("ob-footer")
        .w_full()
        .h(72.0)
        .px(28.0)
        .gap(10.0)
        .center_y()
        .fill(t.bg.alpha(0.35));
    r = r.child(if last {
        el().key("ob-skip-none")
    } else {
        button(t, "Skip", None, Kind::Ghost)
            .key("ob-skip")
            .on_press(|s: &mut App| finish(s))
    });
    r = r
        .child(el().grow(1.0).child(el().w_full().h(1.0)))
        .child(dots)
        .child(el().grow(1.0).child(el().w_full().h(1.0)));
    if let Some(p) = prev_step(step) {
        r = r.child(
            button(t, "Back", None, Kind::Ghost)
                .key("ob-back")
                .on_press(move |s: &mut App| go(s, Some(p), -1)),
        );
    }
    r.child(if last {
        button(t, "Start watching", Some(&ic::PLAY), Kind::Primary)
            .key("ob-finish")
            .default_button()
            .on_press(|s: &mut App| finish(s))
    } else {
        let n = next_step(step);
        button(t, "Next", Some(&ic::FORWARD), Kind::Primary)
            .key("ob-next")
            .default_button()
            .on_press(move |s: &mut App| go(s, n, 1))
    })
}

/// A step's heading block.
fn heading(t: &Theme, title: &str, sub: &str, light: bool) -> El {
    let (a, b) = if light {
        (Color::WHITE, Color::WHITE.alpha(0.8))
    } else {
        (t.text, t.muted)
    };
    col()
        .gap(6.0)
        .child(text(title).font(ty::H1).color(a))
        .child(text(sub).font(ty::BODY).color(b).wrap())
}

fn staged(i: usize, n: usize, e: El) -> El {
    let stagger = Stagger::new(0.06);
    e.transition(Transition::offset(0.0, 14.0).combined(Transition::opacity()))
        .enter_delay(0.08 + stagger.delay(i, n))
}

fn page(inner: El) -> El {
    col()
        .w_full()
        .h_full()
        .px(40.0)
        .pt(36.0)
        .pb(12.0)
        .gap(18.0)
        .child(inner)
}

// ---- steps ----

fn welcome(t: &Theme) -> El {
    let logo = el()
        .key("ob-logo")
        .size(96.0, 96.0)
        .radius(30.0)
        .fill(Color::WHITE.alpha(0.18))
        .border(1.0, Color::WHITE.alpha(0.3))
        .center()
        .child(
            crate::fx::glow(1, 0, 0.7, 22.0)
                .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .radius(30.0),
        )
        .child(icon(&ic::PLAY, 44.0).color(Color::WHITE));
    let _ = t;
    col()
        .w_full()
        .h_full()
        .center()
        .gap(18.0)
        .px(60.0)
        .child(staged(0, 3, logo))
        .child(staged(
            1,
            3,
            text("Vidcue").font(ty::DISPLAY).color(Color::WHITE),
        ))
        .child(staged(
            2,
            3,
            text("Videos, music and radio in one place")
                .font(ty::H3)
                .color(Color::WHITE.alpha(0.88))
                .align(rune::Align::Center),
        ))
}

fn swatch(app: &App, name: &str, palette: Option<&vidcue_core::theme::Palette>) -> El {
    let t = &app.theme;
    let on = app.config.theme == name;
    let preview = palette.map(Theme::from_palette);
    let name_owned = name.to_owned();
    let face = match preview {
        Some(p) => el()
            .size(72.0, 44.0)
            .radius(12.0)
            .fill(p.bg)
            .border(1.0, t.border)
            .clip_rounded(12.0)
            .child(
                el().absolute(10.0, 10.0)
                    .size(30.0, 6.0)
                    .radius(3.0)
                    .fill(p.text.alpha(0.8)),
            )
            .child(
                el().absolute(10.0, 20.0)
                    .size(20.0, 6.0)
                    .radius(3.0)
                    .fill(p.muted),
            )
            .child(
                el().absolute(46.0, 26.0)
                    .size(16.0, 10.0)
                    .radius(5.0)
                    .fill(p.accent),
            ),
        None => el()
            .size(72.0, 44.0)
            .radius(12.0)
            .fill(t.raised)
            .border(1.0, t.border)
            .center()
            .child(icon(&ic::SPARKLES, 20.0).color(t.accent)),
    };
    col()
        .key(format!("ob-theme-{name}"))
        .gap(6.0)
        .items_center()
        .padding(6.0)
        .radius(16.0)
        .border(2.0, if on { t.accent } else { Color::TRANSPARENT })
        .hover(t.hover)
        .press_scale(0.95)
        .motion(motion::SNAP)
        .role("radio")
        .checked(on)
        .label(crate::themes::label(name))
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| {
            let n = name_owned.clone();
            s.edit_config(|c| c.theme = n);
        })
        .child(face)
        .child(
            text(crate::themes::label(name))
                .font(ty::SMALL)
                .color(if on { t.text } else { t.muted })
                .max_w(80.0)
                .truncate(),
        )
}

fn look(app: &App) -> El {
    let t = &app.theme;
    let mut swatches = row().gap(4.0).wrap();
    for (name, palette) in crate::themes::pickable() {
        swatches = swatches.child(swatch(app, name, palette.as_ref()));
    }
    let fx = switch(t, "ob-effects", app.prefs.effects)
        .on_press(|s: &mut App| s.edit_prefs(|p| p.effects = !p.effects));
    let rm = switch(t, "ob-reduced", app.config.reduced_motion)
        .on_press(|s: &mut App| s.edit_config(|c| c.reduced_motion = !c.reduced_motion));
    page(
        col()
            .w_full()
            .gap(16.0)
            .child(heading(
                t,
                "Look and feel",
                "Pick a theme. You can change it any time in Settings.",
                false,
            ))
            .child(staged(
                0,
                3,
                el().w_full()
                    .h(150.0)
                    .scroll(rune::ScrollSpec {
                        stick_bottom: false,
                    })
                    .child(swatches),
            ))
            .child(staged(
                1,
                3,
                setting_row(
                    t,
                    "Visual effects",
                    "Animated gradients, glows and glass.",
                    fx,
                ),
            ))
            .child(staged(
                2,
                3,
                setting_row(
                    t,
                    "Reduce motion",
                    "Fewer animations and no sliding transitions.",
                    rm,
                ),
            )),
    )
}

fn playback(app: &App) -> El {
    let t = &app.theme;
    let c = &app.config;
    let mut q = row().gap(6.0).wrap();
    for (v, label) in QUALITIES {
        q = q.child(
            chip(t, label, c.download.quality == v)
                .key(format!("ob-q-{v}"))
                .on_press(move |s: &mut App| s.edit_config(|c| c.download.quality = v.to_owned())),
        );
    }
    let mut sb = row().gap(6.0);
    for (v, label) in SPONSOR {
        sb = sb.child(
            chip(t, label, c.playback_sponsorblock == v)
                .key(format!("ob-sb-{v}"))
                .on_press(move |s: &mut App| {
                    s.edit_config(|c| c.playback_sponsorblock = v.to_owned())
                }),
        );
    }
    let auto = switch(t, "ob-autoplay", c.autoplay_related)
        .on_press(|s: &mut App| s.edit_config(|c| c.autoplay_related = !c.autoplay_related));
    let resume = switch(t, "ob-resume", c.resume_on_play)
        .on_press(|s: &mut App| s.edit_config(|c| c.resume_on_play = !c.resume_on_play));
    page(
        col()
            .w_full()
            .gap(6.0)
            .child(heading(
                t,
                "Playback",
                "Sensible defaults for watching and downloading.",
                false,
            ))
            .child(staged(
                0,
                4,
                col()
                    .gap(8.0)
                    .pt(10.0)
                    .child(text("Default quality").font(ty::BODY_STRONG).color(t.text))
                    .child(q),
            ))
            .child(staged(
                1,
                4,
                col()
                    .gap(8.0)
                    .py(8.0)
                    .child(
                        text("Sponsor segments (SponsorBlock)")
                            .font(ty::BODY_STRONG)
                            .color(t.text),
                    )
                    .child(sb),
            ))
            .child(staged(
                2,
                4,
                setting_row(
                    t,
                    "Autoplay related videos",
                    "Keep going when a video ends.",
                    auto,
                ),
            ))
            .child(staged(
                3,
                4,
                setting_row(
                    t,
                    "Resume where you left off",
                    "Continue partly watched videos from their last position.",
                    resume,
                ),
            )),
    )
}

fn account(t: &Theme) -> El {
    let open = button(t, "Open Account", Some(&ic::USER), Kind::Primary)
        .key("ob-open-account")
        .on_press(|s: &mut App| {
            finish(s);
            s.go_root(Route::Account);
        });
    page(
        col().w_full().gap(18.0)
            .child(heading(t, "Account", "Signing in is optional.", false))
            .child(staged(0, 3, el().w_full().padding(20.0).radius(sz::R_PANEL).fill(t.raised).child(
                text("Without an account you can search, watch, listen to radio and keep a local library. Sign in to see your own Home, Subscriptions and playlists. Your session stays on this computer, and nothing is read from a browser until you ask.").font(ty::BODY).color(t.muted).wrap(),
            )))
            .child(staged(1, 3, row().gap(12.0).center_y().child(open).child(text("or skip this and add it later from the sidebar").font(ty::SMALL).color(t.faint))))
            .child(staged(2, 3, el().h(1.0))),
    )
}

fn keycap(t: &Theme, k: &str) -> El {
    row()
        .h(28.0)
        .min_w(28.0)
        .px(9.0)
        .center()
        .radius(8.0)
        .fill(t.raised)
        .border(1.0, t.border)
        .child(text(k).font(ty::SMALL_STRONG).color(t.text))
}

fn ready(t: &Theme) -> El {
    let hints: [(&[&str], &str); 5] = [
        (&["Space", "K"], "Play or pause"),
        (&["J", "L"], "Back or forward 10 seconds"),
        (&["F"], "Fullscreen"),
        (&["/"], "Search"),
        (&["Ctrl", "K"], "Quick actions"),
    ];
    let mut list = col().w_full().gap(10.0);
    for (i, (keys, what)) in hints.iter().enumerate() {
        let mut caps = row().gap(6.0).w(150.0);
        for k in *keys {
            caps = caps.child(keycap(t, k));
        }
        list = list.child(staged(
            i,
            hints.len(),
            row()
                .w_full()
                .gap(16.0)
                .center_y()
                .child(caps)
                .child(text(*what).font(ty::BODY).color(t.muted)),
        ));
    }
    page(
        col()
            .w_full()
            .gap(18.0)
            .child(heading(
                t,
                "You're all set",
                "A few shortcuts to get you started.",
                false,
            ))
            .child(list),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_walk_forward_and_back() {
        assert_eq!(next_step(0), Some(1));
        assert_eq!(next_step(STEPS.len() - 1), None);
        assert_eq!(prev_step(0), None);
        assert_eq!(prev_step(3), Some(2));
    }

    #[test]
    fn quality_and_sponsor_choices_are_valid_config() {
        let mut c = vidcue_core::Config::default();
        for (v, _) in QUALITIES {
            c.download.quality = v.to_owned();
            assert!(c.validate().is_ok(), "{v}");
        }
        c = vidcue_core::Config::default();
        for (v, _) in SPONSOR {
            c.playback_sponsorblock = v.to_owned();
            assert!(c.validate().is_ok(), "{v}");
        }
    }
}
