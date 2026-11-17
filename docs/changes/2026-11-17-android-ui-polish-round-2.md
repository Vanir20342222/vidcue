# Android UI polish after independent design reviews (M9)

- **Date:** 2026-11-17
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** unit verified; rendered off-device (Robolectric) for review; emulator walkthrough of
  earlier rounds

## What it does
Follows #166. The owner asked (2026-11-16) for repeated rounds of independent design review
against Spotify, YouTube Music, YouTube and Netflix, each turn fixed and re-graded. Twelve rounds
moved the grade from 6.4 to about 7.3 of 10, where it levelled off; the owner then asked to ship.
The owner also chose (2026-11-16) to take the accent colour from the logo.

What a user notices:

- **Colour.** Vidcue Dark and Vidcue Light use the logo's pink (#FF4D8D, #D6336C on light) as the
  accent instead of mint. The shared palettes, and so the desktop app, are unchanged. Selected chips
  are white with dark text, so they never read as action buttons.
- **Navigation and chrome.** The tab indicator is Material's 56×32dp pill behind the icon. The page
  fades out under the bottom bar and the mini player floats over that fade, in line with the page's
  16dp gutter, with its progress line inside the card. Overflow (⋮) buttons are secondary grey. Pages
  opened from another keep only back and title in their top bar.
- **Home.** Video cards use the 16dp page margin and one metadata line ("Channel · views · age").
  The signed-out topic "Music" is "Music videos".
- **Music.** Section headings share one height and spacing; "Play all" is a compact outlined button;
  Quick picks shows wider columns without lengths or ⋮ (press and hold opens the menu).
- **Playlist pages.** The cover's colour wash runs up into the title row with no seam; the cover is
  larger; Download, Save (circled plus) and ⋮ are bare icons on the gutter, and Play is a round accent
  button.
- **Player.** The header says where the music plays from ("PLAYING FROM PLAYLIST" over its name)
  and has ⋮ at its right; the title row has Save; the cover and controls are centred as one block;
  shuffle and repeat line up with the seek line; the bottom row keeps Queue, Lyrics, Download, Share.
- **Search.** Music results on "All" are grouped: a Top result card, then Songs, Albums, Playlists
  and Videos under headings, without repeating the kind in each row. Suggestions no longer start with
  a "Search for" row; submitting closes the keyboard.
- **Radio.** Stations are a list of rows (cover, name, LIVE and genre); "Saved" is "My stations".
- **Library.** "+" in the top bar downloads from a link; New playlist is on the Playlists section;
  Downloads' empty state is centred under the header with "Find music to download"; Library opens on
  History when there are no downloads but there is history; "Offline playlists" is "Playlists" and
  "Your playlists" (signed in) is "YouTube playlists".
- **Setup.** Progress dots sit above the main button at one height on every page; Skip is in the same
  corner on every page; the theme page shows System, Vidcue Dark and Vidcue Light as equal preview
  cards; the account page is centred, says what signing in brings, and offers "Other ways to sign in"
  under the main button.
- **Type.** Small text is 12.5sp; headings and page titles are a step smaller and less tightly
  tracked.

For review without an emulator, `ScreenTourTest` renders these screens with synthetic content on the
JVM (`VIDCUE_SCREENS=<dir> gradlew :app:testDebugUnitTest -Pscreens --tests '*ScreenTourTest*'`). It
runs only with that variable and property; the normal unit suite skips it and keeps its
resource-free setup.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Unit suite, lint, builds | `gradlew :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest :app:lintDebug :app:assembleDebug` (rebased on #169) | 315 tests, 0 failures (2 skipped: the screen tour without its variable); lint 0 errors | unit |
| Design review | 12 rounds of an independent reviewer grading rendered screens against the four apps | 6.4 → about 7.3 of 10 | review |
| Screen renders | `ScreenTourTest` (Robolectric, 360×800dp) | all tour screens render | unit |

## Not verified and limits
- Not run on a physical device or with a signed-in account; the emulator suites were not run
  locally. Instrumentation tests were updated where behaviour moved (Library "+", New playlist on
  Playlists, the 16dp feed margin).
- Rendered screens use synthetic content; cover loading and list scroll positions differ from a
  device there.
- Playlist duration, station country and bitrate, and a Library overview need engine data and are
  not included.

## For the plan and parity tables
- Android UI: logo-pink accent on the app's own themes; Material navigation indicator; grouped music
  search with a top result; "Playing from" in the player; radio as a station list; setup with preview
  theme cards (2026-11-17).
