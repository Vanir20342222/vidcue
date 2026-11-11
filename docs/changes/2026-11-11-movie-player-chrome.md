# Movies use the YouTube video player chrome (M9)

- **Date:** 2026-11-11
- **PR:** #152
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented

## What it does
The Android Movies player now uses the same native video controls as YouTube playback: the same
top bar, transport, time bar, captions button, gestures (double tap, hold for speed, brightness and
volume drag, pinch zoom, swipe down to leave) and colour grading. The settings button opens the same
frosted side panel as the YouTube fullscreen options, holding the movie's Quality, Audio, Subtitles,
Speed and Source choices. Source failover mid-film, Cinecat request headers, offline copies and
resume position work as before. Seek steps follow the shared seek setting.

The shared chrome gained a standalone mode (`ytui_standalone` view tag). In that mode gestures and the
captions button drive the movie's own player, not the YouTube playback service, and YouTube's
SponsorBlock marks are not drawn on it. The YouTube path is unchanged when the tag is absent.

Review fix: captions now drop track overrides left over from a previous source before they choose a
track, so the captions button still works after a source switch.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Kotlin compile | `apps/android/gradlew --offline :app:compileDebugKotlin` (SDK at `ANDROID_HOME`) | pass, after the captions fix too | implemented |
| Changed Java type-check | `javac --release 17` on `VideoGestures.java` and `FullscreenActivity.java` against the Gradle classpath | pass | implemented |
| Review | sol-loop, 2 rounds, GPT-6.1 Sol via OpenRouter; round 1 CHANGES (one should, fixed), round 2 APPROVE | no open findings | implemented |
| JVM unit tests, lint, APK | CI Android lane (JDK 21) | see PR checks | fixture/unit |

The local JDK is 27, and AGP's `androidJdkImage` jlink step fails on it, so the Java compile, unit
tests and lint were not run here.

## Not verified and limits
- No emulator or device run. The on-screen layout, gestures, captions and the options panel are not
  verified by any run; they were checked by reading the code.
- The Sol round 1 attempt on the Codex route was refused by its usage limit; round 1 ran on the
  OpenRouter route instead.

## For the plan and parity tables
- Android Movies player: uses the shared YouTube video chrome, gestures and options panel (M9,
  implemented, not device verified).
