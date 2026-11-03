# Keep Android action sheets alive through grid relayout (M9)

- **Date:** 2026-11-03
- **PR:** #63
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented; emulator verification pending

## What it does

The browse pane owns an open card action sheet instead of a recyclable grid item.
Opening the sheet releases search focus and dismisses the keyboard. Grid and shelf
relayout can therefore recycle the originating card without discarding its menu.
The same ownership applies to tablet search rows.

Artwork producers are keyed by the exact remembered bitmap state they fill, so a
replacement state cell starts its own producer. Debug builds log load, detach and
completion using hashed media identities rather than URLs.

## Evidence

Android run 41710182428 for head `a89ae24` passed the APK job and shards 1 and 2,
including the previously failing motion-recording test. A single failed-job rerun
retained the missing Download menu in the large-text test and a missing decoded
second cover in a different catalog test. Both cover failures requested every
synthetic image once; a failure-only native probe decoded the missing cover.
These observations motivate the lifetime corrections; they do not establish the
precise cause of the artwork failure.

No assertion, timeout, recording duration, retry count or workflow is changed.
The existing large-text, decoded-cover, tablet-search and motion tests are the
emulator regression checks for this change. Local app and instrumentation-test Kotlin compilation passed. All 80 JVM tests
passed with no failures, errors or skips. `:app:lintDebug` passed. Emulator verification is pending.

## Not verified and limits

No local emulator or physical Android device is available. Real keyboard, touch,
GPU performance and live-service acceptance remain separate from CI emulation.

## For the plan and parity tables

- M9: browse-owned card action sheets survive lazy-grid relayout and release
  search focus when opened; artwork producers follow their bitmap state lifetime.
  See PR #63 for exact-source CI results and remaining device/live limits.
