# Tappable links in the video description (M9)

- **Date:** 2026-11-06
- **PR:** #<number>
- **Milestone:** M9 (Android UI)
- **Platforms:** Android
- **Status:** implemented (unit tests written, not run locally)

## What it does
When the watch-page description is expanded, every `http://` or `https://` URL in it is shown
in the theme's primary colour and can be tapped. A YouTube video, Short, live or playlist link
plays in Vidcue, through the same path as a pasted link. Any other link (merch, Instagram, GitHub,
channel pages) opens in the browser or app that handles it; if no app can, nothing happens. While
the description is collapsed, a tap anywhere still just expands it. The text itself is unchanged.
Sentence punctuation after a URL is left out of the link, as for shared links.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Link ranges and taps | `WatchDetailsTest.descriptionLinksCoverOnlyTheUrls`, `descriptionWithoutLinksIsPlainText` (Robolectric) | not run locally | unit |

## Not verified and limits
- No Android SDK in the authoring environment (dl.google.com is blocked), so the app and tests
  were not compiled or run locally; PR CI compiles and runs them. Not seen on an emulator or device.
- Only explicit `http(s)://` URLs become links. Bare `www.` text, `@handles` and `#hashtags`
  stay plain. YouTube channel URLs open externally rather than as a channel page in the app.
- Timestamps (e.g. `1:23`) in the description are not yet tappable.

## For the plan and parity tables
- M9 watch page: links in the expanded description are tappable; YouTube media links play in
  the app, other links open externally.
