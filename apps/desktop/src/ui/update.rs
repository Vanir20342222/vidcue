//! The update offer: a dialog when the startup check finds a release, and the status block in
//! Settings → About. Both show the same actions, so an update started in one shows in the other.

use std::sync::atomic::Ordering;

use rune::{El, Transition, Tx, col, el, row, text};
use vidcue_services::update::Release;

use crate::app::App;
use crate::icons as ic;
use crate::theme::{Theme, ty};
use crate::ui::widgets::{self, Kind, badge, button};
use crate::updater::{self, Method, State, VERSION};

fn note(t: &Theme, ok: bool, msg: &str) -> El {
    let c = if ok { t.success } else { t.error };
    text(msg).font(ty::SMALL).color(c).wrap()
}

fn release_page(t: &Theme, release: &Release, kind: Kind, label: &str) -> El {
    let url = release.url.clone();
    button(t, label, Some(&ic::EXTERNAL), kind).on_press_tx(move |_: &mut App, tx: &mut Tx| {
        let _ = tx.open_url(&url);
    })
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}

/// What "Update now" will do on this copy, in one sentence.
fn how(method: &Method) -> &'static str {
    match method {
        Method::WindowsInstaller => {
            "Vidcue downloads the installer, closes, updates and opens again. Settings, history and downloads are kept."
        }
        Method::Source { .. } => {
            "Vidcue builds the release from source with install.sh, as when you installed it. This takes a few minutes and needs git and Rust."
        }
        Method::Manual => {
            if cfg!(windows) {
                "This is a portable copy: download Vidcue-portable.zip or the installer from the release page."
            } else {
                "This copy was not installed with install.sh, so Vidcue cannot replace it. Update it the way you installed it."
            }
        }
    }
}

/// The buttons for an available release.
fn offer_actions(app: &App, release: &Release) -> El {
    let t = &app.theme;
    let mut r = row().gap(10.0).center_y();
    if app.update.method == Method::Manual {
        r = r.child(release_page(t, release, Kind::Primary, "Open release page"));
    } else {
        r = r
            .child(
                button(t, "Update now", Some(&ic::DOWNLOAD), Kind::Primary)
                    .key("update-now")
                    .on_press(updater::install),
            )
            .child(release_page(t, release, Kind::Secondary, "Release notes"));
    }
    r
}

/// The progress of an install under way.
fn installing(app: &App, release: &Release) -> El {
    let t = &app.theme;
    let mut c = col().w_full().gap(8.0);
    match app.update.method {
        Method::WindowsInstaller => {
            let got = app.update.progress.bytes.load(Ordering::Relaxed);
            let total = release.windows_installer().map(|a| a.size).unwrap_or(0);
            let frac = if total > 0 {
                (got as f64 / total as f64) as f32
            } else {
                0.0
            };
            let label = if total > 0 && got >= total {
                "Starting the installer. Vidcue will close and open again.".to_owned()
            } else if total > 0 {
                format!(
                    "Downloading {} · {} of {}",
                    release.tag,
                    megabytes(got),
                    megabytes(total)
                )
            } else {
                format!("Downloading {}…", release.tag)
            };
            c = c.child(text(label).font(ty::SMALL).color(t.text)).child(
                el().w_full().h(4.0).radius(2.0).fill(t.overlay).child(
                    el().w_frac(frac.clamp(0.0, 1.0))
                        .h(4.0)
                        .radius(2.0)
                        .fill(t.accent)
                        .motion(rune::tok::motion::GLIDE),
                ),
            );
        }
        _ => {
            let line = app.update.progress.line.lock().unwrap().clone();
            c = c
                .child(
                    text(format!(
                        "Building {} from source. This can take a few minutes; you can keep using Vidcue.",
                        release.tag
                    ))
                    .font(ty::SMALL)
                    .color(t.text)
                    .wrap(),
                )
                .child(
                    text(if line.is_empty() { "Starting…".into() } else { line })
                        .font(ty::NUM)
                        .color(t.muted)
                        .truncate(),
                );
        }
    }
    c
}

/// Settings → About's status block under "Check now".
pub fn status(app: &App) -> El {
    let t = &app.theme;
    match &app.update.state {
        State::Idle => text("").font(ty::SMALL),
        State::Checking => text("Checking for updates…").font(ty::SMALL).color(t.muted),
        State::UpToDate => note(
            t,
            true,
            &format!("You are on the latest version ({VERSION})."),
        ),
        State::Failed(e) => note(t, false, e),
        State::Available(release) => {
            let mut c = col()
                .w_full()
                .gap(8.0)
                .transition(Transition::opacity())
                .child(
                    row()
                        .gap(10.0)
                        .center_y()
                        .child(badge("NEW", t.accent, t.on_accent))
                        .child(
                            text(format!("{} is available", release.tag))
                                .font(ty::BODY_STRONG)
                                .color(t.text),
                        ),
                );
            for n in &release.notes {
                c = c.child(text(n).font(ty::SMALL).color(t.muted).wrap());
            }
            c.child(
                text(how(&app.update.method))
                    .font(ty::SMALL)
                    .color(t.muted)
                    .wrap(),
            )
            .child(offer_actions(app, release))
        }
        State::Installing(release) => installing(app, release),
        State::Installed(release) => installed(app, release),
    }
}

fn installed(app: &App, release: &Release) -> El {
    let t = &app.theme;
    col()
        .w_full()
        .gap(10.0)
        .child(note(
            t,
            true,
            &format!(
                "{} is installed. It starts when you restart Vidcue.",
                release.tag
            ),
        ))
        .child(
            row().child(
                button(t, "Restart now", Some(&ic::REFRESH), Kind::Primary)
                    .key("update-restart")
                    .on_press_tx(|s: &mut App, tx: &mut Tx| updater::restart(s, tx)),
            ),
        )
}

/// The dialog the startup check opens when a newer release exists.
pub fn prompt(app: &App) -> El {
    let t = &app.theme;
    let Some(release) = app.update.release() else {
        return el().key("update-prompt-none");
    };
    let r = 24.0;
    let mut body = col()
        .w_full()
        .gap(12.0)
        .child(
            row()
                .gap(10.0)
                .center_y()
                .child(badge("NEW", t.accent, t.on_accent))
                .child(text("Update available").font(ty::H3).color(t.text)),
        )
        .child(
            text(format!(
                "Vidcue {} is out. You have {VERSION}.",
                release.version
            ))
            .font(ty::BODY)
            .color(t.text)
            .wrap(),
        );
    for n in &release.notes {
        body = body.child(text(n).font(ty::SMALL).color(t.muted).wrap());
    }
    let mut footer = row().w_full().gap(10.0).center_y();
    match &app.update.state {
        State::Available(_) => {
            body = body.child(
                text(how(&app.update.method))
                    .font(ty::SMALL)
                    .color(t.muted)
                    .wrap(),
            );
            footer = footer
                .child(offer_actions(app, release))
                .child(widgets::spacer_el())
                .child(
                    button(t, "Skip this version", None, Kind::Ghost)
                        .key("update-skip")
                        .on_press(updater::skip),
                )
                .child(
                    button(t, "Later", None, Kind::Secondary)
                        .key("update-later")
                        .on_press(|s: &mut App| s.update.prompt = false),
                );
        }
        State::Installing(_) => {
            body = body.child(installing(app, release));
            footer = footer.child(widgets::spacer_el()).child(
                button(t, "Hide", None, Kind::Secondary)
                    .key("update-hide")
                    .on_press(|s: &mut App| s.update.prompt = false),
            );
        }
        State::Installed(_) => {
            body = body.child(installed(app, release));
            footer = footer.child(widgets::spacer_el()).child(
                button(t, "Later", None, Kind::Secondary)
                    .key("update-later")
                    .on_press(|s: &mut App| s.update.prompt = false),
            );
        }
        _ => {}
    }
    let card = col()
        .key("update-card")
        .w(620.0)
        .radius(r)
        .clip_rounded(r)
        .padding(28.0)
        .gap(20.0)
        .fill(t.surface.alpha(0.96))
        .backdrop_blur(30.0)
        .border(1.0, t.border)
        .shadow(24.0, 80.0, t.shadow)
        .transition(Transition::scale(0.95).combined(Transition::opacity()))
        .role("dialog")
        .label("Update available")
        .child(body)
        .child(footer);
    el().key("update-prompt")
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .center()
        .fill(t.scrim)
        .transition(Transition::opacity())
        .child(card)
        .modal(false, |s: &mut App| s.update.prompt = false)
}
