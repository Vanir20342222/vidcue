# Android visual fixes: resume chip, upload age, movie hero, glyphs, download stall (M9)

- **Date:** 2026-11-24
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified

## What it does
- YouTube Home: the full-width "Continue watching" hero is gone. The last video is now a small
  chip (cover + play mark) beside For you and Subscriptions.
- Feed cards show how long ago a video was uploaded on its own line with a clock glyph
  ("1 hour ago", "5 years ago"). The "Upload date" card setting now defaults on; an explicit
  choice is kept. Provider text with hour precision wins over an upload date of today.
- Music (and any tab): pulling to refresh keeps the category chips instead of blanking them
  until the page reloads.
- Movies hero: no synopsis or "Continue watching / Featured" caption. Bold title, small tags
  (episode, time left or runtime, year, seasons, genres), Play/Resume plus a round details
  button, and a resume bar along the bottom edge.
- Glyphs: redrawn Refresh (closed arc with a solid arrowhead) and Music (beamed eighth notes);
  new Clock.
- Sync key sheet: with the key present it said nothing (a lone key icon). It now says
  "Key on this device" and hints where requests appear.
- Download from the player or a card menu no longer freezes the UI: the enqueue (download
  index reads and `StorageManager.getAllocatableBytes`) moved off the main thread, so the
  flying cover no longer hangs mid-flight.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Kotlin/Java compile incl. androidTest | `./gradlew --offline :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` | pass | build |
| JVM unit tests | `./gradlew --offline :app:testDebugUnitTest` | 368 run, 0 failed, 7 skipped (opt-in screen tours) | unit |
| New: `UploadAgeTest`, `CategoryNavigationTest.refreshingAPageKeepsItsCategoryChipsWhileItReloads` | included above | pass | unit |
| Visual check of glyphs, resume chip, age line, movie hero | throwaway Robolectric NATIVE-graphics render (not committed) | looked as intended | render |

## Not verified and limits
- Not run on an emulator or the Galaxy Tab S9; instrumentation tests updated
  (`CardInformationUiTest`, `CatalogUiTest`) but not executed.
- The 10 s download stall was diagnosed from code (main-thread `OfflineStore.add`), not
  reproduced with a profiler. Playlist downloads (`PrepareWorker`) still post `add` to the main
  thread.

## For the plan and parity tables
- Android Home: resume is a chip beside For you / Subscriptions; cards show upload age by default.
