//! Lyrics for the playing item: the local library first, then YouTube Music by video id, then
//! LRCLIB by title and artist (`lyrics_auto`); neither request carries the account. Timed
//! lines follow playback; a per-song offset and imported LRC text are saved locally.

use serde_json::{Value, json};
use vidcue_providers::lyrics::Track;

use crate::app::{App, ToastKind};
use crate::backend::op;

#[derive(Default)]
pub struct Lyrics {
    pub url: String,
    pub loading: bool,
    pub tracks: Vec<Track>,
    pub chosen: usize,
    pub error: Option<String>,
    pub active: Option<usize>,
    pub offset_ms: i64,
    /// True when the shown track is the user's own saved copy.
    pub local: bool,
    pub import: rune::TextEdit,
    pub importing: bool,
}

impl Lyrics {
    pub fn reset(&mut self) {
        *self = Self {
            import: std::mem::take(&mut self.import),
            ..Default::default()
        };
    }
    pub fn track(&self) -> Option<&Track> {
        self.tracks.get(self.chosen)
    }
    pub fn visible(&self) -> bool {
        self.track().is_some_and(|t| t.synced())
    }
}

fn track_from(v: &Value) -> Option<Track> {
    serde_json::from_value(v.clone()).ok()
}

/// Loads lyrics for the playing item (once per item unless `force`).
pub fn load(app: &mut App, force: bool) {
    let Some(now) = &app.player.now else { return };
    let media = now.media.clone();
    let l = &mut app.player.lyrics;
    if !force && (l.url == media.url && (l.loading || !l.tracks.is_empty())) {
        return;
    }
    l.url = media.url.clone();
    l.loading = true;
    l.error = None;
    let url = media.url.clone();
    app.be
        .call(op("lyrics_get", json!({ "url": url })), move |app, r, _| {
            if app.player.lyrics.url != url {
                return;
            }
            if let Ok(v) = &r
                && let Some(track) = track_from(&v["track"])
            {
                let l = &mut app.player.lyrics;
                l.loading = false;
                l.tracks = vec![track];
                l.chosen = 0;
                l.local = true;
                l.offset_ms = v["document"]["offset_ms"].as_i64().unwrap_or(0);
                return;
            }
            let url2 = url.clone();
            app.be.call(
                op("lyrics_auto", json!({ "media": media })),
                move |app, r, _| {
                    let l = &mut app.player.lyrics;
                    if l.url != url2 {
                        return;
                    }
                    l.loading = false;
                    match r {
                        Ok(Value::Array(list)) => {
                            l.tracks = list.iter().filter_map(track_from).collect();
                            l.chosen = l.tracks.iter().position(Track::synced).unwrap_or(0);
                            if l.tracks.is_empty() {
                                l.error = Some("No lyrics were found for this song.".into());
                            }
                        }
                        Ok(_) => l.error = Some("No lyrics were found for this song.".into()),
                        Err(e) => l.error = Some(e.message),
                    }
                },
            );
        });
}

/// Searches LRCLIB with an edited title and artist.
pub fn search(app: &mut App, title: &str, artist: &str) {
    let Some(now) = &app.player.now else { return };
    let media = now.media.clone();
    app.player.lyrics.loading = true;
    app.be.call(
        op(
            "lyrics",
            json!({ "media": media, "title": title, "artist": artist }),
        ),
        |app, r, _| {
            let l = &mut app.player.lyrics;
            l.loading = false;
            match r {
                Ok(Value::Array(list)) => {
                    l.tracks = list.iter().filter_map(track_from).collect();
                    l.chosen = l.tracks.iter().position(Track::synced).unwrap_or(0);
                    l.local = false;
                    l.error = l
                        .tracks
                        .is_empty()
                        .then(|| "LRCLIB had nothing for that title and artist.".into());
                }
                Ok(_) => l.error = Some("LRCLIB had nothing for that title and artist.".into()),
                Err(e) => l.error = Some(e.message),
            }
        },
    );
}

/// LRC text for a track, so an auto-found track can be saved with an offset.
pub fn lrc(track: &Track) -> String {
    let mut out = String::new();
    for line in &track.lines {
        if let Some(ms) = line.at_ms {
            out.push_str(&format!(
                "[{:02}:{:02}.{:02}]",
                ms / 60_000,
                ms / 1000 % 60,
                ms % 1000 / 10
            ));
        }
        out.push_str(&line.text);
        out.push('\n');
    }
    out
}

/// Saves the shown lyrics (or imported LRC text) locally, with the current offset.
pub fn save(app: &mut App, text: Option<String>) {
    let Some(now) = &app.player.now else { return };
    let l = &app.player.lyrics;
    let Some(text) = text.or_else(|| l.track().map(lrc)) else {
        return;
    };
    let document = json!({
        "title": now.media.title,
        "artist": now.media.creator,
        "text": text,
        "offset_ms": l.offset_ms.clamp(-120_000, 120_000),
    });
    let url = now.media.url.clone();
    app.be.call(
        op("lyrics_save", json!({ "url": url, "document": document })),
        |app, r, _| match r {
            Ok(v) => {
                if let Some(t) = track_from(&v["track"]) {
                    let l = &mut app.player.lyrics;
                    l.tracks = vec![t];
                    l.chosen = 0;
                    l.local = true;
                    l.importing = false;
                    l.import.clear();
                }
                app.toast(ToastKind::Success, "Lyrics saved on this computer");
            }
            Err(e) => app.toast(ToastKind::Error, e.message),
        },
    );
}

pub fn remove(app: &mut App) {
    let url = app.player.lyrics.url.clone();
    app.be
        .call(op("lyrics_remove", json!({ "url": url })), |app, _, _| {
            app.player.lyrics.local = false;
            load(app, true);
        });
}

/// Follows playback; true when the highlighted line moved.
pub fn tick(app: &mut App) -> bool {
    let l = &app.player.lyrics;
    let Some(track) = l.track() else { return false };
    let t = app.player.snap.time + l.offset_ms as f64 / 1000.0;
    let active = track.active_line(t);
    if active != l.active {
        app.player.lyrics.active = active;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrc_round_trips_line_times() {
        let lines = vidcue_providers::lyrics::parse("[00:01.23]Hello\n[01:02.50]World").unwrap();
        let track = Track {
            title: "t".into(),
            artist: "a".into(),
            album: String::new(),
            duration: 0.0,
            instrumental: false,
            lines,
        };
        let text = lrc(&track);
        assert!(text.contains("[00:01.23]Hello"), "{text}");
        assert!(text.contains("[01:02.50]World"), "{text}");
    }
}
