//! Muted previews: rest the pointer on a video card and it starts playing in place.
//!
//! A second, small mpv instance (created on first use) plays a low-quality stream from a
//! fifth of the way in, for twenty seconds. Previews pause while the main player plays sound
//! and never record history or positions. The ⋮ menu's Preview starts one without hovering.

use std::time::Duration;

use rune::{NativeSurface, Tx, Waker};
use serde_json::json;
use vidcue_core::{Media, MediaKind};

use super::mpv::{Load, Mpv};
use crate::app::App;
use crate::backend::op;

const DWELL: f64 = 0.85;
const LENGTH: f64 = 20.0;

#[derive(Default)]
pub struct Preview {
    mpv: Option<Mpv>,
    surface: NativeSurface,
    waker: Option<Waker>,
    /// The card under the pointer and when it got there.
    hover: Option<(String, f64)>,
    /// The URL playing (or resolving) in the preview player.
    pub url: Option<String>,
    showing: bool,
    token: u64,
}

pub fn setup(app: &mut App, waker: Waker) {
    app.player_preview_mut().waker = Some(waker);
}

impl App {
    pub fn player_preview_mut(&mut self) -> &mut Preview {
        &mut self.preview
    }
}

/// The surface to draw on a card, while its preview has frames.
pub fn surface_for<'a>(app: &'a App, url: &str) -> Option<&'a NativeSurface> {
    let p = &app.preview;
    (p.showing && p.url.as_deref() == Some(url)).then_some(&p.surface)
}

pub fn hover(app: &mut App, media: &Media, inside: bool, tx: &mut Tx) {
    if !app.prefs.hover_previews || app.offline() || media.kind != MediaKind::Video || media.live {
        return;
    }
    if inside {
        app.preview.hover = Some((media.url.clone(), app.clock));
        let url = media.url.clone();
        let media = media.clone();
        let _ = url;
        tx.wake_after(Duration::from_secs_f64(DWELL));
        app.scratch.insert(
            "preview-candidate".into(),
            serde_json::to_string(&media).unwrap_or_default(),
        );
    } else if app
        .preview
        .hover
        .as_ref()
        .is_some_and(|(u, _)| *u == media.url)
    {
        app.preview.hover = None;
        stop(app);
    }
}

/// Runs each poll: starts a preview once the pointer rested long enough.
pub fn tick(app: &mut App) -> bool {
    let Some((url, since)) = app.preview.hover.clone() else {
        return false;
    };
    if app.clock - since < DWELL || app.preview.url.as_deref() == Some(url.as_str()) {
        let frames = app.preview.mpv.as_ref().map(|m| m.frames()).unwrap_or(0);
        let showing = app.preview.url.is_some() && frames > 0;
        if showing != app.preview.showing {
            app.preview.showing = showing;
            return true;
        }
        return false;
    }
    let Some(media) = app
        .scratch
        .get("preview-candidate")
        .and_then(|s| serde_json::from_str::<Media>(s).ok())
        .filter(|m| m.url == url)
    else {
        return false;
    };
    start(app, &media);
    false
}

pub fn start(app: &mut App, media: &Media) {
    if app.offline() {
        return;
    }
    if app.player.playing() && app.player.now.as_ref().is_some_and(|n| !n.audio) {
        // Never compete with a playing video.
        return;
    }
    let p = &mut app.preview;
    if p.mpv.is_none() {
        let Ok(mpv) = Mpv::new(p.surface.clone()) else {
            return;
        };
        if let Some(w) = &p.waker {
            mpv.set_waker(w.clone());
        }
        mpv.set_target(640.0, 360.0);
        mpv.set("hwdec", "auto-copy-safe");
        mpv.set("aid", "no");
        p.mpv = Some(mpv);
    }
    p.token += 1;
    let token = p.token;
    p.url = Some(media.url.clone());
    p.showing = false;
    let req = json!({ "media": media, "audio": false, "quality": "360", "speculative": true });
    app.be.call(op("resolve", req), move |app, r, _| {
        let p = &mut app.preview;
        if p.token != token {
            return;
        }
        let Ok(v) = r else {
            p.url = None;
            return;
        };
        let Some(url) = v["url"].as_str() else { return };
        let duration = v["duration"].as_f64().unwrap_or(0.0);
        let start = if duration > 60.0 { duration * 0.2 } else { 0.0 };
        let load = Load {
            url: url.to_owned(),
            audio_url: None,
            headers: v["headers"]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                .collect(),
            start: Some(start),
            end: Some(start + LENGTH),
            muted: true,
            ..Default::default()
        };
        if let Some(m) = &p.mpv {
            m.load(&load);
        }
    });
}

pub fn stop(app: &mut App) {
    let p = &mut app.preview;
    p.token += 1;
    if p.url.take().is_some()
        && let Some(m) = &p.mpv
    {
        m.stop();
    }
    p.showing = false;
}
