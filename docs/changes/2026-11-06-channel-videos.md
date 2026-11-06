# Open a channel to see all its videos (M9, M4)

- **Date:** 2026-11-06
- **PR:** #<number>
- **Milestone:** M9 (Android UI), M4 (shared provider browse)
- **Platforms:** Android, shared engine (desktop gets the provider change too)
- **Status:** fixture/unit verified

## What it does
Opening a YouTube channel now shows the channel's **Videos** tab (every upload, newest first,
paging as you scroll) instead of its Home tab, which only had a few featured shelves. The page is
titled "<channel> · Videos". Music artist pages are unchanged.

Video cards now record the channel that published them (`channel_id`). On Android, tapping the
channel picture or name on a card, or the channel row in the player and watch details, opens that
channel. The card's ⋯ sheet has a new "Go to channel" row. The player shrinks to the mini-player,
so playback continues, and Back returns to the previous feed. Cards with no channel link behave as
before: a tap on the name still plays the video.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Channel opens on Videos tab | `cargo test -p vidcue-providers youtube_channels_open_on_their_videos_tab` (loopback: request carries the Videos params, Music and playlists do not; title, items and continuation parsed from a synthetic rich-grid page) | pass | fixture |
| Cards keep their channel | `cargo test -p vidcue-providers video_cards_remember_the_channel` (classic byline, lockup metadata row, channel result; menu links ignored; unknown channel not serialized) | pass | unit |
| Workspace | `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` | pass | unit |
| Android channel ID parsing | `ChannelLinkTest` (Robolectric) | not run locally | unit |

## Not verified and limits
- No Android SDK in the authoring environment: the app and `ChannelLinkTest` were not compiled
  or run locally; PR CI compiles them. Not seen on an emulator or physical device.
- No live YouTube request: the outbound network blocked youtube.com. The Videos-tab params
  (`EgZ2aWRlb3PyBgQKAjoA`) and response shape follow current WEB responses but are not live verified.
- The channel ID comes from the card's byline or avatar link. Feeds that omit the link (some
  Music rows, offline items, history imports) show no channel action.
- Shorts and Live tabs are not listed; only the Videos tab opens.
- The desktop terminal UI has no new key binding; it already opens channel search results, which
  now land on the Videos tab.

## For the plan and parity tables
- M9 browse: tap a card's channel picture/name, the player channel row, or "Go to channel" to
  open the channel's Videos tab.
- Shared provider: `Collection(UC…)` on YouTube requests the channel Videos tab; `Media.channel_id`
  carries the publishing channel.
