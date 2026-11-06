# Hide completed export receipts from Downloads (M9)

- **Date:** 2026-11-05
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented

## What it does

The Android offline snapshot omits export-status entries whose state is
`Export complete`, including receipts persisted by earlier versions. Active,
queued, cancelled and failed exports retain their status cards. Cached downloads,
saved-file records and export workers are unchanged; no media files are deleted.

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Patch whitespace | `git diff --check` | Passed | Static |
| Android compile | `./gradlew --offline :app:compileDebugJavaWithJavac` | Blocked: uncached Gradle distribution, DNS failure for services.gradle.org | Not compiled locally |

## Not verified and limits

No APK or emulator/physical-device verification was performed locally.

## For the plan and parity tables

M9: completed export receipts no longer appear in the Android Downloads snapshot,
including old saved receipts. Active and failed export status remains visible.
Implemented; device acceptance pending.
