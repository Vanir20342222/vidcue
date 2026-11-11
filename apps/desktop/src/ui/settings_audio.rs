//! Settings → Audio: equalizer presets, a ten-band curve editor and the colour grade.

use rune::{Color, Drag, El, Tx, col, el, row, text};
use vidcue_core::equalizer::{self, BANDS, MAX_GAIN};

use super::{chips, group, stacked, toggle};
use crate::app::{App, ToastKind};
use crate::theme::{Theme, ty};
use crate::ui::widgets;

const HEIGHT: f32 = 150.0;

/// Maps a pointer position on a band (0 at the top) to a gain in dB, in half-dB steps.
pub fn gain_from_y(y: f32, h: f32) -> f64 {
    let frac = (y / h.max(1.0)).clamp(0.0, 1.0) as f64;
    ((MAX_GAIN - frac * 2.0 * MAX_GAIN) * 2.0).round() / 2.0
}

/// "31", "125", "1k", "16k".
pub fn band_label(hz: u32) -> String {
    if hz >= 1000 {
        format!("{}k", hz / 1000)
    } else {
        hz.to_string()
    }
}

/// A custom preset name: lower case letters, digits and dashes, at most 24 characters.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if (c == ' ' || c == '-' || c == '_') && !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(24).collect()
}

fn current(app: &App) -> [f64; 10] {
    app.settings.eq_draft.unwrap_or_else(|| {
        equalizer::gains(&app.config.equalizer, &app.config.equalizer_custom).unwrap_or([0.0; 10])
    })
}

fn band(t: &Theme, i: usize, gain: f64) -> El {
    let frac = ((MAX_GAIN - gain) / (2.0 * MAX_GAIN)) as f32;
    let y = frac.clamp(0.0, 1.0) * HEIGHT;
    let mid = HEIGHT / 2.0;
    let (top, len) = if y < mid {
        (y, mid - y)
    } else {
        (mid, y - mid)
    };
    let area = el()
        .key(format!("st-eq-{i}"))
        .w(40.0)
        .h(HEIGHT)
        .role("slider")
        .label(format!("{} Hz", BANDS[i]))
        .range_value(gain, -MAX_GAIN, MAX_GAIN)
        .cursor(rune::Cursor::Pointer)
        .child(
            el().absolute(18.5, 0.0)
                .size(3.0, HEIGHT)
                .radius(1.5)
                .fill(t.overlay),
        )
        .child(el().absolute(14.0, mid - 0.5).size(12.0, 1.0).fill(t.faint))
        .child(
            el().absolute(18.5, top)
                .size(3.0, len.max(1.0))
                .radius(1.5)
                .fill(t.accent),
        )
        .child(
            el().absolute(12.0, y - 8.0)
                .size(16.0, 16.0)
                .radius(8.0)
                .fill(Color::WHITE)
                .shadow(1.0, 4.0, Color::BLACK.alpha(0.35)),
        )
        .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| {
            tx.instant();
            let mut g = current(s);
            g[i] = if d.clicks >= 2 {
                0.0
            } else {
                gain_from_y(d.y, d.h)
            };
            s.settings.eq_draft = Some(g);
        });
    col()
        .w(40.0)
        .gap(6.0)
        .center()
        .child(text(format!("{:+.1}", gain)).font(ty::NUM).color(t.muted))
        .child(area)
        .child(text(band_label(BANDS[i])).font(ty::SMALL).color(t.faint))
}

pub fn view(app: &App) -> El {
    let t = &app.theme;
    let c = &app.config;
    let names = equalizer::names(&c.equalizer_custom);
    let presets: Vec<(String, String)> = names
        .iter()
        .map(|n| {
            (
                if n == equalizer::OFF {
                    "Off".to_owned()
                } else {
                    crate::themes::label(n)
                },
                n.clone(),
            )
        })
        .collect();
    let selected = if app.settings.eq_draft.is_some() {
        ""
    } else {
        c.equalizer.as_str()
    };
    let gains = current(app);
    let mut bands = row().gap(8.0).justify_center().w_full();
    for (i, g) in gains.iter().enumerate() {
        bands = bands.child(band(t, i, *g));
    }
    let editing = app.settings.eq_draft.is_some();
    let custom_selected = !equalizer::is_builtin(&c.equalizer);
    let mut actions = row().w_full().gap(10.0).center_y().child(
        widgets::field(
            t,
            &app.settings.eq_name,
            if custom_selected && !editing {
                c.equalizer.as_str()
            } else {
                "Name this curve, e.g. warm-bass"
            },
            |s| &mut s.settings.eq_name,
        )
        .grow(1.0)
        .min_w(0.0),
    );
    actions = actions
        .child(widgets::button(t, "Save curve", None, widgets::Kind::Primary).on_press(save_curve))
        .child(
            widgets::button(t, "Flat", None, widgets::Kind::Secondary)
                .on_press(|s: &mut App| s.settings.eq_draft = Some([0.0; 10])),
        );
    if custom_selected {
        actions = actions.child(
            widgets::button(
                t,
                "Delete",
                Some(&crate::icons::TRASH),
                widgets::Kind::Danger,
            )
            .on_press(|s: &mut App| {
                let name = s.config.equalizer.clone();
                s.settings.eq_draft = None;
                s.edit_config(|c| {
                    c.equalizer_custom.remove(&name);
                    c.equalizer = equalizer::OFF.into();
                });
            }),
        );
    }
    let editor = col().w_full().gap(16.0).child(bands).child(actions);
    let grades: Vec<(String, String)> = vidcue_core::COLOR_GRADES
        .iter()
        .map(|g| (crate::themes::label(g), (*g).to_owned()))
        .collect();
    col()
        .w_full()
        .gap(24.0)
        .child(group(
            t,
            "Equalizer",
            vec![
                stacked(
                    t,
                    "Preset",
                    "Shapes the sound of everything Vidcue plays.",
                    chips(t, "st-eq-preset", &presets, selected, |s, v| {
                        s.settings.eq_draft = None;
                        s.edit_config(|c| c.equalizer = v);
                    })
                    .justify_start(),
                ),
                stacked(
                    t,
                    "Curve",
                    "Drag a band, double-click to zero it, then save the curve under a name.",
                    editor,
                ),
            ],
        ))
        .child(group(
            t,
            "Picture",
            vec![
                stacked(
                    t,
                    "Colour grade",
                    "A look applied to video playback.",
                    chips(t, "st-grade", &grades, &c.color_grade, |s, v| {
                        s.edit_config(|c| c.color_grade = v)
                    })
                    .justify_start(),
                ),
                toggle(
                    t,
                    "st-autograde",
                    "Automatic colour correction",
                    "Saved for the engine, but not applied by the embedded player yet.",
                    c.auto_color_grade,
                    |s| s.edit_config(|c| c.auto_color_grade = !c.auto_color_grade),
                ),
            ],
        ))
}

fn save_curve(s: &mut App) {
    let typed = slug(&s.settings.eq_name.text);
    let name = if !typed.is_empty() {
        typed
    } else if !equalizer::is_builtin(&s.config.equalizer) {
        s.config.equalizer.clone()
    } else {
        s.toast(ToastKind::Error, "Give the curve a name first.");
        return;
    };
    let curve: Vec<f64> = current(s).iter().map(|g| (g * 2.0).round() / 2.0).collect();
    s.settings.eq_draft = None;
    s.settings.eq_name.clear();
    s.edit_config(|c| {
        c.equalizer_custom.insert(name.clone(), curve);
        c.equalizer = name;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_follows_the_pointer_in_half_db_steps() {
        assert_eq!(gain_from_y(0.0, 150.0), MAX_GAIN);
        assert_eq!(gain_from_y(150.0, 150.0), -MAX_GAIN);
        assert_eq!(gain_from_y(75.0, 150.0), 0.0);
        assert_eq!(gain_from_y(-20.0, 150.0), MAX_GAIN);
        assert_eq!(
            gain_from_y(40.0, 150.0) * 2.0,
            (gain_from_y(40.0, 150.0) * 2.0).round()
        );
    }

    #[test]
    fn band_labels_use_k() {
        assert_eq!(band_label(31), "31");
        assert_eq!(band_label(1000), "1k");
        assert_eq!(band_label(16000), "16k");
    }

    #[test]
    fn slug_makes_engine_valid_names() {
        assert_eq!(slug("Warm Bass!"), "warm-bass");
        assert_eq!(slug("  --My_curve-- "), "my-curve");
        assert_eq!(slug("x".repeat(40).as_str()).len(), 24);
        assert_eq!(slug("???"), "");
        let mut custom = std::collections::BTreeMap::new();
        custom.insert(slug("Warm Bass"), vec![0.0; 10]);
        assert!(equalizer::validate("warm-bass", &custom).is_ok());
    }
}
