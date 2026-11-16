# Android UI and UX suggestions (2026-11-15)

A review of the Android app against Spotify, YouTube Music, YouTube and Netflix. Nothing in this
file is implemented. It is a list of proposed changes for the owner to accept, reject or reorder.

## What this is based on

- The Play Store screenshots of the four reference apps.
- The Kotlin UI code on `main` at `051828d`.
- The repository's own Android screenshots from 2026-11-08 (`docs/screenshots/android/2026-11-08/`).
- One live capture of onboarding and Home on an API 35 emulator.

The player, sheets, Radio, Movies and Downloads were **not** captured on the current build. Findings
about them come from the code and the 2026-11-08 screenshots. Items marked *(confirm on device)*
were seen once or inferred and need a second look before work starts.

Items marked *(reverses a decision)* undo something an earlier change note chose on purpose.

Terms used here: the **dock** is the bottom navigation, the **mini player** is the card above it,
the **player** is the full-screen player, and the **card menu** is the sheet opened by ⋮ on a card.

## The ten changes that matter most

1. Give playlists, albums and channels a real page header (section 3, item 1).
2. Show songs as compact rows, not large squares (section 2, item 1).
3. Add one Library destination (section 1, item 1).
4. Shrink the page header to one row (section 1, item 3).
5. Label the dock (section 1, item 2).
6. Cut onboarding to welcome, theme and sign-in (section 8, item 1).
7. Make swipe-down on the mini player undoable (section 5, item 1).
8. Use one word each for bookmarking and for downloading (section 10, item 1).
9. Add search suggestions while typing (section 4, item 1).
10. Label the player's action row and add like and save (section 6, items 1 and 2).

## 1. Navigation and structure

1. **There is no Library.** A person's own things are in four places. "My playlists" and "Liked"
   are filter chips on Home and Music. "Recently played" is in the ⋮ menu. Bookmarks are under
   ⋮ → Browse → "Saved locally". Offline files are in the Downloads tab. All four reference apps
   have one place for this (Your Library, Library, You, My Netflix). Turn the Downloads tab into
   **Library** with Downloads, Playlists, Liked, Saved and History inside it, and drop the
   library chips from Home and Music.
2. **The dock has icons only** *(reverses a decision, 2026-11-03)*. `AppNavigation` in `Shell.kt`
   draws five glyphs with no text. All four reference apps label every tab. The radio, film and
   download glyphs are not obvious on first use. Add a short label under each icon.
3. **The header is about 190dp tall before any content.** It stacks a title row with three icons,
   a full-width search field, and the filter chips. The search field and chips slide away on
   scroll, but the first screen shows two video cards at most. The reference apps use one row
   plus chips. Make search an icon in the title row that opens a full search page.
4. **The title row has three icons with unclear roles.** Account, Settings and ⋮ sit side by side.
   The ⋮ menu mixes page actions (Browse, Refresh feed), player actions (Playback queue, Lyrics,
   Sleep timer), history and About. Lyrics and Sleep timer appear even when nothing is playing.
   Keep one avatar button that opens account, settings, history and About. Move queue, lyrics
   and sleep timer to the player only. Refresh is already pull-to-refresh.
5. **"Browse" duplicates the chips.** The Browse sheet lists Home, Subscriptions, Liked, Watch
   later and My playlists, which are also chips. With a Library tab it can go.
6. **Movies is a fifth of the dock but is an alpha test page.** It shows "Movies (alpha)",
   "Test streams" and "Play a stream URL". Either hide the tab behind a setting until it has a
   real catalogue, or move it into Home as a shelf.
7. **Swiping sideways changes tabs.** Home and Music are full of rows that also scroll sideways.
   The code lets rows keep their own drags, but a swipe that starts between rows, or at the end
   of a row, changes the tab. None of the reference apps swipe between bottom tabs. Consider
   turning this off by default *(confirm on device how often it misfires)*.

## 2. Home, Music and Radio

1. **Songs are large square cards.** On phones the Music tab is a fixed two-column grid
   (`ComposeHost.kt:1309`) and shelves use 152dp cards, so about four songs fit a screen.
   Spotify and YouTube Music show songs as rows (small cover, title, artist, ⋮) and keep squares
   for albums and playlists. Use rows for `kind == "song"` and squares for collections.
2. **Every shelf has the same shape.** Each one is a row of identical cards. Spotify opens with a
   two-column grid of compact recent items. YouTube Music uses a row list for "Quick picks".
   Vary the first shelves: compact recent tiles first, then a row list, then cover rows.
3. **Home has no featured item.** Movies has a hero; Home and Music start straight into a list.
   A single large "continue watching" or top recommendation would give the page a focal point.
4. **Signed-out Home appeared empty** *(confirm on device)*. After skipping onboarding, Home showed
   the "No videos loaded" empty state. None of the reference apps open on an empty page. Show
   trending or popular content when there is no account.
5. **Library chips show when signed out.** "My playlists", "Liked", "Subscriptions" and "Watch
   later" are always listed (`ComposeHost.kt:1641`). Without an account they lead to an
   "Account needed" error. Hide them, or replace them with one "Sign in" chip.
6. **"Play all" on a shelf is an outlined circle with a play triangle.** It reads as "play the
   first item". The reference apps use a text button ("Play all", "More"). Use a small labelled
   pill.
7. **Radio opens on "Cliamp channels".** The default Radio feed and its Browse entry use the name
   of the upstream project. Call it "Stations" or "Featured stations".
8. **Video cards sit 28dp apart with square corners and edge-to-edge art.** This matches YouTube
   and should stay. Music uses 20dp and rounded covers. The two rhythms are fine, but the Home
   shelves switch to rounded 272dp cards, so one page mixes both. Pick one treatment per page.

## 3. Playlist, album and channel pages

1. **These pages have no header.** Opening a playlist, album or channel swaps the list of cards
   and adds a "Back to previous feed" pill as the first row (`ComposeHost.kt:1361`). The page
   title still says "Home" or "Music". `FeedState.title` exists but is not shown. Every reference
   app gives these pages a cover, the name, the owner, a count, and Play, Shuffle and Download
   buttons. This is the largest gap in the app. Add that header, put a back arrow in the title
   row, and remove the pill.
2. **Playing a whole playlist is hidden.** "Play all" for a collection is a row in the card menu.
   Once the header exists it belongs there, next to Shuffle.
3. **Album tracks are cards, not numbered rows.** After item 1 of section 2, album and playlist
   pages should use the same song rows, with a track number or small cover.
4. **A channel page has no Subscribe and no tabs.** It lists uploads only. See section 6, item 2.

## 4. Search

1. **There are no suggestions while typing.** `SearchSuggestions` shows recent searches and a
   "Paste from clipboard" row, and only while the field is empty. All four reference apps
   complete the query as you type. Add live suggestions under the field.
2. **The field has a "→" button on its right.** It duplicates the keyboard's search key and looks
   disabled when the field is empty. Remove it. Show a clear (×) button only when there is text.
3. **Results cannot be narrowed by type** *(confirm on device)*. I found no Videos, Channels,
   Playlists or Songs filter for results. YouTube and YouTube Music both offer one.
4. **Results use the same large cards as Home.** A results list is scanned, not browsed. Use a
   compact row (thumbnail left, text right) on phones, as the tablet layout already does with
   `WideMediaRow`.
5. **Search and "paste a link" share one field.** The placeholder "Search or paste a link" asks
   the field to do two jobs. Keep link detection, but shorten the placeholder to "Search". The
   clipboard row already covers pasting.
6. **Radio search is a separate field with separate history.** That is reasonable, but the
   placeholder "Search stations or genres" should match whatever the Home field says.

## 5. Mini player

1. **Swiping down stops playback and clears the player.** There is no undo (`Shell.kt:387`). A
   downward swipe is an easy accident, and on other apps it does nothing or only hides the bar.
   Show a short "Playback stopped · Undo" notice, or require a longer drag.
2. **The right-hand button changes meaning.** For music it is Next. For video it is Close, which
   also stops playback (`Shell.kt:468`). Keep Next in both cases. Closing can stay a swipe.
3. **The progress line is clipped by the card's corners.** It runs along the bottom edge of a
   card with 24dp corners, so the first and last few percent are hidden. Inset the line by the
   corner radius, or move it just above the bottom edge.
4. **The first-use hint is a line of text.** "Swipe sideways to skip · swipe down to close"
   appears above the mini player and the card nudges sideways. It is easy to miss and pushes the
   dock content. Fine to keep, but it should not be the only way to learn about stop-on-swipe.
5. **The dock and mini player together cover about 150dp.** With the header, about 43% of an
   800dp screen is chrome while something is playing and the page is at the top. Reducing the
   header (section 1, item 3) fixes most of this.

## 6. Player (music and video)

1. **The action row is up to seven icons with no labels.** Queue, Lyrics, Save, Output, Share, Sleep and
   More sit in one tray (`PlayerAction` in `FocusedPlayer.kt`). YouTube uses labelled pills.
   YouTube Music uses labelled "Up next / Lyrics / Related" tabs. Spotify shows two or three
   icons. Keep three or four with labels (Queue, Lyrics, Download, Share) and move Output and
   Sleep timer into More.
2. **A signed-in person cannot like, subscribe or add to a playlist.** I found no such action
   anywhere in the UI. These are the main things people do with an account on the reference apps.
   This needs engine work, so it is a feature request more than a UI fix, but the player and the
   card menu should leave room for it.
3. **The download glyph is called "Save".** In the player, "Save" means download. In the card
   menu, "Download" means download and "Bookmark" produces "Saved locally". See section 10.
4. **Double-tapping the cover pauses.** A single tap does nothing. On Spotify and YouTube Music a
   single tap on the cover does nothing either, but pause is always on the transport, so the
   double-tap is redundant and undiscoverable. Low priority; consider removing it.
5. **"Stop playback" is offered in three places.** It is in the playback options sheet, in the
   compact player panel, and as the mini player's swipe-down. Reference apps have no Stop at
   all: pause is enough, and the mini player goes away when the queue ends. Keep one.
6. **The empty queue text points at a symbol.** "Nothing queued. Add videos with Add to queue in
   any ⋮ menu." Say "Add videos from the menu on any card" and offer a button that closes the
   player.
7. **Up next rows have no menu.** A row can be played but not removed, queued next or downloaded
   without opening "Edit queue". Add ⋮ or swipe-to-remove on each row.
8. **The compact player panel shows seek jumps as the text "−10" and "+10".** Icons with the
   number inside are the norm and are faster to recognise. Minor.

## 7. Cards and the card menu

1. **Long-pressing a card does nothing.** Only ⋮ opens the menu. Spotify and YouTube Music open the
   same menu on long press. Add it; the ⋮ button can stay.
2. **Every card has a 48dp ⋮ button.** With long press available, song rows and video cards can
   keep ⋮, but small shelf cards could drop it to give titles more room.
3. **The card menu's four tiles are Play, Add to queue, Download and Preview.** "Preview" (a muted
   preview) is rare enough to be a row. "Play next" is missing and is more common than Preview.
4. **"Bookmark" has a long subtitle.** "Keep a link in Saved locally, without downloading" explains
   a naming problem. Fix the names (section 10) and the subtitle can go.
5. **A song card shows no duration and a video card does.** That is fine for squares, but once
   songs are rows they should show duration at the right edge, as the reference apps do.
6. **The playing card turns its title to the accent colour and adds bars.** Good. Keep it.

## 8. Onboarding and first run

1. **Setup has five steps and about ten settings.** Welcome, Look and feel, Playback, Account and
   a summary. The Playback step asks about background playback, autoplay, preloading, swipe
   gestures, auto colour grade, colour grading and the equalizer (`Onboarding.kt:713`). A new
   user cannot judge most of these before using the app, and the defaults are sensible. Keep
   welcome, theme and optional sign-in. Drop the Playback step and the summary.
2. **The notification prompt appears on the first view of Home.** `RustActivity.java:69` asks at
   launch once setup is done. Nothing has played yet, so there is no reason to say yes. Ask the
   first time something plays in the background or a download starts.
3. **The welcome screen is a play glyph in rings.** It says nothing about what the app shows. A
   strip of real artwork, or the app mark from the launcher, would be warmer. Minor.
4. **"Skip" skips everything, including sign-in.** That is right, but Home should then make
   signing in easy to find (section 2, items 4 and 5).

## 9. Settings and account

1. **Developer items sit with user settings.** "Performance: Startup timing report", "Streaming
   cache (MiB)" and "Preload on mobile data" are in the main list. Move diagnostic items under
   an "Advanced" page.
2. **The account page leads with technical import options.** "Choose cookie file · Netscape
   cookie format" and "Cookie header (optional)" are shown under the sign-in button. Keep them,
   but fold them under "Other ways to sign in".
3. **Sign-in carries a warning.** "Google may reject embedded sign-in. You can also import your
   session below." Show this only after a sign-in attempt fails.
4. **The gestures page has nine switches.** A master switch plus eight. Most people will never
   open it. Group them into three (page gestures, player gestures, video gestures) with the
   detail one level down.
5. **Motion has style, speed, animated cards and haptics.** Four animation controls is unusual.
   Reduce motion and haptics are enough for most people; move style and speed to Advanced.
6. **About reads like a build note.** It says "All provider, extraction, account and library
   operations use the shared Rust engine" and mentions "the validation report". Say what the app
   is, show the version, and link to the documentation.
7. **The Settings icon is a pair of sliders.** A gear is what every reference app uses. If the
   avatar button in section 1, item 4 is adopted, this icon goes away from the title row anyway.

## 10. Wording

1. **Two actions share five words.** Bookmarking is called "Bookmark", "Saved locally", "Saved
   stations" and "Favorites". Downloading is called "Download", "Save", "Save offline" and
   "Offline", and the tab is "Downloads" with a first section called "Offline". Use **Save** for
   bookmarking only and **Download** for files only, everywhere.
2. **Some messages are engine messages.** Examples: "Enter a valid HTTP(S) video or playlist URL",
   "No title metadata returned", "Empty resolution", "No provider served the title." Rewrite
   them as what happened and what to do, for example "That link isn't a video or playlist."
3. **"Back to previous feed", "Refresh feed", "Feeds and library".** "Feed" is the app's internal
   word. People think in pages, playlists and channels. A back arrow needs no label.
4. **Empty states are named after loading.** "No videos loaded", "No music loaded", "No stations
   loaded". Say what the person can do: "Nothing here yet" with the existing action button.
5. **"Recently played" and "History & resume" are the same thing.** Use "History" in both places.

## 11. Visual details

1. **There are ten corner radii.** 6, 8, 10, 12, 14, 16, 18, 24, 28dp and the pill, plus an 18dp
   radius hard-coded for the dock highlight (`Shell.kt:259`). Four or five would look the same and
   be easier to keep consistent: 8 (badges, small art), 12 (covers, chips), 16 (cards, fields),
   28 (sheets, dialogs) and the pill.
2. **The primary button is white, and the accent is barely used.** The play disc, selected chip
   and "Get started" are all white on dark. The accent appears on switches and the playing
   title. This is the "screening room" idea and it works, but it means the app has no colour of
   its own in a screenshot. Consider using the accent for the one main action per screen
   *(reverses a decision)*.
3. **Chips are 36dp tall inside a 48dp touch area with 12dp corners.** Fine. The selected chip is
   solid white, which is louder than anything else on the page, including the artwork. A tinted
   fill with white text would sit back more.
4. **The page title is 28sp bold and names the tab.** With labels in the dock it repeats them.
   After section 1, the title row could carry the app mark on Home, as YouTube, YouTube Music and
   Netflix do, and the page name elsewhere.
5. **Secondary text is 13sp.** Card channel names and view counts use `bodySmall`. It is readable
   but small against the 15sp title. 13.5 or 14sp would help scanning without changing the
   layout much.
6. **The duration badge and the Live badge use different shapes of emphasis.** Duration is a dark
   chip, Live is red. Good. Keep both at one size and one corner radius (they are, at 6dp).
7. **Loading uses skeleton cards, then a spinner for more.** Good. Keep it.

## 12. What already holds up

- The type pairing and scale.
- The mini player that takes its colour from the cover.
- The player background that takes its colour from the cover.
- Edge-to-edge video cards on Home with channel avatar, title and metadata.
- Soft press feedback and the shared motion settings.
- Empty states that carry an action button.
- Pull to refresh and skeleton loading.
- Accessible names on every icon-only control.

## 13. Not checked

- Anything on a real device or with a signed-in account.
- The current build's player, sheets, Radio, Movies and Downloads screens as rendered.
- Tablet and landscape layouts.
- Light themes and the imported palettes.
- Large font sizes and TalkBack.
- The video fullscreen controls.
