# Android playlist download membership and offline playlists (M9)

- **Date:** 2026-11-05
- **Milestone:** M9
- **Platforms:** Android and shared engine
- **Status:** fixture/unit verified

## What it does

Playlist parsing now uses the playlist track-list renderer (including continuation
responses) when present. Recommendation carousels and their pagination tokens no
longer become playlist members, so the collection downloader cannot expand their
suggested playlists into hundreds of unrelated downloads. Artist/channel pages
without a playlist track list retain their existing collection expansion.

The native Android full-playlist download action supplies the playlist title and
requests automatic offline playlist creation. Preparation saves the ordered,
deduplicated, non-live members with the same format-specific identities used by
the download index. A deterministic source identity makes retries and repeat
downloads update one offline playlist. The library refreshes when it is created;
existing offline playback only plays completed members. Pending or failed items
remain listed so a partial download does not silently change playlist membership.
Existing offline library validation and capacity limits apply; metadata errors
are reported before queuing media. The empty-state copy explains automatic creation.

## Evidence

Checks run:

- `cargo test -p vidcue-providers --lib --locked`: 92 passed before the final
  terminal-playlist regression was added.
- `cargo test -p vidcue-providers --lib collection_tests --locked`: all 9 passed
  after that addition. Fixtures cover 20 tracks beside 300 recommendations,
  recommendation playlists, track pagination and a terminal track list with a
  recommendation continuation.
- `cargo test -p vidcue-mobile --lib --locked`: all 52 passed, including ordered,
  persisted, transactional and idempotent downloaded-playlist updates.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: passed on the latest `main` base before PR creation.
- Added `PrepareWorkerTest` for download-ID matching, ordering, deduplication,
  live/nested-entry exclusion and stable playlist identity across retries/formats.
  Android JVM tests could not run: after configuring Java's HTTP proxy and fetching
  Gradle, `:app:testDebugUnitTest` stopped because no Android SDK is installed.

## Not verified and limits

No APK, emulator, physical-device or live YouTube Music acceptance was performed.
Previously queued downloads and existing preparation checkpoint catalogs are not
rewritten by this change.

## For the plan and parity tables

M9 playlist downloads: playlist track lists and their own continuations define
download membership; recommendation shelves are excluded. Full playlist downloads
automatically create/update a named offline playlist with ordered download IDs.
Rust fixture/unit checks passed; Android compilation/JVM checks are delegated to
PR CI because the authoring workspace lacks an Android SDK. Device and live
YouTube Music acceptance remain open.
