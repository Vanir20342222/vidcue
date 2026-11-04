# Android download confirmation disc: badge ring and replay (M9)

- **Date:** 2026-11-04
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented (not compiled or run in this environment; see Evidence)

## What it does
After a download is queued, the centered confirmation disc shows a download glyph with a check
badge in its corner. The badge now has a background-colored ring, so it reads as a distinct badge
on the light disc instead of blending into it. If a second confirmation (for example, a second
download) arrives while the disc is still visible, the check redraws instead of staying frozen.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Confirmation trigger path | Read `RustRepository.kt` download flow | `confirm("download")` runs only after the effect succeeds | Code read |
| Edit review | `git diff` on `Notices.kt` | Only `ConfirmationDisc` changed; no shared model or contract touched | Code read |
| Kotlin compile | Not run | No Android SDK (`ANDROID_HOME` unset) in this environment | Not run |
| Emulator / device | Not run | No emulator or physical device available | Not run |

## Not verified and limits
- The change is not compiled. Run `apps/android` Gradle assemble and the Compose UI tests on an emulator before relying on it.
- The visual result (ring contrast on light and dark themes, and the check replay timing) needs a screenshot on a device.
- Reduced-motion behaviour is unchanged: the badge snaps to fully drawn.

## For the plan and parity tables
No plan or parity rows change. This is a visual polish fix to an existing confirmation.
