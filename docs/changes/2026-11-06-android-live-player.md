# Live stream controls in the Android video player (M9)

- **Date:** 2026-11-06
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** unit verified (pure logic only; APK not built in this environment)

## What it does
A YouTube live stream played in the Android video player looked like a finished one-hour video:
the time pill read "1:00:28 / 1:00:00" and the bar sat pinned past its end. Now:

- **Live detection from the stream.** The playback service also checks Media3's loaded
  timeline (`isCurrentMediaItemLive`), not only the catalogue/resolver `live` flag. A stream
  that wasn't marked live (an old card, a shared link) is now marked live at runtime, so it
  gets live reconnects and no resume-position saving.
- **LIVE pill.** In the inline and fullscreen player the time pill now reads `● LIVE`, with a
  red dot at the live edge. Behind the edge it reads `−0:45 ● LIVE` with a grey dot. Tapping
  it jumps back to the edge and resumes playback.
- **DVR scrubbing.** The timebar still scrubs within the stream's DVR window. Positions are
  clamped to the window, so the clock no longer runs past the duration. Forward seeks
  (button, double-tap, `seek`) stop at the live edge.
- **Falling behind the window.** If playback is paused so long that the window moves past it
  (`ERROR_CODE_BEHIND_LIVE_WINDOW`), the player rejoins at the live edge on the same stream.
  It no longer shows a playback failure or re-resolves.
- **Expanded (Compose) player.** A DVR live stream shows a slider over the window, how far
  behind live it is, and a Live badge that jumps to live when tapped. Live without DVR (and
  radio) keeps the existing static badge.
- The snapshot now publishes `live_stream`, `live_seekable` and `live_behind` (seconds), and
  the service accepts a `go_live` command.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| `LivePositionTest` (edge distance, edge tolerance, clamping the reported 1:00:28/1:00:00 case, labels) | `kotlinc` 2.1.21 compile of `LivePosition.kt` + test against minimal Media3 `C`/`Player`/`Timeline` stubs, run with JUnit 4.13.2 | 4/4 pass | unit |

## Not verified and limits
- The Gradle/APK build and the Robolectric suite were not run. The Android SDK host
  (dl.google.com) is blocked in this session, so the Kotlin/Java changes in `PlaybackService`,
  `PlayerChrome` and `FocusedPlayer` are not compile-checked against the real SDK. CI must
  build them.
- No emulator, physical-device or live-stream check. The edge tolerance (4 s) assumes Media3's
  live speed control keeps close to the window's default position on YouTube HLS. Tune it if
  a device shows "−0:0x" flicker at the edge.
- Runtime marking updates the in-memory media JSON for the current session only. It does not
  rewrite cached feed cards.
- The mini-player progress line is unchanged for live: it shows the DVR position.

## For the plan and parity tables
- docs/android/PARITY.md, playback: "Live streams: LIVE / behind-live pill with go-to-live,
  DVR scrubbing within the window, runtime live detection and behind-window recovery
  (unit verified; device/live pending)."
