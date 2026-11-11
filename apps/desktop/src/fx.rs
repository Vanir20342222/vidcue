//! GPU effects: every `El::effect(mark)` slot becomes a rune-fx layer, coloured from the
//! active palette. Effects follow reduced motion (they settle on one pose) and the window only
//! keeps drawing while an animated layer is on screen.

use rune::compose::{Compositor, Frame};
use rune::render::{EffectPhase, EffectSite, KIND_EFFECT};
use rune::{El, effect};
use rune_fx::*;

use crate::app::App;

pub const MESH: u32 = 1;
pub const AURORA: u32 = 2;
pub const GLOW: u32 = 3;
pub const PULSE: u32 = 4;
pub const SPARKLES: u32 = 5;
pub const SHIMMER: u32 = 6;
pub const GRAIN: u32 = 7;
pub const METAL: u32 = 8;
pub const LENS: u32 = 9;
pub const GRAIN_GRADIENT: u32 = 10;

/// A mesh gradient in the palette's colours. `seed` varies the arrangement, `speed` its drift.
pub fn mesh(seed: u32, speed: f32) -> El {
    effect(MESH).effect_data([0.0, speed, seed as f32, 0.0])
}

/// A mesh gradient from an explicit preset (`1` lagoon, `2` orchid, `3` graphite, `4` dusk).
pub fn mesh_preset(preset: u32, seed: u32, speed: f32) -> El {
    effect(MESH).effect_data([preset as f32, speed, seed as f32, 0.0])
}

pub fn aurora(intensity: f32, speed: f32) -> El {
    effect(AURORA).effect_data([intensity, speed, 0.0, 0.0])
}

/// A soft or neon glow around the element's shape, in the accent (0) or another role.
pub fn glow(style: u32, color: u32, intensity: f32, radius: f32) -> El {
    effect(GLOW)
        .effect_data([style as f32, color as f32, intensity, radius])
        .effect_bleed(radius * 2.5)
}

/// A radar or breathing pulse (live badges, on-air radio).
pub fn pulse(style: u32, color: u32, period: f32, reach: f32) -> El {
    effect(PULSE)
        .effect_data([style as f32, color as f32, period, reach])
        .effect_bleed(reach)
}

/// A one-shot burst of sparkles that started at `t0` on the app clock.
pub fn sparkles(t0: f64, count: u32) -> El {
    effect(SPARKLES)
        .effect_data([t0 as f32, count as f32, 3.0, 0.0])
        .effect_bleed(28.0)
}

pub fn grain(amount: f32) -> El {
    effect(GRAIN).effect_data([amount, 24.0, 0.0, 0.0])
}

/// Remembers a sparkle burst for the element keyed `key`.
pub fn burst(app: &mut App, key: &str) {
    app.bursts.insert(key.to_owned(), app.clock);
}

/// The burst playing on `key`, if it started in the last second and a half.
pub fn burst_at(app: &App, key: &str) -> Option<f64> {
    app.bursts
        .get(key)
        .copied()
        .filter(|t0| app.clock - t0 < 1.5)
}

fn role(colors: &[u32; 6], i: f32) -> Rgb {
    rgb(colors[(i.max(0.0) as usize).min(colors.len() - 1)])
}

fn layer(
    kind: u32,
    d: [f32; 4],
    shape: Shape,
    t: f32,
    colors: &[u32; 6],
    dark: bool,
) -> Option<FxLayer> {
    let fx = match kind {
        MESH => {
            let mut p = match d[0] as u32 {
                1 => MeshGradientParams::lagoon(),
                2 => MeshGradientParams::orchid(),
                3 => MeshGradientParams::graphite(),
                4 => MeshGradientParams::dusk(),
                _ => {
                    // The palette: background, accent, selection, success/warning hints.
                    let bg = if dark { 0x0b0c10 } else { 0xf4f5f7 };
                    MeshGradientParams {
                        colors: Stops::hex(&[
                            bg,
                            colors[0],
                            colors[1],
                            colors[5],
                            colors[0] ^ 0x101010,
                        ]),
                        ..MeshGradientParams::dusk()
                    }
                }
            };
            if d[1] > 0.0 {
                p.speed = d[1];
            }
            return Some(Fx::fill(p).on(shape).at(t).seed(d[2] as u32 + 1));
        }
        AURORA => Fx::aurora(AuroraParams {
            intensity: d[0],
            speed: d[1],
            ..Default::default()
        }),
        GLOW => {
            let style = match d[0] as u32 {
                1 => GlowStyle::Neon,
                2 => GlowStyle::Aura,
                _ => GlowStyle::Soft,
            };
            Fx::glow(GlowParams {
                style,
                color: role(colors, d[1]),
                intensity: d[2],
                radius: if d[3] > 0.0 { d[3] } else { 14.0 },
                aura_speed: 0.3,
                ..Default::default()
            })
        }
        PULSE => {
            let style = match d[0] as u32 {
                1 => PulseStyle::Breathe,
                2 => PulseStyle::Heartbeat,
                _ => PulseStyle::Radar,
            };
            Fx::pulse(PulseParams {
                style,
                color: role(colors, d[1]),
                reach: d[3],
                period: if d[2] > 0.0 { d[2] } else { 2.4 },
                ..Default::default()
            })
        }
        SPARKLES => Fx::sparkles(SparkleParams {
            burst: Some(d[0]),
            count: d[1] as u32,
            size: d[2],
            color: role(colors, d[3]),
            margin: 26.0,
            ..Default::default()
        }),
        SHIMMER => Fx::shimmer(ShimmerParams {
            intensity: d[0],
            period: d[1],
            band: d[2],
            ..Default::default()
        }),
        GRAIN => Fx::grain(GrainParams {
            amount: d[0],
            fps: d[1],
            ..Default::default()
        }),
        METAL => {
            let p = match d[0] as u32 {
                1 => LiquidMetalParams::gold(),
                2 => LiquidMetalParams::rose(),
                3 => LiquidMetalParams::gunmetal(),
                _ => LiquidMetalParams::chrome(),
            };
            let p = LiquidMetalParams {
                flow: p.flow * d[2].max(0.1),
                ..p
            };
            Fx::fill(if d[3] > 0.5 {
                p.anchor(FillAnchor::Element)
            } else {
                p
            })
        }
        LENS => Fx::lens(LensParams {
            frost: d[3],
            bevel: 14.0,
            depth: 16.0,
            dispersion: 0.12,
            magnification: 1.03,
            ..Default::default()
        }),
        GRAIN_GRADIENT => Fx::fill(GrainGradientParams {
            kind: GradientKind::Radial,
            colors: Stops::hex(&[colors[0] & 0x3f3f3f, if dark { 0x08080a } else { 0xe8e8ec }]),
            amount: 0.1,
            ..GrainGradientParams::default()
        }),
        _ => return None,
    };
    Some(fx.on(shape).at(t))
}

#[derive(Default)]
pub struct Gfx {
    fx: Option<FxRenderer>,
    animating: bool,
}

impl Compositor<App> for Gfx {
    fn animating(&self) -> bool {
        self.animating
    }

    fn render(&mut self, f: &mut Frame<'_, App>) {
        let (device, queue, size) = (f.device, f.queue, f.size);
        let t = f.rt.t as f32;
        let colors = f.rt.state.theme.fx_colors();
        let dark = f.rt.state.theme.dark;
        let reduced = f.rt.state.config.reduced_motion || !f.rt.state.prefs.effects;
        let marks = &f.rt.marks;
        let mut at: Vec<u32> = Vec::new();
        let mut layers: Vec<FxLayer> = Vec::new();
        let mut placements: Vec<FxSlot> = Vec::new();
        for (n, i) in f.rt.instances.iter().enumerate() {
            if i.params[1] != KIND_EFFECT {
                continue;
            }
            let kind = marks.get(n).copied().unwrap_or(0) & 0xff;
            let k = i.params[3];
            let [x, y, w, h] = i.rect.map(|v| v / k);
            let r = i.radii.iter().fold(0.0f32, |a, v| a.max(v.abs())) / k;
            let Some(l) = layer(kind, i.uv, Shape::rounded(x, y, w, h, r), t, &colors, dark) else {
                continue;
            };
            at.push(n as u32);
            // Decorative layers hold one pose so an idle window can rest: grain and metal always,
            // meshes given no speed. Only hero moments and playback feedback keep moving.
            let still = matches!(kind, GRAIN | METAL) || (kind == MESH && i.uv[1] <= 0.0);
            layers.push(l.reduced(reduced || still));
            placements.push(FxSlot::default().scale(k).clip(i.clip, i.fade));
        }
        self.animating = !reduced && layers.iter().any(FxLayer::is_animating);
        if layers.is_empty() {
            f.draw_ui();
            return;
        }
        let fx = self
            .fx
            .get_or_insert_with(|| FxRenderer::new(device, f.format, FxOptions { dither: true }));
        fx.set_srgb_blend(f.srgb_blend());
        fx.add_format(device, wgpu::TextureFormat::Rgba16Float);
        fx.prepare_with_slots(
            device,
            queue,
            &FxFrame {
                viewport: [size.0, size.1],
                scale: 1.0,
                layers: &layers,
                backdrop: None,
                sources: &[],
            },
            &placements,
        );
        let clear = f.rt.clear_color();
        f.ui.prepare(device, queue, &mut f.rt.text.atlas, &f.rt.instances, size);
        f.ui.draw_with_backdrop(
            f.encoder,
            f.target,
            clear,
            &mut |phase, s: &EffectSite<'_>| {
                let Some(k) = at.iter().position(|n| *n == s.index) else {
                    return;
                };
                match phase {
                    EffectPhase::Backdrop(enc) => {
                        if let Some(bd) = s.backdrop {
                            fx.blur_backdrop(device, queue, enc, bd, k);
                        }
                    }
                    EffectPhase::Draw(pass) => fx.draw_layer_in(pass, s.format, s.origin, k),
                }
            },
        );
    }
}
