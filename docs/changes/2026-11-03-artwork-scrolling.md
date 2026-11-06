# Cover artwork loading while scrolling (M9)

- **Date:** 2026-11-03
- **PR:** #55
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified

## What it does

Cold feed covers wait until scrolling settles, while cached covers appear immediately.
Duplicate viewers share one load, offscreen queued work is dropped, and completed
decodes remain reusable. Quieter placeholders stay under a shorter fade; cards
encountered during scrolling skip delayed entrances. Reduced motion still snaps.

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Unit tests | `:app:testDebugUnitTest` | 64 passed; eight artwork regressions | unit |
| App and test compilation | `:app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` | Passed | compile |
| Lint | `:app:lintDebug` | Passed with warnings | static |
| Patch whitespace | `git diff origin/main --check` | Passed | static |

The Android checks were rerun after rebasing onto main `4cdec38`, with Kotlin
2.1.21, JDK 21 and Android SDK 35.

The regression suite covers 100 concurrent viewers sharing four covers across
worker threads, scroll-away/back joining, cache warming after viewers leave,
dropping abandoned queued work, independent cancellation, the concurrency limit
including orphaned work, and recovery after null/error results. A new loopback
instrumentation test checks decoded scroll-back pixels and request reuse; it is
compiled but has not run on a device.

## Not verified and limits

No emulator or physical device is attached. Device scrolling/jank, live CDN
behavior and memory pressure remain unmeasured. Cold images need their first
fetch. Run the loopback instrumentation regression on a disposable app and check
cold/warm scrolling on phone/tablet with animations and reduced motion, network
failure recovery, and artwork disabled. No release acceptance is claimed.

## For the plan and parity tables

M9 artwork scrolling: Android shares in-flight cover decodes, drops abandoned
queued loads, keeps finished decodes in bounded memory and defers cold feed
artwork until scrolling settles. Cached covers paint immediately; quieter
placeholders remain under a shorter fade, and scrolled-in cards skip staggered
entrances. Unit/compile/static checks pass; device scrolling/jank remains open.

Suggested Android validation entry:

Cover loads now share one Kotlin fetch/JNI decode per media/source/size. Four
loads may run at once. A card leaving the composition cancels its wait and drops
queued work with no remaining viewers; a decode already running finishes into
the existing 24 MiB LRU instead of throwing away pixels. The native image cache,
origin/proxy handling and disk limits remain in use. Bitmap preparation runs off
the main thread, and a cached larger decode can serve a smaller artwork frame.

Cold feed loads wait for scrolling to finish plus a 100 ms settling window.
Cached covers render in their initial composition. Placeholders use a quieter
surface/icon and stay below the 180 ms artwork fade; opacity is read in graphics
layers rather than recomposing the card each animation frame. Reduced motion
still snaps. Cards first encountered during scrolling skip staggered entrances.

`ArtworkLoaderTest` covers sharing, cache hits, scroll-away/back joins, warming
the cache after viewers leave, dropping queued offscreen work, independent viewer
cancellation, concurrency including orphaned work, and recovery after null/error
results. `CatalogUiTest.scrollingBackReusesReadyArtworkWithoutAnotherFetch` adds
a loopback/device check for decoded scroll-back pixels and request reuse.

Local verification after rebasing onto main `4cdec38`, with Kotlin 2.1.21,
JDK 21 and Android SDK 35:

- `:app:testDebugUnitTest`: **64 tests passed**, zero failures/errors/skips;
  all **8** `ArtworkLoaderTest` regressions passed, including 100 concurrent
  viewers sharing four covers across worker threads.
- `:app:compileDebugAndroidTestKotlin`: passed, including the new loopback
  scroll-back test. This compiles instrumentation; it does not execute it.
- `:app:lintDebug`: passed. The combined Gradle build completed successfully.
- `git diff --check`: passed.

No emulator/physical-device scroll timing, GPU frame/jank measurement, live CDN
acceptance or APK publication is claimed by these source changes. The device
regression must run on a disposable app, followed by cold and warm fast scrolling
on phone/tablet with animations enabled and reduced motion, offline/network
failure recovery, and confirmation that artwork-disabled feeds remain usable.
