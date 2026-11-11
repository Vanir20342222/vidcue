//! Updating Vidcue: the startup offer, Settings → About's check and "Update now".
//!
//! Checking only reads GitHub's public release list. Nothing is downloaded or installed until
//! the user presses "Update now", which installs the release the way this copy was installed:
//!
//! - Windows, installed with `VidcueSetup`: download the release's installer, verify it against
//!   GitHub's SHA-256 digest, run it silently and close; the installer reopens Vidcue.
//! - Linux, installed with `install.sh` into `~/.local/bin`: rebuild the release tag from source
//!   with that script, then offer a restart.
//! - Anything else (a portable copy, a development build, a distribution package): open the
//!   release page, since Vidcue cannot know how to replace itself.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rune::Tx;
use vidcue_services::update::{self, Release};

use crate::app::App;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How this copy of Vidcue can update itself.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Method {
    /// Installed with `VidcueSetup-*.exe`: run the new installer over it.
    WindowsInstaller,
    /// Installed by `install.sh`: rebuild the release from source, then start `launcher`.
    Source { launcher: PathBuf },
    /// Vidcue cannot replace this copy itself; the user downloads the release.
    #[default]
    Manual,
}

impl Method {
    pub fn detect() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        detect(std::env::current_exe().ok(), home.as_deref(), cfg!(windows))
    }
}

fn detect(exe: Option<PathBuf>, home: Option<&Path>, windows: bool) -> Method {
    let Some(exe) = exe else {
        return Method::Manual;
    };
    if windows {
        // Inno Setup leaves its uninstaller beside the program; a portable copy has none.
        let installed = exe
            .parent()
            .is_some_and(|dir| dir.join("unins000.exe").is_file());
        return if installed {
            Method::WindowsInstaller
        } else {
            Method::Manual
        };
    }
    // Linux names a replaced executable "<path> (deleted)"; the launcher is still the same path.
    let text = exe.to_string_lossy();
    let exe = PathBuf::from(text.strip_suffix(" (deleted)").unwrap_or(&text));
    match home {
        Some(home) if exe == home.join(".local/bin/vidcue") => Method::Source { launcher: exe },
        _ => Method::Manual,
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum State {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Installing(Release),
    /// Linux: the new build is in place and starts on the next launch or "Restart".
    Installed(Release),
    Failed(String),
}

/// What an install in progress reports to the view.
#[derive(Default)]
pub struct Progress {
    /// Bytes of the installer received so far.
    pub bytes: AtomicU64,
    /// The last line the source build printed.
    pub line: Mutex<String>,
}

#[derive(Default)]
pub struct Updater {
    pub state: State,
    /// The offer dialog is open (the startup check found a release).
    pub prompt: bool,
    pub method: Method,
    pub progress: Arc<Progress>,
    /// What the view last showed of `progress`, so `tick` redraws only on change.
    seen: (u64, usize),
}

impl Updater {
    pub fn new(headless: bool) -> Self {
        Self {
            method: if headless {
                Method::Manual
            } else {
                Method::detect()
            },
            ..Default::default()
        }
    }

    pub fn release(&self) -> Option<&Release> {
        match &self.state {
            State::Available(r) | State::Installing(r) | State::Installed(r) => Some(r),
            _ => None,
        }
    }
}

/// Where a downloaded installer waits to run. Cleared at the next start.
fn download_dir() -> PathBuf {
    std::env::temp_dir().join("vidcue-update")
}

/// Once settings are loaded: clear an earlier installer and, unless the user turned it off,
/// look for a newer release to offer.
pub fn startup(app: &mut App) {
    if app.offline() {
        return;
    }
    let _ = std::fs::remove_dir_all(download_dir());
    if app.config.update_check {
        check(app, true);
    }
}

/// Asks GitHub for a newer release. At startup a failure stays quiet and a release the user
/// skipped is not offered again; a check from Settings reports everything.
pub fn check(app: &mut App, startup: bool) {
    if matches!(app.update.state, State::Checking | State::Installing(_)) {
        return;
    }
    if app.offline() {
        app.update.state = State::Failed("Update checks are off in headless renders".into());
        return;
    }
    app.update.state = State::Checking;
    let proxy = app.config.proxy.clone();
    app.be.task(
        async move {
            update::check(update::API_BASE, proxy.as_deref(), VERSION)
                .await
                .map_err(|e| format!("{e:#}"))
        },
        move |app, out, _| {
            app.update.state = match out {
                Ok(Some(release)) => {
                    app.update.prompt =
                        startup && app.prefs.skipped_update.as_deref() != Some(&release.tag);
                    State::Available(release)
                }
                Ok(None) => State::UpToDate,
                Err(_) if startup => State::Idle,
                Err(e) => State::Failed(format!("The check failed: {e}")),
            }
        },
    );
}

/// "Update now".
pub fn install(app: &mut App) {
    let State::Available(release) = app.update.state.clone() else {
        return;
    };
    let progress = app.update.progress.clone();
    progress.bytes.store(0, Ordering::Relaxed);
    progress.line.lock().unwrap().clear();
    app.update.seen = (0, 0);
    match app.update.method.clone() {
        Method::Manual => {}
        Method::WindowsInstaller => {
            let Some(asset) = release.windows_installer().cloned() else {
                app.update.state = State::Failed(format!(
                    "{} has no Windows installer. Download it from the release page.",
                    release.tag
                ));
                return;
            };
            app.update.state = State::Installing(release);
            let proxy = app.config.proxy.clone();
            app.be.task(
                async move {
                    update::download(&asset, &download_dir(), proxy.as_deref(), &progress.bytes)
                        .await
                        .map_err(|e| format!("{e:#}"))
                },
                |app, out, tx| match out.and_then(|path| {
                    run_installer(&path).map_err(|e| format!("The installer did not start: {e}"))
                }) {
                    // The installer waits for Vidcue to close, replaces it and opens the new one.
                    Ok(()) => tx.quit(),
                    Err(e) => app.update.state = State::Failed(format!("The update failed: {e}")),
                },
            );
        }
        Method::Source { .. } => {
            app.update.state = State::Installing(release.clone());
            app.be.blocking(
                move || build_from_source(&release, &progress),
                |app, out, _| {
                    app.update.state = match (out, app.update.state.clone()) {
                        (Ok(()), State::Installing(release)) => State::Installed(release),
                        (Ok(()), state) => state,
                        (Err(e), _) => State::Failed(format!("The update failed: {e}")),
                    }
                },
            );
        }
    }
}

/// Starts the freshly installed build and quits this one.
pub fn restart(app: &mut App, tx: &mut Tx) {
    let Method::Source { launcher } = &app.update.method else {
        return;
    };
    let mut command = std::process::Command::new(launcher);
    command
        // Rune's relaunch handshake: the new process waits for this one to exit first.
        .env("RUNE_RELAUNCH_AFTER", std::process::id().to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    match command.spawn() {
        Ok(_) => tx.quit(),
        Err(e) => app.toast(
            crate::app::ToastKind::Error,
            format!("Vidcue could not restart: {e}. Open it again from the app launcher."),
        ),
    }
}

/// "Skip this version": no startup offer until a newer release appears.
pub fn skip(app: &mut App) {
    app.update.prompt = false;
    if let Some(tag) = app.update.release().map(|r| r.tag.clone()) {
        app.edit_prefs(|p| p.skipped_update = Some(tag));
    }
}

/// Redraws while an install reports progress.
pub fn tick(app: &mut App, tx: &mut Tx) -> bool {
    if !matches!(app.update.state, State::Installing(_)) {
        return false;
    }
    tx.wake_after(Duration::from_millis(250));
    let p = &app.update.progress;
    let now = (
        p.bytes.load(Ordering::Relaxed),
        p.line.lock().unwrap().len(),
    );
    std::mem::replace(&mut app.update.seen, now) != now
}

#[cfg(windows)]
fn run_installer(path: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let flags = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
    let mut command = std::process::Command::new(path);
    command.args(update::WINDOWS_INSTALLER_ARGS);
    // Outside any job this process runs in, so closing Vidcue does not end the installer.
    if command
        .creation_flags(flags | CREATE_BREAKAWAY_FROM_JOB)
        .spawn()
        .is_ok()
    {
        return Ok(());
    }
    command.creation_flags(flags).spawn().map(drop)
}

#[cfg(not(windows))]
fn run_installer(_path: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "the Windows installer only runs on Windows",
    ))
}

/// Runs the release's `install.sh` from a fresh clone, reporting its output line by line.
fn build_from_source(release: &Release, progress: &Progress) -> Result<(), String> {
    use std::io::BufRead;
    let mut command = update::install_command(release).map_err(|e| e.to_string())?;
    // Started from an app launcher, PATH often lacks rustup's ~/.cargo/bin.
    if let Some(home) = std::env::var_os("HOME") {
        let cargo_bin = PathBuf::from(home).join(".cargo/bin");
        let path = std::env::var_os("PATH").unwrap_or_default();
        if cargo_bin.is_dir() && !std::env::split_paths(&path).any(|p| p == cargo_bin) {
            let joined = std::env::join_paths(
                std::iter::once(cargo_bin).chain(std::env::split_paths(&path)),
            )
            .map_err(|e| e.to_string())?;
            command.env("PATH", joined);
        }
    }
    let (reader, writer) = std::io::pipe().map_err(|e| e.to_string())?;
    command
        .stdin(std::process::Stdio::null())
        .stdout(writer.try_clone().map_err(|e| e.to_string())?)
        .stderr(writer);
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    // The command holds the pipe's write ends; drop them so the read ends at the child's exit.
    drop(command);
    let mut tail: Vec<String> = Vec::new();
    let mut reader = std::io::BufReader::new(reader);
    let mut buf = Vec::new();
    while reader
        .read_until(b'\n', &mut buf)
        .map_err(|e| e.to_string())?
        > 0
    {
        let line = String::from_utf8_lossy(&buf).trim().to_owned();
        buf.clear();
        if line.is_empty() {
            continue;
        }
        *progress.line.lock().unwrap() = line.clone();
        tail.push(line);
        if tail.len() > 4 {
            tail.remove(0);
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "the build stopped ({status}): {}",
            tail.join(" · ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_updates_only_installed_copies() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("vidcue.exe");
        assert_eq!(detect(Some(exe.clone()), None, true), Method::Manual);
        std::fs::write(dir.path().join("unins000.exe"), b"").unwrap();
        assert_eq!(detect(Some(exe), None, true), Method::WindowsInstaller);
        assert_eq!(detect(None, None, true), Method::Manual);
    }

    #[test]
    fn linux_updates_only_the_install_script_launcher() {
        let home = Path::new("/home/u");
        let launcher = PathBuf::from("/home/u/.local/bin/vidcue");
        assert_eq!(
            detect(Some(launcher.clone()), Some(home), false),
            Method::Source {
                launcher: launcher.clone()
            }
        );
        // Still recognised after install.sh replaced the running binary.
        assert_eq!(
            detect(
                Some("/home/u/.local/bin/vidcue (deleted)".into()),
                Some(home),
                false
            ),
            Method::Source { launcher }
        );
        for other in [
            "/home/u/src/vidcue/target/release/vidcue-desktop",
            "/usr/bin/vidcue",
            "/home/other/.local/bin/vidcue",
        ] {
            assert_eq!(
                detect(Some(other.into()), Some(home), false),
                Method::Manual,
                "{other}"
            );
        }
        assert_eq!(
            detect(Some(PathBuf::from("/x")), None, false),
            Method::Manual
        );
    }

    fn release(tag: &str) -> Release {
        Release {
            tag: tag.into(),
            version: tag.trim_start_matches("desktop-").into(),
            name: tag.into(),
            url: "https://example.invalid/r".into(),
            notes: vec!["New things".into()],
            assets: Vec::new(),
        }
    }

    #[test]
    fn skipping_records_the_offered_tag_and_closes_the_offer() {
        let mut app = App::offline_for_tests();
        app.update.state = State::Available(release("desktop-9.0.0"));
        app.update.prompt = true;
        skip(&mut app);
        assert!(!app.update.prompt);
        assert_eq!(app.prefs.skipped_update.as_deref(), Some("desktop-9.0.0"));
    }

    #[test]
    fn install_without_a_windows_installer_explains_why() {
        let mut app = App::offline_for_tests();
        app.update.method = Method::WindowsInstaller;
        app.update.state = State::Available(release("desktop-9.0.0"));
        install(&mut app);
        assert!(
            matches!(&app.update.state, State::Failed(e) if e.contains("no Windows installer")),
            "{:?}",
            app.update.state
        );
    }

    #[test]
    fn manual_copies_never_install() {
        let mut app = App::offline_for_tests();
        app.update.state = State::Available(release("desktop-9.0.0"));
        install(&mut app);
        assert_eq!(app.update.state, State::Available(release("desktop-9.0.0")));
    }

    #[test]
    fn an_odd_tag_never_reaches_the_build() {
        let error = build_from_source(&release("v1;rm"), &Progress::default()).unwrap_err();
        assert!(error.contains("unusual release tag"), "{error}");
    }
}
