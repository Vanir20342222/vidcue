//! The artwork-led visuals of the music stage: classic cover, vinyl record, VU meters and the
//! orbit. Each takes a square (VU a wider) box and draws only the picture; the title, seek bar
//! and transport live in `music.rs`.

use rune::{Color, El, LoopSpec, Spring, SurfaceFit, col, el, icon, row, text};

use crate::app::App;
use crate::icons as ic;
use crate::images::{Size, thumb_url};
use crate::player::session::Now;
use crate::theme::ty;

/// The four player styles (the `-fullscreen` variants of a style set `full`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Classic,
    Vinyl,
    Vu,
    Orbit,
}

impl Style {
    pub const ALL: [Style; 4] = [Style::Classic, Style::Vinyl, Style::Vu, Style::Orbit];

    pub fn label(self) -> &'static str {
        match self {
            Style::Classic => "Classic",
            Style::Vinyl => "Vinyl",
            Style::Vu => "VU",
            Style::Orbit => "Orbit",
        }
    }
    fn id(self) -> &'static str {
        match self {
            Style::Classic => "classic",
            Style::Vinyl => "vinyl",
            Style::Vu => "vu",
            Style::Orbit => "orbit",
        }
    }
    /// The saved `music_player_style` for this style and full-bleed flag.
    pub fn config_value(self, full: bool) -> String {
        if full && self != Style::Classic {
            format!("{}-fullscreen", self.id())
        } else {
            self.id().to_string()
        }
    }
}

/// Parses a saved style; unknown values fall back to classic. Classic has no full-bleed variant
/// on Android, so its flag stays false.
pub fn parse_style(value: &str) -> (Style, bool) {
    let (base, full) = match value.strip_suffix("-fullscreen") {
        Some(b) => (b, true),
        None => (value, false),
    };
    let style = Style::ALL
        .into_iter()
        .find(|s| s.id() == base)
        .unwrap_or(Style::Classic);
    (style, full && style != Style::Classic)
}

/// Width of a style's visual for a square side `s`.
pub fn visual_width(style: Style, s: f32) -> f32 {
    if style == Style::Vu { s * 1.3 } else { s }
}

/// Top-left of a `dot`-wide dot sitting on a ring of box size `ring`, `angle` radians clockwise
/// from the top.
pub fn ring_dot(ring: f32, dot: f32, angle: f32) -> (f32, f32) {
    let r = ring / 2.0;
    (
        r + r * angle.sin() - dot / 2.0,
        r - r * angle.cos() - dot / 2.0,
    )
}

/// Tonearm rotation: over the record while playing, parked off it otherwise.
pub fn tonearm_angle(playing: bool) -> f32 {
    if playing { 0.44 } else { -0.2 }
}

/// The cover, or a tinted placeholder while it loads.
pub fn art(app: &App, now: &Now, size: f32, radius: f32, matched: bool) -> El {
    let t = &app.theme;
    let mut e = el()
        .size(size, size)
        .radius(radius)
        .clip_rounded(radius)
        .fill(t.raised);
    if matched {
        e = e.key("music-art").matched(format!("art-{}", now.media.url));
    }
    match app
        .images
        .get(&thumb_url(&now.media, Size::SQUARE), Size::SQUARE)
    {
        Some(img) => e.child(
            el().w_full()
                .h_full()
                .pixels_fit(img.pixels(), SurfaceFit::Fill),
        ),
        None => e.center().child(
            icon(
                if matches!(now.source, crate::player::session::Source::Radio) {
                    &ic::RADIO
                } else {
                    &ic::MUSIC
                },
                size * 0.3,
            )
            .color(t.faint),
        ),
    }
}

/// Large rounded artwork with a deep shadow.
pub fn classic(app: &App, now: &Now, s: f32) -> El {
    el().size(s, s)
        .shadow(s * 0.06, s * 0.16, Color::BLACK.alpha(0.5))
        .radius(s * 0.05)
        .child(art(app, now, s, s * 0.05, true))
}

fn disc(d: f32) -> El {
    el().size(d, d).radius(d / 2.0)
}

/// A record on a turntable: sleeve behind, a spinning disc, and an arm that lands when playing.
pub fn vinyl(app: &App, now: &Now, s: f32, playing: bool) -> El {
    let d = s * 0.9;
    let mut record = disc(d)
        .absolute(s * 0.1, s * 0.05)
        .fill(Color::BLACK.mix(Color::WHITE, 0.07))
        .shadow(s * 0.03, s * 0.08, Color::BLACK.alpha(0.55));
    // Grooves: concentric rings between the label and the rim.
    let mut f = 0.97;
    while f > 0.46 {
        let g = d * f;
        record = record.child(disc(g).absolute((d - g) / 2.0, (d - g) / 2.0).border(
            1.0,
            Color::WHITE.alpha(if (f * 100.0) as i32 % 6 == 0 {
                0.09
            } else {
                0.04
            }),
        ));
        f -= 0.045;
    }
    // A sheen so the rotation reads.
    record = record
        .child(
            el().absolute(d / 2.0 - 1.0, d * 0.04)
                .size(2.0, d * 0.42)
                .radius(1.0)
                .fill(Color::WHITE.alpha(0.12)),
        )
        .child(
            el().absolute(d / 2.0 - 1.0, d * 0.54)
                .size(2.0, d * 0.42)
                .radius(1.0)
                .fill(Color::WHITE.alpha(0.05)),
        );
    let label = d * 0.36;
    let mut lab = disc(label)
        .absolute((d - label) / 2.0, (d - label) / 2.0)
        .clip_rounded(label / 2.0)
        .fill(app.theme.accent);
    if let Some(img) = app
        .images
        .get(&thumb_url(&now.media, Size::SQUARE), Size::SQUARE)
    {
        lab = lab.child(
            el().w_full()
                .h_full()
                .pixels_fit(img.pixels(), SurfaceFit::Fill),
        );
    }
    record = record.child(lab).child(
        disc(d * 0.035)
            .absolute(d * 0.4825, d * 0.4825)
            .fill(Color::BLACK.alpha(0.9)),
    );
    if playing {
        record = record.looping(LoopSpec::rotation(1.8));
    }
    let sleeve = s * 0.78;
    let sleeve_el = el()
        .absolute(0.0, s * 0.11)
        .size(sleeve, sleeve)
        .shadow(s * 0.02, s * 0.06, Color::BLACK.alpha(0.5))
        .radius(s * 0.03)
        .child(art(app, now, sleeve, s * 0.03, true));
    // Tonearm: pivots from its top; its tip crosses the grooves when rotated in.
    let (arm_w, arm_len) = (s * 0.045, s * 0.6);
    let pivot = (s * 0.9, s * 0.06);
    let arm = el()
        .key("tonearm")
        .absolute(pivot.0 - arm_w / 2.0, pivot.1)
        .size(arm_w, arm_len)
        .origin(0.5, 0.04)
        .rotate(tonearm_angle(playing))
        .motion(Spring::new(0.9, 0.18))
        .child(
            el().w_full()
                .h_full()
                .radius(arm_w / 2.0)
                .fill(Color::WHITE.alpha(0.78)),
        )
        .child(
            el().absolute(-arm_w * 0.4, arm_len - arm_w * 1.6)
                .size(arm_w * 1.8, arm_w * 2.6)
                .radius(arm_w * 0.5)
                .fill(Color::WHITE.alpha(0.92)),
        );
    let base = disc(s * 0.12)
        .absolute(pivot.0 - s * 0.06, pivot.1 - s * 0.05)
        .fill(Color::WHITE.alpha(0.16))
        .border(1.0, Color::WHITE.alpha(0.25));
    el().size(s, s)
        .child(record)
        .child(sleeve_el)
        .child(base)
        .child(arm)
}

/// Two LED-ladder level meters. The needle fill follows the live level without a view rebuild:
/// a bottom-anchored `live_scale`, clipped into green, amber and red zones.
pub fn vu(app: &App, _now: &Now, s: f32, dots: bool) -> El {
    let t = &app.theme;
    let lv = &app.player.lives;
    let (w, h) = (visual_width(Style::Vu, s), s * 0.62);
    let track_w = w - 56.0 - 34.0;
    let track_h = if dots { 14.0 } else { 16.0 };
    let panel = t.bg.mix(Color::BLACK, 0.35);
    for m in lv.meters.iter() {
        m.set_size(track_w, track_h);
    }
    let gradient = [
        (0.0, t.success),
        (0.62, t.success.mix(t.warning, 0.5)),
        (0.82, t.warning),
        (1.0, t.error),
    ];
    let ticks = if dots { 24 } else { 40 };
    let channel = |label: &str, idx: usize| {
        let mut track = el()
            .w(track_w)
            .h(track_h)
            .radius(track_h / 2.0)
            .clip_rounded(track_h / 2.0)
            .fill(Color::WHITE.alpha(0.06))
            // The coloured scale, revealed from the left by the live level.
            .child(
                el().absolute(0.0, 0.0)
                    .h(track_h)
                    .clip_rounded(0.0)
                    .live_rect(&lv.meters[idx].rect)
                    .child(
                        el().absolute(0.0, 0.0)
                            .w(track_w)
                            .h(track_h)
                            .linear_gradient(std::f32::consts::FRAC_PI_2, gradient),
                    ),
            );
        // Segment gaps turn the bar into LEDs (round dots on Nothing palettes).
        for k in 1..ticks {
            let x = k as f32 * track_w / ticks as f32;
            track = track.child(
                el().absolute(x - 1.0, 0.0)
                    .w(2.0)
                    .h(track_h)
                    .fill(panel.alpha(0.7))
                    .inert(),
            );
        }
        track = track.child(
            el().absolute(0.0, 0.0)
                .h(track_h)
                .radius(1.5)
                .fill(Color::WHITE.alpha(0.95))
                .shadow(0.0, 6.0, Color::WHITE.alpha(0.5))
                .live_rect(&lv.meters[2].rect),
        );
        row()
            .gap(16.0)
            .center_y()
            .child(text(label).font(ty::H3).color(t.text.alpha(0.85)).w(18.0))
            .child(track)
    };
    let scale = {
        let mut r = el().w(track_w).h(14.0).ml(34.0);
        for (label, f) in [
            ("-36", 0.0),
            ("-30", 0.2),
            ("-24", 0.4),
            ("-18", 0.6),
            ("-12", 0.8),
            ("-6", 1.0),
        ] {
            let x = (f * track_w
                - if f >= 1.0 {
                    14.0
                } else if f > 0.0 {
                    9.0
                } else {
                    0.0
                })
            .max(0.0);
            r = r.child(
                el().absolute(x, 0.0)
                    .child(text(label).font(ty::CAPTION).color(t.faint)),
            );
        }
        r
    };
    col()
        .size(w, h)
        .px(28.0)
        .py(24.0)
        .gap(18.0)
        .justify_center()
        .radius(s * 0.06)
        .fill(panel.alpha(0.6))
        .border(1.0, Color::WHITE.alpha(0.08))
        .child(
            row()
                .w_full()
                .center_y()
                .child(
                    text(if dots { "SIGNAL" } else { "STEREO LEVEL" })
                        .font(ty::CAPTION)
                        .color(t.faint),
                )
                .child(rune::spacer())
                .child(
                    el().size(8.0, 8.0)
                        .radius(4.0)
                        .fill(t.error)
                        .live_opacity(&lv.glow),
                ),
        )
        .child(channel("L", 0))
        .child(channel("R", 1))
        .child(scale)
}

/// The cover in a disc, with soft palette halos that breathe with the music and a gradient
/// ring turning around it while it plays.
pub fn orbit(app: &App, now: &Now, s: f32, playing: bool) -> El {
    let t = &app.theme;
    let lv = &app.player.lives;
    let c = s * 0.5;
    let centred = |d: f32| disc(d).absolute((s - d) / 2.0, (s - d) / 2.0);
    let halo = |d: f32, color: Color, spring: f32| {
        centred(d)
            .radial_gradient(
                [0.5, 0.5],
                d / 2.0,
                [
                    (0.0, color.alpha(0.55)),
                    (0.55, color.alpha(0.18)),
                    (1.0, color.alpha(0.0)),
                ],
            )
            .inert()
            .live_scale(lv.pulse.smooth(Spring::new(spring, 0.0)))
    };
    let ring_d = c * 1.18;
    let mut ring = centred(ring_d)
        .linear_gradient(
            0.8,
            [
                (0.0, t.accent),
                (0.5, t.accent.alpha(0.0)),
                (
                    1.0,
                    crate::theme::rgb(t.palette.selection).mix(t.accent, 0.4),
                ),
            ],
        )
        .inert();
    if playing {
        ring = ring.looping(LoopSpec::rotation(6.0));
    }
    let orbit_d = s * 0.86;
    let (dx, dy) = ring_dot(orbit_d, 10.0, 0.6);
    let mut path = centred(orbit_d)
        .border(1.0, t.text.alpha(0.1))
        .inert()
        .child(
            disc(10.0)
                .absolute(dx, dy)
                .fill(t.accent)
                .shadow(0.0, 10.0, t.accent.alpha(0.8)),
        );
    if playing {
        path = path.looping(LoopSpec::rotation(18.0));
    }
    el().size(s, s)
        .child(halo(s, t.accent, 0.35))
        .child(
            halo(
                s * 0.8,
                crate::theme::rgb(t.palette.selection).mix(t.accent, 0.3),
                0.18,
            )
            .live_opacity(lv.glow.smooth(Spring::new(0.25, 0.0))),
        )
        .child(path)
        .child(ring)
        .child(
            el().absolute((s - c) / 2.0, (s - c) / 2.0)
                .size(c, c)
                .radius(c / 2.0)
                .shadow(s * 0.02, s * 0.08, Color::BLACK.alpha(0.55))
                .child(art(app, now, c, c / 2.0, true)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_round_trips_every_saved_value() {
        for v in vidcue_core::MUSIC_PLAYER_STYLES {
            let (s, full) = parse_style(v);
            assert_eq!(s.config_value(full), *v, "{v}");
        }
        assert_eq!(parse_style("nonsense"), (Style::Classic, false));
        assert_eq!(Style::Classic.config_value(true), "classic");
    }

    #[test]
    fn ring_dots_sit_on_the_circle() {
        let (x, y) = ring_dot(100.0, 10.0, 0.0);
        assert!((x - 45.0).abs() < 1e-3 && (y - -5.0).abs() < 1e-3);
        let (x, y) = ring_dot(100.0, 10.0, std::f32::consts::FRAC_PI_2);
        assert!((x - 95.0).abs() < 1e-3 && (y - 45.0).abs() < 1e-3);
    }
}
