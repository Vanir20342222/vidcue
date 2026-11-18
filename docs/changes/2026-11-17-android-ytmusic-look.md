# Android UI: YouTube Music's look with less text, icon-only dock (M9)

- **Date:** 2026-11-17 (review loop until 2026-11-18 08:00 CET)
- **PR:** #173
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified (unit suite and lint) and rendered off-device (Robolectric) for
  review; not run on an emulator or device; ships the defaults of 11 open owner decisions

## Summary (written 2026-11-18, 07:47 CET; PR state added on opening #173)

- **Latest score:** 7.5 of 10 (round 20 of 20; 6.0 at round 1, 7.0 from round 4 to 19).
- **What changed:** floating icon-only dock; a colour wash from the covers at the top of Home,
  Music, Radio, search and Library; less text (icons for Show all and Play all, names on covers,
  no overlines, card views and dates off by default); white main buttons with pink for state; one
  look for loading, error and empty pages; Radio "Saved" chip and save buttons; a play button on
  the top search result; Library opening on three tiles; two layout bugs fixed (the playlist
  cover's height cap, a label beside the mini player at 320 dp).
- **Verified:** 321 unit tests pass (5 skipped), lint 0 errors, builds pass, instrumentation
  sources compile; 320 dp and de-DE renders checked; text counts within budget except Music and
  Radio.
- **Still open:** no emulator or device run; the instrumentation suites have not run, including the
  edited `CatalogUiTest.kt` and `NativeUiTest.kt` (compiled, unrun on a device). No specific review
  idea is in progress or half-done; none of the deferred ones was started.
- **Decisions that need the owner (default if no answer):** 1 join mini player and dock into one
  card (keep two pills); 2 drop the channel line on Home cards (keep it); 3 Song/Video switch in
  the player (no); 4 like button (Save only); 5 second Library shelf and Radio genre grid (no);
  6 card views and dates off by default (keep off); 7 white main buttons instead of logo pink (keep
  white); 8 Music and Radio over their text budgets (raise the budgets); 9 wash strength (as now);
  10 player cover position (as now); 11 Radio list heading (none, as now). Details in the table
  below.
- **State:** the overnight work stayed uncommitted, as the owner asked. On 2026-11-18 the owner
  asked to open a PR and see it merged ("create a pr and babysit it until its merged"), so #173
  ships every decision's default; each is one follow-up edit from the other choice.

## What it does
The owner chose (2026-11-17) an icon-only bottom bar (option "A + B, icons only" from the HTML
mockups: a floating dock, filled white icon for the selected tab, grey outlines, no indicator), and
asked for a review loop that brings YouTube Music's visual strengths into the app (the colour wash
at the top was the example) without its amount of text. The owner rejected "label only on the
selected tab". This note holds the acceptance checklist for that loop, the grades so far, the open
reviewer points and what changed.

## UI acceptance checklist

How to read it: each screen must pass in every listed state. "Rendered" means the off-device tour
(`ScreenTourTest`, 360×800 dp, xhdpi; also 320×640 dp and de-DE for the main screens) shows it and
an independent reviewer graded it; the number in brackets is the round-20 grade. "Not reviewed"
means no render or device run has looked at that state in this loop. Nothing here has run on a
device. Text budget = visible text runs on the first screen (each
separate label, title, caption or chip; icons, the status bar and the clock are not counted). The
reference YouTube Music screenshot has about 20.

Shared rules for every screen:
- Phone width 360 dp: no horizontal scroll of the page itself (only intended carousels scroll
  sideways); content on a 16 dp gutter (32 px at xhdpi); nothing clipped at 320 dp.
- English UI on a German device (de_DE): no German number formats inside English labels
  ("1.2K", "1 hr 1 min", "12 MB"), no truncated chips, provider text in German (YouTube titles,
  bylines) still cleaned (no "Titel •", "Aufrufe" left over in rows).
- Pink is for state only (playing title, active shuffle/repeat, switches); main actions are white.
- Floating mini player and dock: 16 dp side margins, page fades out behind them, no content text
  readable between or beside them.

| Screen | Signed out | Signed in | Empty | Loading | Error | Offline | Playing | Text budget |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Setup 1 Welcome | Rendered | n/a | n/a | n/a | n/a | Not reviewed | n/a | ≤ 6 |
| Setup 2 Look and feel | Rendered | n/a | n/a | Not reviewed ("Loading your settings…") | Not reviewed (settings not loaded) | n/a | n/a | ≤ 10 |
| Setup 3 Account | Rendered | Not reviewed ("Signed in" card) | n/a | n/a | Not reviewed (sign-in error line) | Not reviewed | n/a | ≤ 10 |
| Home (videos) | Rendered, topic feed "All" (7.5) | Not reviewed (For you, Subscriptions) | Not reviewed (no results: topics and sign-in) | Rendered, skeletons (8) | Not reviewed ("Sign in to see this") | Not reviewed | Rendered (mini player) | ≤ 14 |
| Music | Rendered, Jump back in, Quick picks, mixes (8.5) | Not reviewed | Not reviewed | Not reviewed | Rendered, "This page didn't load" (7) | Rendered, as the failed page with Downloads (7) | Rendered | ≤ 18 |
| Music "Show all" shelf grid | Not reviewed | Not reviewed | n/a | n/a | n/a | n/a | Not reviewed | ≤ 18 |
| Radio (Featured) | Rendered (7.5) | n/a | Rendered, "No saved stations" (8.5) | Not reviewed | Not reviewed | Not reviewed | Rendered | ≤ 16 |
| Search: suggestions | Not reviewed | Not reviewed | Not reviewed | n/a | n/a | Not reviewed | n/a | ≤ 14 |
| Search: results "All" | Rendered, music (8) | Not reviewed | Rendered, "No results" (7.5) | Not reviewed | Not reviewed | Not reviewed | Rendered (current song marked) | ≤ 22 |
| Search: filtered results | Not reviewed | Not reviewed | Not reviewed | Not reviewed | Not reviewed | Not reviewed | Not reviewed | ≤ 22 |
| Playlist / album page | Rendered, playlist (8) | Not reviewed | Not reviewed | Not reviewed | Not reviewed | Not reviewed | Rendered | ≤ 18 |
| Artist / channel page | Not reviewed | Not reviewed | Not reviewed | Not reviewed | Not reviewed | Not reviewed | Not reviewed | ≤ 18 |
| Full music player | Rendered, paused (7) | n/a | n/a | Not reviewed (track loading) | Not reviewed (error line) | Not reviewed (offline copy) | Rendered | ≤ 8 |
| Video watch page | Not reviewed | Not reviewed | n/a | Not reviewed | Not reviewed | Not reviewed | Not reviewed | ≤ 14 |
| Library overview (All) | Rendered, with history (6.5) | Not reviewed (6 tiles) | Rendered, "Nothing yet" (8) | n/a | n/a | Not reviewed | Rendered | ≤ 14 |
| Library: Downloads | Not reviewed | n/a | Not reviewed ("No downloads yet") | n/a | n/a | Not reviewed | Not reviewed | ≤ 14 |
| Library: Playlists, Saved, History | Not reviewed | Not reviewed | Not reviewed | Not reviewed | Not reviewed | Not reviewed | Not reviewed | ≤ 14 |
| Mini player + dock | Rendered | n/a | n/a (hidden when nothing plays) | n/a | n/a | n/a | Rendered (paused only) | ≤ 2 |
| Card menu, queue, lyrics, settings sheets | Not reviewed | Not reviewed | Not reviewed | Not reviewed | Not reviewed | Not reviewed | Not reviewed | — |
| Tablet (sidebar layout) | Out of scope for this phone loop (the dock is phone-only). Rendered once at 1280×800 dp landscape on 2026-11-18 for the owner, not graded; portrait not rendered | | | | | | | |
| All main screens at 320×640 dp | Rendered (7) | | | | | | | as above |
| All main screens on de-DE | Rendered (strings compared, not graded) | | | | | | | as above |

### Open decisions — need the owner
Each of these was asked for by most reviewers, but changes a choice the owner made or adds a
feature (rows 1–8), or reviewers asked for opposite things (rows 9–11), so none was decided
overnight. **Needs owner** on every line. "Default" is what #173 ships if the owner does not
answer; every default is one follow-up edit away from the other choice.

| # | Owner picks between | Asked in | Default if no answer | Status |
| --- | --- | --- | --- | --- |
| 1 | **Bottom bar:** (a) keep the mini player and the icon-only dock as two floating pills, or (b) join them into one floating card (mini player on top, a hairline, the dock icons below; ~10 dp more content on screen) | 9 rounds | (a) keep two pills: it is the owner's pick from the mockups | **Needs owner** |
| 2 | **Home video cards:** (a) keep the channel line under the title, or (b) drop it and let the channel avatar stand for the channel (3 → 2 lines per card) | 8 rounds | (a) keep the channel line: dropping it removes the channel name as text | **Needs owner** |
| 3 | **Song/Video switch** in the music player (icons only): (a) no switch, or (b) build it (switch a music video between audio and video in place) | 10 rounds | (a) no switch: a new feature is not started without the owner | **Needs owner** |
| 4 | **Like button** in the player: (a) keep Save (⊕) only, or (b) add like, which needs a signed-in YouTube "like" action in the engine | 10 rounds | (a) Save only: needs engine work and an account | **Needs owner** |
| 5 | **More shelves:** (a) leave Library and Radio as they are, or (b) add a second shelf on Library (e.g. downloads) and a genre grid on Radio | 4 rounds | (a) as they are: new content sources | **Needs owner** |
| 6 | **Card details:** (a) keep view counts and upload dates off by default on cards (done overnight; the player still shows both; a setting the user chose is kept), or (b) back to on by default | 9 rounds | (a) off: it is what this PR ships, and it cuts a text line per card | **Needs owner** to confirm or revert |
| 7 | **Main button colour:** (a) white pills, pink only for state (done overnight), or (b) back to logo-pink main buttons (the owner chose the logo pink as the accent on 2026-11-16) | 4 rounds | (a) white: it is what this PR ships; reverting is one line (`Action` back to `PillStyle.Primary`) | **Needs owner** to confirm or revert |
| 8 | **Text budgets:** Music has 24 text runs (budget 18) and Radio 19 (budget 16), all song, artist, station and genre names. (a) raise the two budgets to 24 and 20, or (b) cut Quick picks to three rows and drop Radio's genre lines | final check | (a) raise the budgets: no content is cut without the owner | **Needs owner** |
| 9 | **Wash strength** (reviewers disagreed): (a) as now, two covers' colours at moderate strength, or (b) stronger and more saturated, or (c) weaker | rounds 8–18, both ways | (a) as now | **Needs owner** (best judged on a device with real covers) |
| 10 | **Player cover position** (reviewers disagreed): (a) as now, centred between the header and the controls, or (b) higher, with the title close under it | rounds 16–19, both ways | (a) as now | **Needs owner** (best judged on a device) |
| 11 | **Radio list:** (a) as now, no heading and a 24 dp gap under the featured covers, or (b) a short heading ("Stations") above the list | rounds 1–19, both ways | (a) as now | **Needs owner** |

## Grades so far

Independent reviewer: a fresh Opus 5.5 subagent at high effort per round, given only the
screenshots and the reference YouTube Music screenshot, never the code or my reasoning.

This loop (goal: YouTube Music's richness and strengths, clearly less text):

| Round | Renders | Overall | Notes from the reviewer |
| --- | --- | --- | --- |
| 1 | y2 | 6.0 | wash too weak, no scrim under the bottom chrome, Library and Home text-heavy |
| 2 | y3 | 6.5 | polish ≈7, richness ≈6, less text ≈7; Home still a YouTube feed, Library sparse |
| 3 | y4 | 6.0 | "Jump back in" text tiles and grey cards muddy on the wash |
| 4 | y5 | 7.0 | Music lacks a large-art shelf above the fold; muddy two-hue wash; flat Library tiles |
| 5 | y6 | 6.5 | wants a multi-colour wash (round 4 wanted one hue); bottom stack heavy; Library empty below |
| 6 | y7 | 7.0 | wash not tied to the first cover; Jump back in tiles unlabelled; Library empty lower third |
| 7 | y8 | 7.0 | wash reads as a theme tint (same green on three tabs); player background dull; Radio rows and Library tiles utility-like |
| 8 | y9 (+ states, 320 dp) | 7.0 | first round with per-screen scores (below); search clipping, error state, pink vs white buttons |
| 9 | y10 | 7.0 | search 5.5 → 7.5 after the render fix; states flat black; Library empty a dead end |
| 10 | y11 | 7.0 | states much better (error 5 → 7, empty search 6 → 7.5); wash flat; bottom fade dims rows |
| 11 | y12 | 7.0 | playlist page and Music 8; pink buttons on states off-system; wash a single hue |
| 12 | y13 | 7.0 | Radio cards touched the list (gap regression, fixed in 13); carousel peek looked like overflow |
| 13 | y14 | 7.0 | Library tile style differed between empty and filled; sign-in pill loud |
| 14 | y15 | 7.0 | Music 8.5, empty My stations 8; Home the text-heavy outlier; search top result has no action |
| 15 | y16 | 7.2 | translucent pills, "Browse" button, "All" selected while loading; stacked pills, Home text, player switch remain |
| 16 | y17 | 7.0 | Radio save buttons, account dot (read as an unread alert: removed in 17), Library book icon |
| 17 | y18 | 7.0 | a label showed beside the mini player at 320 dp (fixed in 18); fade read as a band |
| 18 | y19 | 7.0 | playlist cover at 320 dp still 230 dp: its height cap never applied (fixed in 19) |
| 19 | y20 | 7.0 | top result gets a play button; player rhythm and wash depth remain |
| 20 | y21 | 7.5 | empty and error messages at one height; Music 8.5, saved-stations empty 8.5, playlist 8, search 8 |

| Screen | R8 | R10 | R12 | R14 | R15 | R16 | R17 | R18 | R19 | R20 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Home | 7 | 7 | 7 | 6.5 | 6.5 | 7.5 | 7.5 | 6 | 7 | 7.5 |
| Music | 8 | 7.5 | 7.5 | 8.5 | 8 | 8 | 8 | 8 | 7.5 | 8.5 |
| Playlist | 7.5 | 7.5 | 7.5 | 7.5 | 7.5 | 8 | 8 | 7 | 7.5 | 8 |
| Player | 6 | 7 | 7 | 7.5 | 7.5 | 7.5 | 7 | 7.5 | 6.5 | 7 |
| Search results | 5.5 | 7 | 7 | 7.5 | 7 | 7 | 7 | 7 | 7.5 | 8 |
| Radio | 6.5 | 6.5 | 6 | 7 | 7 | 7 | 7.5 | 7.5 | 7 | 7.5 |
| Library | 7 | 6.5 | 6.5 | 7 | 7.5 | 7 | 7.5 | 6.5 | 6.5 | 6.5 |
| Loading | 7.5 | 6.5 | 8 | 7.5 | 8 | 8 | 8 | 8 | 7 | 8 |
| Error (offline) | 4.5 | 7 | 6.5 | 7 | 7 | 6.5 | 6.5 | 6.5 | 7 | 7 |
| Empty saved stations | 7 | 7.5 | 7.5 | 8 | 7 | 7 | 8 | 7 | 7 | 8.5 |
| Empty search | 4.5 | 7.5 | 7.5 | 6.5 | 6.5 | 7 | 7 | 7.5 | 6.5 | 7.5 |
| Empty Library | 6.5 | 5.5 | 6.5 | 7.5 | 7.5 | 7 | 8 | 7.5 | 7 | 8 |
| 320 dp set | 7 | 7 | 6.5 | 7 | 7 | 7 | 7 | 7 | 7 | 7 |

Earlier per-screen grades exist only from the previous loop's design (round 10 of #171's loop, before the
dock and the wash; kept for reference): Welcome 7.0, Look 7.5, Account 7.5, Home 7.5, Music 7.0,
Playlist 8.0, Player 7.5, Search 7.0, Radio 7.0, Library 6.0. From round 8 of this loop on, the
reviewer is also asked for per-screen scores.

## Open reviewer points

Marked **specific** when the point prescribes one layout, colour, row count or piece of copy. Per
the owner's overnight rule those are not executed before the general checks below, and not at all
when two rounds asked for opposite things (marked **conflicting**).

General (not tied to one prescription):
- Text count per screen against the budgets above (rounds 1–6 all mention text on Home and Library).
- Phone width and gutters: no horizontal page scroll, 16 dp gutters, nothing clipped at 320 dp.
- German locale: the English UI must keep English number formats and clean German provider text.
- States other than "signed out with content": empty, loading, error, offline have not been looked
  at in this loop.

Done overnight after the general checks (each asked for in several rounds, none contradicted):
Radio "My stations" → "Saved" and a save button on station rows; top search result with a play
button and no "Top result" heading; card dates off by default (as views); white main buttons
(pink for state only); one quiet fallback wash on loading, error and empty pages; Library without
a chip row on its overview, without a sign-in banner (sign-in stays in the account button) and
with the same three tiles empty or not; empty states with an icon that fits ("Offline", search),
shorter copy and one height on every page; Radio and Music hero covers the same size with the name
on the art.

**Owner decisions:** see "Open decisions — need the owner" under the checklist; every one of
them needs the owner and none was executed beyond the two reversible defaults it names.

**Conflicting** (reviewers asked for opposite things; left as is):
- Wash: one hue (round 4), several covers' colours (rounds 5–12, done in 11), weaker (round 8,
  16), stronger (rounds 13, 18).
- Player cover: higher with the controls under it (rounds 16, 19) vs centred (round 17) vs controls
  pinned low (round 1).
- Radio gap and heading above the list; Quick picks column width; mini player outline; bottom
  fade length; whether pink equaliser bars show while paused.

## Overnight general checks (2026-11-17/18)

Run before any reviewer-specific change, as the owner asked. The tour now also writes each
screen's visible text (the merged semantics tree, as TalkBack reads it) to `<screen>.txt`.

**Text count against the budgets** (360 dp, signed out, sample history; the mini player's two
lines and one-time hints are counted separately; rows partly visible at an edge or under the
dock count too, so the numbers err high):

| Screen | Count | Budget | Result |
| --- | --- | --- | --- |
| Home | 14 | ≤ 14 | pass (at the limit: two video cards carry 3 lines each) |
| Music | 24 (+ 3 names drawn on covers) | ≤ 18 | **over**: Quick picks lists 6 songs × 2 lines including the peeking column |
| Radio | 18 | ≤ 16 | **over**: a genre line under every station |
| Library overview | 14 | ≤ 14 | pass |
| Search results "All" | 19 | ≤ 22 | pass |
| Playlist page | 15 | ≤ 18 | pass (the title appears twice: page and toolbar) |
| Full player | 5 | ≤ 8 | pass |
| Loading (Home) | 6 | — | chrome only |
| Error (Music, as when offline) | 5 | — | pass |
| Empty: My stations, search, Library | 7 each | — | pass |

**Phone width (320 dp × 640 dp render):** no horizontal page scroll on any screen; content stays
on the 16 dp gutter. Found and fixed: the Library tile label "Downloads" truncated ("Downloa…"),
and the playlist cover (fixed 256 dp) left almost no room for songs; it now scales with the width.

**German device (de-DE render):** every visible string matches the English render exactly
(numbers "1 hr 1 min", "7:00", "1 week ago" stay English). The sample content has no German
provider text, so cleaning of German bylines from YouTube is not covered by this check.

**States (new `stateScreens` render):** loading, failed page (as when offline), empty My stations,
empty search and an empty Library were rendered for the first time. Found and fixed: the Home
loading skeleton ran edge to edge while the cards it stands in for sit on the 16 dp gutter.

**Render artifact, not an app bug:** search results (and earlier, Music and Radio on their first
visit) were captured with their first item under the header. Logging showed the page never
recomposed after its header grew: Robolectric lays a window out only when the tour draws it, so a
header that grew during that draw changed state too late for the same capture. The tour now
draws once, lets the frame settle, then captures; search shows its "Top result" heading and the
empty-search icon whole. The app keeps one related change: a new page starts from the last
header height instead of zero, which avoids a one-frame jump on a device. Not checked on a device.

**Error state:** a page that failed (as when offline) no longer shows "Jump back in" or the resume
card above "This page didn't load".

## Final checks (2026-11-18, 04:45–04:55 CET)

Same renders as above, on the code as it stands (renders in `shots/final`, not graded again).

| Check | Result |
| --- | --- |
| Text count, 360 dp (mini player's 2 lines and the one-time hint not counted) | Home 14/14, Music **24/18**, Radio **19/16**, Library 11/14, search 18/22, playlist 15/18, player 5/8; states 1–6 each |
| Phone width, 320×640 dp | no page scrolls sideways; content on the 16 dp gutter; Library tiles, the playlist cover (192 dp) and the search top result fit; only the mini player's artist ellipsizes ("The Quiet Harbo…"), as intended |
| German device (de-DE) | every visible string on all 9 captured screens is identical to the English render |
| Unit suite, lint, builds | 321 tests, 0 failures, 5 skipped; lint 0 errors; debug APK and instrumentation test sources build (run finished 04:23; no source change since) |

## Changes
Rounds 1–7 of this loop, on branch `ui/ytmusic-loop` from `f920f73`:
- Dock: floating, icons only, 16 dp gutters, mini player's 16 dp corners; filled white icon for the
  selected tab, quieter grey outlines; Library's filled icon redrawn so it differs from the outline.
- Colour wash across the top of Home, Music, Radio, search results and the Library overview, from
  the first cover on screen (one hue and a neighbour hue, saturation and lightness clamped so a
  muddy cover cannot turn the top olive); it fades as the page scrolls; the header draws the same
  wash so there is no seam.
- A new page starts from the last header height instead of zero (no first-frame jump).
- A failed page shows only its error (no shortcuts above it); the Home loading skeleton sits on
  the 16 dp gutter; Library tiles fit "Downloads" at 320 dp; the playlist cover scales with width.
- Chips, search field, Library tiles and sign-in card are translucent so the wash shows through.
- Section actions are icons (› for Show all, round ▶ for Play all) instead of words.
- Music: "Jump back in" is a row of 148 dp covers with the name on the art; shelf covers 176 dp
  with a small play mark and the title only; Quick picks without ⋮ (press and hold opens the menu).
- Radio: no "LIVE" text, station tiles 176 dp with the name only, rows show the genre only, no ⋮.
- Playlist page: centred title, one subtitle line (creator · length), white play button, 256 dp cover.
- Player: no "PLAYING FROM" overline (the source name is a quiet label), cover colour kept behind
  the whole player, speech-bubble Lyrics icon, evenly spaced bottom icons.
- Search: top result tinted from its picture, no "Artist" under a round picture.
- Library overview: no chip row (sections are tiles; chips with "All" lead back from a section),
  tiles with icon and name only, "Recently played" as two rows of covers.
- View counts and upload dates are off by default on cards (`show_views`, `show_upload_date`
  default false; an explicit setting is kept; the player still shows both).

Rounds 8–15 (overnight):
- Main buttons (`Action`) are white pills (`PillStyle.Inverse`); pink stays for playing state,
  active modes and switches. Neutral pills (`PillStyle.Tonal`) and empty-state icon circles are
  translucent.
- Pages without covers (loading, failed, empty) use one quiet wash from the accent; the wash
  fades between colours (snaps under reduced motion) and is built from two covers.
- Search: no "Top result" heading and a smaller top-result card; no filter chips when "All"
  found nothing.
- Radio: "Saved" chip (was "My stations").
- Library: three tiles (icon over name), no History tile (Recently played's › opens it), "Nothing
  yet" empty state with "Find music"; Recently played as two rows.
- Empty and error states: "Offline" icon, no "You can still search…" line, a Downloads pill on a
  failed page, "No saved stations" with "Browse".
- Lyrics icon: quote marks in a bubble.
- Render harness: sample listening history, cover sizes preloaded, 320 dp and de-DE variants, a
  states tour (loading, error, empty), per-screen visible-text files, and a layout pass before
  each capture.

Rounds 16–20 (overnight):
- Round 16: a quiet save button (⊕) at the end of each station row; Library without its sign-in
  banner (sign-in stays in the account button); Library dock icon drawn as books instead of "||\".
- Round 17: the account-button dot from round 16 removed (it read as an unread alert); station save
  buttons grey and a size down; Music and Radio hero covers sized so two whole covers and a slice of
  the third show on any width; 8 dp between the mini player and the dock; the empty Library uses
  the shared empty state.
- Round 18: the fade above the bottom bars eased and solid by the mini player (a row's label no
  longer shows beside it at 320 dp); "and more" dropped from the playlist subtitle; a Downloads
  pill with an icon on a failed page; wider fades at the ends of chip rows; save buttons at 75 %.
- Round 19: the playlist cover is min(72 % of the width, 256 dp, 30 % of the height); the old
  width cap never applied (the cover was 230 dp at 320 dp); the top search result (an artist) has
  a round play button that plays the songs found.
- Round 20: empty and error messages start at the same height on every page, search included.

Test files changed:
- `apps/android/app/src/test/java/dev/ytui/android/ScreenTourTest.kt` (JVM, Robolectric): the render
  harness: 320 dp and de-DE variants, `stateScreens`, visible-text files, sample history, cover
  preloading and a layout pass before each capture; on 2026-11-18 also a landscape tablet
  (1280×800 dp) and a portrait tablet (800×1280 dp) tour. It runs only with `VIDCUE_SCREENS` and
  `-Pscreens`; the normal suite skips its seven tests.
- `apps/android/app/src/androidTest/java/dev/ytui/android/CatalogUiTest.kt` (instrumentation):
  the multi-select test (`library-entry-Downloads` instead of the `library-Offline` chip) and
  `offlinePlaylistsUseEditableNativeFieldsAndRealRustPersistence` (`library-entry-Playlists`
  instead of the `library-Playlists` chip) open their section from the Library overview's tile,
  because the overview no longer has a chip row.
- `apps/android/app/src/androidTest/java/dev/ytui/android/NativeUiTest.kt` (instrumentation): the
  wide-layout chip-height check opens Downloads by its tile before measuring the Library chips.
- **The two instrumentation files compile but have not run on an emulator or device.** Their
  assertions are unchanged; only the step that reaches the section moved.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Screen renders | `VIDCUE_SCREENS=… gradlew :app:testDebugUnitTest -Pscreens --tests '*ScreenTourTest.<phoneScreens|phoneScreensNarrow|phoneScreensGerman|stateScreens>'` (one test per invocation) | renders y2–y21 | unit |
| Tablet render | the same with `*ScreenTourTest.tabletScreens` (1280×800 dp landscape), 2026-11-18 | 8 screens render with the sidebar and the "Now playing" panel; not graded. `tabletScreensPortrait` was stopped before it captured | unit |
| Independent review | 20 rounds, fresh reviewer each | 6.0 → 7.0 (rounds 6–19) → 7.5 (round 20) | review |
| Unit suite, lint, builds | `gradlew :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest :app:lintDebug :app:assembleDebug`, last run 2026-11-18, finished 04:23 CET; no source file changed after it | 321 tests, 0 failures (5 skipped: the render tours without their variable); lint 0 errors; debug APK builds | unit |
| Before the PR | `gradlew :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest :app:lintDebug`, 2026-11-18 16:01 CET, after the tablet tours were added | 323 tests, 0 failures (7 skipped: the render tours); lint 0 errors | unit |
| Regression caught | `WatchDetailsTest.playerAlwaysIncludesAvailableCountsAndLength` failed after the card defaults changed | fixed: the player asks for views and dates itself | unit |
| Instrumentation tests vs. tonight's UI | static scan of `androidTest` for tags and copy changed tonight; then `gradlew :app:compileDebugAndroidTestKotlin` (05:14) | 3 steps reached Library chips that the overview no longer shows: `CatalogUiTest` (selection test, offline playlists test) and `NativeUiTest` (chip heights) now open Downloads or Playlists by their overview tile first; compiles. Not run | compile |

## Not verified and limits
- Nothing in this note has run on an emulator or device; only the "Rendered" cells above were seen.
- Instrumentation suites not run (no free emulator overnight; emulator-5554 belonged to another
  session). Pull-request CI compiles them; the emulator suite runs nightly. The edited `CatalogUiTest.kt` and `NativeUiTest.kt` compile but are **unrun on a
  device**; other tests may still meet moved UI on a device run (the scan found no others).

- Tablets were outside this loop: the landscape render was looked at once and not graded; the
  sidebar still has text labels. The portrait tablet tour has not rendered.
- Reviewer grades are one model's opinion of renders with synthetic covers; the wash and the
  player layout are best judged on a device with real artwork (decisions 9 and 10).

## For the plan and parity tables
- Android UI: icon-only floating bottom dock; a colour wash from the covers at the top of Home,
  Music, Radio, search and Library; one style for loading, error and empty pages; saved radio
  stations with a save button per row; Library opens on three tiles with Recently played; card
  view counts and dates off by default (2026-11-18).
