# Keep Android Now Playing covers visible (M9)

- **Date:** 2026-11-19
- **Milestone:** M9
- **Platforms:** Android
- **Status:** Unit verified

## What it does

The mini player and full player draw cover art in their own layouts. The cover
moves with the sliding player panel. This removes
the shared overlay that could leave the cover blank while the page and its
color wash were visible. A smaller cached cover that becomes available while
the full-size image loads now starts the same reveal as the full-size image.

## Evidence

| Check | Result |
| --- | --- |
| `:app:assembleDebug :app:assembleDebugAndroidTest` | Both APKs built using the native libraries from the successful Android build of the unchanged Rust source. |
| `:app:testDebugUnitTest` | 323 passed, 7 skipped, 0 failures. |
| `tools/android/inspect-apk.sh` | Debug APK signature, both Rust ABIs, 16 KiB alignment, and notices passed. |

The existing animated music-player instrumentation case now checks actual pixels
from a loopback-served cover on the first open and after minimizing and reopening.
It also checks that playback retains the same decoder.

## Not verified and limits

The local emulator failed to boot, so the pixel assertions were not run. The
reported cover was not reproduced on a physical device.

## For the plan and parity tables

Android Now Playing draws the cover in the panel and reveals smaller cached
artwork while the requested size loads.
