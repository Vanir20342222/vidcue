//! Vidcue palettes (`vidcue_core::theme`) mapped onto the desktop design tokens.
//!
//! A palette has nine roles plus a light flag. Everything else the interface needs (raised
//! surfaces, hover and press tints, glass, scrims, contrast-safe text on the accent) is derived
//! in OKLab, so every built-in, imported Caelestia/Omarchy or custom palette yields a coherent
//! theme. Colour changes animate: Rune springs colours in OKLab, so switching themes crossfades.

use rune::{Color, TextStyle};
use vidcue_core::theme::Palette;

pub fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    rune::color::hex(&format!("#{r:02x}{g:02x}{b:02x}")).unwrap_or(Color::BLACK)
}

/// Moves a colour's OKLab lightness by `dl` (−1..1), keeping hue and chroma.
pub fn shift(c: Color, dl: f32) -> Color {
    let [l, a, b, alpha] = c.to_oklab();
    Color::from_oklab([(l + dl).clamp(0.0, 1.0), a, b, alpha])
}

fn lightness(c: Color) -> f32 {
    c.to_oklab()[0]
}

/// Black or white, whichever reads better on `c`.
pub fn ink_on(c: Color) -> Color {
    let dark = Color::from_oklab([0.16, 0.0, 0.0, 1.0]);
    let contrast = |a: Color, b: Color| {
        let (x, y) = (a.luminance() + 0.05, b.luminance() + 0.05);
        x.max(y) / x.min(y)
    };
    if contrast(c, dark) >= contrast(c, Color::WHITE) {
        dark
    } else {
        Color::WHITE
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub dark: bool,
    /// Nothing palettes draw dot-matrix numerals and meters.
    pub dot_matrix: bool,
    pub bg: Color,
    /// The sidebar and other chrome a step below the page.
    pub chrome: Color,
    pub surface: Color,
    pub raised: Color,
    pub overlay: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub on_accent: Color,
    pub error: Color,
    pub warning: Color,
    pub success: Color,
    pub border: Color,
    pub hairline: Color,
    pub hover: Color,
    pub scrim: Color,
    /// Frosted glass tint drawn over a backdrop blur.
    pub glass: Color,
    pub shadow: Color,
    /// The palette as authored, for effects that take raw colours.
    pub palette: Palette,
}

impl Theme {
    pub fn from_palette(p: &Palette) -> Self {
        let dark = !p.light;
        let bg = rgb(p.background);
        let surface = rgb(p.surface);
        let text = rgb(p.text);
        let muted = rgb(p.muted);
        let accent = rgb(p.accent);
        let lift = if dark { 1.0 } else { -1.0 };
        // Surfaces step away from the page in lightness; dark themes lift, light themes sink.
        let surface = if (lightness(surface) - lightness(bg)).abs() < 0.015 {
            shift(bg, 0.04 * lift)
        } else {
            surface
        };
        let raised = shift(surface, 0.035 * lift);
        let overlay = shift(surface, 0.06 * lift);
        let chrome = if dark {
            shift(bg, -0.015)
        } else {
            shift(bg, -0.02)
        };
        let faint = muted.mix(bg, 0.35);
        Self {
            dark,
            dot_matrix: p.dot_matrix(),
            bg,
            chrome,
            surface,
            raised,
            overlay,
            text,
            muted,
            faint,
            accent,
            accent_soft: accent.alpha(if dark { 0.18 } else { 0.14 }),
            on_accent: ink_on(accent),
            error: rgb(p.error),
            warning: rgb(p.warning),
            success: rgb(p.success),
            border: text.alpha(if dark { 0.12 } else { 0.14 }),
            hairline: text.alpha(if dark { 0.07 } else { 0.09 }),
            hover: text.alpha(if dark { 0.07 } else { 0.06 }),
            scrim: Color::BLACK.alpha(if dark { 0.55 } else { 0.35 }),
            glass: if dark { bg.alpha(0.62) } else { bg.alpha(0.7) },
            shadow: Color::BLACK.alpha(if dark { 0.45 } else { 0.18 }),
            palette: p.clone(),
        }
    }

    pub fn fallback() -> Self {
        Self::from_palette(
            &vidcue_core::theme::builtin("vidcue-dark").expect("vidcue-dark is built in"),
        )
    }

    /// The rune-kit token set for kit controls (text fields, switches, menus).
    pub fn kit(&self) -> rune_kit::Kit {
        let mut k = rune_kit::Kit::from_palette(rune_kit::PaletteSeed {
            bg: self.bg,
            fg: self.text,
            accent: self.accent,
        });
        k.radius.sm = 8.0;
        k
    }

    /// Colours an effect layer draws in, as packed sRGB.
    pub fn fx_colors(&self) -> [u32; 6] {
        let pack = |(r, g, b): (u8, u8, u8)| (r as u32) << 16 | (g as u32) << 8 | b as u32;
        let p = &self.palette;
        [
            pack(p.accent),
            pack(p.selection),
            pack(p.success),
            pack(p.warning),
            pack(p.error),
            pack(p.surface),
        ]
    }
}

/// Type scale: Inter throughout, tabular figures where numbers change.
pub mod ty {
    use super::TextStyle;
    pub const DISPLAY: TextStyle = TextStyle::new(40.0, 760.0).lh(46.0).tracking(-0.6);
    pub const H1: TextStyle = TextStyle::new(28.0, 720.0).lh(34.0).tracking(-0.3);
    pub const H2: TextStyle = TextStyle::new(20.0, 680.0).lh(26.0).tracking(-0.15);
    pub const H3: TextStyle = TextStyle::new(16.0, 640.0).lh(21.0);
    pub const TITLE: TextStyle = TextStyle::new(14.5, 600.0).lh(19.0);
    pub const BODY: TextStyle = TextStyle::new(14.0, 420.0).lh(20.0);
    pub const BODY_STRONG: TextStyle = TextStyle::new(14.0, 560.0).lh(20.0);
    pub const SMALL: TextStyle = TextStyle::new(12.5, 450.0).lh(17.0);
    pub const SMALL_STRONG: TextStyle = TextStyle::new(12.5, 600.0).lh(17.0);
    pub const CAPTION: TextStyle = TextStyle::new(11.0, 600.0).lh(14.0).tracking(0.5);
    pub const NUM: TextStyle = TextStyle::new(12.5, 520.0).lh(16.0).features("'tnum' 1");
    pub const NUM_BIG: TextStyle = TextStyle::new(30.0, 700.0).lh(34.0).features("'tnum' 1");
    pub const BUTTON: TextStyle = TextStyle::new(13.5, 600.0).lh(18.0);
}

/// Corner radii and spacing used across screens.
pub mod sz {
    pub const R_CARD: f32 = 14.0;
    pub const R_PILL: f32 = 999.0;
    pub const R_PANEL: f32 = 18.0;
    pub const SIDEBAR: f32 = 232.0;
    pub const SIDEBAR_COMPACT: f32 = 76.0;
    pub const TOPBAR: f32 = 64.0;
    pub const DOCK: f32 = 84.0;
    pub const PAGE_PAD: f32 = 32.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_palette_derives_readable_accent_ink() {
        for name in vidcue_core::theme::THEMES
            .iter()
            .filter(|n| **n != "system" && **n != "terminal")
        {
            let t = Theme::from_palette(&vidcue_core::theme::builtin(name).unwrap());
            let (a, b) = (t.accent.luminance() + 0.05, t.on_accent.luminance() + 0.05);
            let contrast = a.max(b) / a.min(b);
            assert!(contrast >= 3.0, "{name}: accent ink contrast {contrast}");
            assert_eq!(t.dark, !t.palette.light);
        }
    }

    #[test]
    fn surfaces_step_away_from_the_page() {
        let t = Theme::from_palette(&vidcue_core::theme::builtin("nothing-dark").unwrap());
        assert!(lightness(t.raised) > lightness(t.bg));
        let l = Theme::from_palette(&vidcue_core::theme::builtin("flexoki-light").unwrap());
        assert!(lightness(l.raised) < lightness(l.surface) + 0.001);
    }
}
