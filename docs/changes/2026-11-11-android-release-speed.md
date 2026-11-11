# Faster signed Android release build (M9)

- **Date:** 2026-11-11
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** CI-release
- **Status:** fixture/unit verified

## What it does
The "Android signed APK" workflow no longer runs an unused debug Gradle pass. Before, `build.sh`
ran `assembleDebug`, `testDebugUnitTest` and `lintDebug`, and the release Gradle build then ran
anyway. `YTUI_ANDROID_SKIP_GRADLE=1` stops `build.sh` after the native libraries and license
notices. The release workflow now compiles Rust with 4 jobs instead of the script's
small-PC default of 2. It also restores the release-profile Rust cache that `android.yml` saves on
main (shared key `android-release-profile`) instead of starting cold. It uses the same Gradle
worker and build-cache settings as CI, and checks the debuggable flag with `grep` instead of `rg`,
which the runner image does not guarantee.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Packaging script, including the new skip path | `python3 -I -m unittest discover -s tools/android -p 'test_*.py'` | pass | fixture/unit |
| Workflow syntax | actionlint 1.7.7 | pass, apart from one stale-metadata false positive on `cache-workspace-crates` (already used on main) | static |

## Not verified and limits
The signed release workflow was not dispatched; agents must not dispatch release workflows. The
first `android.yml` main push after merge saves the cache under the new key; until then the release
restores the closest key prefix or builds cold. The release still runs `testReleaseUnitTest` and
`lintRelease`. Only the duplicate debug pass is removed, because it already ran on the exact main
revision that the release requires.

## For the plan and parity tables
- Signed Android release: one release Gradle pass, 4 Rust jobs and a Rust cache shared with main CI.
