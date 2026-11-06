//! Render the "new version available" popup to a WQHD PNG for documentation.
//! Uses a fictional release; no network access. Usage:
//! `cargo run -p vidcue --example capture_update -- OUTPUT.png [FONT.ttf]`
use anyhow::{Context, Result};
use fontdue::{Font, FontSettings};
use image::{Rgb, RgbImage};
use ratatui::{Terminal, backend::TestBackend, style::Color};
use ratatui_image::picker::Picker;
use std::collections::HashMap;
use tokio::sync::mpsc;
use vidcue::{
    app::{App, RuntimeHandles, UiEvent},
    ui,
};
use vidcue_core::Config;
use vidcue_services::{paths::Paths, store::Snapshot, update::Release};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let output = args
        .next()
        .context("Usage: capture_update OUTPUT.png [FONT.ttf]")?;
    let font_path = args
        .next()
        .unwrap_or("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf".into());
    let font = Font::from_bytes(std::fs::read(font_path)?, FontSettings::default())
        .map_err(anyhow::Error::msg)?;
    let temp = tempfile::tempdir()?;
    let root = temp.path();
    let paths = Paths {
        config_override: None,
        xdg_state: root.join("xdg-state"),
        config: root.join("config"),
        state: root.join("state"),
        cache: root.join("cache"),
        legacy_data: root.join("legacy"),
        home: root.into(),
    };
    paths.create()?;
    let mut config = Config {
        theme: "vidcue-dark".into(),
        auth_store: "file".into(),
        notifications: false,
        ..Default::default()
    };
    paths.fill_directories(&mut config);
    let (player, _player_rx) = mpsc::channel(32);
    let (jobs, _jobs_rx) = mpsc::channel(32);
    let mut app = App::new(
        config,
        paths,
        Snapshot::default(),
        None,
        RuntimeHandles {
            player,
            jobs,
            picker: Picker::from_fontsize((8, 16)),
        },
        true,
        false,
    )?;
    app.apply(UiEvent::Update {
        manual: false,
        result: Ok(Some(Release {
            tag: "v0.3.0".into(),
            version: "0.3.0".into(),
            name: "Vidcue 0.3.0 · fictional example".into(),
            url: String::new(),
            notes: vec![
                "Brighter, more colourful themes".into(),
                "One-press updates from GitHub".into(),
                "Faster search and smoother playback".into(),
            ],
        })),
    });
    let (cols, rows) = (140u16, 39u16);
    let mut terminal = Terminal::new(TestBackend::new(cols, rows))?;
    terminal.draw(|f| ui::render(f, &mut app))?;
    let buffer = terminal.backend().buffer().clone();
    let (fg_default, bg_default) = (app.palette.text, app.palette.background);
    let cw = (2544 / u32::from(cols)).min(1392 / (2 * u32::from(rows)));
    let ch = cw * 2;
    let left = (2560 - u32::from(cols) * cw) / 2;
    let top = (1440 - u32::from(rows) * ch) / 2;
    let rgb = |c: Color, default: (u8, u8, u8)| match c {
        Color::Rgb(r, g, b) => [r, g, b],
        _ => [default.0, default.1, default.2],
    };
    let mut image = RgbImage::from_pixel(2560, 1440, Rgb(rgb(Color::Reset, bg_default)));
    let mut glyphs = HashMap::new();
    for row in 0..rows {
        for col in 0..cols {
            let cell = &buffer[(col, row)];
            let (fg, bg) = (rgb(cell.fg, fg_default), rgb(cell.bg, bg_default));
            let (x, y) = (left + u32::from(col) * cw, top + u32::from(row) * ch);
            for py in y..y + ch {
                for px in x..x + cw {
                    image.put_pixel(px, py, Rgb(bg));
                }
            }
            for character in cell.symbol().chars().filter(|c| !c.is_whitespace()) {
                let (metrics, bitmap) = glyphs
                    .entry(character)
                    .or_insert_with(|| font.rasterize(character, ch as f32 * 0.77));
                let gx = x as i32 + metrics.xmin;
                let gy =
                    y as i32 + (ch as f32 * 0.82) as i32 - metrics.height as i32 - metrics.ymin;
                for (offset, alpha) in bitmap.iter().enumerate() {
                    let px = gx + (offset % metrics.width) as i32;
                    let py = gy + (offset / metrics.width) as i32;
                    if *alpha == 0 || px < 0 || py < 0 || px >= 2560 || py >= 1440 {
                        continue;
                    }
                    let base = image.get_pixel(px as u32, py as u32).0;
                    let a = u32::from(*alpha);
                    let blend = std::array::from_fn(|i| {
                        ((u32::from(fg[i]) * a + u32::from(base[i]) * (255 - a)) / 255) as u8
                    });
                    image.put_pixel(px as u32, py as u32, Rgb(blend));
                }
            }
        }
    }
    image.save(&output)?;
    app.shutdown().await;
    println!("Saved {output}");
    Ok(())
}
