# Next from Music and Radio browsing (M5/M9)

- **Date:** 2026-11-04
- **Milestones:** M5 / M9
- **Platforms:** Android, shared providers and mobile engine
- **Status:** Rust fixture/unit verified; Android compilation/device checks pending

## What it does

Selecting an individual Music song now requests its YouTube Music song-radio
queue. The shared provider parses `playlistPanelVideoRenderer` entries, including
wrapped entries, instead of overlooking the watch-next playlist panel. Unavailable
and duplicate tracks, alternate video counterparts and menu actions are excluded.
Songs replayed from Library also retain the Music provider context. Standard
video related requests retain their existing payload.

Radio selections capture a bounded snapshot of the visible station/track list.
Next visits the entries after the selection and then those before it once, so
selecting the last visible station still offers the other stations. Live stations
are admitted into this explicit radio context; Next changes stations, while a
broadcast disconnect still uses reconnect behavior. Radio track libraries retain
their existing full-collection loading. Explicit queued items retain priority;
failed candidates remain available until played or deliberately skipped.

Context preparation still starts at playback handoff, including during buffering.
An explicit Next retries a failed context request using its retained descriptor.
Snapshots stay in memory and are invalidated by a new selection; they do not
replace the saved queue or depend on YouTube's related-autoplay preference.

## Evidence

PR preparation applied the fix cleanly to latest `main` `f189ce9`.
`cargo fmt --all -- --check`, strict workspace/all-target Clippy and
`cargo test --workspace --locked` pass on that base: **402 passed, 0 failed,
31 existing opt-in ignores**. The earlier authoring checks below were performed
on implementation base `f7feb4a` and are retained with their original limits.

- The loopback Music-related regression passes: checks the WEB_REMIX mix request,
  nested/wrapped playlist-panel songs, artist/duration parsing, unavailable and
  duplicate filtering, and the unchanged video request/renderer path.
- `cargo test -p ytui-mobile -p ytui-providers --locked`: 126 passed, including
  live-station admission, explicit queue priority, idempotent preparation without
  restoring consumed stations, and stale-context rejection.
- Workspace formatting, `git diff --check` and final
  `cargo clippy --workspace --all-targets --locked -- -D warnings` pass.
- `cargo build --release --locked`: passed (cold optimized host build,
  24m 40s). `target/release/ytui --version` passes. This is not an Android APK.
- `cargo test --workspace --locked` initially stopped at the existing terminal
  smoke test's `Workspaces` heading assertion. All three terminal tests passed
  when rerun without changing their source or timing budgets. The release build
  was given lower scheduling priority for the rerun; contention is not proven as
  the cause. The remaining suites were run separately: core/services 120 passed
  (15 existing opt-in ignores), desktop vinyl 7 passed, desktop doctests passed.
  Across those runs, 398 distinct workspace tests passed, with 31 existing
  opt-in ignores. The initial full-workspace command itself did not pass.
- Added four Android JVM context-selection cases and a Media3 instrumentation
  case for leaving a buffering live station through Next. These were not run.

## Not verified and limits

`tools/android/build.sh` stops with `Set ANDROID_HOME to an installed Android SDK.`
No Android SDK, emulator or connected device is present, so Android app/test
compilation, JVM execution, instrumentation, audible playback and physical-device
acceptance are unverified. Public YouTube/Music/radio services are outside the
workspace's permitted network destinations. No live-service result, APK or release
is claimed. Radio snapshots contain at most 500 currently visible entries; they
do not fetch additional station-directory pages or control songs within a live
broadcast.

## For the plan and parity tables

M5/M9: Music related continuation reads WEB_REMIX song-radio playlist panels.
Android Radio Next uses a bounded visible-list context that admits live stations
and retains explicit queue priority. Rust fixture/unit evidence is separate from
the pending Android compile, emulator and physical/live acceptance checks.
