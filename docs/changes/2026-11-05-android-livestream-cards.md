# Mark YouTube livestreams as live on Android feeds (M9)

- **Date:** 2026-11-05
- **PR:** #99
- **Milestone:** M9
- **Platforms:** shared engine (providers), Android
- **Status:** fixture/unit verified

## What it does
YouTube broadcasts that are on air now are marked `live` on modern lockup cards (home, subscriptions,
search, related), not only on classic renderers. On Android a card then shows the red pulsing **Live**
badge instead of a duration and drops the Download and Preview actions. Auto-play-next contexts skip it,
and playback uses the existing live path (HLS, no seek bar or quality menu, reconnects on disconnect).
Classic cards now also accept the `LIVE` time-status overlay, not just the `LIVE_NOW` badge.
Detection reads only badge style fields, so "LIVE" in a title never marks a card, and upcoming premieres
and archived broadcasts stay ordinary videos. If any duplicate of a video on the page has the badge,
that video is marked live. With no creator name, the mini player reads "Live stream" for a live video
and keeps "Live radio" for stations.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Live, premiere, archive, title-only, playlist and duplicate cards | `cargo test -p vidcue-providers --locked current_broadcast` | pass | fixture/unit |
| Workspace | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` | pass | fixture/unit |

## Not verified and limits
- No Android SDK in the authoring environment: the one-line `Shell.kt` label change was not compiled,
  and no emulator or device run was made.
- No live-service check. The lockup badge shape (`thumbnailBadgeViewModel.badgeStyle =
  THUMBNAIL_OVERLAY_BADGE_STYLE_LIVE`) follows current YouTube responses but was tested with synthetic
  fixtures only. Playing an actual live broadcast through the native extractor on a device is still
  unverified.
- Lockup cards still show no viewer count ("N watching").

## For the plan and parity tables
- M9 Android: live YouTube broadcasts on lockup and classic feed cards carry the Live badge and live
  playback semantics (fixture/unit verified; device and live-service playback of a broadcast pending).
