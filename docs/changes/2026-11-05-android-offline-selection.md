# M9: Android offline multi-selection

- **Date:** 2026-11-05
- **Milestone:** M9
- **Platforms:** Android and shared engine
- **Status:** fixture/unit verified

## What it does

The native Android Offline list now has a Select action. In selection mode,
checkboxes and row taps toggle saved songs/videos without starting playback.
The selected count, Select all / Deselect all, Cancel selection and Android Back
support choosing a batch. Selection follows saved-copy IDs and drops entries
that disappear from the current completed-download list. Changing library
sections clears selection; transfer controls remain separate.

Selected items can be deleted together after confirmation, added to an existing
offline playlist, or used to create a new offline playlist. Deletion uses the
existing removal adapter for each selected ID, including its saved folder copy;
playlist references stay intact, as with single-item removal. Cancelling the
confirmation preserves the selection.

Playlist additions preserve library order, copy the exact offline IDs, and skip
URLs already present (including alternate downloaded copies of the same song).
Creating a playlist submits its initial members in one Rust mutation so validation
and persistence are atomic. Creating an empty playlist remains supported. The
single-item Add to offline playlist → New playlist path now also carries the song
into the new playlist.

## Evidence

- `cargo test -p vidcue-mobile --lib --locked`: 52 passed, including initial-member
  creation and rejected malformed creation leaving the persisted library intact.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: 467 passed, 0 failed, 31 existing opt-in ignores.
  These checks and the Android checks below were rerun after applying the change
  to latest main (61ef259), preserving the new downloaded-playlist operation.
- `cargo fmt --all` and `git diff --check`: clean.
- Android JVM regressions cover member order, saved-copy identity, source metadata
  preservation and duplicate filtering. A Compose instrumentation case covers
  selection, deletion cancellation, populated playlist creation, adding a batch to
  an existing playlist, and deselecting all.

- `:app:compileDebugKotlin :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest :app:lintDebug`: passed;
  170 JVM tests, no failures or skips. Lint reported 0 errors and 96 warnings; the UI regression
  test compiled but was not executed.

## Not verified and limits

No emulator or physical-device acceptance has been run for this change. In
particular, actual multi-file deletion and phone/tablet layout still need device
qualification. This change does not publish or install an Android release.

## For the plan and parity tables

M9 Android Offline: users can select multiple saved songs/videos for confirmed
deletion, addition to an existing offline playlist, or atomic creation of a
populated playlist. Duplicate URLs are skipped while saved-copy IDs and library
order are preserved. Rust and Android unit checks passed; instrumentation source
compiled. Emulator and physical-device acceptance remain open.
