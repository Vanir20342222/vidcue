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

/// `vidcue_services::theme::resolve` marks the palette it falls back to when the desktop
/// offers none (always on Windows) with a source starting with this.
const FALLBACK: &str = "Fallback";

/// Applies a palette resolved for "Follow desktop". Without a desktop palette, the system's
/// light/dark setting picks Vidcue Light or Vidcue Dark.
fn follow(app: &mut App, p: &Palette) {
    if p.source.starts_with(FALLBACK)
        && app.system_dark.get() == Some(false)
        && let Ok(mut light) = vidcue_core::theme::builtin("vidcue-light")
    {
        light.source = p.source.clone();
        return set(app, &light);
    }
    set(app, p);
}

/// Called by the view, the only place Rune reports the system appearance. A change re-applies
/// a fallback "Follow desktop" palette on the next poll; desktop palettes are left alone.
pub fn observe_system_dark(app: &App, dark: bool) {
    if app.system_dark.replace(Some(dark)) == Some(dark) {
        return;
    }
    app.be.land(Box::new(|app, _| {
        if app.config.theme == "system" && app.theme.palette.source.starts_with(FALLBACK) {
            let source = app.theme.palette.source.clone();
            if let Ok(mut dark) = vidcue_core::theme::builtin("vidcue-dark") {
                dark.source = source;
                follow(app, &dark);
            }
        }
    }));
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
                    be.land(Box::new(move |app, _| follow(app, &p)));
                }
                let mut rx = vidcue_services::theme::watch(config, paths, cancel.clone());
                while !cancel.is_cancelled() && rx.changed().await.is_ok() {
                    let p = rx.borrow().clone();
                    be.land(Box::new(move |app, _| follow(app, &p)));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fallback() -> Palette {
        let mut p = vidcue_core::theme::builtin("vidcue-dark").unwrap();
        p.source = "Fallback · no desktop palette available".into();
        p
    }

    #[test]
    fn follow_desktop_without_a_palette_matches_the_system_light_or_dark_setting() {
        let mut app = App::offline_for_tests();
        app.system_dark.set(Some(false));
        follow(&mut app, &fallback());
        let light = vidcue_core::theme::builtin("vidcue-light").unwrap();
        assert_eq!(app.theme.palette.name, light.name);
        assert!(app.theme.palette.source.starts_with(FALLBACK));

        app.system_dark.set(Some(true));
        follow(&mut app, &fallback());
        assert_eq!(app.theme.palette.name, fallback().name);
    }

    #[test]
    fn a_real_desktop_palette_is_never_replaced() {
        let mut app = App::offline_for_tests();
        app.system_dark.set(Some(false));
        let mut omarchy = vidcue_core::theme::builtin("gruvbox").unwrap();
        omarchy.source = "omarchy".into();
        follow(&mut app, &omarchy);
        assert_eq!(app.theme.palette, omarchy);
    }
}
