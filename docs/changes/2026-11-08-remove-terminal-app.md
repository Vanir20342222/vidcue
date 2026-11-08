# 2026-11-08: Remove the terminal app, backend-only workspace

Owner request: clear all TUI code and prepare the repository as a backend plus
Android app, with a README covering only Android for now.

## Removed

- `apps/desktop` — the ratatui/crossterm terminal app (the only TUI binary).
- `install.sh`, `install-rust.sh`, `packaging/` (Arch package, desktop entry, icon)
  and `examples/config.toml` — desktop-install packaging.
- `tools/package-release.sh`, `tools/rollback-install.sh` and
  `.github/workflows/release-candidate.yml` — desktop release packaging.
- `tools/test-native-playback.sh`, `tools/test-playback-matrix.sh`,
  `tools/test-youtube.sh`, `tools/test-single-video.sh`, `tools/test-network.sh`,
  `tools/test-app-audit.sh`, `tools/native-playback-urls.txt` and
  `.github/workflows/youtube-live.yml` — acceptance harnesses that drove the
  desktop binary (`target/debug/vidcue`) in a PTY.
- `tools/ci-install-media.sh`, `tools/test-ci-install-media.sh` — only used by the
  removed desktop live-YouTube workflow.

## Kept and moved

- `tools/license_bundle.rs` restored as
  `crates/vidcue-mobile/examples/license_bundle.rs` (its only consumer after the
  removal is the Android license inventory example used by `tools/android/build.sh`).
- `crates/vidcue-services` stays whole (store, playback, downloads, library,
  MPRIS, diagnostics): backend for the Android worker and a future desktop GUI.

## Changed

- Workspace members drop `apps/desktop`.
- `tools/ci/should-run.sh` path filters and `tools/ci/test-should-run.sh`
  expectations updated for the removed paths.
- `README.md` rewritten for the Android app only.
- `AGENTS.md` desktop-source pointer updated to record the removal.

## Checks

- `cargo check` passes; `cargo test --workspace --locked` passes (offline).
- `bash tools/ci/test-should-run.sh`: all checks passed.
- Android Gradle build not run in this environment (no SDK configured locally).
