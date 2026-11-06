# Cover art in the Android media notification (M9)

- **Date:** 2026-11-05
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented (not compiled or device-tested in the authoring environment)

## What it does
The media notification, lock screen and system media controls showed only title and creator
because the session's `MediaMetadata` had no artwork. `PlaybackService` now sets an artwork URI
for every playback item (largest listed thumbnail, then `thumbnail`, then YouTube's standard
`hqdefault` image for a watch URL). A new `SessionArtwork` bitmap loader, wrapped in Media3's
`CacheBitmapLoader`, decodes that cover through the existing Rust image cache
(`NativeBridge.thumbnail`) on its own thread, retrying with YouTube's standard image when a
listed cover fails, the same way in-app covers do. URIs it did not register go to Media3's
`DataSourceBitmapLoader`. Items without any cover (for example a station with no logo) keep an
empty artwork slot.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Cover URI selection | `SessionArtworkTest` (Robolectric) | Added; not run here (no Android SDK) | unit |
| Compile | `apps/android/gradlew -p apps/android :app:compileDebugKotlin` | Not run: no Android SDK in the environment | — |

## Not verified and limits
- Not compiled, and not checked on an emulator or physical device. Needs a look at the
  notification and lock screen on the Galaxy Tab S9.
- Embedded artwork in a downloaded file's tags takes priority over the URI (Media3 behaviour).
- The cover is not hidden when in-app thumbnails are turned off.

## For the plan and parity tables
- Android media notification/lock screen show the current item's cover art (M9), pending
  device acceptance.
