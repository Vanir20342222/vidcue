# Android UI: Library overview, radio shelves and review fixes (M9)

- **Date:** 2026-11-17
- **PR:** #171
- **Milestone:** M9
- **Platforms:** Android
- **Status:** unit verified; rendered off-device (Robolectric) for review

## What it does
Follows #170. The owner asked (2026-11-17) to continue the independent design reviews against
Spotify, YouTube Music, YouTube and Netflix, with permission for larger features. Six more rounds
graded 7.2–7.5 of 10; the reviews contradicted each other between rounds, and the owner chose to
ship.

What a user notices:

- **Library** opens on an overview ("All"): signed out, a card to sign in; then one row per
  collection (Downloads, offline playlists, Saved, History, and signed in YouTube playlists, Liked,
  Watch later) with what it holds, then Recently played with "Show all". The other chips still open
  each section.
- **Radio.** Featured starts with "Popular now", large square station tiles, then the remaining
  stations as rows. Rows say genre and country ("Jazz · France") instead of "● LIVE" on every
  station.
- **Music.** Shelves of albums or playlists have "Show all", which opens the shelf as a grid on the
  same page (Back returns). Section actions are quiet text buttons. Quick picks have wider columns
  and their menu again. Music rows are 64dp throughout, and only album pages show song lengths.
- **Search.** On "All", each group shows four results; "Show all" on a group that was cut short
  opens that filter. The search bar has the same height as page titles.
- **Playlist pages.** The creator has its own line; the line below adds the total length
  ("Playlist · 14 songs · 1 hr 1 min"). Download, Save and More (⋯) share one icon style.
- **Player.** The cover sits under the header and the controls lower down; shuffle and repeat show
  their state; the bottom row is icons only (Queue, Lyrics, Download, Share) on the same columns
  as the controls above; the play glyph is larger.
- **Mini player** is 56dp tall with a 40dp cover; its progress line runs along its lower edge.
- **Setup.** One footer height on every step; the account step offers "Not now" and "Sign in" side
  by side with "Other ways to sign in" under the list; theme cards read System, Dark and Light,
  System previews half dark, half light, and the selected card has a check. Copy is shorter.
- **Details.** ⋮ buttons line up with the header icons; the current song keeps its bars while
  paused; the tab indicator is easier to see; the Home wordmark is smaller.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Unit suite, lint, builds | `gradlew :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest :app:lintDebug :app:assembleDebug` | 318 tests, 0 failures (2 skipped: the screen tour without its variable); lint 0 errors | unit |
| New unit tests | `StationDetailsTest` (station genre and country, collection length) | pass | unit |
| Screen renders | `ScreenTourTest` (Robolectric, 360×800dp) | all tour screens render | unit |
| Design review | 6 rounds of an independent reviewer grading rendered screens against the four apps | 7.2–7.5 of 10 | review |

## Not verified and limits
- Not run on an emulator or physical device, or signed in. Instrumentation tests were updated where
  behaviour moved (Library opens on its overview: the selection test opens Downloads first).
- Station genre comes from Radio Browser tags; stations without tags show only their country.
- "Show all" on Music shelves shows the cards the shelf already has; it does not load more.

## For the plan and parity tables
- Android UI: Library overview; radio featured tiles with genre and country; "Show all" on music
  shelves and search groups; playlist length on collection pages (2026-11-17).
