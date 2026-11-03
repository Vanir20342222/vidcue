# Audio language selection and Next navigation (M9)

- **Date:** 2026-11-03
- **PR:** [#62](https://github.com/Vanir20342222/ytui/pull/62)
- **Milestone:** M9
- **Platforms:** Android, mobile Rust engine
- **Status:** implemented; Rust fixture/unit verified

## What it does

Android playback prefers original-language audio before codec compatibility,
then the provider's default audio when no original is identified. Previously,
the AAC branch could select a dub even when original audio existed in Opus.
Playback options → Audio & subtitles now lists extractor audio languages with
an Original (automatic) reset. Explicit choices are scoped to the current item,
kept through quality changes and stream refreshes, and never silently replaced
by another language. Position, pause, speed, volume and collection state survive
the replacement. Offline/local media retain their existing decoder track menu.

Repeated Next taps advance without waiting for intermediate audio/video output.
Unstarted queue entries are skipped without being recorded in history. A failed
resolution retains the item until the user explicitly skips it; subsequent Next
can move past it. History candidates can also be skipped without committing an
unplayed history cursor. Old EOF events and delayed history callbacks cannot
overwrite a newer navigation intent, and Previous clears queued Next taps.

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Formatting | `cargo fmt --all -- --check` | Pass | Static |
| Strict lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass | Static |
| Workspace regression suite | `cargo test --workspace --locked` | Pass; explicit live/tool opt-in ignores retained | Fixture/unit |
| Final mobile regressions | `cargo test -p ytui-mobile --locked` | Pass; original Opus vs dubbed AAC, explicit-language no-fallback, language cache isolation, split-stream language catalog, noncommitting history skips | Fixture/unit |
| Android app/test compilation | `apps/android/gradlew -p apps/android :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin :app:compileDebugAndroidTestJavaWithJavac` | Pass after installing a local SDK/JDK outside the repository | Compile |
| Android JVM regressions | `:app:testDebugUnitTest` | 80 passed; zero failures, errors or skips | Unit |
| Android lint | `:app:lintDebug` | Pass | Static |
| Initial Android CI | [Run 41686657797](https://github.com/Vanir20342222/ytui/actions/runs/41686657797) | APK and test APK pass; emulator shards 1/2 pass, 0/3 fail in native pause and failed-resolution recovery waits | Partial emulator evidence |

### Retained CI failures and synchronization correction

The failed-resolution recovery wait could read the preceding expected error
before the asynchronous Next command ran. Its failure diagnostic already showed
Recovery playing. The test now dispatches the deliberate skip through the native
controls on the main looper before starting the same fail-fast playback wait.
New playback errors, queue consumption and the absence of unplayed history
entries remain asserted.

The native play/pause test acquired the bottom exit button as its signal that
the centre transports were usable. Media3 keeps that button present while the
centre controls fade. Acquisition now also requires the controller's fully shown
state and centre visibility/opacity, then renews the same hide timeout before
the real UiAutomator tap. No assertion, deadline, retry count or production
auto-hide setting is relaxed. Both corrections require a new CI emulator run.

## Not verified and limits

The initial local build lacked an Android SDK. The follow-up installed a local
SDK/JDK outside Git and compiles both application and instrumentation sources.
No emulator or physical device is available locally. Full corrected Android
instrumentation acceptance still requires the next CI run.
No live YouTube/account/audio-output acceptance was performed. Language choices
depend on extractor metadata; when the provider does not identify an original,
automatic selection falls back to its default/available audio. Switching an online
track re-resolves that item and can require network buffering. This change does
not claim M9 completion or alter desktop playback.

Open PR #48 overlaps `PlaybackService.java`, `ComposeHost.kt`, and the mobile
engine. Edits to those files are limited to wiring and playback behavior; no
workflow, version, vendor or shared evidence documents are changed.
Open PR #47 also touches `MediaRuntimeTest.java`; the correction here is confined
to fullscreen-controller acquisition.

## For the plan and parity tables

M9: Android original-language-first playback, an extractor audio-language picker,
language-scoped resolution leases, and Next skips during buffering/resolution
failure are implemented. Rust fixture/unit regressions and Android source
compilation pass. The initial CI APK build passed, with two emulator transport
failures retained above. Full corrected emulator, physical-device and live
multilingual playback acceptance remain unverified.
