# Play all on home shelves (M9)

- **Date:** 2026-11-05
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented (not compiled or run in the authoring container)

## What it does
Titled shelves on the Android Home and Music tabs ("Quick picks", "Listen again", …) now
show a **Play all** pill next to the heading. It starts the shelf's first playable entry
and continues through the rest of the shelf in order, using the existing finite
visible-list context (`radio_context`), so Next and auto-advance stay inside the shelf
instead of switching to related recommendations. Albums, playlists, channels and live
entries are skipped, and the pill is hidden on a shelf that has none of the remaining kinds.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Shelf context unit test | `PlaybackContextsTest.shelfPlayAllStartsAtTheFirstPlayableEntryAndKeepsShelfOrder` | Added, not run here (no Android SDK in the container) | unit |

## Not verified and limits
- Not compiled; the Robolectric suite and APK build must pass in CI.
- No emulator or physical-device check of the heading layout or playback continuation.
- The previous/next scroll arrows in the reference screenshot are not part of this change.

## For the plan and parity tables
- docs/android/PARITY.md: "Home/Music shelves: Play all plays a shelf in order (unit only)."
