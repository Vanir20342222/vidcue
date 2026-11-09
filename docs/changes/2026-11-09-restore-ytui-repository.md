# Restore the ytui repository name with a rename fallback (M9)

- **Date:** 2026-11-09
- **PR:** #<number> <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android, CI-release
- **Status:** fixture/unit verified

## What it does

The GitHub repository is `Vanir20342222/ytui` again after a brief rename to
`vidcue`. The publish and Preview workflow guards check `ytui` again, the app
updater queries `ytui` as its canonical release source, and — the fallback the
owner requested — asset URLs under the briefly renamed `vidcue` repository are
still accepted, so releases published during the rename window (beta.8) remain
installable from every future version.

## Cause

The rename to `vidcue` silently skipped both Android workflow jobs (their
guards matched the old name) and, after the guards were fixed for `vidcue`,
broke the beta.7 in-app updater the same way (hard-coded prefix). Restoring the
name removes both problems at the source; the app-side fallback keeps either
name working if the repository is ever renamed again.

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Update-check unit tests | `./gradlew :app:compileDebugKotlin :app:testDebugUnitTest --tests dev.ytui.android.AppUpdatesTest` | 7 tests, 0 failures (fallback-prefix case included) | fixture/unit verified |
| Workflow guards | `git grep github.repository` on the branch | both jobs match `Vanir20342222/ytui` | fixture/unit verified |
| Repository state | `gh repo view` | `Vanir20342222/ytui` restored | live verified |

## Not verified and limits

- No device run yet: beta.7 → beta.9 in-place updating through the app's
  updater still needs the real-device acceptance check.
- beta.8 remains published and valid; its asset URLs redirect under the
  restored name.

## For the plan and parity tables

No plan or parity row changes: repository naming and update-check URL
acceptance only.
