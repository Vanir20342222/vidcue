# M9: Android feed loading after account and network changes

- **Date:** 2026-11-04
- **Milestone:** M9
- **Platforms:** Android native UI
- **Status:** fixture/unit verified

## What it does

Account changes and network-setting edits clear the Video and Music feed states
and increment their generations to reject old replies. Tab selection previously
treated any nonzero generation as evidence of a loaded feed. Opening an
invalidated tab therefore left it empty until a manual refresh.

Feed state now tracks whether it needs its initial request separately from the
generation. Selecting an unrequested or invalidated tab loads its default source
automatically: Home for Video/Music, Cliamp for Radio. Completed empty results and
in-flight requests retain their state when switching tabs.

Initial tab loading waits for account/settings initialization, then loads the
currently selected tab. Selecting Music or Radio during startup no longer leaves
startup loading Video instead. Existing source-chip requests, pagination, cache
scope, explicit refresh and bounded transient retry behavior remain in place.

## Evidence

- `:app:testDebugUnitTest`: 86 passed, zero failures/errors/skips. Six new offline
  ViewModel regressions cover loading all three default feeds, actual shared
  account/network invalidation, switching away/back during a pending fetch, valid
  empty results, readiness/offline guards, and selection during actual ViewModel
  initialization with synthetic JNI replies.
- Android app Java/Kotlin and instrumentation Kotlin compilation passed.
- `:app:lintDebug`: passed, zero errors, 96 warnings and five informational hints
  on the PR base `f7feb4a` (the earlier pre-redesign base had 41 warnings).
- `git diff --check`: passed.

The container initially lacked an Android SDK and Java compiler. Build tools
were installed outside the repository. Early build attempts failed because of
missing compiler/proxy/CA configuration; the first Robolectric run failed to
download its Android dependency because the test JVM did not inherit the proxy.
The successful run used the environment proxy and system CA store, including in
test JVMs, with loopback excluded from proxying. No repository build configuration
was changed for those environment fixes.

## Not verified and limits

No emulator, physical-device, real-account or live-feed check was run. No APK was
published. These checks verify loading/state behavior with synthetic provider
pages and do not close M9's live-service/device acceptance gates.

## For the plan and parity tables

M9 Android feed loading: selecting an invalidated Video/Music tab now starts its
default request automatically, independently of its stale-response generation.
Startup loads the selected tab after settings/account initialization. Offline
ViewModel regressions pass; physical-device and live-account acceptance remain
open.
