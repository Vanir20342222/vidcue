# M9 Android background and process-death resilience

- **Date:** 2026-11-09 (initial implementation and verification: 2026-11-05)
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified

The user requested more resilience when Android kills the background app.
Active playback already uses Media3's foreground media service, audio focus,
becoming-noisy handling, and network wake mode. This change reduces expendable
memory and preserves a small recovery checkpoint rather than starting a permanent
idle service or trying to override Android's process policy.

## What it does

- The latest accepted non-preview playback item is checkpointed every five seconds while playing,
  on pause, seek and background transitions, and on orderly service destruction.
  Abrupt process death does not require an `onDestroy` callback for recovery.
- With no live playback item, reopening shows a paused recovery item. Videos have
  a native Compose Resume playback action; music uses the existing Play action.
  Only that user action resolves fresh playback URLs (or opens the existing
  offline library entry) and seeks to the checkpoint. Live radio restarts at the
  live edge. No automatic audio or network resolution starts merely by reopening.
- Recovery stores allowlisted media identity, title/creator, mode, quality and
  position in private preferences. It excludes stream descriptors, headers,
  artwork URLs and transient/private context. YouTube links are canonicalized;
  query-bearing direct links cannot be recovered unless an offline entry exists.
  Corrupt, future-dated, unknown-version or older-than-seven-day data is ignored.
- Explicit Stop, normal completion, account disconnect, clearing history and
  disabling playback history clear the checkpoint. Disabling playback history
  also suppresses further checkpoint writes. Stop cancels pending UI resolution.
- Delayed playback handoffs re-check background policy before starting a service.
  Internal queue/preview restoration also checks it before playing. Previews pause
  when backgrounded, and a preview timer cannot bypass disabled background playback.
- The recent artwork cache is now bounded by bytes (8 MiB), alongside the existing
  24 MiB primary cache. Both release references on UI-hidden/low-memory callbacks;
  displayed bitmaps are not recycled underneath Compose. No media/download cache
  or user library is deleted.

## Verification

Final source checks passed on 2026-11-05 with JDK 17 and Android SDK 35:

- `:app:testDebugUnitTest`: 176 tests, zero failures/errors/skips.
- `:app:compileDebugAndroidTestJavaWithJavac`: passed, including Kotlin and
  Java app compilation and compilation of all four new instrumentation cases.
- `:app:lintDebug`: passed (existing advisory warnings remain).
- `git diff --check`: passed.

The environment's proxy and certificate trust were supplied to Gradle and the
Robolectric test JVM using a temporary init script outside the repository;
no build configuration, test assertion or lint check was weakened. Initial
Robolectric artifact downloads failed before that runtime setup was supplied.
No device instrumentation was run.

Added seven Robolectric checkpoint tests and a background-policy matrix test.
Added four device instrumentation regressions for continued background audio,
paused/delayed starts, foreground-service rejection, and preview restoration using generated local audio only.
The instrumentation is for a disposable test installation.

## Not verified and limits

Android and OEM battery policy can still stop the app. This change restores a
paused item when the user returns; it does not restart audio after force-stop,
reboot, or an OS kill. Physical screen-off/Doze, notification/headset/Bluetooth,
Android 15 foreground-service policy and Samsung battery-management checks remain
required. There is no emulator or connected device in this workspace.

The checkpoint restores the current item/position, not transient browsing feeds,
full playlist/related context, session sleep timers or track overrides. Existing
persistent queues, settings, history and offline downloads retain their normal
storage. No APK was published and no release/signing/workflow inputs changed.

## For the plan and parity tables

M9 Android reduces expendable artwork memory and checkpoints the accepted item
and position for paused, user-initiated recovery after process death. Background
policy also applies to delayed online/offline handoffs and preview restoration.
The current item is recoverable; transient browsing and playlist context are not.
Physical/OEM kill, screen-off and Doze acceptance remain separate gates.

## Current-main integration (2026-11-09)

Reapplied onto `88e11f3` without reverting its sleep timers, adaptive ticker,
serialized position saves or preview-restoration semantics. A checkpoint is not
written while a native resume-position lookup is pending; recovery's explicit
position bypasses that lookup and does not leave the pending flag set.

Current-main verification passed on 2026-11-09 (JDK 17, Android SDK 35):

| Check | Result | Level |
| --- | --- | --- |
| `:app:testDebugUnitTest` | 261 tests, zero failures/errors/skips | unit |
| `:app:compileDebugAndroidTestJavaWithJavac` | app and instrumentation Kotlin/Java compiled | compile |
| `:app:lintDebug` | passed, zero errors | static |
| `git diff --check` | passed | static |

The four lifecycle instrumentation cases remain compiled-only; no emulator,
physical-device or live-service result is implied by the current-main checks.
