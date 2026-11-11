# M9: Movies page matches the other Android tabs

The owner reported (with a phone screenshot) that the Android Movies (alpha) page looked
out of place next to the other tabs. On the built-in Nothing palettes, the page used the
dot-matrix display face for the featured title, a monospace red kicker, a labelled form
field for search and a monospace time readout on the resume card.

## Changes (`apps/android/.../MoviesPage.kt`)

- Search uses the shared `SearchField` from the Home/Music/Radio header (search glyph,
  clear and submit controls, reading type) instead of the labelled `MediaTextField`.
- The featured hero is inset and rounded like other artwork. It is shorter (34% of the
  screen height, 220–340 dp) and uses reading type: `titleLarge` bold for the title and
  a quiet `titleSmall` kicker. The dot-matrix face stays reserved for page names.
- The movie sheet title uses `titleLarge` bold, like the playing title in the player pane.
- Continue cards show the remaining time with the shared `ArtworkBadge`. The shelf leaves
  out the title the hero already features, so a single resume entry is not shown twice.
- Card titles use `titleMedium`, matching feed cards.

## Checks

- `./gradlew :app:compileDebugKotlin --offline` passed.
- Not run locally: `:app:testDebugUnitTest`. The only installed JDK (27) fails the
  Android `jlink` JDK-image transform; CI runs these tests.
- Not checked: emulator or physical-device rendering. No device was attached.
