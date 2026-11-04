# Android feedback round 2: skip rings, visualiser, scrolling, podcasts, skeletons, downloads (M9)

- **Date:** 2026-11-04
- **PR:** not opened yet
- **Milestone:** M9
- **Platforms:** Android, shared engine
- **Status:** fixture/unit verified (Rust, visualiser math); Android compiled by CI only

## What it does

Fixes six issues the owner reported from a Nothing Phone 3a after #77:

| Report | Change |
| --- | --- |
| Numbers not centred in the video skip arrows | The rewind/forward buttons use the app's symmetric ring icons instead of Media3's off-centre arrows. The seek amount is padded so the digits' own middle, not the text box, sits at the ring's centre. |
| Visualiser still looks bad | Each band is scaled to its own recent peak (auto-gain), so treble moves. Pulse Orbit drops the lopsided liquid blob and gets 96 fine bars, mirrored with the bass at the bottom, plus beat ripples, a halo and a cover ring. Neon VU gets a calmer liquid, a mirrored 32-bar equaliser and a beat glow. The cover pulses on detected bass beats. |
| Scrolling still laggy in places | The search header floats over the grid and moves only its layer; the old `AnimatedVisibility` re-measured the grid every frame. ICU `CompactDecimalFormat` instances (one per video card per composition) and a regex are now cached. The per-card `AnimatedVisibility` for playing bars is gone. Non-animating cards skip their animation state. Cover composition locals are provided once around the grid. |
| Podcasts still empty | Empty pages whose content sits behind a continuation (seen for Music categories, most likely signed-in Podcasts): the engine follows up to two continuations, and the app loads more once when a page arrives empty with a continuation. |
| Skeletons not smoothly animated | All shimmer blocks share one uptime clock and one eased band in window coordinates, instead of separate, drifting, restarting animations. |
| Download progress updates every ~10 s | The Media3 download index stores progress only every few seconds. While downloads run, the main thread samples `getCurrentDownloads()` every 500 ms, and the snapshot prefers that live progress; the bar glides between samples. |

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Continuation follow | `cargo test -p ytui-providers empty_category_shell` | Passed | unit |
| Provider/engine suites | `cargo test -p ytui-providers -p ytui-mobile --locked` | 50 + 17 + 73 passed | unit |
| Lint | `cargo fmt --all`, `cargo clippy -p ytui-providers -p ytui-mobile --all-targets --locked -- -D warnings` | Clean | static |
| Visualiser math | `SpectrumMath.kt` with a harness mirroring the two new `SpectrumMathTest` cases, compiled by Gradle's bundled Kotlin 2.0.21 and run | Treble auto-gain reaches 1.0, silence stays 0, a bass hit starts kick and ripple, both fade | unit (harness) |
| Live podcast shape | Anonymous WEB_REMIX Podcasts chip, hl=de / gl=DE | 30 episode renderers, which the #77 parser handles | live, one-off |
| Kotlin | Bundled compiler over the changed files without Android dependencies | No parse errors | syntax only |

## Not verified and limits

- No Android SDK in the session, so the app and the JUnit tests were not built here. The CI dispatch on this branch is the first compile.
- The signed-in Podcasts response could not be inspected (no account in the session). The continuation fix covers the most likely shape. If Podcasts is still empty while signed in, a sanitized response is needed.
- Scrolling was not measured on a device. The Preview APK is a debuggable build, which runs Compose noticeably slower than a release build.
- Seek-ring centring depends on the font's metrics; it is computed at runtime, not checked on a device.

## For the plan and parity tables

M9 Android round 2: the video seek amounts are centred in symmetric rings. The visualisers use per-band auto-gain, beat detection, a symmetric orbit ring with ripples, and a mirrored VU equaliser. Feed scrolling avoids grid re-measurement and per-card formatter allocation. Empty Music category shells follow their continuation. Skeleton shimmer is one shared sweep. Download progress refreshes every 500 ms from live Media3 state. Rust unit-verified; Android compiled in CI only; not device-measured.
