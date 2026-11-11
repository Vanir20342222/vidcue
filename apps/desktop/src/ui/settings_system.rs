//! Settings → Downloads, Network, Privacy and Advanced.

use std::path::PathBuf;

use rune::{El, Tx, col, row, text};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use vidcue_core::{Media, MediaKind};

use super::{Probe, chips, field_row, group, opts, path_text, stacked, toast_result, toggle};
use crate::app::{App, ToastKind};
use crate::backend::op;
use crate::icons as ic;
use crate::theme::ty;
use crate::ui::widgets::{self, Kind};

const DEFAULT_TEMPLATE: &str = "%(title).180s [%(id)s].%(ext)s";

// ---- pure helpers ----

/// Empty text means "no proxy".
pub fn proxy_from_text(text: &str) -> Option<String> {
    let t = text.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

/// "de, at ch" -> ["DE", "AT", "CH"]; rejects anything that is not a two-letter code.
pub fn parse_countries(text: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for part in text
        .split([',', ' ', ';'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        if part.len() != 2 || !part.bytes().all(|b| b.is_ascii_alphabetic()) {
            return Err(format!("“{part}” is not a two-letter country code"));
        }
        let code = part.to_ascii_uppercase();
        if !out.contains(&code) {
            out.push(code);
        }
    }
    Ok(out)
}

pub fn station(name: &str, url: &str) -> Media {
    Media {
        id: url.to_owned(),
        title: name.to_owned(),
        creator: "Custom station".into(),
        url: url.to_owned(),
        kind: MediaKind::Station,
        ..Default::default()
    }
}

// ---- Downloads ----

fn dir_row(
    t: &crate::theme::Theme,
    key: &'static str,
    title: &str,
    path: &std::path::Path,
    audio: bool,
) -> El {
    let pick = widgets::button(t, "Choose…", Some(&ic::FOLDER), Kind::Secondary).on_press_tx(
        move |_: &mut App, tx: &mut Tx| {
            let opts = rune::FileDialogOptions {
                title: "Choose a download folder".into(),
                ..Default::default()
            };
            tx.open_folder(opts, move |s: &mut App, r, _| {
                if let Ok(Some(p)) = r {
                    s.edit_config(|c| {
                        if audio {
                            c.download.audio_directory = p
                        } else {
                            c.download.directory = p
                        }
                    });
                }
            });
        },
    );
    let reset = widgets::button(t, "Default", None, Kind::Ghost).on_press(move |s: &mut App| {
        s.edit_config(|c| {
            if audio {
                c.download.audio_directory = PathBuf::new()
            } else {
                c.download.directory = PathBuf::new()
            }
        })
    });
    let words = col()
        .grow(1.0)
        .shrink()
        .gap(3.0)
        .child(text(title).font(ty::BODY_STRONG).color(t.text))
        .child(
            text(path_text(path, "Default folder"))
                .font(ty::SMALL)
                .color(t.muted)
                .wrap(),
        );
    row()
        .key(key)
        .w_full()
        .py(12.0)
        .gap(12.0)
        .center_y()
        .child(words)
        .child(reset)
        .child(pick)
}

pub(super) fn downloads(app: &App) -> El {
    let t = &app.theme;
    let c = &app.config;
    let d = &c.download;
    let backend = ["auto", "native", "yt-dlp"]
        .iter()
        .position(|b| *b == c.download_backend)
        .unwrap_or(0);
    let save_template = widgets::button(t, "Save", None, Kind::Primary).on_press(|s: &mut App| {
        let v = s.settings.template.text.trim().to_owned();
        s.edit_config(|c| c.download.filename_template = v);
    });
    let reset_template = widgets::button(t, "Reset", None, Kind::Ghost).on_press(|s: &mut App| {
        s.settings.template.replace_text(DEFAULT_TEMPLATE);
        s.edit_config(|c| c.download.filename_template = DEFAULT_TEMPLATE.into());
    });
    col()
        .w_full()
        .gap(24.0)
        .child(group(
            t,
            "Locations",
            vec![
                dir_row(t, "st-dir-video", "Video folder", &d.directory, false),
                dir_row(t, "st-dir-audio", "Audio folder", &d.audio_directory, true),
            ],
        ))
        .child(group(
            t,
            "Format",
            vec![
                widgets::setting_row(
                    t,
                    "Video container",
                    "",
                    chips(
                        t,
                        "st-container",
                        &opts(&[("MP4", "mp4"), ("MKV", "mkv"), ("WebM", "webm")]),
                        &d.container,
                        |s, v| s.edit_config(|c| c.download.container = v),
                    ),
                ),
                widgets::setting_row(
                    t,
                    "Audio format",
                    "",
                    chips(
                        t,
                        "st-acontainer",
                        &opts(&[
                            ("MP3", "mp3"),
                            ("M4A", "m4a"),
                            ("Opus", "opus"),
                            ("FLAC", "flac"),
                            ("Vorbis", "vorbis"),
                            ("WAV", "wav"),
                            ("AAC", "aac"),
                        ]),
                        &d.audio_container,
                        |s, v| s.edit_config(|c| c.download.audio_container = v),
                    ),
                ),
                widgets::setting_row(
                    t,
                    "Audio quality",
                    "0 is best for variable bitrate; or a bitrate.",
                    chips(
                        t,
                        "st-aquality",
                        &opts(&[
                            ("Best", "0"),
                            ("Good", "2"),
                            ("Small", "5"),
                            ("320K", "320K"),
                            ("192K", "192K"),
                            ("128K", "128K"),
                        ]),
                        &d.audio_quality,
                        |s, v| s.edit_config(|c| c.download.audio_quality = v),
                    ),
                ),
                widgets::setting_row(
                    t,
                    "Engine",
                    "Auto tries the built-in extractor first and falls back to yt-dlp.",
                    widgets::segmented(
                        t,
                        "st-dlbackend",
                        &["Auto", "Native", "yt-dlp"],
                        backend,
                        |s, i| {
                            s.edit_config(|c| {
                                c.download_backend = ["auto", "native", "yt-dlp"][i].into()
                            })
                        },
                    ),
                ),
            ],
        ))
        .child(group(
            t,
            "Extras",
            vec![
                toggle(
                    t,
                    "st-subs",
                    "Subtitles",
                    "Download English subtitles when available.",
                    !d.subtitles.is_empty(),
                    |s| {
                        s.edit_config(|c| {
                            c.download.subtitles = if c.download.subtitles.is_empty() {
                                vec!["en".into()]
                            } else {
                                Vec::new()
                            }
                        })
                    },
                ),
                toggle(
                    t,
                    "st-autosubs",
                    "Automatic captions",
                    "Include machine-made captions.",
                    d.auto_subtitles,
                    |s| s.edit_config(|c| c.download.auto_subtitles = !c.download.auto_subtitles),
                ),
                toggle(
                    t,
                    "st-embedsubs",
                    "Embed subtitles",
                    "Put subtitles inside the video file.",
                    d.embed_subtitles,
                    |s| s.edit_config(|c| c.download.embed_subtitles = !c.download.embed_subtitles),
                ),
                toggle(
                    t,
                    "st-meta",
                    "Write metadata",
                    "Title, artist and description tags.",
                    d.metadata,
                    |s| s.edit_config(|c| c.download.metadata = !c.download.metadata),
                ),
                toggle(
                    t,
                    "st-thumb",
                    "Save thumbnail",
                    "Embed the cover art.",
                    d.thumbnail,
                    |s| s.edit_config(|c| c.download.thumbnail = !c.download.thumbnail),
                ),
                toggle(
                    t,
                    "st-sbremove",
                    "Remove sponsor segments",
                    "Cut SponsorBlock segments out of downloads.",
                    d.sponsorblock_remove,
                    |s| {
                        s.edit_config(|c| {
                            c.download.sponsorblock_remove = !c.download.sponsorblock_remove
                        })
                    },
                ),
                widgets::setting_row(
                    t,
                    "Simultaneous downloads",
                    "How many files download at once.",
                    super::int_slider(
                        t,
                        "st-concurrency",
                        c.concurrency as u32,
                        1,
                        16,
                        "",
                        |s, v, done| {
                            if done {
                                s.edit_config(|c| c.concurrency = v as usize);
                            } else {
                                s.config.concurrency = v as usize;
                            }
                        },
                    ),
                ),
                stacked(
                    t,
                    "File name template",
                    "yt-dlp output template. It must stay inside the destination folder.",
                    field_row(
                        t,
                        &app.settings.template,
                        DEFAULT_TEMPLATE,
                        |s| &mut s.settings.template,
                        row().gap(6.0).child(reset_template).child(save_template),
                    ),
                ),
            ],
        ))
}

// ---- Network ----

pub(super) fn network(app: &App) -> El {
    let t = &app.theme;
    let c = &app.config;
    let save_proxy = widgets::button(t, "Save", None, Kind::Primary).on_press(|s: &mut App| {
        let v = proxy_from_text(&s.settings.proxy.text);
        s.edit_config(|c| c.proxy = v);
    });
    let add_server =
        widgets::button(t, "Add", Some(&ic::PLUS), Kind::Primary).on_press(|s: &mut App| {
            let v = s.settings.server.text.trim().to_owned();
            if v.is_empty() {
                return;
            }
            s.settings.server.clear();
            s.edit_config(|c| c.radio_servers.push(v));
        });
    let mut servers = col().w_full().gap(8.0);
    for (i, url) in c.radio_servers.iter().enumerate() {
        servers = servers.child(
            row()
                .key(format!("st-server-{i}"))
                .w_full()
                .gap(10.0)
                .center_y()
                .child(
                    text(url)
                        .font(ty::BODY)
                        .color(t.text)
                        .grow(1.0)
                        .shrink()
                        .max_lines(1),
                )
                .child(
                    widgets::icon_button(t, &ic::TRASH, "Remove server", 32.0).on_press(
                        move |s: &mut App| {
                            s.edit_config(|c| {
                                if i < c.radio_servers.len() {
                                    c.radio_servers.remove(i);
                                }
                            })
                        },
                    ),
                ),
        );
    }
    let servers = servers.child(field_row(
        t,
        &app.settings.server,
        "https://de1.api.radio-browser.info",
        |s| &mut s.settings.server,
        add_server,
    ));

    let add_station =
        widgets::button(t, "Add", Some(&ic::PLUS), Kind::Primary).on_press(|s: &mut App| {
            let (name, url) = (
                s.settings.station_name.text.trim().to_owned(),
                s.settings.station_url.text.trim().to_owned(),
            );
            if name.is_empty() || url.is_empty() {
                s.toast(ToastKind::Error, "A station needs a name and a stream URL.");
                return;
            }
            s.settings.station_name.clear();
            s.settings.station_url.clear();
            s.edit_config(|c| c.custom_stations.push(station(&name, &url)));
        });
    let mut stations = col().w_full().gap(8.0);
    for (i, m) in c.custom_stations.iter().enumerate() {
        stations = stations.child(
            row()
                .key(format!("st-station-{i}"))
                .w_full()
                .gap(10.0)
                .center_y()
                .child(
                    col()
                        .grow(1.0)
                        .shrink()
                        .child(
                            text(&m.title)
                                .font(ty::BODY_STRONG)
                                .color(t.text)
                                .max_lines(1),
                        )
                        .child(text(&m.url).font(ty::SMALL).color(t.muted).max_lines(1)),
                )
                .child(
                    widgets::icon_button(t, &ic::TRASH, "Remove station", 32.0).on_press(
                        move |s: &mut App| {
                            s.edit_config(|c| {
                                if i < c.custom_stations.len() {
                                    c.custom_stations.remove(i);
                                }
                            })
                        },
                    ),
                ),
        );
    }
    let stations = stations
        .child(widgets::field(
            t,
            &app.settings.station_name,
            "Station name",
            |s| &mut s.settings.station_name,
        ))
        .child(field_row(
            t,
            &app.settings.station_url,
            "https://stream.example.com/live.mp3",
            |s| &mut s.settings.station_url,
            add_station,
        ));

    let save_countries =
        widgets::button(t, "Save", None, Kind::Primary).on_press(
            |s: &mut App| match parse_countries(&s.settings.countries.text) {
                Ok(list) => s.edit_config(|c| c.radio_pinned_countries = list),
                Err(e) => s.toast(ToastKind::Error, e),
            },
        );
    col()
        .w_full()
        .gap(24.0)
        .child(group(
            t,
            "Connection",
            vec![stacked(
                t,
                "Proxy",
                "HTTP, HTTPS, socks5 or socks5h, e.g. socks5h://127.0.0.1:1080. Leave empty for a direct connection.",
                field_row(t, &app.settings.proxy, "No proxy", |s| &mut s.settings.proxy, save_proxy),
            )],
        ))
        .child(group(
            t,
            "Radio",
            vec![
                stacked(
                    t,
                    "Directory servers",
                    "Extra radio-browser mirrors (up to eight). Empty uses the public list.",
                    servers,
                ),
                stacked(
                    t,
                    "Pinned countries",
                    "Two-letter codes, comma separated, shown as shortcuts on the Radio tab.",
                    field_row(t, &app.settings.countries, "DE, US, GB", |s| &mut s.settings.countries, save_countries),
                ),
                stacked(t, "Custom stations", "Your own streams, listed with your saved stations.", stations),
            ],
        ))
}

// ---- Privacy ----

pub(super) fn privacy(app: &App) -> El {
    let t = &app.theme;
    let danger = |label: &'static str, key: &'static str| {
        widgets::button(t, label, Some(&ic::TRASH), Kind::Danger).key(key)
    };
    col()
        .w_full()
        .gap(24.0)
        .child(group(
            t,
            "History",
            vec![
                toggle(
                    t,
                    "st-p-history",
                    "Keep watch history",
                    "When off, Vidcue stops recording what you play.",
                    app.config.playback_history,
                    |s| s.edit_config(|c| c.playback_history = !c.playback_history),
                ),
                widgets::setting_row(
                    t,
                    "Watch history",
                    "Forget what you watched, resume positions and recently played.",
                    danger("Clear history", "st-clear-history").on_press(|s: &mut App| {
                        s.be.call(op("history_clear", json!({})), |app, r, _| {
                            toast_result(app, r, "Watch history cleared");
                            app.player.refresh_up_next(&app.be);
                        })
                    }),
                ),
                widgets::setting_row(
                    t,
                    "Recent searches",
                    "Video and radio search suggestions.",
                    danger("Clear searches", "st-clear-searches").on_press(|s: &mut App| {
                        s.edit_prefs(|p| {
                            p.recent_media.clear();
                            p.recent_radio.clear();
                        });
                        s.toast(ToastKind::Success, "Recent searches cleared");
                    }),
                ),
            ],
        ))
        .child(group(
            t,
            "Caches",
            vec![
                widgets::setting_row(
                    t,
                    "Image cache",
                    "Thumbnails and avatars saved on disk. They download again when needed.",
                    danger("Clear images", "st-clear-images").on_press(|s: &mut App| {
                        let dir = s.be.paths.cache.join("images");
                        s.be.blocking(
                            move || match std::fs::remove_dir_all(&dir) {
                                Ok(()) => Ok(()),
                                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                                Err(e) => Err(e.to_string()),
                            },
                            |app, out, _| match out {
                                Ok(()) => app.toast(ToastKind::Success, "Image cache cleared"),
                                Err(e) => app.toast(
                                    ToastKind::Error,
                                    format!("The image cache could not be cleared: {e}"),
                                ),
                            },
                        );
                    }),
                ),
                widgets::setting_row(
                    t,
                    "Stream cache",
                    "Resolved stream details kept in memory.",
                    danger("Clear streams", "st-clear-resolve").on_press(|s: &mut App| {
                        s.be.call(op("resolve_cache_clear", json!({})), |app, r, _| {
                            toast_result(app, r, "Stream cache cleared")
                        })
                    }),
                ),
            ],
        ))
}

// ---- Advanced ----

/// Looks for Node 22+ or Deno 2.9.7 with the engine's own selection logic.
pub(super) fn detect_runtime(app: &mut App) {
    if app.offline() {
        app.settings.probe = Probe::Failed("Detection is off in headless renders".into());
        return;
    }
    app.settings.probe = Probe::Checking;
    let config = app.config.clone();
    app.be.task(
        async move {
            vidcue_services::native_runtime::select(&config, CancellationToken::new())
                .await
                .map_err(|e| e.to_string())
        },
        |app, out, _| {
            app.settings.probe = match out {
                Ok(Some(p)) => Probe::Found(p.display().to_string()),
                Ok(None) => Probe::Missing,
                Err(e) => Probe::Failed(e),
            }
        },
    );
}

fn status(t: &crate::theme::Theme, ok: bool, msg: &str) -> El {
    super::note(t, ok, msg)
}

pub(super) fn advanced(app: &App) -> El {
    let t = &app.theme;
    let c = &app.config;
    let probe = match &app.settings.probe {
        Probe::Idle => text("Not checked yet").font(ty::SMALL).color(t.muted),
        Probe::Checking => text("Looking for Node.js or Deno…")
            .font(ty::SMALL)
            .color(t.muted),
        Probe::Found(p) => text(p).font(ty::SMALL).color(t.success).wrap(),
        Probe::Missing => text("not found (install Node 22+ or Deno 2.9.7)")
            .font(ty::SMALL)
            .color(t.warning)
            .wrap(),
        Probe::Failed(e) => text(e).font(ty::SMALL).color(t.error).wrap(),
    };
    let save_js = widgets::button(t, "Save", None, Kind::Primary).on_press(|s: &mut App| {
        let v = s.settings.js_runtime.text.trim().to_owned();
        s.edit_config(|c| c.native_js_runtime = (!v.is_empty()).then(|| PathBuf::from(v)));
        detect_runtime(s);
    });
    let mpv = match &app.player.unavailable {
        None => status(t, true, "libmpv is available; embedded playback is ready."),
        Some(e) => status(t, false, e),
    };
    let root = app.be.paths.root.display().to_string();
    let cache = app.be.paths.cache.display().to_string();
    let path_row = |title: &str, p: String| {
        let copy = p.clone();
        widgets::setting_row(
            t,
            title,
            &p,
            widgets::icon_button(t, &ic::COPY, "Copy path", 34.0).on_press_tx(
                move |s: &mut App, tx: &mut Tx| {
                    tx.copy(copy.clone());
                    s.toast(ToastKind::Success, "Path copied");
                },
            ),
        )
    };
    col()
        .w_full()
        .gap(24.0)
        .child(group(
            t,
            "Playback engine",
            vec![
                widgets::setting_row(
                    t,
                    "Playback backend",
                    "Streams are resolved by the built-in native extractor. yt-dlp is never started for playback unless you choose it.",
                    widgets::badge(&c.playback_backend, t.accent_soft, t.accent),
                ),
                stacked(
                    t,
                    "JavaScript runtime",
                    "The native extractor solves YouTube's challenges offline with Node.js 22+ or Deno 2.9.7. Vidcue never installs them.",
                    col()
                        .w_full()
                        .gap(10.0)
                        .child(
                            row()
                                .gap(10.0)
                                .center_y()
                                .child(probe.grow(1.0).shrink())
                                .child(widgets::button(t, "Check again", Some(&ic::REFRESH), Kind::Secondary).on_press(|s: &mut App| detect_runtime(s))),
                        )
                        .child(field_row(
                            t,
                            &app.settings.js_runtime,
                            "Automatic (empty), or a path to node or deno",
                            |s| &mut s.settings.js_runtime,
                            save_js,
                        )),
                ),
                stacked(t, "Embedded player (libmpv)", "", mpv),
            ],
        ))
        .child(group(
            t,
            "Where Vidcue keeps things",
            vec![path_row("State folder", root), path_row("Cache folder", cache)],
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_text_trims_and_empties() {
        assert_eq!(proxy_from_text("  "), None);
        assert_eq!(
            proxy_from_text(" socks5h://127.0.0.1:1080 "),
            Some("socks5h://127.0.0.1:1080".into())
        );
    }

    #[test]
    fn countries_parse_to_upper_case_codes() {
        assert_eq!(parse_countries("de, at ch;DE").unwrap(), ["DE", "AT", "CH"]);
        assert!(parse_countries("").unwrap().is_empty());
        assert!(parse_countries("deu").is_err());
        assert!(parse_countries("d1").is_err());
        let c = vidcue_core::Config {
            radio_pinned_countries: parse_countries("us, gb").unwrap(),
            ..Default::default()
        };
        assert!(c.validate().is_ok());
    }

    #[test]
    fn custom_station_is_a_station() {
        let m = station("Jazz", "https://example.com/jazz.mp3");
        assert_eq!(m.kind, MediaKind::Station);
        assert_eq!(m.url, "https://example.com/jazz.mp3");
        assert_eq!(m.title, "Jazz");
    }
}
