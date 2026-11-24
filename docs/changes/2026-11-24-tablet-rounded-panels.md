# Tablet panels clear the rounded display corners (M9)

- **Date:** 2026-11-24
- **PR:** #184
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified

## What it does
On tablets the navigation sidebar is now a floating panel with 28 dp rounded corners, inset 20 dp
from the left and bottom edges like the Now playing panel opposite it. The fullscreen Playback
options sheet (video and movie players) floats 12 dp in from every edge with all corners rounded,
instead of running square into the display's rounded corners. The browse header's top colour wash
now uses the page's position and width, so it no longer shows as a lighter square-cornered band
behind the Music chips when the header is a narrower middle pane.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Compile app and test APK | `apps/android/gradlew -p apps/android :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` | Pass | build |
| JVM unit tests | `apps/android/gradlew -p apps/android :app:testDebugUnitTest --offline` | Pass | fixture/unit |

## Not verified and limits
Not checked on an emulator or the Galaxy Tab S9 after the change; the defects were reported from
Tab S9 screenshots. The insets take about 32 dp of width from the tablet content area.

## For the plan and parity tables
- M9 Android tablet layout: sidebar, player panel and fullscreen options sheet are rounded floating panels clear of the display corners; header wash matches the page in three-pane layouts (unit verified, device check pending).
