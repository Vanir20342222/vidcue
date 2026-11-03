# Audio language selection and Next navigation (M9)

- **Date:** 2026-11-03
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
| Android app/test compilation | `apps/android/gradlew -p apps/android :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` | Blocked: Android SDK location not found, after downloading Gradle through the environment proxy | Not compiled |
| Android regressions added | `AudioLanguageUiTest`, `PlaybackSkipTest` | Added; not run here | No device evidence |

## Not verified and limits

No Android SDK, emulator or physical device is available here. The Kotlin/Java
changes and new instrumentation tests require Android CI compilation and runs.
No live YouTube/account/audio-output acceptance was performed. Language choices
depend on extractor metadata; when the provider does not identify an original,
automatic selection falls back to its default/available audio. Switching an online
track re-resolves that item and can require network buffering. This change does
not claim M9 completion or alter desktop playback.

Open PR #48 overlaps `PlaybackService.java`, `ComposeHost.kt`, and the mobile
engine. Edits to those files are limited to wiring and playback behavior; no
workflow, version, vendor or shared evidence documents are changed.

## For the plan and parity tables

M9: Android original-language-first playback, an extractor audio-language picker,
language-scoped resolution leases, and Next skips during buffering/resolution
failure are implemented. Rust fixture/unit regressions pass. Native Android
build, emulator/device transport checks and live multilingual playback remain
unverified; the local build is blocked by the absent Android SDK.
