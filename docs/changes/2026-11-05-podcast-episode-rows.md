# Podcast episodes play instead of opening an empty page (M9)

- **Date:** 2026-11-05
- **PR:** not opened yet
- **Milestone:** M9
- **Platforms:** shared engine (Android and desktop)
- **Status:** fixture/unit verified; live verified (anonymous)

## What it does

Opening a podcast show now lists its episodes as playable audio items with the show name and
length. Tapping an episode plays it. The playback queue and show downloads skip collection cards,
so they now include a show's episodes too (code path read, not run). Before this change, every episode in a show was a "playlist" card. Tapping one
opened an empty page saying "No supported items were returned" with "Back to previous feed"
(owner screenshot, 2026-11-05).

## What differs from songs

| | Song row | Podcast episode row |
| --- | --- | --- |
| Title link | `watchEndpoint` (plays the video) | `browseEndpoint` to `MPED<video ID>`, page type `MUSIC_PAGE_TYPE_NON_MUSIC_AUDIO_TRACK_PAGE` |
| Second column | Artist (`UC…`) | Show (`MPSP<playlist ID>`) |
| Video ID | `playlistItemData.videoId` | Same, and also inside the `MPED` id |
| Stream | Native extractor, opus audio | Same (resolved live, see Evidence) |

The parser lets a browse link win over a row's video ID, so that albums, artists and playlists
open as collections rather than playing a decorative play button. Episode rows were caught by that
rule. An `MPED` page is an episode description with no playable items, so the parser now treats
any row or tile that links to `MPED` plus an 11-character video ID as the playable episode. It
takes the video ID from the row, or from the `MPED` id when a tile has none. Shows (`MPSP`),
albums (`MPRE`), artists and playlists are unchanged.

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Episode rows and tiles | `cargo test -p vidcue-providers show_episode_rows_play` | Passed (synthetic fixture shaped like the live rows) | unit |
| Workspace | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` | Clean; all suites passed | unit |
| Reproduce the screenshot | Engine `Browser::fetch(Collection("VLMPED…"))` and `Collection("MPED…")` for a live episode, anonymous | 0 items and the exact notice from the screenshot | live, one-off |
| Show pages before the fix | Engine `Browser::fetch(Collection("VLPL…"))` for 3 shows from anonymous Music Home | 94–100 items each, all `Playlist` cards with `MPED…` ids | live, one-off |
| Show pages after the fix | Same 3 shows plus 3 others | 100 items each, all playable, 0 `MPED` cards, show name and length set | live, one-off |
| Podcasts chip | Engine Home → Podcasts category, anonymous; region probe for US, DE, GB, IN, BR, JP | 26–30 episodes, a continuation with 27 more; parsed before and after | live, one-off |
| Episode playback | Vendored native extractor, `bestaudio`, two episodes and one song, anonymous | All three resolved to opus audio in 6–8 s | live, one-off |

The live checks used temporary examples that were not committed. No account was used.

## Not verified and limits

- Not run on a phone. The change is in the shared parser, and the Android app consumes its output
  unchanged.
- A signed-in Podcasts chip response has never been inspected. Anonymous Podcasts parsed in every
  region tried, so if the chip itself is still empty while signed in, a sanitized response is needed.
- The queue and show-download effects follow from the code path (both skip collection cards); they
  were not run.
- YouTube can change the episode page prefix or the row shape.

## For the plan and parity tables

M9 podcasts: episode rows in show playlists and episode tiles (`MPED…` links) parse as playable
audio with show name and length, instead of collection cards that open an empty episode page. The
queue and show downloads no longer skip them. Unit-verified; live-verified anonymously against six shows
and native episode playback; no signed-in or device check.
