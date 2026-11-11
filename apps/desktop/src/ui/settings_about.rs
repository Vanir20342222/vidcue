//! Settings → Alpha and About.

use rune::{Color, El, TextStyle, Transition, Tx, col, el, icon, row, text};

use super::{UpdateState, field_row, group, note, stacked};
use crate::app::{App, ToastKind};
use crate::icons as ic;
use crate::theme::{sz, ty};
use crate::ui::widgets::{self, Kind};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// ---- Alpha ----

pub(super) fn alpha(app: &App) -> El {
    let t = &app.theme;
    let accepted = app.prefs.alpha_notice_accepted;
    let notice = col()
        .key("st-alpha-notice")
        .w_full()
        .gap(12.0)
        .padding(20.0)
        .radius(sz::R_PANEL)
        .fill(t.warning.alpha(0.10))
        .border(1.0, t.warning.alpha(0.35))
        .transition(Transition::opacity())
        .child(
            row()
                .gap(10.0)
                .center_y()
                .child(icon(&ic::WARNING, 20.0).color(t.warning))
                .child(text("Before you turn this on").font(ty::H3).color(t.text)),
        )
        .child(
            text("Movies streams films hosted by third parties, through the providers behind Cinecat. Vidcue does not host or check any of it.")
                .font(ty::BODY)
                .color(t.text)
                .wrap(),
        )
        .child(
            text("Availability is unreliable: providers come and go, and some serve videos that are blocked or will not play.")
                .font(ty::BODY)
                .color(t.muted)
                .wrap(),
        )
        .child(
            text("You are responsible for having the right to watch this content where you live.")
                .font(ty::BODY)
                .color(t.muted)
                .wrap(),
        )
        .child(
            row().child(
                widgets::button(t, "I understand", Some(&ic::CHECK), Kind::Primary)
                    .key("st-alpha-accept")
                    .on_press(|s: &mut App| s.edit_prefs(|p| p.alpha_notice_accepted = true)),
            ),
        );

    let on = app.prefs.alpha_movies && accepted;
    let control = if accepted {
        widgets::switch(t, "st-alpha-movies", on)
            .label("Movies (Cinecat)")
            .on_press(|s: &mut App| s.edit_prefs(|p| p.alpha_movies = !p.alpha_movies))
    } else {
        widgets::switch(t, "st-alpha-movies", false)
            .label("Movies (Cinecat)")
            .opacity(0.4)
            .on_press(|s: &mut App| {
                s.toast(ToastKind::Info, "Read and accept the notice above first.")
            })
    };
    let save_key = widgets::button(t, "Save", None, Kind::Primary).on_press(|s: &mut App| {
        let key = s.settings.tmdb.text.trim().to_owned();
        s.edit_prefs(|p| p.tmdb_key = key);
        s.toast(ToastKind::Success, "TMDB key saved");
    });
    let mut out = col().w_full().gap(24.0);
    if !accepted {
        out = out.child(notice);
    }
    out.child(group(
        t,
        "Movies",
        vec![widgets::setting_row(
            t,
            "Movies (Cinecat)",
            "Adds a Movies destination to the sidebar. Alpha: expect rough edges.",
            control,
        )],
    ))
    .child(group(
        t,
        "TMDB",
        vec![stacked(
            t,
            "API key",
            "Optional. Enables movie search and posters. Get a free key at themoviedb.org. It is stored on this computer only.",
            field_row(t, &app.settings.tmdb, "TMDB API key (v3)", |s| &mut s.settings.tmdb, save_key),
        )],
    ))
}

// ---- About ----

pub const SHORTCUTS: &[(&str, &str)] = &[
    ("Space or K", "Play / pause"),
    ("J / L", "Seek back / forward (seek step)"),
    ("← / →", "Seek 5 seconds"),
    ("↑ / ↓", "Volume up / down"),
    ("0 – 9", "Jump to 0% – 90%"),
    ("M", "Mute"),
    ("F", "Full screen"),
    ("T", "Theater mode"),
    ("C", "Toggle captions"),
    ("N / P", "Next / previous"),
    ("< / >", "Slower / faster"),
    ("/ or Ctrl+K", "Search"),
    ("Ctrl+,", "Settings"),
    ("Ctrl+[ / Ctrl+]", "Back / forward"),
    ("Ctrl+B", "Collapse the sidebar"),
    ("Esc", "Close a menu, leave full screen or the player"),
];

const CREDITS: &[(&str, &str)] = &[
    ("Rune", "The GPU interface toolkit Vidcue is drawn with."),
    ("rune-fx", "Mesh, glow and shimmer effect layers."),
    ("libmpv", "Embedded video and audio playback."),
    (
        "yt-dlp-rs",
        "The native extractor behind watching and downloads.",
    ),
    (
        "yt-dlp and FFmpeg",
        "Optional external tools for downloads and conversion.",
    ),
];

fn check_update(app: &mut App) {
    if app.offline() {
        app.settings.update =
            UpdateState::Failed("Update checks are off in headless renders".into());
        return;
    }
    app.settings.update = UpdateState::Checking;
    let proxy = app.config.proxy.clone();
    app.be.task(
        async move {
            vidcue_services::update::check(
                vidcue_services::update::API_BASE,
                proxy.as_deref(),
                VERSION,
            )
            .await
            .map_err(|e| e.to_string())
        },
        |app, out, _| {
            app.settings.update = match out {
                Ok(Some(r)) => UpdateState::Available {
                    tag: r.tag,
                    url: r.url,
                    notes: r.notes,
                },
                Ok(None) => UpdateState::UpToDate,
                Err(e) => UpdateState::Failed(e),
            }
        },
    );
}

pub(super) fn view(app: &App) -> El {
    let t = &app.theme;
    let header = el()
        .w_full()
        .h(150.0)
        .radius(sz::R_PANEL)
        .clip_rounded(sz::R_PANEL)
        .child(
            crate::fx::mesh(11, 0.0)
                .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .radius(sz::R_PANEL),
        )
        .child(
            row()
                .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .px(28.0)
                .gap(18.0)
                .center_y()
                .child(
                    el().size(64.0, 64.0)
                        .radius(18.0)
                        .fill(Color::BLACK.alpha(0.28))
                        .border(1.0, Color::WHITE.alpha(0.25))
                        .center()
                        .child(icon(&ic::PLAY, 28.0).color(Color::WHITE)),
                )
                .child(
                    col()
                        .gap(2.0)
                        .child(
                            text("Vidcue")
                                .font(TextStyle::new(30.0, 760.0).tracking(-0.5))
                                .color(Color::WHITE),
                        )
                        .child(
                            text(format!("Version {VERSION}"))
                                .font(ty::BODY)
                                .color(Color::WHITE.alpha(0.85)),
                        ),
                ),
        );

    let status: El = match &app.settings.update {
        UpdateState::Idle => text("").font(ty::SMALL),
        UpdateState::Checking => text("Checking for updates…").font(ty::SMALL).color(t.muted),
        UpdateState::UpToDate => note(
            t,
            true,
            &format!("You are on the latest version ({VERSION})."),
        ),
        UpdateState::Failed(e) => note(t, false, &format!("The check failed: {e}")),
        UpdateState::Available { tag, url, notes } => {
            let url = url.clone();
            let mut c = col()
                .w_full()
                .gap(8.0)
                .transition(Transition::opacity())
                .child(
                    row()
                        .gap(10.0)
                        .center_y()
                        .child(widgets::badge("NEW", t.accent, t.on_accent))
                        .child(
                            text(format!("{tag} is available"))
                                .font(ty::BODY_STRONG)
                                .color(t.text),
                        ),
                );
            for n in notes {
                c = c.child(text(n).font(ty::SMALL).color(t.muted).wrap());
            }
            c.child(
                row().child(
                    widgets::button(t, "Open release page", Some(&ic::EXTERNAL), Kind::Secondary)
                        .on_press_tx(move |_: &mut App, tx: &mut Tx| {
                            let _ = tx.open_url(&url);
                        }),
                ),
            )
        }
    };
    let checking = app.settings.update == UpdateState::Checking;
    let mut shortcuts = col().w_full();
    for (i, (keys, what)) in SHORTCUTS.iter().enumerate() {
        shortcuts = shortcuts.child(
            row()
                .key(format!("st-sc-{i}"))
                .w_full()
                .h(34.0)
                .gap(16.0)
                .center_y()
                .child(
                    row().w(190.0).child(
                        el().px(8.0)
                            .py(3.0)
                            .radius(6.0)
                            .fill(t.raised)
                            .border(1.0, t.hairline)
                            .child(text(*keys).font(ty::NUM).color(t.text)),
                    ),
                )
                .child(text(*what).font(ty::BODY).color(t.muted)),
        );
    }
    let mut credits = col().w_full();
    for (name, what) in CREDITS {
        credits = credits.child(
            row()
                .w_full()
                .py(8.0)
                .gap(16.0)
                .child(text(*name).font(ty::BODY_STRONG).color(t.text).w(190.0))
                .child(
                    text(*what)
                        .font(ty::BODY)
                        .color(t.muted)
                        .wrap()
                        .shrink()
                        .grow(1.0),
                ),
        );
    }
    col()
        .w_full()
        .gap(24.0)
        .child(header)
        .child(group(
            t,
            "Updates",
            vec![
                widgets::setting_row(
                    t,
                    "Check for updates",
                    "Reads the public release list. Nothing is installed without you.",
                    widgets::button(
                        t,
                        if checking { "Checking…" } else { "Check now" },
                        Some(&ic::REFRESH),
                        Kind::Primary,
                    )
                    .on_press(check_update),
                ),
                super::toggle(
                    t,
                    "st-updatecheck",
                    "Check at startup",
                    "Offer new releases when Vidcue opens.",
                    app.config.update_check,
                    |s| s.edit_config(|c| c.update_check = !c.update_check),
                ),
                col().w_full().py(8.0).child(status),
            ],
        ))
        .child(group(
            t,
            "Welcome",
            vec![widgets::setting_row(
                t,
                "Onboarding",
                "Take the first-run tour again.",
                widgets::button(t, "Replay", Some(&ic::PLAY), Kind::Secondary)
                    .on_press(|s: &mut App| s.onboarding = Some(0)),
            )],
        ))
        .child(group(
            t,
            "Keyboard shortcuts",
            vec![col().w_full().py(8.0).child(shortcuts)],
        ))
        .child(group(
            t,
            "Built with",
            vec![col().w_full().py(6.0).child(credits)],
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver_like() {
        assert!(VERSION.split('.').count() >= 3, "{VERSION}");
    }

    #[test]
    fn shortcuts_have_unique_keys() {
        let mut keys: Vec<_> = SHORTCUTS.iter().map(|(k, _)| *k).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), SHORTCUTS.len());
    }
}
