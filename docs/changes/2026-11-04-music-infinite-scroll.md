# M9: Music infinite scrolling

- **Date:** 2026-11-04
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified; new UI regression cases compiled only

## What it does

Music feeds with grouped sections stopped at the **Load more** button because
automatic pagination compared a visible lazy-grid index with the number of
songs. Each section contributes only two grid items (heading and horizontal
shelf), regardless of how many songs it contains.

`BrowsePane` now checks the last visible index against the lazy grid's actual
item count and loads the next page within five items of the end. It observes
layout changes and re-evaluates when the loaded card count, continuation,
loading state or error changes. The shared browse footer no longer requires a
**Load more** tap. Existing request deduplication, finite-feed termination,
retained results and explicit error retry remain in place.
The trigger also waits until the laid-out item count matches the current page,
including navigation and notice headers, so an older page or skeleton cannot
trigger pagination using its shorter item count.

## Evidence

Two new `NativeUiTest` regressions cover Music shelves and Music search grids.
Each checks that pagination waits until near the end, starts without a button
tap, preserves all loaded cards on failure and does not immediately retry the
failed continuation. The synthetic continuation has an invalid endpoint that
the provider rejects before HTTP; the offline UI mode also blocks speculative
playback requests for these synthetic media URLs.

Local checks passed on the implementation based on `f7feb4a`, using Android
SDK/build-tools 35 and a local JDK 21 installed outside the repository:

- Main Kotlin and Java compilation.
- `:app:testDebugUnitTest`: 80 tests across 17 suites, zero failures/errors/skips.
- `:app:compileDebugAndroidTestKotlin`: includes both new UI regression cases;
  compilation only, not an instrumentation run.
- `:app:lintDebug`: zero errors, 96 warnings and five hints.
- `git diff --check`.

The combined Gradle command completed successfully after the layout guard was
added. The change was then applied cleanly to main `f8d3da0` for the PR;
`:app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` passed again on
that base, including main Java compilation and both new UI regression sources.

## Not verified and limits

No emulator/device was attached, no APK was released and no live Music request
was tested. The two new UI cases require an emulator/device run; compilation
does not establish that they passed. Emulator, physical-device and live Music
acceptance remain separate M9 gates.

## For the plan and parity tables

M9 Android browsing: Music shelves and ordinary result grids now use the lazy
grid's actual item count for automatic pagination. The Load more footer is
removed; page-layout matching prevents early requests from an older, shorter
layout. Existing continuation termination, loaded cards and explicit retry on
failure are preserved. Local compilation/unit/lint checks pass on the original
implementation base, and app/instrumentation-source compilation passes again
on main `f8d3da0`; the two new Music UI regressions are compiled only.
