# German-locale and Galaxy Tab S9 fixes found on a physical device (M9)

- **Date:** 2026-11-16
- **Milestone:** M9
- **Platforms:** Android
- **Status:** physical-device verified (Galaxy Tab S9 SM-X710, Android 16, de_DE)

## What it does
The first full instrumentation run on the owner's Galaxy Tab S9 (German locale, Samsung
Keyboard, 753 dp portrait) failed 10 of 89 tests. App fixes:
- Like/subscriber counts and download sizes use English digits, matching the English
  labels around them. A German device showed "2834 likes" next to "1.2K views" and
  "1,2 GB" in "12 MB of 1,2 GB".
- An export cancelled before it starts no longer leaves an empty destination file.
- Reopening the music player during its ~200 ms closing fade reused the closing panel,
  still marked as leaving: Minimize then did nothing, its cover stayed in flight over the
  page, and the app behind stayed hidden from accessibility (`clearAndSetSemantics`).
  Every opening now composes a fresh panel.

Test fixes (each failure was a wrong assumption about the device, not a loosened check):
- Real-keyboard test types once Samsung Keyboard has finished sliding in, and waits for
  `input text` delivery; typing earlier kept only the first key.
- Artwork grid test expects covers to fill their column; a 753 dp tablet correctly shows
  two columns.
- Folder-picker test presses the system picker's buttons by id (`android:id/button1`),
  not by English text.
- The metadata review driver, and the wait for decoded fixture video, advance one Compose
  test frame per poll. The test rule drives every composition with its clock; without a
  frame, settings changed outside Compose were never drawn and the video surface was never
  composed.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| JVM tests | `:app:testDebugUnitTest` incl. `TransferFormatTest.sizesKeepEnglishDigitsOnAGermanDevice` | pass | unit |
| Full instrumentation suite, before | `am instrument` (all classes) on SM-X710, 2026-11-15 | 10 of 89 failed | physical device |
| Fixed tests, singly | keyboard, grid, transfer cards, export, folder picker, video information (twice), music transitions | pass | physical device |
| Full instrumentation suite, after | see PR description for the final run | | physical device |

## Not verified and limits
- `tabletPlayerKeepsSideAndFocusedControlsReachable`, `miniPlayerAndLargeTransportControlTheActualDecoder`
  and `MediaRuntimeTest#actualVideoPauseVolumeSwitchDownloadAndOfflinePlayback` each failed
  once in a full run and passed alone; order/timing dependence not investigated further.
- One device and one locale (de_DE) only.

## For the plan and parity tables
- Android number formatting is English throughout the English UI on non-English devices; physical-device verified (de_DE) 2026-11-16.
