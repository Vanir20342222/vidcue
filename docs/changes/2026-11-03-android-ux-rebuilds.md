# Android UX rebuilds: notices, sleep timer, video chrome, flows, tablet (M9)

- **Date:** 2026-11-03
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android, shared engine
- **Status:** implemented; compiled, unit tested, and rendered locally (Robolectric, host-built engine). No emulator or device run.

## What it does
This follows the redesign in `2026-11-03-android-screening-room-redesign.md`. The owner asked
(2026-11-03) to build all 14 items from a UI/UX review.

1. **Errors and confirmations reach the screen.**
   - Messages such as "Added to queue", and errors such as an invalid pasted link, were set on
     `vm.status`, which only the Account sheet displayed.
   - A notice layer above the dock now shows errors as a card with Dismiss (up to 10 s), and the
     invalid-link and live-recording errors now set `actionError`.
   - Completed actions (queue, download, bookmark, lyrics, palette) get a wordless check disc that
     screen readers announce. The test that requires "Added to queue" to stay off-screen as text
     still holds.
2. **Sleep timer.**
   - The service publishes `sleep_deadline`. The mini player shows a moon countdown, and the
     player's Sleep action turns into the remaining time.
   - The sheet shows time left with "+15 min" and adds "End of this track".
3. **Video controller.**
   - Media3's default control icons are replaced through drawable aliases with the app's line
     glyphs.
   - The titles and timecodes use the app's fonts, with tabular digits for the timecodes, and the
     timebar is slimmer.
   - The contrast discs, the accent and the sponsor marks are unchanged.
4. **Continuous player open.** The playing cover is one shared element that flies between the
   mini player and the music player (`SharedTransitionLayout`, caller-managed visibility, music
   only, off under Reduced motion). The video path is unchanged.
5. **Theme previews.**
   - New engine call `palettes` returns every built-in palette ("system" as a dark/light pair).
   - The theme row, the theme dialog and the onboarding chips show a miniature of each theme.
   - The row reads "Theme / ytui Dark" instead of the raw ID.
6. **Queue and offline playlists.**
   - Rows can be dragged by their handle to reorder and swiped left to remove.
   - Removing from the queue offers Undo inside the sheet, and "Clear queue" can be undone too.
   - The existing menu actions remain.
7. **Search memory.**
   - While search is focused and empty, recent searches (kept on this device, honouring History
     & resume, cleared with history) and a "Paste from clipboard" row cover the feed.
   - The clipboard is read only when tapped.
8. **Shelves.** Feeds with two or more provider sections show titled horizontal shelves on Home
   and Music; single-section feeds and search stay a grid.
9. **Item sheet.**
   - Four quick tiles (Play now, Add to queue, Download, Preview) sit above the rows.
   - "Save to library" is now "Bookmark", with an explanation underneath.
10. **Downloads.**
    - Labelled "Save a link" and "New playlist" buttons replace the two icon buttons.
    - The Offline list shows a count and total size.
    - Transfers show live speed and time left, from new `bytes`/`total` fields, and get
      "Pause all".
11. **Lyrics.**
    - With no lyrics, the sheet offers "Find lyrics" (uses the playing title and artist) or
      "Write or import" instead of an open form.
    - The playing line fills with a karaoke-style sweep, extrapolated per frame between
      snapshots.
12. **Settings.**
    - Sub-pages keep only Back; the top page closes.
    - "Video information" is now "What cards show".
13. **Gesture tips.**
    - The first mini player nudges sideways and captions its swipes, once; offline UI tests skip
      it.
    - Settings › Appearance › Gestures can show the tips again.
14. **Tablet.**
    - The page title sits over the content column, so the navigation dock starts at the top.
    - The side player can be resized by dragging its inner edge (300–480 dp) or folded into a
      strip with cover, play/pause and next.
    - From 960 dp wide, settings show sections and content side by side.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Rust format | `cargo fmt --all -- --check` | pass | build |
| Rust lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` (`--ignore-rust-version`: local rustc 1.97) | pass | build |
| Rust tests | `cargo test --workspace --locked` (same flag), incl. new `palettes_preview_every_built_in_theme` | 392 passed, 0 failed, 31 ignored | unit |
| Kotlin compile | `gradlew :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` | pass | build |
| Android unit tests | `gradlew :app:testDebugUnitTest` | 80 tests, 0 failures | unit |
| Android lint | `gradlew :app:lintDebug` | pass | build |
| Visual review | Local Robolectric harness running `RustActivity` against the host-built engine: notices, sleep sheet and badges, queue with items, recent searches, shelves (video and music), item sheet, downloads with sizes and transfers, lyrics empty and timed, theme swatches, tablet expanded, folded and two-pane settings, and every earlier screen | reviewed | fixture render |

Test updates (copy and navigation changes, none loosened):
- `CatalogUiTest` expects "Bookmark" instead of "Save to library".
- `CatalogUiTest` and `NativeUiTest` go back before closing settings sub-pages.
- `NativeUiTest` finds the theme row by its new "Theme" label plus the theme's readable name.

CI correction (first emulator run): `ContrastUiTest.enabledNativeTransportsKeepContrastOnBrightVideo`
failed with "forward must have 3:1 white-icon contrast on actual white video". The test samples a
point inside each transport disc, beside the glyph. The new forward arrowhead reached that point
(1.2 dp inside the stroke), while the mirrored rewind arrow did not. Both glyphs are now drawn at
80% so the arrow stays clear of it (2.3 dp from the stroke edge for forward). The test and its 3:1
threshold are unchanged. Checked by computing the stroke geometry, not by an emulator run; the next
CI emulator run is the real check.

## Not verified and limits
- No emulator or device: the instrumentation suites, real drag, swipe and shared-element motion,
  IME behaviour and the Media3 overlay were not run. Robolectric cannot draw `TextureView`.
- The gesture hint is suppressed in offline UI mode, so it was checked by reading the code, not
  rendered.
- Transfer speed depends on Media3 reporting `bytesDownloaded`/`contentLength` during downloads.
  It is unmeasured on real transfers.
- Shelves depend on providers filling `section`; their look with real YouTube Music data is
  unverified.
- Errors raised while a bottom sheet is open appear in the app's notice layer under the sheet's
  scrim. Queue undo is shown inside the sheet for that reason.

## For the plan and parity tables
- Android: visible error notices and wordless confirmations; sleep timer countdown, +15 min and
  end of track; restyled video controller; shared-element mini/full player; theme previews (new
  `palettes` RPC); drag and swipe queue editing with undo; recent searches and clipboard paste;
  section shelves; quick actions; download sizes, speed and Pause all; find-first lyrics with
  sweep; back-only settings sub-pages; one-time gesture tips; tablet resizable or foldable player
  and two-pane settings (M9; fixture-rendered, not device verified).
