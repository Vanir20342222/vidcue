# Android runtime review fixes: AR-01, AR-02, AR-22, AR-23 (M9)

- **Date:** 2026-11-05
- **PR:** not opened yet
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified

## What it does

Fixes four findings from the 2026-11-05 Android review (fix group B, runtime):

- **AR-01:** the isolated YouTube challenge solver no longer stays broken until the app restarts.
  A failed `JavaScriptSandbox` connection is not cached, and when the sandbox process has died
  (`SandboxDeadException`) or was closed under `createIsolate`, the connection is closed and the
  evaluation is retried once on a fresh connection inside the same 15 s deadline. Script errors,
  isolate memory limits and timeouts do not reconnect. The 128 MiB heap limit, FD-only evaluation,
  1 MiB result limit and two concurrent slots are unchanged. The exception message is now a short
  reason ("timed out", "unavailable; update Android System WebView", or a named limit) without the
  main-thread stack trace; details go to logcat (`VidcueSolver`).
- **AR-02:** the sanitized error returned by `NativeBridge.init` is kept. When the engine fails to
  start, the first screen shows it once as an error notice and keeps it as the status, instead of
  only "Engine is not initialized".
- **AR-22:** `usesCleartextTraffic="true"` is replaced by `res/xml/network_security_config.xml`.
  Cleartext stays allowed in the base config because internet radio stations are arbitrary HTTP
  hosts (this also keeps loopback test servers working), while youtube.com, youtu.be,
  youtube-nocookie.com, googlevideo.com, ytimg.com, ggpht.com, googleusercontent.com, google.com,
  googleapis.com and gstatic.com (with subdomains) refuse cleartext. Trust anchors stay the system
  default. The policy applies to Android framework clients (OkHttp, `HttpURLConnection`); the Rust
  engine's own HTTP client is not governed by it.
- **AR-23:** the notification permission is no longer requested on top of first-run setup. It is
  requested after setup is finished or skipped, or at launch when setup is not shown; at most once
  per app process and only on Android 13+.

Design notes: the reconnect decision, retry rule and messages are in `SandboxRecovery` so they can
be unit tested without a sandbox. The library allows one bound sandbox at a time, so a stale
connection is closed (unbound) before a new one is created. `OnboardingStore.showsAtLaunch`
mirrors the condition `ComposeHost` uses to show setup; `ComposeHost` was not changed.

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Kotlin compile, app and test APK sources | `apps/android/gradlew -p apps/android --no-daemon :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` | Passed | implemented |
| JVM unit tests | `apps/android/gradlew -p apps/android --no-daemon :app:testDebugUnitTest` | 182 passed, 0 failed (168 before, 14 new: `SandboxRecoveryTest`, `EngineStartupTest`, `NotificationPermissionTest`, `NetworkSecurityConfigTest`) | fixture/unit verified |
| Android lint | `apps/android/gradlew -p apps/android --no-daemon :app:lintDebug` | Passed, 0 errors; no warnings in the files this change adds or rewrites | fixture/unit verified |

## Not verified and limits

- No emulator or physical device was run. The sandbox reconnect path, the start-error notice, the
  network policy at runtime and the permission timing were not exercised on Android.
- A sandbox death was not reproduced; the classification follows the `androidx.javascriptengine`
  1.1.1 behaviour (dead sandbox: dead isolate failing with `SandboxDeadException`; closed sandbox:
  `IllegalStateException` from `createIsolate`).
- A connection future that never completes is still waited on until each evaluation's deadline;
  only failed, cancelled or dead connections are replaced.
- Instrumented tests (`RuntimeTest`, `OnboardingUiTest`, media tests with loopback HTTP servers)
  were compiled but not run. They grant `POST_NOTIFICATIONS`, so the moved request should not
  show a dialog in them.
- Any Java-side request to a listed Google host over plain `http://` now fails by design.

## For the plan and parity tables

- docs/android/VALIDATION.md: "AR-01/AR-02/AR-22/AR-23 (2026-11-05): isolated solver reconnects
  once after a failed or dead sandbox; engine start errors are shown once and kept as status;
  cleartext is scoped by a network security config that forbids it for YouTube/Google hosts;
  notification permission waits for first-run setup. Fixture/unit verified; no emulator or
  device run."
- docs/audit/ANDROID_REVIEW_2026-11-05.md: AR-01, AR-02, AR-22, AR-23 status **fixed**, see this
  note.
