# Android UI and UX review changes (M9)

- **Date:** 2026-11-15
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** emulator verified (manual walkthrough on an API 35 emulator, 360×800 dp, signed out); unit verified

## What it does
The owner asked (2026-11-15) for the suggestions in
[`docs/android/UI_UX_SUGGESTIONS_2026-11-15.md`](../android/UI_UX_SUGGESTIONS_2026-11-15.md) to be
implemented. That review compared the app with Spotify, YouTube Music, YouTube and Netflix. This
change reverses three earlier decisions at the owner's request: the dock now has labels, the main
action on a screen uses the theme's accent, and swiping between tabs is off by default.

What a user notices:

- **Navigation.** The dock shows a label under each icon. Downloads became **Library**: downloads,
  offline playlists and transfers, plus (signed in) your playlists, liked videos and watch later,
  and what you saved and your history, all behind one chip row. Movies (alpha) leaves the dock
  and can be turned on in Settings → Advanced. Swiping sideways between tabs is off by default
  (Settings → Gestures → Pages).
- **One-row header.** Home shows the app mark, other tabs their name. Search is an icon that
  opens the field; the field is gone otherwise, so content starts much higher. The ⋮ menu is
  gone. The round button on the right opens **You**: the account first, then History, Playback
  queue and Sleep timer, then the settings groups, Advanced, Setup guide and About. Lyrics opens
  from the player.
- **Pages opened from another page.** Playlists, albums, artists and channels get a header with
  the cover, name, owner and count, and Play, Shuffle, Download and Save. The page title row
  shows a back arrow and the page's name; the "Back to previous feed" pill is gone. Album tracks
  are numbered rows.
- **Music and Home.** Songs are compact rows (cover, title, artist, length). Shelves of songs page
  sideways four rows at a time. Music opens with "Jump back in" tiles from your history; Home
  opens with "Continue watching" for the last video. Home's video cards sit inside a 12dp margin
  with rounded corners, like shelf cards. "Play all" is a labelled button. Home's library chips
  moved to Library; Subscriptions stays on Home when signed in. Signed out, an empty Home offers
  topics to start from and Sign in instead of an empty page. Radio has Featured, Saved and
  "Countries and genres" chips; "Cliamp channels" is now "Featured stations".
- **Search.** Results are rows (thumbnail left) on phones. Video results can be narrowed to
  Videos, Channels or Playlists, music results to Songs, Albums, Artists, Playlists or Videos.
  While typing, suggestions come from your earlier searches, your history and the page you are
  on. The field's "→" button is gone (the keyboard's search key submits); "×" shows only with
  text. The placeholder is "Search" (or "Search stations").
- **Cards.** Press and hold opens a card's menu. Its four tiles are Play, Play next (new), Add to
  queue and Download; the muted preview became a row, and "Bookmark" is "Save to Library". Small
  shelf cards drop their ⋮ button.
- **Mini player.** Swiping it down stops playback and offers Undo. Its right button is Next for
  video too (closing is the swipe). The progress line no longer runs into the rounded corners.
- **Player.** The action row shows labels: Queue, Lyrics (music), Download, Share and More.
  Audio output and the sleep timer moved into More (a running timer still shows its countdown in
  the row). Up next rows have a menu (Play next, Add to queue, Remove from queue), and an empty
  queue offers a way back to browsing. Double-tapping the music cover no longer pauses. Stop
  left the tablet's compact player; it stays in Playback options. The seek buttons are circular
  arrows with the number inside.
- **First run.** Setup has three pages (welcome, look, account) instead of five; playback choices
  keep their defaults. The welcome page shows the launcher mark. Notification permission is
  asked when something first plays or a download starts, not on the first view of Home.
- **Settings and account.** Gestures have their own page in three groups (Pages, Players, Video).
  Animation style and speed, streaming cache, preload on mobile data, the startup timing report
  and the Movies switch moved to Advanced. Session import is folded under "Other ways to sign
  in", which also explains when to use it. About says what the app is.
- **Words.** Save means saving to your Library; Download means a file on the device. Downloads,
  Library and Saved replace Offline, Saved locally, Bookmark and Save offline. "Recently played"
  is History. Empty states say what to do ("Nothing here yet", "No results"). Movie errors and
  two engine messages shown by the app are written as what happened and what to do.
- **Look.** Corner radii are 8, 12, 16 and 28dp plus the pill. The selected chip is the accent
  instead of solid white. Primary buttons use the accent.
  The settings icon is a gear and Library has its own icon.

After the first emulator walkthroughs, an independent design review graded the screenshots against
Spotify, YouTube Music, YouTube and Netflix (5.8/10, then 6.4/10). Its findings were fixed in the
same change:

- **Signed-out Home** shows what is popular this week: topic chips (All, Music, Gaming, News, …),
  each a YouTube video search sorted by views and limited to this week. It replaces the empty
  "Start with a topic" page when YouTube returns no anonymous recommendations. These feeds keep
  the page header and chips; they are not searches.
- **Music titles** drop "Artist - " prefixes and tags such as "(Official Video)" in song rows, the
  mini player and the music player ("Wunder", not "AYLIVA x APACHE 207 - Wunder (Official
  Video)"). Bylines drop kind words, play counts and durations shown elsewhere, with one separator.
  Video titles are unchanged.
- **Bottom of the screen.** On phones the navigation is a full-width bar in the page colour; the
  page fades out behind it and the mini player floats above the fade. The mini player has a 16dp
  radius, a plain play/pause icon (no white disc) and its progress line on its flat lower edge.
- **Type and spacing.** Row titles are 14.5sp medium, small text 13sp, and headings are less
  tightly tracked. Rows in search results and on playlist pages sit 4dp apart. The header is 8dp
  shorter.
- **Chips.** The selected chip is the accent with its own ink; unselected chips use full-contrast
  text. Chip rows fade at an edge with more to scroll.
- **Playlist pages** have a full-width wash that rises from the page colour (no seam under the
  title row); the title row shows the name only after the header's own title scrolls away.
- **Search.** Suggestions while typing offer single names ("Fleetwood Mac", not the whole artist
  list) and mark earlier searches with a history icon; a picked or restored query puts the cursor
  at its end; submitting closes the keyboard and cursor; the field is filled, without an outline.
  Music results use 56dp art, round for artists.
- **Player.** The scrolling title fades at both ends; the seek line lines up with the times, which
  sit just under it and show elapsed and total length.
- **Radio** shows two stations per row on phones; a station whose genre repeats its name shows its
  description.
- **Setup.** Sign in is the account page's main button in the bottom slot, with "Maybe later"
  under it and "Other ways to sign in" (session import) on the page. The theme page shows System,
  Vidcue Dark and Vidcue Light, with the rest behind "More themes"; the list fades out above the
  button.
- **Smaller fixes.** Quick picks shows the next page's covers; Music and Radio shelves scroll to the
  screen edges; view counts read "11M views" and ages "7 days ago"; Library's actions are "From a
  link" and "New playlist", and empty Downloads offers "Browse music"; "Card details" and
  "visualisers" wording.

Merged with the tablet sidebar (#165): the sidebar lists the same destinations as the phone bar
(Library instead of Downloads; Movies only when turned on), its group is "Your activity" (History,
Playback queue, Sleep timer) to match the You sheet, and the toolbar's You button is left out on
tablets because the sidebar's foot holds account and settings.

Not done because the shared engine has no support yet: live search suggestions from YouTube,
like, subscribe and add to playlist, a channel page with Subscribe and tabs, and trending content
for signed-out Home.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| App and instrumentation compile | `gradlew :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` (JDK 21) | pass | build |
| Affected unit tests | `gradlew :app:testDebugUnitTest --tests OnboardingStepsTest --tests MediaDesignTest --tests Mp3SettingTest --tests NotificationPermissionTest` | 14 tests, 0 failures | unit |
| New unit tests | `SuggestionPhrasesTest`, `MusicTitlesTest`, `CardTextTest` | 9 tests, 0 failures | unit |
| Emulator walkthrough | Debug APK on an API 35 emulator (1080×2400 and 720×1600), signed out, fresh install: setup, Home, Music, a playlist, playback, player, search, Radio, Library, You | screens render and work as described; screenshots reviewed by an independent grader | emulator |
| Full unit suite and lint (CI's Android unit job) | `gradlew :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest :app:lintDebug :app:assembleDebug` after rebasing on #165 | 303 tests, 0 failures; lint 0 errors | unit |
| Instrumentation suites | not run locally (they run in the merge queue) | — | — |

## Not verified and limits
- Not run on a physical device or with a signed-in account. The instrumentation tests were
  updated for the new paths (search button, You sheet, Library tab, three-page setup, card menu
  labels, rounded feed cards) but not run locally.
- The playback service still stores the video's full title; only the display is cleaned.
- Signed-out Home's topic feeds rely on YouTube's search filters; if YouTube changes them, Home
  falls back to the "Start with a topic" page.
- The player's play disc stays white over the cover's colour, as in the reference apps; the
  accent is used for primary buttons elsewhere.
- Playback queue and Sleep timer are reachable from the You sheet as well as the player, so they
  work when nothing is playing.

## For the plan and parity tables
- Android UI: labelled dock with a Library destination; one-row header with search on demand and
  a You sheet; collection pages with headers; song rows and sideways song shelves; typed search
  filters; Play next; Undo after swipe-to-stop; three-page setup; notification permission asked
  at first playback or download (2026-11-15 UI/UX review).
