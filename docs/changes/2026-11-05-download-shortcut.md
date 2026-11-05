# Downloads shortcut when the navigation is covered (M9)

- **Date:** 2026-11-05
- **PR:** none (committed to the designated working branch)
- **Milestone:** M9 (Android native Compose UI)
- **Platforms:** Android (Compose). Desktop is unchanged.
- **Status:** implemented with JVM unit tests added. Not compiled or run locally (see below).

## What it does
When you save something, its cover flies into the Downloads tab. If the navigation is hidden,
the cover now lands on something you can see. Hidden means the full-screen player or a panel
is open, or the keyboard has hidden the phone dock. Before this change, the cover flew to the
hidden tab's last known spot (or a guessed corner) and seemed to vanish. Now a floating
"Downloads" pill appears near the bottom as the cover sets off and catches it with a small
bounce. The pill stays for 3 seconds after the cover lands, then fades out. Tapping the pill
closes the player or panel and opens Downloads. If you save again, the timer starts over.
With reduced motion, the pill appears at once without the flight.
When the navigation is visible, the cover still flies into the Downloads tab as before.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Routing logic | `DownloadFlightTest` (Robolectric): visible tab vs covered/missing tab, landing counters, a stale timer cannot dismiss a newer shortcut | added; not run here | unit |

## Not verified and limits
- No Android SDK in this session (SDK install was refused by the session policy). Kotlin
  compile, unit tests, APK build, emulator and device checks all rely on CI or a later local run.
- The pill's position and timing on a physical device have not been looked at.

## For the plan and parity tables
- docs/android/PARITY.md (downloads): "Saving while the player or a panel covers the navigation
  shows a temporary Downloads shortcut (3 s after landing, tappable) instead of flying into a
  hidden tab."
