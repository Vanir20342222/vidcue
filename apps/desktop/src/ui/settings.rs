//! Settings: a section list on the left (the selection pill flies between rows) and a
//! scrollable panel of grouped rows on the right. Engine settings go through
//! `App::edit_config`, desktop-only choices through `App::edit_prefs`.
//!
//! This file holds the state, the shared row builders and the Appearance and Playback
//! sections. The other sections live in sibling modules.

use rune::tok::motion;
use rune::{
    Color, Cx, Drag, El, Icon, Phase, ScrollSpec, TextEdit, Transition, Tx, col, el, icon, row,
    text,
};

use crate::app::{App, Route, SettingsSection};
use crate::icons as ic;
use crate::theme::{Theme, sz, ty};
use crate::ui::widgets;

#[path = "settings_about.rs"]
mod about;
#[path = "settings_audio.rs"]
mod audio;
#[path = "settings_system.rs"]
mod system;

/// Result of the native JavaScript runtime probe.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Probe {
    #[default]
    Idle,
    Checking,
    Found(String),
    Missing,
    Failed(String),
}

#[derive(Default)]
pub struct SettingsUi {
    pub import: TextEdit,
    /// Outcome of the last palette import: (ok, message).
    pub import_note: Option<(bool, String)>,
    pub proxy: TextEdit,
    pub template: TextEdit,
    pub js_runtime: TextEdit,
    pub server: TextEdit,
    pub station_name: TextEdit,
    pub station_url: TextEdit,
    pub countries: TextEdit,
    pub eq_name: TextEdit,
    pub tmdb: TextEdit,
    /// An equalizer curve being edited; `None` shows the selected preset as it is.
    pub eq_draft: Option<[f64; 10]>,
    pub probe: Probe,
}

pub fn enter(app: &mut App) {
    let c = app.config.clone();
    let s = &mut app.settings;
    s.proxy.replace_text(c.proxy.clone().unwrap_or_default());
    s.template
        .replace_text(c.download.filename_template.clone());
    s.js_runtime.replace_text(
        c.native_js_runtime
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    );
    s.countries
        .replace_text(c.radio_pinned_countries.join(", "));
    s.tmdb = TextEdit::new("").secure();
    s.tmdb.replace_text(app.prefs.tmdb_key.clone());
    s.eq_draft = None;
    if matches!(app.route, Route::Settings(SettingsSection::Advanced)) {
        system::detect_runtime(app);
    }
}

fn icon_for(sec: SettingsSection) -> &'static Icon {
    match sec {
        SettingsSection::Appearance => &ic::PALETTE,
        SettingsSection::Playback => &ic::PLAY,
        SettingsSection::Audio => &ic::SLIDERS,
        SettingsSection::Downloads => &ic::DOWNLOAD,
        SettingsSection::Network => &ic::GLOBE,
        SettingsSection::Privacy => &ic::KEY,
        SettingsSection::Advanced => &ic::GAUGE,
        SettingsSection::Alpha => &ic::SPARKLES,
        SettingsSection::About => &ic::INFO,
    }
}

pub const SECTIONS: [(SettingsSection, &str); 9] = [
    (SettingsSection::Appearance, "Appearance"),
    (SettingsSection::Playback, "Playback"),
    (SettingsSection::Audio, "Audio"),
    (SettingsSection::Downloads, "Downloads"),
    (SettingsSection::Network, "Network"),
    (SettingsSection::Privacy, "Privacy"),
    (SettingsSection::Advanced, "Advanced"),
    (SettingsSection::Alpha, "Alpha"),
    (SettingsSection::About, "About"),
];

pub fn view(app: &App, cx: &mut Cx<App>, section: SettingsSection) -> El {
    let t = &app.theme;
    let mut nav = col().w(220.0).gap(2.0).role("tablist").focus_group();
    for (sec, label) in SECTIONS {
        nav = nav.child(nav_row(t, sec, label, sec == section));
    }

    let body = match section {
        SettingsSection::Appearance => appearance(app),
        SettingsSection::Playback => playback(app),
        SettingsSection::Audio => audio::view(app),
        SettingsSection::Downloads => system::downloads(app),
        SettingsSection::Network => system::network(app),
        SettingsSection::Privacy => system::privacy(app),
        SettingsSection::Advanced => system::advanced(app),
        SettingsSection::Alpha => about::alpha(app),
        SettingsSection::About => about::view(app),
    };
    let label = SECTIONS
        .iter()
        .find(|(s, _)| *s == section)
        .map(|(_, l)| *l)
        .unwrap_or("Settings");
    let panel = col()
        .key(format!("st-panel-{section:?}"))
        .w_full()
        .max_w(760.0)
        .gap(22.0)
        .transition(Transition::blur_replace())
        .child(widgets::page_title(t, label, Some(blurb(section))))
        .child(body);
    let scroll = col()
        .key("st-scroll")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .scroll_edge_fade(18.0)
        .child(el().w_full().pr(8.0).pb(sz::DOCK + 48.0).child(panel));
    let _ = cx;
    row()
        .w_full()
        .h_full()
        .pt(sz::TOPBAR + 24.0)
        .px(sz::PAGE_PAD)
        .gap(36.0)
        .child(nav)
        .child(scroll)
}

fn blurb(sec: SettingsSection) -> &'static str {
    match sec {
        SettingsSection::Appearance => "Themes, card details and visual effects.",
        SettingsSection::Playback => "Quality, resume, SponsorBlock and seeking.",
        SettingsSection::Audio => "Equalizer curves and the video colour grade.",
        SettingsSection::Downloads => "Where files go and how they are made.",
        SettingsSection::Network => "Proxy, radio directories and custom stations.",
        SettingsSection::Privacy => "History and the data Vidcue keeps on this computer.",
        SettingsSection::Advanced => "Playback backend, JavaScript runtime and state folders.",
        SettingsSection::Alpha => "Experimental features that may change or disappear.",
        SettingsSection::About => "Version, updates, shortcuts and credits.",
    }
}

fn nav_row(t: &Theme, sec: SettingsSection, label: &'static str, selected: bool) -> El {
    let fg = if selected { t.text } else { t.muted };
    let r = row()
        .key(format!("st-nav-{sec:?}"))
        .h(40.0)
        .w_full()
        .px(14.0)
        .gap(12.0)
        .center_y()
        .radius(sz::R_MD)
        .fill(if selected {
            t.accent_soft
        } else {
            rune::Color::TRANSPARENT
        })
        .hover(if selected { t.accent_soft } else { t.hover })
        .role("tab")
        .label(label)
        .checked(selected)
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| {
            if s.route != Route::Settings(sec) {
                s.route = Route::Settings(sec);
                s.menu = None;
                if sec == SettingsSection::Advanced {
                    system::detect_runtime(s);
                }
            }
        });
    r.child(icon(icon_for(sec), 18.0).color(if selected { t.accent } else { fg }))
        .child(text(label).font(ty::BODY_STRONG).color(fg))
}

// ---- shared builders ----

/// A titled group of rows on a surface card, separated by hairlines.
pub(super) fn group(t: &Theme, title: &str, rows: Vec<El>) -> El {
    let mut body = col().w_full().px(20.0).py(6.0);
    let n = rows.len();
    for (i, r) in rows.into_iter().enumerate() {
        body = body.child(r);
        if i + 1 < n {
            body = body.child(el().w_full().h(1.0).fill(t.hairline));
        }
    }
    col()
        .w_full()
        .gap(10.0)
        .child(text(title).font(ty::LABEL).color(t.muted).px(6.0))
        .child(
            widgets::panel(t)
                .w_full()
                .clip_rounded(sz::R_PANEL)
                .child(body),
        )
}

/// A row with a switch on the right; the whole switch is the press target.
pub(super) fn toggle(
    t: &Theme,
    key: &str,
    title: &str,
    detail: &str,
    on: bool,
    f: impl Fn(&mut App) + 'static,
) -> El {
    widgets::setting_row(
        t,
        title,
        detail,
        widgets::switch(t, key, on).label(title).on_press(f),
    )
}

/// A row of single-choice chips with dynamic values.
pub(super) fn chips(
    t: &Theme,
    key: &str,
    options: &[(String, String)],
    selected: &str,
    pick: impl Fn(&mut App, String) + Clone + 'static,
) -> El {
    let mut r = row().gap(6.0).flex_wrap().justify_end();
    for (label, value) in options {
        let on = value == selected;
        let (v, p) = (value.clone(), pick.clone());
        r = r.child(
            widgets::chip(t, label, on)
                .key(format!("{key}-{value}"))
                .on_press(move |s: &mut App| p(s, v.clone())),
        );
    }
    r
}

pub(super) fn opts(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(l, v)| ((*l).to_owned(), (*v).to_owned()))
        .collect()
}

/// A setting whose control sits under its description (long controls: chip lists, fields).
pub(super) fn stacked(t: &Theme, title: &str, detail: &str, control: El) -> El {
    let mut c = col()
        .w_full()
        .py(12.0)
        .gap(10.0)
        .child(text(title).font(ty::BODY_STRONG).color(t.text));
    if !detail.is_empty() {
        c = c.child(text(detail).font(ty::SMALL).color(t.muted).wrap());
    }
    c.child(control)
}

/// A text field with a trailing action button.
pub(super) fn field_row(
    t: &Theme,
    edit: &TextEdit,
    placeholder: &str,
    access: fn(&mut App) -> &mut TextEdit,
    action: El,
) -> El {
    row()
        .w_full()
        .gap(10.0)
        .center_y()
        .child(
            widgets::field(t, edit, placeholder, access)
                .grow(1.0)
                .min_w(0.0),
        )
        .child(action)
}

pub(super) fn note(t: &Theme, ok: bool, msg: &str) -> El {
    let c = if ok { t.success } else { t.error };
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(icon(if ok { &ic::CHECK } else { &ic::WARNING }, 15.0).color(c))
        .child(text(msg).font(ty::SMALL).color(c).wrap().shrink())
}

/// A horizontal slider that accepts a closure, so it can carry its range.
pub(super) fn drag_slider(
    t: &Theme,
    key: &str,
    value: f32,
    width: f32,
    on_change: impl Fn(&mut App, f32, bool) + 'static,
) -> El {
    let v = value.clamp(0.0, 1.0);
    let track = el()
        .w(width)
        .h(4.0)
        .radius(2.0)
        .fill(t.overlay)
        .child(el().w(width * v).h(4.0).radius(2.0).fill(t.accent));
    let knob = el()
        .size(16.0, 16.0)
        .radius(8.0)
        .fill(Color::WHITE)
        .shadow(1.0, 4.0, Color::BLACK.alpha(0.35))
        .absolute(width * v - 8.0, 2.0);
    el().key(key.to_owned())
        .w(width)
        .h(20.0)
        .role("slider")
        .range_value(v as f64, 0.0, 1.0)
        .cursor(rune::Cursor::Pointer)
        .child(el().absolute(0.0, 8.0).child(track))
        .child(knob)
        .on_drag(move |s: &mut App, d: &mut Drag, tx: &mut Tx| {
            tx.instant();
            on_change(
                s,
                (d.x / d.w.max(1.0)).clamp(0.0, 1.0),
                d.phase == Phase::Up,
            );
        })
}

/// A slider over whole numbers with its value printed beside it.
pub(super) fn int_slider(
    t: &Theme,
    key: &str,
    value: u32,
    min: u32,
    max: u32,
    unit: &str,
    on_change: impl Fn(&mut App, u32, bool) + 'static,
) -> El {
    let frac = if max > min {
        (value.saturating_sub(min)) as f32 / (max - min) as f32
    } else {
        0.0
    };
    row()
        .gap(14.0)
        .center_y()
        .child(drag_slider(t, key, frac, 190.0, move |s, f, done| {
            on_change(s, step_value(f, min, max), done)
        }))
        .child(
            text(format!("{value}{unit}"))
                .font(ty::NUM)
                .color(t.muted)
                .w(48.0),
        )
}

/// Maps a slider fraction onto an inclusive integer range.
pub fn step_value(frac: f32, min: u32, max: u32) -> u32 {
    let f = frac.clamp(0.0, 1.0);
    (min + (f * (max - min) as f32).round() as u32).clamp(min, max)
}

pub(super) fn toast_result(
    app: &mut App,
    r: Result<serde_json::Value, crate::backend::ApiError>,
    ok: &str,
) {
    match r {
        Ok(_) => app.toast(crate::app::ToastKind::Success, ok),
        Err(e) => app.toast(crate::app::ToastKind::Error, e.to_string()),
    }
}

pub(super) fn path_text(p: &std::path::Path, fallback: &str) -> String {
    if p.as_os_str().is_empty() {
        fallback.to_owned()
    } else {
        p.display().to_string()
    }
}

// ---- Appearance ----

fn appearance(app: &App) -> El {
    let t = &app.theme;
    let c = &app.config;
    let mut grid = row().w_full().gap(14.0).flex_wrap();
    let mut names = crate::themes::pickable();
    if c.theme == "custom" && !names.iter().any(|(n, _)| *n == "custom") {
        names.push(("custom", None));
    }
    for (name, palette) in names {
        grid = grid.child(theme_card(app, name, palette.as_ref()));
    }

    let import = col()
        .w_full()
        .gap(10.0)
        .child(
            rune::text_input(&app.settings.import, ty::SMALL)
                .placeholder(
                    "Paste a Vidcue palette (JSON), Caelestia scheme or Omarchy colors.toml",
                    t.faint,
                )
                .on_edit(|s: &mut App| &mut s.settings.import)
                .color(t.text)
                .h(110.0)
                .padding(12.0)
                .radius(10.0)
                .fill(t.raised)
                .border(1.0, t.border),
        )
        .child(
            row()
                .gap(10.0)
                .center_y()
                .child(
                    widgets::button(
                        t,
                        "Import palette",
                        Some(&ic::PALETTE),
                        widgets::Kind::Primary,
                    )
                    .on_press(|s: &mut App| {
                        let text = s.settings.import.text.clone();
                        import_palette(s, text);
                    }),
                )
                .child(
                    widgets::button(
                        t,
                        "Import file…",
                        Some(&ic::FOLDER),
                        widgets::Kind::Secondary,
                    )
                    .on_press_tx(|_: &mut App, tx: &mut Tx| {
                        let opts = rune::FileDialogOptions {
                            title: "Import a palette".into(),
                            ..Default::default()
                        }
                        .filter("Palette", &["json", "toml", "conf", "css", "txt"]);
                        tx.open_file(opts, |s: &mut App, r, _| {
                            if let Ok(Some(path)) = r {
                                s.be.blocking(
                                    move || {
                                        std::fs::read_to_string(path).map_err(|e| e.to_string())
                                    },
                                    |app, out, _| match out {
                                        Ok(text) => {
                                            app.settings.import.replace_text(text.clone());
                                            import_palette(app, text);
                                        }
                                        Err(e) => {
                                            app.settings.import_note = Some((
                                                false,
                                                format!("The file could not be read: {e}"),
                                            ))
                                        }
                                    },
                                );
                            }
                        });
                    }),
                )
                .child(widgets::spacer_el()),
        );
    let import = match &app.settings.import_note {
        Some((ok, msg)) => import.child(note(t, *ok, msg)),
        None => import,
    };

    let ci = &c.card_info;
    let styles: Vec<(String, String)> = vidcue_core::MUSIC_PLAYER_STYLES
        .iter()
        .map(|s| (widgets_label(s), (*s).to_owned()))
        .collect();

    col()
        .w_full()
        .gap(24.0)
        .child(
            col()
                .w_full()
                .gap(10.0)
                .child(text("Theme").font(ty::LABEL).color(t.muted).px(6.0))
                .child(grid),
        )
        .child(group(
            t,
            "Import a palette",
            vec![stacked(
                t,
                "Custom palette",
                "Paste a palette or pick a file. Vidcue saves it and switches to it.",
                import,
            )],
        ))
        .child(group(
            t,
            "Cards",
            vec![
                toggle(
                    t,
                    "st-thumbs",
                    "Thumbnails",
                    "Show artwork on cards and rows.",
                    c.thumbnails,
                    |s| s.edit_config(|c| c.thumbnails = !c.thumbnails),
                ),
                toggle(
                    t,
                    "st-ci-channel",
                    "Channel",
                    "Show who published an item.",
                    ci.channel,
                    |s| s.edit_config(|c| c.card_info.channel = !c.card_info.channel),
                ),
                toggle(t, "st-ci-views", "View count", "", ci.views, |s| {
                    s.edit_config(|c| c.card_info.views = !c.card_info.views)
                }),
                toggle(t, "st-ci-date", "Upload date", "", ci.upload_date, |s| {
                    s.edit_config(|c| c.card_info.upload_date = !c.card_info.upload_date)
                }),
                toggle(
                    t,
                    "st-ci-duration",
                    "Duration",
                    "Badge on the artwork.",
                    ci.duration,
                    |s| s.edit_config(|c| c.card_info.duration = !c.card_info.duration),
                ),
                toggle(
                    t,
                    "st-ci-section",
                    "Section",
                    "Which shelf an item came from.",
                    ci.section,
                    |s| s.edit_config(|c| c.card_info.section = !c.card_info.section),
                ),
                toggle(t, "st-ci-likes", "Likes", "", ci.likes, |s| {
                    s.edit_config(|c| c.card_info.likes = !c.card_info.likes)
                }),
                toggle(
                    t,
                    "st-ci-desc",
                    "Description",
                    "A short description under the title.",
                    ci.description,
                    |s| s.edit_config(|c| c.card_info.description = !c.card_info.description),
                ),
                toggle(
                    t,
                    "st-dense",
                    "Compact grid",
                    "Smaller cards, more per row.",
                    app.prefs.grid_dense,
                    |s| s.edit_prefs(|p| p.grid_dense = !p.grid_dense),
                ),
            ],
        ))
        .child(group(
            t,
            "Motion and effects",
            vec![
                toggle(
                    t,
                    "st-effects",
                    "GPU effects",
                    "Glow, mesh gradients and shimmer. Turn off on slow graphics hardware.",
                    app.prefs.effects,
                    |s| s.edit_prefs(|p| p.effects = !p.effects),
                ),
                toggle(
                    t,
                    "st-ambient",
                    "Ambient light",
                    "A soft glow of the video's colours behind the player.",
                    app.prefs.ambient,
                    |s| s.edit_prefs(|p| p.ambient = !p.ambient),
                ),
                toggle(
                    t,
                    "st-hover",
                    "Hover previews",
                    "Play a short muted preview when you rest on a video.",
                    app.prefs.hover_previews,
                    |s| s.edit_prefs(|p| p.hover_previews = !p.hover_previews),
                ),
                toggle(
                    t,
                    "st-motion",
                    "Reduce motion",
                    "Cut animations down to simple fades.",
                    c.reduced_motion,
                    |s| s.edit_config(|c| c.reduced_motion = !c.reduced_motion),
                ),
                toggle(
                    t,
                    "st-sidebar",
                    "Compact sidebar",
                    "Icons only. Also Ctrl+B.",
                    app.prefs.sidebar_compact,
                    |s| s.edit_prefs(|p| p.sidebar_compact = !p.sidebar_compact),
                ),
                toggle(
                    t,
                    "st-hints",
                    "Show hints",
                    "Tips and shortcut hints.",
                    c.show_hints,
                    |s| s.edit_config(|c| c.show_hints = !c.show_hints),
                ),
                stacked(
                    t,
                    "Music player style",
                    "How the music player looks while a song plays.",
                    chips(t, "st-mps", &styles, &c.music_player_style, |s, v| {
                        s.edit_config(|c| c.music_player_style = v)
                    }),
                ),
            ],
        ))
}

fn widgets_label(s: &str) -> String {
    crate::themes::label(s)
}

/// Imports palette text through the engine, then switches to the custom theme.
fn import_palette(app: &mut App, text: String) {
    if text.trim().is_empty() {
        app.settings.import_note = Some((false, "Paste a palette or choose a file first.".into()));
        return;
    }
    app.settings.import_note = None;
    app.be.call(
        crate::backend::op("import_theme", serde_json::json!({ "text": text })),
        |app, r, _| match r {
            Ok(_) => {
                let changed = app.config.theme != "custom";
                let mut c = app.config.clone();
                c.theme = "custom".into();
                app.apply_config(c);
                if !changed {
                    crate::themes::reload(app);
                }
                app.settings.import_note = Some((true, "Palette imported and applied.".into()));
            }
            Err(e) => app.settings.import_note = Some((false, e.to_string())),
        },
    );
}

/// A theme as a miniature of the app drawn in its own palette.
fn theme_card(app: &App, name: &'static str, palette: Option<&vidcue_core::theme::Palette>) -> El {
    let t = &app.theme;
    let selected = app.config.theme == name;
    let m = match (name, palette) {
        (_, Some(p)) => Theme::from_palette(p),
        ("custom", None) => t.clone(),
        _ => t.clone(),
    };
    let ring = if selected { t.accent } else { t.border };
    let mock = row()
        .w(172.0)
        .h(108.0)
        .radius(sz::R_MD)
        .clip_rounded(sz::R_MD)
        .fill(m.bg)
        .border(if selected { 2.0 } else { 1.0 }, ring)
        .shadow(
            if selected { 6.0 } else { 0.0 },
            18.0,
            if selected {
                t.accent.alpha(0.28)
            } else {
                Color::TRANSPARENT
            },
        )
        .child(
            col()
                .w(34.0)
                .h_full()
                .fill(m.chrome)
                .px(8.0)
                .py(10.0)
                .gap(6.0)
                .child(el().size(18.0, 18.0).radius(6.0).fill(m.accent))
                .child(el().w_full().h(6.0).radius(3.0).fill(m.accent_soft))
                .child(el().w_full().h(6.0).radius(3.0).fill(m.hover))
                .child(el().w_full().h(6.0).radius(3.0).fill(m.hover)),
        )
        .child(
            col()
                .grow(1.0)
                .h_full()
                .padding(10.0)
                .gap(7.0)
                .child(el().w(70.0).h(7.0).radius(3.5).fill(m.text))
                .child(el().w(46.0).h(5.0).radius(2.5).fill(m.muted))
                .child(
                    row()
                        .w_full()
                        .gap(6.0)
                        .child(
                            el().grow(1.0)
                                .h(34.0)
                                .radius(6.0)
                                .fill(m.raised)
                                .child(el().w_full().h(4.0).radius(2.0).fill(m.accent)),
                        )
                        .child(
                            el().grow(1.0)
                                .h(34.0)
                                .radius(6.0)
                                .fill(m.surface)
                                .border(1.0, m.hairline),
                        ),
                )
                .child(
                    row()
                        .gap(5.0)
                        .child(el().w(26.0).h(10.0).radius(5.0).fill(m.accent))
                        .child(el().size(10.0, 10.0).radius(5.0).fill(m.success))
                        .child(el().size(10.0, 10.0).radius(5.0).fill(m.warning))
                        .child(el().size(10.0, 10.0).radius(5.0).fill(m.error)),
                ),
        );
    let mut stage = el().w(172.0).h(108.0).child(mock);
    if name == "system" {
        stage = stage.child(
            el().pin(None, Some(8.0), Some(8.0), None)
                .size(24.0, 24.0)
                .radius(12.0)
                .center()
                .fill(Color::BLACK.alpha(0.55))
                .child(icon(&ic::GLOBE, 14.0).color(Color::WHITE)),
        );
    }
    let label = crate::themes::label(name);
    let mut caption = row().gap(6.0).center_y().child(
        text(label.clone())
            .font(if selected {
                ty::SMALL_STRONG
            } else {
                ty::SMALL
            })
            .color(if selected { t.text } else { t.muted }),
    );
    if selected {
        caption = caption.child(icon(&ic::CHECK, 13.0).color(t.accent));
    }
    col()
        .key(format!("st-theme-{name}"))
        .gap(8.0)
        .role("radio")
        .label(label)
        .checked(selected)
        .focusable(true)
        .cursor(rune::Cursor::Pointer)
        .press_scale(0.96)
        .motion(motion::SNAP)
        .on_press(move |s: &mut App| s.edit_config(|c| c.theme = name.to_owned()))
        .child(stage)
        .child(caption)
}

// ---- Playback ----

const QUALITIES: [(&str, &str); 7] = [
    ("Best", "best"),
    ("2160p", "2160"),
    ("1440p", "1440"),
    ("1080p", "1080"),
    ("720p", "720"),
    ("480p", "480"),
    ("360p", "360"),
];
pub const SPONSOR_CATEGORIES: [(&str, &str); 8] = [
    ("Sponsor", "sponsor"),
    ("Self-promotion", "selfpromo"),
    ("Interaction reminder", "interaction"),
    ("Intro", "intro"),
    ("Outro", "outro"),
    ("Preview / recap", "preview"),
    ("Off-topic music", "music_offtopic"),
    ("Filler", "filler"),
];

/// Adds or removes `value` from a category list, keeping the list free of duplicates.
pub fn toggle_in(list: &mut Vec<String>, value: &str) {
    if let Some(i) = list.iter().position(|v| v == value) {
        list.remove(i);
    } else {
        list.push(value.to_owned());
    }
}

fn playback(app: &App) -> El {
    let t = &app.theme;
    let c = &app.config;
    let mode = ["skip", "mark", "off"]
        .iter()
        .position(|m| *m == c.playback_sponsorblock)
        .unwrap_or(0);
    let mut cats = row().gap(6.0).flex_wrap();
    for (label, value) in SPONSOR_CATEGORIES {
        let on = c.playback_sponsor_categories.iter().any(|v| v == value);
        cats = cats.child(
            widgets::chip(t, label, on)
                .key(format!("st-cat-{value}"))
                .on_press(move |s: &mut App| {
                    s.edit_config(|c| toggle_in(&mut c.playback_sponsor_categories, value))
                }),
        );
    }
    col()
        .w_full()
        .gap(24.0)
        .child(group(
            t,
            "Playback",
            vec![
                stacked(
                    t,
                    "Default quality",
                    "Used when a video starts and for new downloads. The player falls back to the best available.",
                    chips(t, "st-quality", &opts(&QUALITIES), &c.download.quality, |s, v| {
                        s.edit_config(|c| c.download.quality = v)
                    })
                    .justify_start(),
                ),
                toggle(
                    t,
                    "st-resume",
                    "Resume where you left off",
                    "Start partly watched videos from the saved position.",
                    c.resume_on_play,
                    |s| s.edit_config(|c| c.resume_on_play = !c.resume_on_play),
                ),
                toggle(
                    t,
                    "st-autoplay",
                    "Autoplay related",
                    "When the queue ends, continue with related videos.",
                    c.autoplay_related,
                    |s| s.edit_config(|c| c.autoplay_related = !c.autoplay_related),
                ),
                toggle(
                    t,
                    "st-preload",
                    "Preload next",
                    "Resolve the next item shortly before the current one ends.",
                    c.preload_next,
                    |s| s.edit_config(|c| c.preload_next = !c.preload_next),
                ),
                toggle(
                    t,
                    "st-offline",
                    "Prefer downloaded files",
                    "Play a finished download instead of streaming the same item.",
                    c.prefer_offline,
                    |s| s.edit_config(|c| c.prefer_offline = !c.prefer_offline),
                ),
                toggle(
                    t,
                    "st-captions",
                    "Captions on by default",
                    "Turn captions on when a video has them.",
                    app.prefs.captions,
                    |s| s.edit_prefs(|p| p.captions = !p.captions),
                ),
                widgets::setting_row(
                    t,
                    "Seek step",
                    "Seconds moved by J and L.",
                    int_slider(t, "st-seek", c.seek_seconds, 5, 60, " s", |s, v, done| {
                        if done {
                            s.edit_config(|c| c.seek_seconds = v);
                        } else {
                            s.config.seek_seconds = v;
                        }
                    }),
                ),
            ],
        ))
        .child(group(
            t,
            "SponsorBlock",
            vec![
                widgets::setting_row(
                    t,
                    "While playing",
                    "Skip segments, mark them on the seek bar, or leave playback alone.",
                    widgets::segmented(t, "st-sb", &["Skip", "Mark", "Off"], mode, |s, i| {
                        s.edit_config(|c| c.playback_sponsorblock = ["skip", "mark", "off"][i].into())
                    }),
                ),
                stacked(t, "Categories", "Which kinds of segments SponsorBlock acts on.", cats),
            ],
        ))
        .child(group(
            t,
            "History",
            vec![toggle(
                t,
                "st-history",
                "Keep watch history",
                "Remember what you play so you can find it again.",
                c.playback_history,
                |s| s.edit_config(|c| c.playback_history = !c.playback_history),
            )],
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_value_spans_the_range() {
        assert_eq!(step_value(0.0, 5, 60), 5);
        assert_eq!(step_value(1.0, 5, 60), 60);
        assert_eq!(step_value(0.5, 1, 16), 9);
        assert_eq!(step_value(-3.0, 1, 16), 1);
        assert_eq!(step_value(9.0, 1, 16), 16);
    }

    #[test]
    fn toggle_in_adds_and_removes_once() {
        let mut v = vec!["sponsor".to_owned()];
        toggle_in(&mut v, "intro");
        assert_eq!(v, ["sponsor", "intro"]);
        toggle_in(&mut v, "sponsor");
        assert_eq!(v, ["intro"]);
    }

    #[test]
    fn every_section_has_a_label_and_icon() {
        assert_eq!(SECTIONS.len(), 9);
        for (s, l) in SECTIONS {
            assert!(!l.is_empty());
            let _ = icon_for(s);
            assert!(!blurb(s).is_empty());
        }
    }

    #[test]
    fn sponsor_categories_are_valid_for_the_engine() {
        let c = vidcue_core::Config {
            playback_sponsor_categories: SPONSOR_CATEGORIES
                .iter()
                .map(|(_, v)| (*v).to_owned())
                .collect(),
            ..Default::default()
        };
        assert!(c.validate().is_ok());
    }
}
