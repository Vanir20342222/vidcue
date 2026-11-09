# M9: exclude locally watched videos from Android recommendations

- **Date:** 2026-11-09
- **Milestone:** M9
- **Platforms:** Android / shared mobile engine
- **Status:** fixture/unit verified

## Cause and change

Android rendered provider recommendations without comparing them with local playback
history. Prepared pages, pagination and navigation snapshots could therefore keep
showing a video after actual playback started. The Rust related playback context
also selected watched entries.

- Load retained history before the initial feed. Refilter visible discovery feeds
  when playback history updates, and filter Home, category and related pages when
  publishing fresh, cached, paginated or restored content.
- Compare video IDs, including IDs recovered from watch, Music, short-link and Shorts
  URLs when metadata omits the ID. Timestamp/list parameters do not change identity.
- Recheck Rust related contexts when displaying Up next and selecting the next
  candidate. This also handles contexts prepared before history changes.
- Preserve intentional replay through search, saved/history views, finite playlists
  and explicit queue entries. Keep raw provider pages cached so revisiting them
  after clearing history can restore their entries.

This uses existing retained local playback history (bounded to 100 entries / 2 MiB),
not a synchronized account-wide watched database. Recording remains governed by the
existing playback-history preference and actual-start logic; previews do not record
history. Videos absent from retained local history cannot be excluded by this fix.

## Verification

Both regression paths failed before their fixes:

- `:app:testDebugUnitTest --tests dev.ytui.android.WatchedRecommendationsTest`:
  expected only the unseen video, but both watched and unseen cards appeared.
- `cargo test -p vidcue-mobile --lib watched_videos_are_not -- --nocapture`:
  expected one related candidate, received two.

After the fix:

- Android `:app:testDebugUnitTest`: **249 passed**, no failures/errors/skips.
  Includes fresh/cache/pagination/Back filtering, history-update removal, cleared
  history, URL variants and deliberate replay; existing startup/context tests pass.
- `cargo test -p vidcue-mobile --offline`: **54 passed** (37 unit + 17 integration).
  The regression exercises recorded history after engine restart, related Next and
  Up next, finite contexts and explicit queue replay.
- `cargo fmt --all` and `git diff --check`: pass.

Android checks used the already-installed JDK 21 at
`/home/vanir/.local/share/mise/installs/java/openjdk-21` and SDK at
`/home/vanir/android-sdk`. The initial default JDK 27 attempt failed in Android's
JDK-image transform; no system configuration was changed.

No APK publication, installation, emulator/physical-device run or live-service
acceptance was performed. This is an M9 source fix, not a completed mobile milestone.

## Pre-PR repository checks

- `cargo fmt --all -- --check`: pass.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: pass.
- `cargo test --workspace --locked`: 300 passed, 15 existing opt-in tests ignored.
- Android app and instrumentation Kotlin compilation, JVM suite and `:app:lintDebug`: pass.

## For the plan and parity tables

M9 Android discovery now excludes videos found in retained local playback history
from Home/category/related cards and related Next/autoplay, including cached,
paginated and Back-restored pages. Intentional replay stays available in explicit
queues, finite playlists, search and saved/history views. JVM and Rust fixtures
verify this behavior; physical-device/live acceptance remains unverified.
