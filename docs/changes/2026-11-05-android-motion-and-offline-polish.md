# Player motion, live download card, offline selection and search, watch page (M9)

- **Date:** 2026-11-05
- **PR:** none yet
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented; compiled and unit tested (no emulator or device run)

## What it does
Owner request (2026-11-05): finish incomplete animations and fix several UI problems.

- **Player opens like it closes.** The full player (music and video) now rises from the bottom
  of the screen, starting one screen down and slightly shrunk, which is the exact reverse of the
  swipe-to-dismiss. The minimize button, the video's collapse button and system Back now glide
  the player down the same way instead of fading or cutting. The video player previously had no
  open animation at all.
- **Download card at the bottom right with real progress.** When Save is pressed where the
  Downloads tab cannot be seen (for example in the player), the cover flies to a card in the
  bottom-right corner instead of a static "Downloads" pill in the middle. The card shows the
  item's cover and title and follows the actual transfer: queued, percent with a real progress
  bar, failed, or saved. It still opens Downloads when tapped.
- **Offline selection.** Press and hold any offline item to start selecting it. While selecting,
  a header shows a rolling "N selected" count, a close button and Select all; Add to playlist,
  New playlist and Delete sit in a floating action dock above the navigation. Items show an
  animated round check and their cover eases back when chosen. Removed items animate out.
- **Offline search.** A search field filters Offline media (title, artist/channel, quality,
  ignoring case and accents) and Offline playlists (name or member titles/artists).
- **Watch page.** Under an inline video: title with views and age and a "…more" affordance,
  channel row with a larger avatar and subscriber count, a shorter action row (Queue, Save,
  Share, output; the sleep timer only while running, otherwise in ⋮), and a collapsible
  description card. The details area is capped so Up next keeps room.
- **Video player style.** Inline video has soft rounded corners (bottom corners in portrait,
  all corners beside the details in landscape). Taller three-stop scrims replace the banded
  ones, the timecode sits in a translucent pill, the scrubber is larger, and every video control
  sinks on press and springs back with a small overshoot (off with reduced motion).
- **Playback options sheet.** Speeds are one row of chips instead of a separate list page, and
  sub-pages (quality, captions, audio) slide in and out with the sheet height easing to fit.
- **Other incomplete animations fixed.** The mini player now slides away when playback stops
  or is closed (it previously vanished between frames) and transfer cards move smoothly when
  one finishes.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| App compiles | `apps/android/gradlew -p apps/android :app:compileDebugKotlin` | see PR verification | build |
| Test APK compiles | `apps/android/gradlew -p apps/android :app:compileDebugAndroidTestKotlin` | see PR verification | build |
| Unit tests | `apps/android/gradlew -p apps/android :app:testDebugUnitTest` (new `OfflineSearchTest`, existing `DownloadFlightTest`) | see PR verification | unit |

## Not verified and limits
- No emulator or physical device run: the animations, the long-press gesture, the rounded
  TextureView clip and the Media3 press springs have not been seen on a screen.
- `CatalogUiTest.offlineSelectionCreatesPopulatedPlaylistsAndAddsOnlyMissingSongs` keeps its
  tags (`offline-select`, `offline-select-all`, `offline-selection-count`,
  `offline-add-selected`, `offline-playlist-selected`, `offline-delete-selected`) but was not run.
- The download card matches a transfer by media URL (title for playlist jobs); a provider that
  rewrites the URL on save would show "Queued" with an indeterminate bar until it lands.
- Subscriber count and description appear only when the resolved stream provides them.

## For the plan and parity tables
- M9 Android: full player opens and closes with the same slide (button, Back and swipe);
  download confirmation is a bottom-right card with live transfer progress; offline library has
  long-press multi-select with a floating action dock and a search filter; watch page shows
  subscribers and a collapsible description. Implemented and unit tested; device check pending.
