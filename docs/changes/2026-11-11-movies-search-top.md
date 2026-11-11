# Movies search bar at the top of the Android page (M9)

- **Date:** 2026-11-11
- **PR:** pending <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented

## What it does
On the Android Movies page the search field is now the first item, above the alpha notice and the
featured artwork, so it is reachable without scrolling past the hero. While the search field has text,
the featured artwork is hidden and the results appear directly under the field. Clearing the field
restores the artwork. The search field scrolls with the page; it does not pin.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Kotlin compile | `apps/android/gradlew -p apps/android :app:compileDebugKotlin` in a clean worktree of the branch | see PR Verification | implemented |
| JVM unit tests and lint | `:app:testDebugUnitTest :app:lintDebug` in CI (Android lane) | see PR checks | fixture/unit |

Local JVM unit tests and lint were not run on this machine: the installed JDK is 27 and the Android
`androidJdkImage` jlink step fails on it. CI uses JDK 21.

## Not verified and limits
- No emulator or physical-device run of the new layout.
- The search field's position relative to the notice and the Continue watching shelf was checked by
  reading the code only.
- Pinning the search field during scroll was not implemented.

## For the plan and parity tables
- Android Movies page: search is the first control on the page; featured artwork hides while a query is
  active (M9, implemented, not device verified).
