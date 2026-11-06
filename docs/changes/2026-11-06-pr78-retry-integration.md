# Preserve interrupted playlist resume and failed-only Restart (M9)

- **Date:** 2026-11-06
- **PR:** #78
- **Milestone:** M9
- **Platforms:** Android, shared-provider fixture
- **Status:** implemented; Rust and retry-policy unit verified; Android CI pending

## What it does

The owner chose option 1 on 2026-11-06: preserve both current main's stable-key
checkpoint resume and #78's failed-entry-only retry after a completed pass.
Interrupted jobs keep their original cursor, checkpoint and bounded retry behavior.
A completed pass records failed non-live entries in a separate private retry checkpoint;
manual Restart reuses the same job key and row. This avoids putting a potentially large
media list into WorkManager's 10 KiB input bundle. A completed failure retains the
checkpoint needed by Restart; success and removal purge both checkpoint files and all
progress keys. Live-only failures retain an empty retry list instead of re-resolving the
whole playlist.

Failed-only retry requests disable offline-playlist creation so retrying a subset cannot
replace the full saved playlist's membership. The original request's audio and quality
settings are retained. Existing main's broader lockup-avatar parser is preserved, and
#78's original synthetic avatar fixture remains alongside its newer tests.

The empty-search keyboard dismissal fix remains, so an empty, unsubmitted search exits
when its keyboard closes and exposes Settings again.

## Evidence

- `cargo fmt --all -- --check`: passed.
- `cargo test --workspace --locked`: 456 passed, zero failed, 31 existing opt-in ignores.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `PreparationPolicyTest` compiled via the installed JDK compiler module and JUnit 4.13.2:
  all 9 tests passed (retry limits, checkpoint cursor, failed-entry lists and cleanup keys).
- Android app/instrumentation compile, JVM tests and lint: pending GitHub CI; this
  environment has no Android SDK.
- Added Robolectric regressions for repeated failed-only passes, full-playlist preservation,
  successful cleanup, large retry lists, interrupted retry cursors and live-only failures.
- Kept the existing retry/cursor and avatar regressions.

## Not verified and limits

No emulator, physical-device or live-service check was run for this integration. PR CI
compiles instrumentation sources but does not execute the large-text navigation test.
The prior 2026-11-04 note describes the original PR; its fresh-job Restart design is
superseded by this stable-key integration.

## For the plan and parity tables

- M9 Android: interrupted playlist preparation resumes with the existing job key and cursor;
  a completed pass retries only failed non-live entries while preserving the full saved
  playlist. Required retry state stays until success or removal. Device checks pending.
