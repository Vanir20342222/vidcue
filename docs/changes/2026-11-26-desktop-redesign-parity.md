# Desktop: mobile-style shell, YouTube Music layouts and Android parity (M9)

- **Date:** 2026-11-26
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** desktop
- **Status:** fixture/unit verified (headless renders, offline UI tests); not run in a window or live

## What it does
The owner asked (2026-11-26) for a desktop app with Android feature parity, the same speed,
fewer bugs and a more polished, mobile-like UI based on YouTube and YouTube Music. From three
HTML mockups they chose the Android tablet's floating sidebar (option B) and a YouTube Music
capsule player that opens a full Now Playing view (option C), and they chose to keep Rune.

Shell and layout:
- **Sidebar:** a floating rounded panel, inset from the window edges as on the tablet. It has
  Home, Music, Radio and Movies, then a Library group (Queue, Recently played, Saved, Playlists,
  Downloads), with the account and settings at the foot. The selected row is a neutral pill with
  a filled glyph; pink is kept for state. Windows narrower than 1100 pt get an icon rail with
  badges.
- **Top bar:** clear over the page with a soft search pill. Its backing fades in as the page
  scrolls, using a scroll-linked live value, so no view re-runs. The account button moved to
  the sidebar.
- **Colour wash:** Home, Music, Radio and collection pages fade from the cover's two sampled
  colours into the page. The colours come once from an 8×8 decode and are drawn as plain
  gradients, so scrolling costs no blur passes.
- **Capsule player:** replaces the full-width dock. It floats at the bottom centre (64 pt,
  frosted) and holds cover or live video, title, shuffle, previous, play, next, repeat, lyrics,
  queue, volume, open and stop. Progress runs along its lower edge and can be dragged to seek.
- **Now Playing:**
  - The stage sits on the left. A floating panel on the right has tabs: Up next, Lyrics (music)
    and Chapters (videos with chapters).
  - For music, icon actions sit under the stage: save, download, add to queue, copy link and
    artist.
  - Videos keep YouTube's details under the player. Chapters moved from the description card to
    their own tab.
- **Music page:** provider sections become shelves, as on Android. A song shelf is a Quick picks
  grid (four rows, columns scroll sideways). Other shelves are carousels with ‹ › arrows, Play
  all, and Show all (the shelf as a grid on the same page). Shelves never repeat their section
  name on each card.
- **Home:** the chips are For you and Subscriptions, plus Android's resume chip (the last video's
  cover with a play mark). Cards show upload age on its own line with a clock ("3 days ago"),
  counted from the upload date when YouTube gives no text. Liked, Watch later and the YouTube
  playlists moved to Library.
- **Search:**
  - Video search has filter chips: All, Videos, Channels, Playlists.
  - Music search has filter chips: All, Songs, Albums, Artists, Playlists, Videos.
  - Music "All" is grouped into Top result, Artists, Songs, Albums, Playlists and Videos.

Android parity (engine ops the desktop never called before):
- **Playlists:** the engine's own playlists, synced once a Vidcue account is signed in.
  - Library → Playlists shows them with a create card. Each playlist has a page with Play,
    Shuffle, rename, delete (with confirmation), and per-entry move up, move down and remove.
  - A card's ⋮ menu has "Add to playlist…", which opens a picker. It lists synced playlists, a
    new playlist, offline playlists and a new offline playlist.
- **Follow channels:** a Follow/Following button on channel pages. Followed channels appear
  under Library → Playlists.
- **Feed filters (Settings → Feeds):** hide Shorts, watched videos and live streams; hide
  keywords; and "Hide this channel" from any card's menu. Loaded feeds reload after a change.
- **Play next** in every card menu: queue the item, then move it to the front, as Android does.
- **Vidcue account (Settings → Sync):**
  - Sign in to a self-hosted server: address, username, password and authenticator code.
  - Or join from a `vidcue://pair` link. A check shows the server's origin and warns on plain
    http. A confirm card then joins, with or without the YouTube sign-in, or cancels.
  - Signed in, the page shows the state and last sync, Sync now, and sign out (optionally
    removing synced data).
  - It lists devices with online status and what they are playing, and can remove them.
  - "Add a device" shows a pairing QR code for a phone to scan.
  - Ten per-collection "What syncs" switches.
  - The engine's sync loop now starts with the desktop app. Its events refresh the library,
    history, saved items and settings.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Format | `cargo fmt --all -- --check` | clean | — |
| Lint | `cargo clippy -p vidcue-desktop --all-targets --locked -- -D warnings` | clean | — |
| Desktop tests | `cargo test -p vidcue-desktop --locked` | 101 passed, 4 ignored (live, opt-in); 93 before. New unit tests cover shelf grouping, music search groups, search filters, upload ages, swatches, sync status lines, QR runs and engine JSON shapes | unit/fixture |
| Headless UI interaction | `ui_tests.rs` (Rune runtime, real pointer and keys) | passes. The chip test now presses Subscriptions, since Watch later moved to Library; it checks the same in-place swap | fixture |
| Visual | `vidcue-desktop --snap … --scene <home, music, collection, song, watch, dock, library-playlists, settings-feeds, settings-sync, settings-sync-signed-in> --size 1440x900`, compared against renders of `main` | as described | headless render |
| Scroll cost, before and after | Same build machine and target. `main` stashed vs this change; `--scene <home/music/watch> --size 1440x900 --script scroll.json` (60 wheel steps) | Home CPU p50 1.09 ms vs 1.22 ms on `main` (p95 4.2 vs 3.5). Music p50 1.56 vs 1.25 ms (p95 3.2 vs 2.8; shelves add layout nodes, 524 vs 315). First frame 306–368 ms vs 390–460 ms. All are single headless runs on a software GPU; GPU time not measured | headless |
| Independent review | A second agent read the whole diff against the engine sources | 11 findings, all fixed. The serious one: a link join never called `sync_pair_confirm`, so sync never started. Smaller ones: sync events missing `account_updated` and `shared_playlist_changed`; no wake-up for sync events; out-of-order fast reorders; deleting a shared playlist failed; a failed colour sample hid its picture; Enter skipped the busy checks; secret fields kept; a non-ASCII upload date could panic; status events refetching devices; the top bar backing staying on after a Library tab switch | review |

## Not verified and limits
- **No windowed or live run:** nothing was tried on Omarchy, Ghostty or Windows. No Vidcue
  server sign-in, pairing, device removal or live sync was exercised. Engine JSON shapes come
  from the engine source and unit tests.
- **Sync secrets on disk:** on the desktop, the engine keeps the sync session token in
  `state/desktop/sync/` as a plain file in the private state folder, as it keeps the YouTube
  session. Android seals the same file with the Keystore.
- **Sync events:** while an account is signed in, the window checks the engine's queue every
  2 s (an empty check redraws nothing). A change from another device shows within about 2 s,
  not instantly.
- **Not ported yet:**
  - Jam rooms, friends, inbox and "Send to friend"
  - stats and Wrapped
  - the Vidcue recommendations shelf
  - account security (recovery codes, password, authenticator)
  - key relay, remote control and handoff
  - the admin console
  - Movies through the engine's movie ops (the desktop still uses its own Cinecat path)
  - playlist sharing
  - Android's three-page onboarding (the desktop keeps its own)
- **Unused space:** the wide album-page layout (art left, tracks right) was not adopted. The
  collection header stays above the track list.

## For the plan and parity tables
- Desktop shell (2026-11-26): floating tablet-style sidebar, scroll-linked top bar, colour wash,
  YouTube Music capsule player and Now Playing panel (Up next / Lyrics / Chapters); Music
  shelves (Quick picks grid, carousels, Show all); search filter chips and grouped music
  search; Home resume chip and upload-age line. Fixture/unit verified.
- Desktop parity (2026-11-26): synced playlists, follows, feed filters, Play next, and the
  Vidcue account (sign in, pairing link, status, Sync now, devices, pairing QR, collection
  switches) through the shared engine. Fixture/unit verified; no live server run. Social, Jam,
  stats, recommendations, account security, admin and engine-backed Movies remain
  Android-only.
