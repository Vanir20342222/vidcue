# Brace the watch-page sleep-timer action for lint (M9)

- **Date:** 2026-11-05
- **PR:** none yet
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented

## What it does
#113 was merged before its last commit, leaving an unbraced `if` around the sleep-timer action
in `FocusedPlayer.kt`. Android lint (`SuspiciousIndentation`) fails on it, so the Android
workflow is red on `main` at d543eb1. This adds the braces. No behaviour change.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Compile | `apps/android/gradlew :app:compileDebugKotlin` | passed | build |
| Lint | Android `app` job in CI | see PR | build |

## Not verified and limits
Nothing beyond lint; the code path is unchanged.

## For the plan and parity tables
Nothing to add.
