# Watch-page action tray, watch details, hiding offline header (M9)

- **Date:** 2026-11-05
- **PR:** none yet
- **Milestone:** M9
- **Platforms:** Android and shared engine
- **Status:** fixture/unit verified

## What it does
Owner feedback on a phone (2026-11-05), after #112:

- **No sideways scrolling under the video.** The watch-page actions (Queue, Save, Share, audio
  output, sleep timer while running, ⋮) are now one soft tray of evenly spaced icons like the
  music player's, sized to fit the width, instead of labelled pills that overflowed and had to
  be scrolled. Each icon keeps its spoken name.
- **Upload date, likes and subscribers on the watch page.** With every "What cards show" switch
  on, the watch page still showed only the view count: the fast playback path returns views and
  the description but not the date, likes or subscriber count. The engine now has a
  `watch_details` call that reads them from YouTube's `next` response (one small JSON request
  after the video starts, the endpoint related videos already use; no watch-page HTML). Android
  merges them only where the stream left gaps, and the existing switches (views, upload date,
  likes, channel) decide what is shown. Labels stay as YouTube localized them; a like count is a
  number only when written out in full, never a guessed "1.2K".
- **Offline search bar hides like Home and Music.** The Downloads header (sections, search,
  actions) now floats over the list, slides up as the list scrolls down and returns on any
  upward scroll. It stays in place while selecting or typing a search.

Feed cards that ignore the same switches are the subject of the separate open PR #111.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Parser | `cargo test -p vidcue-providers --locked watch_details` | 4 passed | fixture |
| Rust workspace | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` | see PR | fixture |
| Android compile | `:app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` | passed | build |
| Android unit tests | `:app:testDebugUnitTest` (new `WatchDetailsTest`) | see PR | fixture |

## Not verified and limits
- No emulator, device or live YouTube run. The `next` response paths follow the renderer names
  yt-dlp uses (`videoPrimaryInfoRenderer`, `likeButtonViewModel`, `subscriberCountText`), but
  have not been checked against a live response from this environment.
- Likes appear only when the label carries the full number.
- The watch-page description card is shown whenever a description exists; the "Description"
  switch still applies to cards only.

## For the plan and parity tables
- M9 Android watch page: date, likes and subscribers load after playback starts through the
  engine's `watch_details` call; actions are a fitted icon tray. Offline header hides on scroll.
  Fixture verified; live and device checks pending.
