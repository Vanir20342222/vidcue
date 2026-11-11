# Update the desktop app from inside Vidcue (M9)

- **Date:** 2026-11-11
- **PR:** #<number> <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** desktop (Linux, Windows)
- **Status:** fixture/unit verified

## What it does

When Vidcue opens and a newer desktop release exists, a dialog offers it: **Update now**,
**Release notes**, **Later** or **Skip this version** (a skipped release is not offered at startup
again; newer ones are). Settings → About → **Check now** shows the same offer and the install
progress. "Check at startup" (on by default) was already in Settings but did nothing until now.

"Update now" installs the release the way this copy was installed:

- **Windows, installed with `VidcueSetup`:** downloads the release's `VidcueSetup-*.exe` (with a
  progress bar), checks its SHA-256 against the digest GitHub publishes for the asset, starts it
  with `/SILENT /CLOSEAPPLICATIONS /RELAUNCH=1` and closes Vidcue. The installer replaces the
  files and opens the new version (new `[Run]` entry in `tools/windows/vidcue.iss`). Downloads
  are refused unless they come from this project's release URLs, have a GitHub digest and match it.
- **Linux, installed with `install.sh` (`~/.local/bin/vidcue`):** runs the existing source updater
  (clone the release tag, run its `install.sh`) in the background, showing the build's latest
  output line, then offers **Restart now**. `~/.cargo/bin` is added to `PATH` when the app was
  started from a launcher that lacks it.
- **Anything else** (portable Windows zip, a development build, a distribution package): only
  **Open release page**, with a sentence saying why.

Nothing is downloaded or installed without the user pressing "Update now".

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Updater unit tests | `cargo test -p vidcue-services --lib update::` | 9 passed: asset/digest parsing, checksum match, tampered file removed, HTTP error, foreign URL / odd file name / missing digest refused | fixture/unit verified |
| Desktop tests | `cargo test --locked -p vidcue-desktop` | 88 passed: install-method detection (Windows uninstaller, `~/.local/bin/vidcue` incl. `(deleted)`), skip, no-installer error, headless dialog test (skip, later, progress, restart button) | fixture/unit verified |
| Lint and format | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean | fixture/unit verified |
| Headless renders | `vidcue-desktop --snap … --scene update-offer / update-downloading / settings-update-building` | reviewed; the dialog was widened after the first render clipped "Later" | fixture |
| Real release metadata | `gh api repos/Vanir20342222/vidcue/releases/tags/desktop-0.2.0-alpha.16` | both assets carry `digest: sha256:…` and `browser_download_url` under the accepted prefix | live (read only) |

## Not verified and limits

- No real update has been installed yet on either platform. The Windows path needs a release
  built from this branch plus a newer one to update to: the first release with this code must be
  installed by hand, because older versions do not have the updater.
- The `cfg(windows)` code (detached installer start) was not compiled locally (no MinGW toolchain
  on this machine); the Windows release workflow compiles it.
- Inno Setup's silent install with `/CLOSEAPPLICATIONS` and the `/RELAUNCH=1` restart are
  untested on Windows.
- Linux updates still build from source: they need git, Rust and the build libraries
  (libmpv, WebKitGTK 4.1) and take minutes. There is no prebuilt Linux release asset to download.
- The portable Windows zip is not updated in place.

## For the plan and parity tables

- REMAINING_WORK "Updates/packaging": the desktop app checks for releases at startup and installs
  them on request (Windows installer with SHA-256 verification; Linux source rebuild through
  `install.sh`). Real-machine update acceptance on both platforms is still open.
