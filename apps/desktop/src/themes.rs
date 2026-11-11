//! Applies the configured theme: a built-in palette, the imported custom palette, or — for
//! `system` — whatever the desktop uses (Caelestia, Omarchy, the XDG portal), followed live.

use std::sync::{Arc, Mutex};

use serde_json::json;
use tokio_util::sync::CancellationToken;
use vidcue_core::theme::Palette;

use crate::app::App;
use crate::backend::op;
use crate::theme::Theme;

static WATCH: Mutex<Option<CancellationToken>> = Mutex::new(None);

fn set(app: &mut App, p: &Palette) {
    let t = Theme::from_palette(p);
    if t.palette != app.theme.palette {
        app.theme = t;
    }
}

pub fn reload(app: &mut App) {
    if let Some(c) = WATCH.lock().unwrap().take() {
        c.cancel();
    }
    let name = app.config.theme.clone();
    match name.as_str() {
        "system" if !app.offline() => {
            let cancel = CancellationToken::new();
            *WATCH.lock().unwrap() = Some(cancel.clone());
            let config = Arc::new(app.config.clone());
            let paths = app.be.paths.shared.clone();
            let be = app.be.clone();
            app.be.rt.spawn(async move {
                if let Ok(p) = vidcue_services::theme::resolve(&config, &paths).await {
                    be.land(Box::new(move |app, _| set(app, &p)));
                }
                let mut rx = vidcue_services::theme::watch(config, paths, cancel.clone());
                while !cancel.is_cancelled() && rx.changed().await.is_ok() {
                    let p = rx.borrow().clone();
                    be.land(Box::new(move |app, _| set(app, &p)));
                }
            });
        }
        "custom" => {
            app.be
                .call(op("theme", json!({ "dark": true })), |app, r, _| {
                    if let Some(p) = r
                        .ok()
                        .and_then(|v| serde_json::from_value::<Palette>(v).ok())
                    {
                        set(app, &p);
                    }
                });
        }
        other => {
            let p = vidcue_core::theme::builtin(if other == "system" {
                "vidcue-dark"
            } else {
                other
            })
            .or_else(|_| vidcue_core::theme::builtin("vidcue-dark"));
            if let Ok(p) = p {
                set(app, &p);
            }
        }
    }
}

/// Every pickable theme, for the Appearance grid: `system` first, as the engine lists them.
pub fn pickable() -> Vec<(&'static str, Option<Palette>)> {
    vidcue_core::theme::THEMES
        .iter()
        .filter(|n| **n != "terminal")
        .map(|n| {
            (
                *n,
                if *n == "system" {
                    None
                } else {
                    vidcue_core::theme::builtin(n).ok()
                },
            )
        })
        .collect()
}

pub fn label(name: &str) -> String {
    match name {
        "system" => "Follow desktop".into(),
        "custom" => "Imported palette".into(),
        n => n
            .split('-')
            .map(|w| {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}
