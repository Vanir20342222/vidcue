# Canonical vidcue repository with a ytui release mirror (M9)

- **Date:** 2026-11-09
- **PR:** #134
- **Milestone:** M9
- **Platforms:** Android / shared engine / CI-release
- **Status:** fixture/unit verified (plus live mirror state)

## What it does
The repository is `Vanir20342222/vidcue`. The Android app updater and the desktop updater query
`vidcue`. Both accept APK asset URLs from `vidcue` and from the `Vanir20342222/ytui` release
mirror. The `github.repository` guards in the preview and publish-local release workflows now name
`vidcue`; while they named `ytui`, those workflows skipped every run on the renamed repository. The
mirror repository copies every non-draft release on a schedule, so installs that still query `ytui`
keep receiving updates.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Desktop updater | `cargo test -p vidcue-services` | 114 passed, 8 ignored | unit |
| Android updater | `AppUpdatesTest` | 7 passed | unit |
| Mirror | `gh release list -R Vanir20342222/ytui`; mirror workflow runs | beta.7 to beta.9 mirrored; recent runs succeeded (2026-11-11) | live |
| Old-install download URL | `ytui/releases/download/android-0.3.0-beta.9/vidcue-0.3.0-beta.9.apk` | works | live |

## Not verified and limits
No physical-device update check against the mirror. GitHub disables scheduled workflows after
60 days without repository activity, which would stop the mirror.

## For the plan and parity tables
- Canonical repository `Vanir20342222/vidcue`; `Vanir20342222/ytui` is a release mirror for old installs.
