# M9: complete UI/UX overhaul pass

On 2026-11-02 the owner asked for the Android app to be redesigned after the
restyle brief (`ytui-youtube-restyle-agent-prompt.md`) and its reference
(`youtube-style-and-feel.md`). The goal is a fully usable UI and UX, and the
owner allowed deviations from the brief where it is wrong. This pass builds on
the judged refinement in PR #31 (`feat/android-ui-fidelity`, revision 3
`e11dd04`). It does not redo that work: the merged content-first restyle (#29)
already covers the feed, chips, app bar, card menu, music grid and settings
page.

## Starting point

The rev. 3 native artifact (Android run 41419655053, 225 PNGs) still showed
these gaps against the brief and against basic usability:

| Area | Defect in rev. 3 captures |
| --- | --- |
| Downloads | Large "Library" page heading, tinted 20dp cards, rows of text buttons, dropdown menus, "Pause / retry" as one ambiguous text action |
| Video "Now playing" | 16:9 video at the top, then about half a screen of black before the title (`native-video-return`) |
| Playback options | Duplicated the whole player (title, seek, transport), plus a tinted volume card, a stack of primary pills for speed, tracks and segments, and a nested speed sheet |
| Account | Three primary pills (sign in, import header, reload); sign-in still primary when already connected |
| Sign-in activity | Stock Material colours (purple baseline), not the app palette |
| Phone landscape | Navigation rail clipped the Downloads label below the fold |
| Feed error | Tinted 24dp card with an accent icon |
| Tablet side player | "NOW PLAYING" overline; accent-coloured −10/+10 labels |
| Lyrics | Editor form first, actual lyrics last; five primary pills |
| Settings | Folder choices as stacked primary pills; quality and audio-only duplicated across two pages; teal "Copy timing report" link |
| Search | Native failures `realKeyboard…` and `focusedSearchBack…`: after leaving search the field is still focused and the keyboard reopens |

## Changes

Shared components (`MediaDesign.kt`):

- `ValueRow`: menu and settings row with leading icon, subtitle, current value and chevron.
- `SecondaryAction`: outlined neutral pill.
- `QuietAction`: neutral or destructive text action.
- `MediaTextField`: neutral filled input; a pill for single-line fields.
- `MediaListRow`: quiet thumbnail list row.
- `SectionHeader` and `GlyphTile`.
- New outlined glyphs: chevron, plus, captions, speed and edit. The link glyph is redrawn.

Each screen now has at most one primary (inverted) pill.

- **Downloads.** No page heading. One chip row (the Transfers chip shows a
  count), with save-from-link and new-playlist icons at its end.
  - Offline items are list rows with a bottom-sheet menu: header, Play, Add to
    playlist, Export, Retry folder save, and Remove (error colour).
  - Transfers show state and percentage. Their icon matches what
    `download_toggle` really does: pause, resume, retry, or restart preparation.
  - The playlist sheet has Play all as its primary action, Rename as secondary
    and Delete as a destructive text action. Its actions wrap at 2× text, and
    item actions sit in a menu.
- **Player.**
  - Portrait video is a watch page: 16:9 video, then title and metadata, then
    the action row, then an inline **Up next** list of the real queue
    (`vm.queue`, refreshed per item). "Edit queue" opens the existing queue
    sheet.
  - Music centres its art and adds an action row: queue, lyrics, sleep timer,
    save offline and more. Save offline uses the existing `play(download = true)`.
  - **Playback options** is a menu: volume slider, Playback speed, Video
    quality and Audio & subtitles (each with its current value), queue, sleep
    timer, marked segments and Stop. Speed, quality and tracks open as in-sheet
    sub-pages, so there is still one sheet with one Close.
  - The tablet side panel keeps its transport with neutral seek labels and
    "Open full player" as a secondary pill.
  - The mini player gains a close button that runs the existing `stop` command.
- **Search focus.** Leaving search (the Back icon, or the platform Back once the
  keyboard is hidden) now requests focus on a zero-size, non-text focus sink.
  The sink is the first focusable node. The old path called
  `clearFocus(force = true)`. That let Android give focus back to the Compose
  root view, which re-entered the first focusable node (the search field) and
  showed the keyboard again, matching both rev. 3 failure captures. **This
  diagnosis has not yet been confirmed by a native run.**
- **Account.**
  - Signed out: a status row, one primary "Sign in to YouTube", and the
    embedded-sign-in caveat.
  - Signed in: "Signed in to YouTube" with Reload, Sign in again and
    Disconnect as non-primary actions.
  - Importing sits in its own section. The cookie file is a value row, the
    header is a neutral field, and "Import header" is secondary.
  - Every warning is kept, including that Android cannot read another browser's
    private data.
- **Sign-in activity.** Uses the last applied app colour scheme and typography.
  It has a slim bar with a Close icon, the title, and one "Use this session"
  primary. The `sign-in-cancel` and `sign-in-complete` tags are kept.
- **Feed error.** Uses the neutral empty-state pattern ("Account needed" or "This
  feed is unavailable"), with one primary action and Try again. The
  "Back to previous feed" link is neutral, and the feed notice is inset on the
  full-bleed tab.
- **Settings.**
  - Root rows have subtitles.
  - The quality limit appears once, under Playback, and notes that it also
    limits offline saves. Audio-only appears once, under Offline & storage.
  - Music and video folders are value rows showing the current folder.
  - The proxy uses the neutral field, and the timing report button is secondary.
- **Lyrics.** Song header first. Saved or found lyrics are large readable lines
  with the playing line emphasised. "Edit or find lyrics" toggles the editor:
  Find is the only primary action, Import and Save are secondary, and Remove is
  destructive. Match choices are rows with a check.
- **Navigation rail.** On short windows (phone landscape) the four destinations
  share the height before scrolling, so Downloads and its label stay visible.
- **Other.** About links are rows; Radio country and genre browsing uses fields
  with a go icon; "Load more" is secondary.

## Decisions where the brief left room or was wrong

- **Snackbars are not reintroduced.** `aaf12c5` removed confirmations on
  purpose, and a native test asserts there is no "Added to queue" overlay.
  Actions give feedback in place instead: the Transfers count and the queue list.
- **Save to library is not in the player action row.** `save_media` toggles,
  and the player has no saved state to show, so a toggle there could silently
  remove an item. It stays in the card menu.
- **The video page shows Up next.** The brief allows only restyling, but the
  dead space was a usability defect. The list only displays existing queue data
  and reuses the existing play-queued action.
- **The mini player has a close button.** It runs the existing stop command, as
  YouTube's mini player does. No playback mode was added.
- **Seek amounts and their configuration are unchanged.** The native overlay's
  −5/+15 asymmetry is still flagged for a human decision, as in earlier reports.
- **The "YouTube" tab label is unchanged.** Renaming it to "Home" still needs
  owner approval.

## Preserved contracts

Every test tag and content description used by the instrumentation suites is
kept. That includes `account-import`, `cookie-header-field`, `library-*`,
`add-download`, `create-offline-playlist`, `offline-playlist-*`,
`volume-slider`, `pane-*`, `player-*`, `settings-*` and `proxy-field`. The
texts "Audio & subtitles", "Toggle captions", "Open full player", "Create",
"Rename", "Delete playlist", "Delete", "Account needed" and the sleep-timer
labels are also kept. Exactly one "Close" control is visible per open sheet.
Two evidence captures are added: `restyle-player-options.png` and
`restyle-player-options-tracks.png`. No assertion was relaxed.

No playback, provider, Rust, networking, cache or persistence code changed.

## Evidence

Local (Linux, no KVM), on the final source:

- Debug APK for x86_64 with the Rust engine.
- Instrumentation APK.
- 21 JVM tests pass.
- Lint has 0 errors and 39 warnings (baseline 40, none suppressed).
- ktfmt (kotlinlang style) is clean.

**Pending:** a native Android CI run and screenshot review of this revision. The
session could not push (GitHub returned 403), so no native, contrast, motion or
theme-matrix result exists for this change yet. This document does not claim
visual acceptance. The rev. 3 failures in large-text navigation measurement,
offline-playlist rename timing and HLS start timing belong to the PR #31 work
and remain open. Physical-device and live-YouTube acceptance remain separate
gates.
