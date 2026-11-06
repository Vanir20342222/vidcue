# Tappable links, channels, hashtags and timestamps in the video description (M9, M3)

- **Date:** 2026-11-06
- **PR:** #<number>
- **Milestone:** M9 (Android UI), M3 (shared YouTube browse provider)
- **Platforms:** Android, shared engine
- **Status:** implemented; provider fixture verified; Android unit tests written, not run locally

## What it does
When the watch-page description is expanded, these parts of it are shown in the theme's primary
colour and can be tapped. While it is collapsed, a tap anywhere still just expands it, and the text
itself never changes.

- `http(s)://` URLs and bare `www.` addresses: a YouTube video, Short, live or playlist link plays
  in Vidcue (as a pasted link does). Anything else opens in the app that handles it; if none can,
  nothing happens.
- YouTube channel links (`youtube.com/@handle`, `/@handle/videos`, `/channel/UC…`) and bare
  `@handles` open that channel's Videos tab in Vidcue. The player shrinks to the mini-player.
- `#hashtags` (with at least one letter) run a YouTube search on the Videos tab.
- Timestamps (`1:23`, `1:02:03`) seek the playing video. Ones past its duration stay plain.

The engine now accepts a YouTube `@handle` as a `Collection` ID and looks up its `UC…` channel
through `youtubei/v1/navigation/resolve_url` before browsing the channel's Videos tab.
Email addresses, URL fragments, `#1`, ratios like `16:9` and times like `10:30am` stay plain.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Handle resolves to channel Videos tab | `cargo test -p vidcue-providers channel_handles_resolve_to_the_channel_videos_tab` (loopback: resolve_url request and URL, then browse with Videos params; unknown handle errors without a browse) | pass | fixture |
| Workspace | `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` | pass | unit |
| Description patterns | the `www`/handle/hashtag/timestamp regexes run through `jshell` (same `java.util.regex` engine) on the test text | matched as the tests expect | manual |
| Description parsing and links | `DescriptionLinksTest`, `WatchDetailsTest` (JUnit/Robolectric) | not run locally | unit |

## Not verified and limits
- No Android SDK in the authoring environment (dl.google.com is blocked), so the app and its tests
  were not compiled or run locally; PR CI compiles and runs them. Not seen on an emulator or device.
- No live YouTube request: `navigation/resolve_url`'s response shape follows current WEB responses
  but is not live verified.
- Legacy `/c/name` and `/user/name` channel URLs, non-ASCII handles and YouTube Music artist links
  still open outside the app.
- The desktop terminal UI does not render description links; only the provider change reaches it.

## For the plan and parity tables
- M9 watch page: in the expanded description, links, `www.` addresses, channel handles/links,
  hashtags and timestamps are tappable (play/open channel/search/seek in app; other links external).
- Shared provider: `Collection("@handle")` on YouTube resolves the handle and opens the channel
  Videos tab.
