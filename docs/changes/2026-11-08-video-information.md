# M9 — video information on Home and in the player

Home, search and shelf cards continue to follow the existing information switches.
Duration stays in the thumbnail badge, avoiding a repeated time label. Matching
loaded cards pick up metadata learned during playback and
watch-detail loading without changing their title, artwork, URL or feed section.

The watch page always shows available views, publication date, likes, duration,
channel/subscribers, feed section and a description preview. Card switches no
longer hide player information. Counts wrap with the available width; likes no
longer require expanding the description. The tablet's compact player also shows
available counts and duration. Unknown fields remain omitted.

## Validation

- Rust engine builds: ARM64 and x86_64 release profiles passed (rustc 1.99.1,
  NDK 27.2.12479018); their packaged JNI libraries were retained.
- `assembleDebug`, `testDebugUnitTest`, `lintDebug`, `assembleDebugAndroidTest`:
  passed. All 249 JVM tests passed, zero skipped; lint has zero errors and 96
  warnings. The new formatting test explicitly uses API 35 for Android ICU.
- `tools/android/inspect-apk.sh`: passed signature, ZIP alignment, both ABIs,
  exported JNI symbols, 16 KiB ELF alignment and dependency notices.
- APK: `apps/android/app/build/outputs/apk/debug/app-debug.apk`, debug-signed
  `dev.vidcue.android.preview`; this is not a stable-channel release.
- Focused emulator checks and review captures: pending below.

The checks use original offline fixtures, not a live account or live YouTube
acceptance. Software-only API 35 emulator startup required a sparse data image
and OS hardware timeout multiplier 10. Initial setup failures included exhausted
build storage, an unsupported formatter option, and the OS watchdog restarting
its framework. The first JVM run had one Android-ICU stub failure; selecting
API 35 corrected that test environment without changing its assertions.
Build logs and APK inspection are retained in `target/android-metadata-evidence`.


## For the plan and parity tables

M9 Android: Home and search reuse metadata learned from playback and watch
details, including after a refresh while paused. The full video player exposes
known information independently of card visibility preferences. Unknown fields
remain omitted. Physical-device and live-provider acceptance remain separate.

## Continued verification, 2026-11-09

Reapplied cleanly to current main (including watched-recommendation filtering).
The two isolated card-information emulator tests passed. The full-app capture
initially failed on emulator sleep, then on Compose global-idle synchronization,
and then on accessibility-tree text discovery; those logs remain in
`target/android-metadata-evidence`. The review driver now checks exact laid-out
Compose text with non-empty visible bounds and saves the OS screenshot bytes
through `screencap`, failing explicitly if the PNG is invalid. Visual acceptance
is pending and the PR remains draft until checked.
