//! Downloads: the shared download worker (`vidcue_services::download`, native extractor or
//! yt-dlp per the download backend setting) behind a desktop queue with transfers, the
//! downloaded library and offline playlists. Finished files play locally when
//! "prefer offline" is on. The queue survives restarts; interrupted jobs resume.

use std::collections::HashMap;
use std::path::Path;

use rune::{Color, Cx, El, ScrollSpec, Transition, Tx, col, el, icon, row, spacer, text};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use vidcue_core::{DownloadQueue, Job, JobEvent, JobState, Media, MediaKind};

use crate::app::{App, FeedRoute, ToastAction, ToastKind};
use crate::backend::{Backend, op};
use crate::icons as ic;
use crate::images::{Size, thumb_url};
use crate::theme::{sz, ty};
use crate::ui::widgets::{self, Kind, button};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DlTab {
    #[default]
    Transfers,
    Library,
    Playlists,
}

pub struct Downloads {
    pub queue: DownloadQueue,
    cancels: HashMap<Uuid, CancellationToken>,
    pub tab: DlTab,
    dirty: bool,
    loaded: bool,
}

impl Downloads {
    pub fn new(be: &Backend) -> Self {
        let mut queue: DownloadQueue = std::fs::read(be.paths.file("downloads.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        for j in &mut queue.jobs {
            j.restart_recovery();
        }
        Self {
            queue,
            cancels: HashMap::new(),
            tab: DlTab::default(),
            dirty: false,
            loaded: false,
        }
    }

    pub fn active_count(&self) -> usize {
        self.queue
            .jobs
            .iter()
            .filter(|j| j.state.active() || j.state == JobState::Queued)
            .count()
    }

    pub fn overall_fraction(&self) -> f64 {
        let active: Vec<&Job> = self
            .queue
            .jobs
            .iter()
            .filter(|j| j.state.active() || j.state == JobState::Queued)
            .collect();
        if active.is_empty() {
            return 0.0;
        }
        active
            .iter()
            .map(|j| j.progress.fraction().unwrap_or(0.0))
            .sum::<f64>()
            / active.len() as f64
    }

    fn save(&mut self, be: &Backend) {
        self.dirty = false;
        if be.offline {
            return;
        }
        if let Ok(bytes) = serde_json::to_vec(&self.queue) {
            let _ = vidcue_providers::paths::atomic_write(&be.paths.file("downloads.json"), &bytes);
        }
    }
}

pub fn enter(app: &mut App) {
    crate::ui::library::refresh_playlists(app);
}

/// Resumes interrupted jobs after settings load.
pub fn resume(app: &mut App) {
    app.downloads.loaded = true;
}

/// A finished file for `media`, when prefer-offline is on.
pub fn local_file(app: &App, media: &Media, audio: bool) -> Option<String> {
    if !app.config.prefer_offline {
        return None;
    }
    app.downloads
        .queue
        .jobs
        .iter()
        .filter(|j| {
            j.state == JobState::Done
                && j.media.url == media.url
                && (j.options.audio == audio || !audio)
        })
        .flat_map(|j| j.outputs.iter())
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
}

fn options(app: &App, audio: bool) -> vidcue_core::DownloadOptions {
    let mut config = app.config.clone();
    app.be.paths.shared.fill_directories(&mut config);
    let mut o = config.download.clone();
    o.audio = audio;
    o
}

pub fn add(app: &mut App, media: &Media, audio: bool) {
    if app.queue_contains(media, audio) {
        app.toast(ToastKind::Info, "That's already in Downloads");
        return;
    }
    let job = Job::new(media.clone(), options(app, audio));
    app.downloads.queue.jobs.push(job);
    app.downloads.dirty = true;
    app.toast_with(
        ToastKind::Success,
        format!("Downloading “{}”", media.title),
        Some(("View".into(), ToastAction::OpenDownloads)),
    );
}

impl App {
    fn queue_contains(&self, media: &Media, audio: bool) -> bool {
        self.downloads.queue.jobs.iter().any(|j| {
            j.media.url == media.url
                && j.options.audio == audio
                && !matches!(j.state, JobState::Error | JobState::Cancelled)
        })
    }
}

/// Downloads a whole playlist or album as one grouped job (yt-dlp/native playlist mode).
pub fn add_collection(app: &mut App, route: &FeedRoute) {
    let state = app.feeds.get(route);
    let title = state
        .map(|s| s.title.clone())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| route.title.clone());
    let items: Vec<Media> = state
        .map(|s| {
            s.items
                .iter()
                .filter(|m| !m.collection())
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if items.is_empty() {
        app.toast(ToastKind::Info, "Nothing to download yet");
        return;
    }
    let audio = route.tab == crate::app::Tab::Music;
    let group = Uuid::new_v4().to_string();
    let n = items.len();
    for (i, m) in items.into_iter().enumerate() {
        if app.queue_contains(&m, audio) {
            continue;
        }
        let mut job = Job::new(m, options(app, audio));
        job.group_id = Some(group.clone());
        job.group_title = Some(title.clone());
        job.entry_index = Some(i);
        app.downloads.queue.jobs.push(job);
    }
    app.downloads.dirty = true;
    app.toast_with(
        ToastKind::Success,
        format!("Downloading {n} items from “{title}”"),
        Some(("View".into(), ToastAction::OpenDownloads)),
    );
}

pub fn enqueue_collection(app: &mut App, route: &FeedRoute) {
    app.be.call(
        op(
            "enqueue_collection",
            json!({ "request": route.request, "music": route.tab.music() }),
        ),
        |app, r, _| match r {
            Ok(v) => {
                app.toast(
                    ToastKind::Success,
                    format!("Added {} to the queue", v["added"].as_u64().unwrap_or(0)),
                );
                app.player.refresh_up_next(&app.be);
            }
            Err(e) => app.toast(ToastKind::Error, e.message),
        },
    );
}

fn start(app: &mut App, id: Uuid) {
    let Some(job) = app.downloads.queue.jobs.iter_mut().find(|j| j.id == id) else {
        return;
    };
    job.invalidate(JobState::Resolving);
    job.error = None;
    let job = job.clone();
    let cancel = CancellationToken::new();
    app.downloads.cancels.insert(id, cancel.clone());
    let mut config = app.config.clone();
    app.be.paths.shared.fill_directories(&mut config);
    let paths = app.be.paths.shared.clone();
    let be = app.be.clone();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<JobEvent>(64);
    app.be.rt.spawn(vidcue_services::download::execute(
        job, config, paths, None, cancel, tx,
    ));
    app.be.rt.spawn(async move {
        while let Some(ev) = rx.recv().await {
            be.land(Box::new(move |app, _| {
                let finished = matches!(
                    ev.update,
                    vidcue_core::JobUpdate::Finished | vidcue_core::JobUpdate::Failed { .. }
                );
                let id = ev.id;
                if app.downloads.queue.apply(ev) {
                    app.downloads.dirty |= finished;
                }
                if finished {
                    app.downloads.cancels.remove(&id);
                    if let Some(j) = app.downloads.queue.jobs.iter().find(|j| j.id == id) {
                        match j.state {
                            JobState::Done => {
                                let t = j.media.title.clone();
                                app.toast(ToastKind::Success, format!("Downloaded “{t}”"));
                            }
                            JobState::Error => {
                                let t = j.media.title.clone();
                                app.toast(ToastKind::Error, format!("“{t}” failed to download"));
                            }
                            _ => {}
                        }
                    }
                }
            }));
        }
    });
}

/// Starts queued jobs up to the concurrency setting; saves the queue when it changed.
pub fn tick(app: &mut App) -> bool {
    if !app.downloads.loaded || app.offline() {
        return false;
    }
    let limit = app.config.concurrency.clamp(1, 16);
    let now = vidcue_core::schedule::unix_now();
    let running = app
        .downloads
        .queue
        .jobs
        .iter()
        .filter(|j| j.state.active())
        .count();
    let ready: Vec<Uuid> = app
        .downloads
        .queue
        .jobs
        .iter()
        .filter(|j| j.ready(now))
        .map(|j| j.id)
        .take(limit.saturating_sub(running))
        .collect();
    let started = !ready.is_empty();
    for id in ready {
        start(app, id);
    }
    if app.downloads.dirty || started {
        let be = app.be.clone();
        app.downloads.save(&be);
    }
    started
}

pub fn pause(app: &mut App, id: Uuid) {
    if let Some(c) = app.downloads.cancels.remove(&id) {
        c.cancel();
    }
    if let Some(j) = app.downloads.queue.jobs.iter_mut().find(|j| j.id == id) {
        j.invalidate(JobState::Paused);
    }
    app.downloads.dirty = true;
}

pub fn resume_job(app: &mut App, id: Uuid) {
    if let Some(j) = app.downloads.queue.jobs.iter_mut().find(|j| j.id == id) {
        j.invalidate(JobState::Queued);
        j.not_before = None;
        j.error = None;
    }
    app.downloads.dirty = true;
}

pub fn remove(app: &mut App, id: Uuid, delete_files: bool) {
    pause(app, id);
    if delete_files && let Some(j) = app.downloads.queue.jobs.iter().find(|j| j.id == id) {
        for p in &j.outputs {
            let _ = std::fs::remove_file(p);
        }
    }
    app.downloads.queue.remove(id);
    app.downloads.dirty = true;
}

fn state_label(j: &Job) -> String {
    match j.state {
        JobState::Queued => "Waiting".into(),
        JobState::Resolving => "Preparing".into(),
        JobState::Downloading => {
            let mut s = String::new();
            if let Some(f) = j.progress.fraction() {
                s.push_str(&format!("{:.0}%", f * 100.0));
            }
            if let Some(sp) = j.progress.speed {
                s.push_str(&format!(" · {}/s", bytes(sp as u64)));
            }
            if let Some(eta) = j.progress.eta {
                s.push_str(&format!(" · {} left", crate::player::mpv::clock(eta)));
            }
            if s.is_empty() {
                "Downloading".into()
            } else {
                s
            }
        }
        JobState::Merging => "Merging audio and video".into(),
        JobState::Converting => "Converting".into(),
        JobState::Backoff => format!("Retrying (attempt {})", j.attempt + 1),
        JobState::Paused => "Paused".into(),
        JobState::Cancelled => "Cancelled".into(),
        JobState::Done => "Downloaded".into(),
        JobState::Error => j.error.clone().unwrap_or_else(|| "Failed".into()),
    }
}

pub fn bytes(n: u64) -> String {
    let n = n as f64;
    if n >= 1e9 {
        format!("{:.1} GB", n / 1e9)
    } else if n >= 1e6 {
        format!("{:.1} MB", n / 1e6)
    } else {
        format!("{:.0} kB", n / 1e3)
    }
}

pub fn view(app: &App, cx: &mut Cx<App>) -> El {
    let t = &app.theme;
    let d = &app.downloads;
    let tabs = widgets::segmented(
        t,
        "dl-tabs",
        &["Transfers", "Library", "Offline playlists"],
        d.tab as usize,
        |s, i| {
            s.downloads.tab = [DlTab::Transfers, DlTab::Library, DlTab::Playlists][i];
            if i == 2 {
                crate::ui::library::refresh_playlists(s);
            }
        },
    );
    let dir = {
        let mut c = app.config.clone();
        app.be.paths.shared.fill_directories(&mut c);
        c.download.directory.clone()
    };
    let open_dir = dir.clone();
    let head = row()
        .w_full()
        .gap(16.0)
        .center_y()
        .flex_wrap()
        .child(widgets::page_title(
            t,
            "Downloads",
            Some("Save videos and music to this computer, and play them offline"),
        ))
        .child(spacer())
        .child(
            button(t, "Open folder", Some(&ic::FOLDER), Kind::Ghost).on_press_tx(
                move |_: &mut App, tx: &mut Tx| {
                    open_folder(tx, &open_dir);
                },
            ),
        )
        .child(tabs);
    let body = match d.tab {
        DlTab::Transfers => transfers(app),
        DlTab::Library => library(app, cx),
        DlTab::Playlists => crate::ui::library::playlists_grid(app, cx),
    };
    col()
        .key("downloads")
        .w_full()
        .h_full()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(
            col()
                .w_full()
                .px(sz::PAGE_PAD)
                .pt(sz::TOPBAR + 24.0)
                .pb(sz::DOCK + 48.0)
                .gap(20.0)
                .child(head)
                .child(body),
        )
}

fn transfers(app: &App) -> El {
    let t = &app.theme;
    let jobs: Vec<&Job> = app
        .downloads
        .queue
        .jobs
        .iter()
        .filter(|j| j.state != JobState::Done)
        .collect();
    if jobs.is_empty() {
        return widgets::empty_state(
            t,
            &ic::DOWNLOAD,
            "No transfers",
            "Download a video or song from its ⋮ menu, or a whole playlist from its page.",
        );
    }
    let mut c = col().gap(10.0);
    for j in jobs {
        c = c.child(transfer_row(app, j));
    }
    c
}

fn transfer_row(app: &App, j: &Job) -> El {
    let t = &app.theme;
    let id = j.id;
    let frac = j.progress.fraction().unwrap_or(0.0) as f32;
    let size = if j.options.audio {
        Size::SQUARE
    } else {
        Size::CARD
    };
    let mut art = el()
        .w(if j.options.audio { 56.0 } else { 100.0 })
        .h(56.0)
        .radius(sz::R_MD)
        .clip_rounded(sz::R_MD)
        .fill(t.raised)
        .center();
    if let Some(img) = app.images.get(&thumb_url(&j.media, size), size) {
        art = art.child(
            el().w_full()
                .h_full()
                .pixels_fit(img.pixels(), rune::SurfaceFit::Fill),
        );
    } else {
        art = art.child(
            icon(
                if j.options.audio {
                    &ic::MUSIC
                } else {
                    &ic::PLAY
                },
                20.0,
            )
            .color(t.faint),
        );
    }
    let failed = j.state == JobState::Error;
    let status_color = if failed { t.error } else { t.muted };
    let bar = el().w_full().h(4.0).radius(2.0).fill(t.overlay).child(
        el().w_frac(frac.clamp(0.0, 1.0))
            .h(4.0)
            .radius(2.0)
            .fill(if failed { t.error } else { t.accent })
            .motion(rune::tok::motion::GLIDE),
    );
    let mut words = col()
        .grow(1.0)
        .shrink()
        .gap(6.0)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text(&j.media.title)
                        .font(ty::BODY_STRONG)
                        .color(t.text)
                        .truncate()
                        .shrink(),
                )
                .child(widgets::badge(
                    if j.options.audio { "Audio" } else { "Video" },
                    t.raised,
                    t.muted,
                )),
        )
        .child(
            text(state_label(j))
                .font(ty::SMALL)
                .color(status_color)
                .max_lines(2),
        );
    if let Some(g) = &j.group_title {
        words = words.child(text(format!("From “{g}”")).font(ty::SMALL).color(t.faint));
    }
    if !failed && j.state != JobState::Paused {
        words = words.child(bar);
    }
    let mut actions = row().gap(4.0).center_y();
    match j.state {
        JobState::Paused | JobState::Error | JobState::Cancelled => {
            actions = actions.child(
                widgets::icon_button(
                    t,
                    if failed { &ic::REFRESH } else { &ic::PLAY },
                    if failed { "Retry" } else { "Resume" },
                    36.0,
                )
                .on_press(move |s: &mut App| resume_job(s, id)),
            );
        }
        _ => {
            actions = actions.child(
                widgets::icon_button(t, &ic::PAUSE_OUTLINE, "Pause", 36.0)
                    .on_press(move |s: &mut App| pause(s, id)),
            );
        }
    }
    actions = actions.child(
        widgets::icon_button(t, &ic::CLOSE, "Remove", 36.0)
            .on_press(move |s: &mut App| remove(s, id, false)),
    );
    row()
        .key(format!("job-{id}"))
        .w_full()
        .padding(12.0)
        .gap(14.0)
        .center_y()
        .radius(sz::R_CARD)
        .fill(t.surface)
        .border(
            1.0,
            if failed {
                t.error.alpha(0.3)
            } else {
                t.hairline
            },
        )
        .transition(Transition::offset(0.0, 10.0).combined(Transition::opacity()))
        // Square audio covers sit in the same slot as video thumbnails so titles line up.
        .child(row().w(100.0).h(56.0).center_y().child(art))
        .child(words)
        .child(actions)
}

fn library(app: &App, cx: &mut Cx<App>) -> El {
    let t = &app.theme;
    let done: Vec<&Job> = app
        .downloads
        .queue
        .jobs
        .iter()
        .filter(|j| j.state == JobState::Done)
        .rev()
        .collect();
    if done.is_empty() {
        return widgets::empty_state(
            t,
            &ic::FOLDER,
            "Nothing downloaded yet",
            "Finished downloads appear here and play without a connection.",
        );
    }
    let width = cx.layout_size("downloads").map(|s| s[0]).unwrap_or(1200.0) - sz::PAGE_PAD * 2.0;
    let cols = ((width + 20.0) / 300.0).floor().max(1.0) as usize;
    let card_w = ((width - 20.0 * (cols as f32 - 1.0)) / cols as f32).floor();
    let mut grid = row().w_full().gap(20.0).flex_wrap();
    for j in done {
        let mut media = j.media.clone();
        let file = j.outputs.iter().find(|p| p.is_file()).cloned();
        let missing = file.is_none();
        let id = j.id;
        let path = file.clone();
        let audio = j.options.audio;
        if audio {
            media.kind = MediaKind::Song;
        }
        let size = if audio { Size::SQUARE } else { Size::CARD };
        let mut art = el()
            .w(card_w)
            .aspect_ratio(16.0 / 9.0)
            .radius(sz::R_CARD)
            .clip_rounded(sz::R_CARD)
            .fill(t.raised)
            .center();
        if let Some(img) = app.images.get(&thumb_url(&j.media, size), size) {
            art = art.child(
                el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                    .pixels_fit(img.pixels(), rune::SurfaceFit::Fill),
            );
        }
        art = art.child(
            el().pin(None, None, Some(8.0), Some(8.0))
                .child(widgets::badge(
                    if missing {
                        "Missing"
                    } else if audio {
                        "Audio"
                    } else {
                        "Offline"
                    },
                    if missing {
                        t.error
                    } else {
                        Color::BLACK.alpha(0.7)
                    },
                    Color::WHITE,
                )),
        );
        let size_text = path
            .as_deref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| bytes(m.len()))
            .unwrap_or_default();
        let open = path.clone();
        let card = col()
            .key(format!("dl-{id}"))
            .w(card_w)
            .gap(8.0)
            .hover_group()
            .press_scale(0.975)
            .cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| {
                if let Some(p) = &path {
                    let mut m = media.clone();
                    m.url = p.to_string_lossy().into_owned();
                    play_local(s, m, audio);
                }
            })
            .child(art)
            .child(
                text(&j.media.title)
                    .font(ty::TITLE)
                    .color(t.text)
                    .max_lines(2),
            )
            .child(
                row()
                    .gap(6.0)
                    .center_y()
                    .child(
                        text(format!("{} · {}", j.media.creator, size_text))
                            .font(ty::SMALL)
                            .color(t.muted)
                            .grow(1.0)
                            .truncate(),
                    )
                    .child(el().group_opacity(0.0, 1.0).child(
                        widgets::icon_button(t, &ic::FOLDER, "Show in folder", 30.0).on_press_tx(
                            move |_: &mut App, tx: &mut Tx| {
                                if let Some(dir) = open.as_deref().and_then(Path::parent) {
                                    open_folder(tx, dir);
                                }
                            },
                        ),
                    ))
                    .child(
                        el().group_opacity(0.0, 1.0).child(
                            widgets::icon_button(t, &ic::TRASH, "Delete file", 30.0)
                                .on_press(move |s: &mut App| remove(s, id, true)),
                        ),
                    ),
            );
        grid = grid.child(card);
    }
    grid
}

fn play_local(app: &mut App, media: Media, audio: bool) {
    crate::player::session::play(
        app,
        media,
        crate::player::session::PlayOptions {
            audio: Some(audio),
            context: crate::player::session::Context::None,
            ..Default::default()
        },
    );
}

/// Opens `dir` in the file manager. `Url::from_directory_path` writes a valid `file:///` URL
/// on every platform: `file:///C:/Users/…` on Windows, with spaces and `#` percent-encoded.
fn open_folder(tx: &mut Tx, dir: &Path) {
    if let Ok(url) = url::Url::from_directory_path(dir) {
        let _ = tx.open_url(url.as_str());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_sizes_read_naturally() {
        assert_eq!(bytes(1_500), "2 kB");
        assert_eq!(bytes(12_300_000), "12.3 MB");
        assert_eq!(bytes(2_000_000_000), "2.0 GB");
    }
}
