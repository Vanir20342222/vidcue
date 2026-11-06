# Keep Android Strings inside the visualiser stage (M9)

- **Date:** 2026-11-04
- **PR:** see the pull request for this branch
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified

## What it does

Strings no longer exceed the top and bottom of the music artwork stage on strong
beats. Previously the beat multiplier could raise their amplitude to 76% of the
stage height, beyond the 50% available above or below the centre. The drawing now
limits each string's amplitude to the available space, reserving room for its glow
and antialiasing. Rounded stroke joins keep the stroke inside that margin. Waves
below the limit retain their existing band and beat response.

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Kotlin compilation | Compile `SpectrumMath.kt` and `SpectrumMathTest.kt` with Kotlin 2.1.21, `-Werror`, JVM target 17 | Passed | static |
| Visualiser maths | `org.junit.runner.JUnitCore dev.ytui.android.SpectrumMathTest` on the compiled sources | 13 tests passed; regression covers 5,250 combinations of stage height, density, string, band level and beat strength | unit |
| Whitespace | `git diff --check` | Passed | static |

## Not verified and limits

The workspace has no Android SDK or emulator. Compose compilation, APK builds and
on-device appearance were not checked. Host unit results cover the amplitude and
glow bounds, not rendered Android pixels.

## For the plan and parity tables

M9 Android: Strings amplitude accounts for beat amplification and glow width to
avoid top/bottom clipping. Thirteen visualiser maths tests pass on the host;
Android build and device rendering remain unverified for this change.
