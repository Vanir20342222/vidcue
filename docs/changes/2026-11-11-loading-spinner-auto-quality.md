# Loading spinner and opt-in auto video quality (M9)

- **Date:** 2026-11-11
- **PR:** not opened (branch pushed for review)
- **Milestone:** M9
- **Platforms:** Android, shared mobile engine
- **Status:** unit verified; not run on a device or against live YouTube

## What it does

Resolves GitHub issue #143.

- **Loading spinner.** The music cover shows a spinner while a track loads or
  stalls, and video shows one when playback stalls mid-stream (the existing
  preparing overlay still covers the initial load). Stalls shorter than 400 ms
  show nothing, so normal jitter does not flicker. `PlayerLoading.kt`.
- **Auto quality.** The player's Video quality sheet has an "Auto" entry above
  "Best". It is opt-in; the default is unchanged. Auto offers every H.264 MP4
  rendition up to 1080p plus the best AAC audio as one in-memory DASH
  presentation (`indexed_playback::adaptive`), so ExoPlayer can step down on a
  poor connection without re-resolving. Without a complete indexed set it plays
  the capped 1080p selector.
- **Engine field names.** The Vidcue rename (#82) changed the fields this crate
  reads to `vidcue_init_range`, `vidcue_index_range`, `vidcue_duration_ms` and
  `vidcue_playback_route`, but the pinned yt-dlp-rs engine still writes the
  `ytui_` names (kept for compatibility per AGENTS.md). Since then no indexed
  DASH presentation was built, for the existing selected-pair path as well as
  for Auto, and startup timings recorded the route as `direct-media`.
  `indexed_playback::engine_field` now reads either name, `vidcue_` first.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Mobile engine tests | `cargo test -p vidcue-mobile --locked` | Pass, including the ladder, auto selector and `ytui_` field tests | Unit |
| Workspace | `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked` | Pass | Unit |
| Android | `gradlew :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest :app:lintDebug` | Pass | Local |

## Not verified and limits

- Not run on a device or against live YouTube. Re-enabling the indexed DASH
  path restores the behaviour that existed before the rename; it should be
  checked on a device before release.
- Auto adapts only between H.264 MP4 renditions up to 1080p; VP9/AV1 and higher
  resolutions stay on the explicit quality choices.

## For the plan and parity tables
- Player: opt-in "Auto" video quality (adaptive H.264 up to 1080p) and a delayed
  loading spinner for music loads and video stalls (Android, unit verified).
