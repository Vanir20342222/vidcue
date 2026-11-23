# Android crash hardening: channel-tap crash and engine guards (M9)

- **Date:** 2026-11-23
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android / shared engine
- **Status:** emulator verified

## What it does
Fixes the crash reported when tapping a channel avatar on the signed-in Home feed. The feed's
header wash wrote Compose state inside `snapshotFlow` (a read-only snapshot). It threw
`IllegalStateException: Cannot modify a state object in a read-only snapshot` whenever the grid
returned to the top after a touch scroll, which is what opening a channel does. Signed-in cards
carry channel links, so the crash showed up there. The same pass removes other crash paths:
- Kotlin values that could be null at the moment they were read (feed error/notice items, the
  collection id in `playFrom`, offline remove/delete dialogs).
- Duplicate history keys in Library/History.
- A playback-start foreground-service promise that was never kept when the UI left during startup.
- Download-service handling of the Android 15 data-sync timeout.

Shared engine:
- Every JNI export now catches panics. A panic leaving an `extern "system"` function aborts the
  process.
- std locks recover from poisoning instead of cascading panics.
- Each request runs on a scoped 8 MB-stack thread, so recursive page walkers cannot overflow
  ~1 MB Java worker stacks.
- Panic locations are logged to logcat (tag `vidcue`), without the panic message.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Reproduce crash | `adb shell monkey -p dev.vidcue.android.preview -s 4242 --throttle 120 -v 4000` on HEAD 80a64d1 debug APK, emulator at 2560x1600 / 320 dpi | Crashed at event 1225: read-only snapshot write, `ComposeHost.kt:1290` | emulator |
| Second seed on HEAD | monkey `-s 777 -v 8000 --ignore-crashes` | Same exception | emulator |
| Fixed build, seed 4242 | same monkey command, 4000 events | 0 crashes. One cold-start ANR: main thread runnable in first composition, ~5.4 s runqueue wait on the just-booted, CPU-starved emulator. One System UI ANR. | emulator |
| Fixed build, seed 777 | monkey `-s 777 -v 6000` | 0 crashes, 0 app ANRs | emulator |
| Channel tap, tablet layout | Tap channel avatar on Home at 2560x1600 (signed out) | Channel page opens | emulator |
| Rust tests | `cargo test -p vidcue-mobile -p vidcue-providers -p vidcue-core --locked` | all pass | unit |
| Android unit tests | `./gradlew :app:testDebugUnitTest --rerun-tasks` | pass | unit |
| APK build | `RUSTUP_TOOLCHAIN=1.99.0 YTUI_ANDROID_ABIS=x86_64 tools/android/build.sh` | APK built | build |

## Not verified and limits
- Not tested signed in, and not tested on the physical Tab S9. Confirm the channel tap on the
  owner's account and device.
- Only the x86_64 build was produced. arm64 was not built.
- The playback foreground-service and download-timeout fixes came from code review. They were not
  reproduced on a device.
- The 8 MB request stack is a mitigation. No overflow was observed.
- Lock poison recovery keeps whatever state the panicking holder left.

## For the plan and parity tables
- docs/android/VALIDATION.md: "2026-11-23: monkey stress (seeds 4242/777, 10k events, tablet
  2560x1600 emulator) crash-free after the read-only snapshot write fix; all JNI exports guard
  panics."
