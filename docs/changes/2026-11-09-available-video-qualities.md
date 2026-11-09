# Available video qualities (M9)

- **Date:** 2026-11-09
- **PR:** #pending
- **Milestone:** M9
- **Platforms:** Android / shared engine
- **Status:** fixture/unit verified

## What it does

The Android playback quality menu now lists the distinct video heights returned by the extractor, from highest to lowest, alongside Best. Resolution choices come from the format data already fetched for playback. Selecting a listed quality resolves the video using that height as the existing maximum-resolution cap.

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Rust tests | `cargo test --workspace --locked` | 284 passed, 15 ignored | unit |
| Rust lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed | unit |
| Rust formatting | `cargo fmt --all -- --check` | Passed | unit |
| Diff whitespace | `git diff --check` | Passed | unit |

## Not verified and limits

Android compilation, emulator and physical-device behavior, and live-service playback were not verified. The Gradle wrapper could not download Gradle 8.14.3 because the connection to services.gradle.org failed. The available-resolution list depends on heights exposed by the extractor; device codec support can still constrain playback.

## For the plan and parity tables

Android playback quality selection displays every distinct, usable video resolution returned by extraction, sorted from highest to lowest, plus Best. Selecting a resolution retains the existing maximum-height and codec preference behavior. Android build and device qualification remain open.
