//! The lyrics side panel: timed lines that follow playback (click one to seek), a per-song
//! offset, candidate picker, LRCLIB search, LRC paste import and a local copy.

use rune::{Cx, El, ScrollSpec, Spring, Tx, col, el, icon, row, spacer, text};
use vidcue_providers::lyrics::Track;

use crate::app::App;
use crate::icons as ic;
use crate::player::session::Now;
use crate::theme::{sz, ty};
use crate::ui::widgets::{self, Kind};

const SCROLL: &str = "lyrics-scroll";
const CENTERED: &str = "lyrics-centered";

/// Panel height for a window height: tracks the music stage beside it.
pub fn panel_height(win_h: f32) -> f32 {
    (win_h - sz::TOPBAR - 160.0).clamp(420.0, 720.0)
}

/// Where a lyric source came from. Saved copies are local; otherwise tracks with a duration
/// come from LRCLIB and those without from YouTube Music.
pub fn source_label(local: bool, track: &Track) -> &'static str {
    if local {
        "On this computer"
    } else if track.duration > 0.0 {
        "LRCLIB"
    } else {
        "YouTube Music"
    }
}

/// Media time in seconds a line is shown at, given the per-song offset.
pub fn line_seek_seconds(at_ms: u64, offset_ms: i64) -> f64 {
    ((at_ms as i64 - offset_ms) as f64 / 1000.0).max(0.0)
}

pub fn offset_text(offset_ms: i64) -> String {
    format!("{:+.1}s", offset_ms as f64 / 1000.0)
}

/// How a line is drawn relative to the active one: (opacity, is_active).
pub fn line_tone(i: usize, active: Option<usize>) -> (f32, bool) {
    match active {
        Some(a) if a == i => (1.0, true),
        Some(a) if i < a => (0.42, false),
        Some(_) => (0.68, false),
        None => (0.68, false),
    }
}

pub fn view(app: &App, cx: &mut Cx<App>, w: f32) -> El {
    let t = &app.theme;
    let win_h = cx.layout_size("watch-page").map_or(800.0, |s| s[1]);
    let h = panel_height(win_h);
    let l = &app.player.lyrics;
    let Some(now) = &app.player.now else {
        return el();
    };

    let mut header = row()
        .w_full()
        .gap(10.0)
        .center_y()
        .child(text("Lyrics").font(ty::H2).color(t.text));
    if let Some(track) = l.track() {
        header = header.child(widgets::badge(
            source_label(l.local, track),
            t.accent_soft,
            t.accent,
        ));
    }
    header = header.child(spacer()).child(
        widgets::icon_button(t, &ic::CLOSE, "Close lyrics", 34.0).on_press(|s: &mut App| {
            s.scratch.remove(CENTERED);
            s.edit_prefs(|p| p.show_lyrics = false);
        }),
    );

    let body: El = if l.loading && l.tracks.is_empty() {
        col()
            .w_full()
            .grow(1.0)
            .center()
            .gap(12.0)
            .child(super::watch_controls::spinner(t.accent))
            .child(
                text("Looking for lyrics…")
                    .font(ty::SMALL_STRONG)
                    .color(t.muted),
            )
    } else if let Some(track) = l.track() {
        lines(app, track, h)
    } else {
        empty(app, l.error.as_deref())
    };

    let mut card = col()
        .key("lyrics-panel")
        .w(w)
        .h(h)
        .radius(sz::R_PANEL)
        .clip_rounded(sz::R_PANEL)
        .fill(t.glass)
        .border(1.0, t.hairline)
        .shadow(10.0, 36.0, t.shadow)
        .p_card()
        .gap(12.0)
        .child(header);
    if l.tracks.len() > 1 {
        card = card.child(picker(app));
    }
    card = card.child(body).child(toolbar(app, now));
    if l.importing {
        card = card.child(import_form(app));
    }
    card
}

trait Pad {
    fn p_card(self) -> Self;
}
impl Pad for El {
    fn p_card(self) -> Self {
        self.px(18.0).py(16.0)
    }
}

fn picker(app: &App) -> El {
    let l = &app.player.lyrics;
    let mut r = row().w_full().gap(6.0).center_y();
    for (i, tr) in l.tracks.iter().enumerate().take(6) {
        let label = format!(
            "{} {}",
            i + 1,
            if tr.synced() { "· Timed" } else { "· Text" }
        );
        r = r.child(
            widgets::chip(&app.theme, &label, i == l.chosen)
                .role("button")
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| {
                    let l = &mut s.player.lyrics;
                    l.chosen = i;
                    l.active = None;
                    s.scratch.remove(CENTERED);
                }),
        );
    }
    r.flex_wrap()
}

fn empty(app: &App, error: Option<&str>) -> El {
    let t = &app.theme;
    col()
        .w_full()
        .grow(1.0)
        .center()
        .gap(10.0)
        .px(12.0)
        .child(icon(&ic::LYRICS, 30.0).color(t.faint))
        .child(
            text(error.unwrap_or("No lyrics for this song yet."))
                .font(ty::BODY)
                .color(t.muted)
                .wrap()
                .max_lines(4)
                .align(rune::Align::Center),
        )
        .child(
            text("Search LRCLIB below, or paste LRC text.")
                .font(ty::SMALL)
                .color(t.faint)
                .wrap()
                .align(rune::Align::Center),
        )
}

fn lines(app: &App, track: &Track, h: f32) -> El {
    let t = &app.theme;
    let l = &app.player.lyrics;
    if track.instrumental {
        return empty(app, Some("This track is instrumental."));
    }
    let synced = track.synced();
    let pad = (h * 0.32).max(80.0);
    let mut list = col()
        .w_full()
        .gap(if synced { 14.0 } else { 8.0 })
        .pr(14.0)
        .pt(pad)
        .pb(pad);
    if !synced {
        list = list.pt(4.0).pb(8.0);
    }
    let url = l.url.clone();
    for (i, line) in track.lines.iter().enumerate() {
        if line.text.trim().is_empty() {
            list = list.child(el().h(if synced { 10.0 } else { 6.0 }));
            continue;
        }
        if !synced {
            list = list.child(
                text(&line.text)
                    .font(ty::BODY)
                    .color(t.text.alpha(0.85))
                    .w_full()
                    .wrap()
                    .selectable(true),
            );
            continue;
        }
        let (alpha, active) = line_tone(i, l.active);
        let at = line.at_ms;
        let off = l.offset_ms;
        let font = ty::H2;
        let mut e = el()
            .key(format!("lyric-{i}"))
            .w_full()
            .motion(Spring::new(0.5, 0.15))
            .py(2.0)
            .cursor(rune::Cursor::Pointer)
            .role("button")
            .label(line.text.clone())
            .child(
                text(&line.text)
                    .font(font)
                    .color(if active { t.text } else { t.text.alpha(alpha) })
                    .w_full()
                    .wrap(),
            );
        if let Some(at) = at {
            e = e.on_press(move |s: &mut App| s.player.seek_to(line_seek_seconds(at, off), true));
        }
        if active {
            let url = url.clone();
            e = e.on_layout_tx(move |s: &mut App, _r, tx: &mut Tx| {
                let mark = format!("{url}#{i}");
                if s.scratch.get(CENTERED) != Some(&mark) {
                    s.scratch.insert(CENTERED.into(), mark);
                    tx.scroll_center_in(SCROLL, format!("lyric-{i}"));
                }
            });
        }
        list = list.child(e);
    }
    el().w_full().grow(1.0).min_h(0.0).child(
        col()
            .key(SCROLL)
            .w_full()
            .h_full()
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .scroll_edge_fade(36.0)
            .child(list),
    )
}

fn toolbar(app: &App, now: &Now) -> El {
    let t = &app.theme;
    let l = &app.player.lyrics;
    let mut r = row().w_full().gap(8.0).center_y().flex_wrap();
    if l.track().is_some_and(Track::synced) {
        let k = t.kit();
        let step = |label: &str, delta: i64| {
            widgets::icon_button(
                t,
                if delta < 0 { &ic::BACK } else { &ic::FORWARD },
                label,
                30.0,
            )
            .on_press(move |s: &mut App| {
                let l = &mut s.player.lyrics;
                l.offset_ms = (l.offset_ms + delta).clamp(-120_000, 120_000);
                l.active = None;
            })
        };
        r = r.child(
            row()
                .gap(6.0)
                .center_y()
                .child(step("Lyrics earlier 0.5 seconds", -500))
                .child(
                    el().w(56.0).center().child(
                        rune_kit::RollingText::new("lyrics-offset", offset_text(l.offset_ms))
                            .font(ty::NUM)
                            .color(t.text)
                            .build(&k),
                    ),
                )
                .child(step("Lyrics later 0.5 seconds", 500)),
        );
    }
    let (title, artist) = (now.media.title.clone(), now.media.creator.clone());
    if l.track().is_some() {
        r = r.child(
            widgets::button(t, "Save", Some(&ic::DOWNLOAD), Kind::Secondary)
                .on_press(|s: &mut App| crate::lyrics::save(s, None)),
        );
    }
    r = r.child(
        widgets::button(t, "Search LRCLIB", Some(&ic::SEARCH), Kind::Secondary)
            .on_press(move |s: &mut App| crate::lyrics::search(s, &title, &artist)),
    );
    r = r.child(
        widgets::button(t, "Paste LRC", Some(&ic::COPY), Kind::Ghost)
            .on_press(|s: &mut App| s.player.lyrics.importing = !s.player.lyrics.importing),
    );
    if l.local {
        r = r.child(
            widgets::button(t, "Remove copy", Some(&ic::TRASH), Kind::Ghost)
                .on_press(|s: &mut App| crate::lyrics::remove(s)),
        );
    }
    r
}

fn import_form(app: &App) -> El {
    let t = &app.theme;
    col()
        .w_full()
        .gap(8.0)
        .child(widgets::field(
            t,
            &app.player.lyrics.import,
            "Paste LRC text, e.g. [00:12.00]First line",
            |s| &mut s.player.lyrics.import,
        ))
        .child(row().gap(8.0).child(
            widgets::button(t, "Save pasted lyrics", None, Kind::Primary).on_press(
                |s: &mut App| {
                    let text = s.player.lyrics.import.text.trim().to_string();
                    if !text.is_empty() {
                        crate::lyrics::save(s, Some(text));
                    }
                },
            ),
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seek_accounts_for_offset() {
        assert_eq!(line_seek_seconds(12_000, 500), 11.5);
        assert_eq!(line_seek_seconds(12_000, -500), 12.5);
        assert_eq!(line_seek_seconds(100, 5_000), 0.0);
    }

    #[test]
    fn tones_dim_past_and_future() {
        assert_eq!(line_tone(3, Some(3)), (1.0, true));
        assert!(line_tone(1, Some(3)).0 < line_tone(5, Some(3)).0);
        assert!(!line_tone(0, None).1);
    }

    #[test]
    fn labels() {
        assert_eq!(offset_text(500), "+0.5s");
        assert_eq!(offset_text(-1500), "-1.5s");
        let mut t = Track {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            duration: 0.0,
            instrumental: false,
            lines: vec![],
        };
        assert_eq!(source_label(false, &t), "YouTube Music");
        t.duration = 180.0;
        assert_eq!(source_label(false, &t), "LRCLIB");
        assert_eq!(source_label(true, &t), "On this computer");
        assert!(panel_height(300.0) >= 420.0 && panel_height(3000.0) <= 720.0);
    }
}
