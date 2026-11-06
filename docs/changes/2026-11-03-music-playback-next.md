# M9: Music playback recovery and Next

- **Date:** 2026-11-03
- **Milestone:** M9
- **Platforms:** Android / shared engine
- **Status:** fixture/unit verified

## What it does

The reported Android Music screen stops with `ERROR_CODE_IO_BAD_HTTP_STATUS`;
Next also fails to advance. The screenshot does not identify the HTTP status or
establish which extractor response caused it.

Changes:

- Refresh rejected YouTube VOD stream URLs once for HTTP 403/410. The Rust
  `fresh` request bypasses the stream lease and uses full watch/client acquisition
  instead of repeating the direct-player shortcut. Playback indexes, quality,
  account and network scope are retained; normal pooled clients stay unchanged.
  HTTP 429 never triggers this automatic refresh.
- Preserve the current position, pause intent and collection during recovery;
  record history/commit the queue only when an unplayed item actually starts.
  Play explicitly retries after an error, and recovery reports buffering rather
  than a terminal error while resolving. Stale recovery replies cannot replace a
  newer navigation/play/stop intent.
- Prepare the playback context at media handoff, even if playback never starts.
  Next no longer waits for a buffering item to start. Explicitly skipping a
  failed/unstarted queue entry consumes that entry without adding played history;
  an error by itself still retains it. Errors from the item being left cannot
  cancel navigation. Failed navigation handoffs clear the pending flag.
- Add an offline extractor regression for full audio recovery with index metadata,
  and Android instrumentation regressions for Next during initial buffering and
  Next after a failed queued item. These instrumentation cases require a device.

## Evidence

PR preparation transplanted the unchanged fixes cleanly onto latest `main`
`f49e666`. Re-verification on that base passed workspace formatting, strict
workspace/all-target Clippy and `cargo test --workspace --locked`: 391 passed,
0 failed, 31 explicitly ignored. Android app/test compilation and all 78 JVM
tests also pass on this base. The initial targeted results below were recorded
before this transplant.

- `cargo test -p ytui-mobile --locked`: 56 passed (40 unit, 16 integration).
- Vendored `mobile_playback`: 16 passed; the generic-extraction lazy-runtime
  configuration regression also passed.
- `cargo clippy -p ytui-mobile --all-targets --locked -- -D warnings`: passed.
- `cargo check -p ytui-services --locked`: passed; desktop callers explicitly
  retain their existing acquisition behavior.
- Workspace formatting, touched vendor Rust file formatting and `git diff --check`:
  passed. The untouched vendor challenge runtime has pre-existing formatting
  differences, so the entire vendor formatting check is not clean.
- Android `testDebugUnitTest`, `compileDebugAndroidTestJavaWithJavac` and
  `lintDebug`: passed, including the final production-source recheck. All 65 JVM
  tests passed; lint reports 0 errors and 40 warnings, none in the changed Java
  files. The final instrumentation test's asynchronous queue assertion was
  recompiled successfully as well; the device tests were not executed.

## Not verified and limits

No emulator/KVM or connected Android device is available. Live YouTube/CDN hosts
are outside this environment's allowed network destinations. Consequently these
checks do not establish playback of the screenshot's specific song or physical
Next-button acceptance. No APK, deployment or release was produced.

## For the plan and parity tables

M9 Android Music: implemented bounded HTTP 403/410 stream recovery through fresh
full native acquisition, explicit Play retry, and Next during buffering/after a
failed queue entry. Fixture/unit and compilation checks pass; the new navigation
instrumentation cases, physical-device playback and the reported song remain
unverified. This change does not close the live-service or release gates.
