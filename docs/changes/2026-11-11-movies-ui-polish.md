# Movies page browsing and detail polish (M9)

- **Date:** 2026-11-11
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified

## What it does
The Android Movies (alpha) page gains filter chips under the search field (All, Films, Series,
and Downloads when something is saved). Films, Series and search results show as a wrapping
poster grid: three columns on a phone, more on tablets. Shelf posters are larger on screens at
least 600 dp wide. The detail sheet opens on a wide artwork banner with the poster, the title and
the close control. A full-width Play button ("Play episode N" or "Resume from …") with resume
progress comes before the synopsis and episode list, so it is reachable on long series. Season
chips scroll sideways instead of overflowing, and source or provider errors now show under the
actions, not at the bottom of the sheet. Only `MoviesPage.kt` changes. Catalog data, playback,
downloads and Cinecat behaviour are unchanged.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Android unit tests + lint | `JAVA_HOME=~/.tooling/java17 ./gradlew :app:testDebugUnitTest :app:lintDebug` (JDK 27's jlink breaks AGP's `androidJdkImage`, so a task-local JDK 17 was used) | 291 tests, 0 failures; lint passed | unit/static |

## Not verified and limits
- Not run on an emulator or device; no screenshots of the grid, chips or banner.
- The grid is a non-lazy block in the page list. That is fine for the current catalog size, but a
  much larger result set would need a lazy grid.

## For the plan and parity tables
- Android PARITY, Movies (alpha): "filter chips, poster grid for search/filters, banner detail
  sheet with full-width Play — unit/lint verified; device acceptance open."
