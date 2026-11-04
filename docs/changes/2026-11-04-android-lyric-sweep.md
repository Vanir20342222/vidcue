# Android lyric sweep follows reading order (M9)

- **Date:** 2026-11-04
- **PR:** not opened yet
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented (Kotlin syntax-checked only; unit tests, build and device not run)

## What it does

The playing lyric fills with ink one character at a time, in reading order, with a soft edge.
Before, the fill was a horizontal gradient across the whole text box, so a lyric that wrapped
onto a second visual line filled both lines at the same time. Lines that are not sung yet, and
the unsung part of the playing line, are now 60% ink instead of 38%, so the lines ahead read as
lyrics rather than as dimmed text. The 60% level is an interpretation of the device feedback
("only the first two lines are highlighted in the right colour, the following ones are very
simply highlighted"); the device check should confirm it.

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Sweep arithmetic | The fill formula mirrored in a throwaway Python script | Monotonic along the line, endpoints at 0 and 1, a wrapped tail stays unsung while the start fills, midpoint at the sweep point | static |
| Kotlin syntax | Gradle's bundled `kotlin-compiler-embeddable` 2.0.21 over `LyricSweep.kt`, `ComposeHost.kt` and `LyricSweepTest.kt` | No parse errors. Remaining errors are type errors from the missing Android/Compose classpath, including in untouched code | syntax only |
| Unit tests | `apps/android/app/src/test/java/dev/ytui/android/LyricSweepTest.kt` (6 tests) | Written, not run | none |
| Android build | `apps/android/gradlew -p apps/android :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest` | Not run. The Android SDK install was not completed (blocked by the session's permission check) | none |

## Not verified and limits

- No compile against Compose, no JVM unit test run, and no emulator or device run. The look of the
  edge width and the 60% level is untested by eye.
- The per-character colour spans rebuild the playing line each frame while playback runs. Scroll
  and frame cost on a device are not measured.
- Reduced-motion users still get no sweep; the playing line stays fully inked, as before.

## For the plan and parity tables

M9 Android lyrics: the playing line's karaoke fill runs in reading order across wrapped lines, and
unsung lines are 60% ink. Implemented; not compiled or checked on a device in this change.
