# Player opening releases search focus (M9)

- **Date:** 2026-11-05
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented (not compiled or device-tested in this session)

## What it does
Reported on a physical device: after filtering the offline library (e.g. "sia") and playing a
song, the keyboard kept popping up over the Now Playing screen. The player covers the page but
keeps it composed, so the offline search field (and the browse search field) kept input focus
underneath and Android reopened the IME whenever the window regained focus. Opening the player
now moves focus to the existing non-text focus sink and hides the keyboard. Text fields inside
the player's own sheets and dialogs are unaffected because the effect runs only when the player
opens.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Android compile | `apps/android/gradlew -p apps/android :app:compileDebugKotlin` | Not run: no Android SDK in this environment | — |

## Not verified and limits
- Not compiled here and not tried on a device. Manual check: Library → type in the offline
  search → tap a song → open Now Playing → pull down the notification shade / switch apps and
  return; the keyboard should stay closed.
- No instrumented test added yet for this flow.

## For the plan and parity tables
- Android: opening the player releases search-field focus so the keyboard does not reappear
  over Now Playing (2026-11-05, implemented, device check pending).
