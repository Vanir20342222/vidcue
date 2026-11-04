# Android audit fixes: large-text Settings, playlist Restart, lockup avatars (M9)

- **Date:** 2026-11-04
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android, shared engine (provider parser)
- **Status:** implemented; Rust fixture/unit verified; Android not compiled locally

## What it does
1. **Settings is reachable again after an empty search.** If the keyboard is dismissed while the
   search field is focused, empty and nothing has been searched, the app leaves search mode. The
   page title, Settings and More come back after one Back instead of two. A typed query or
   search results keep their session as before (`NativeUiTest` still expects that).
2. **Playlist Restart retries only the failed entries.** The worker records which entries failed.
   When a preparation finishes with failures, it stores a restart request that carries just
   those entries. Restart then resolves only those entries instead of listing the playlist again
   and resolving every entry. Skipped live entries are counted but never retried. The finished
   row says what Restart will do ("Restart retries 2 entries" or "nothing to retry").
3. **Restart and Remove leave no orphaned state.** Restart used to cancel the old job but keep its
   row, so a duplicate "Playlist preparation" row appeared next to the new one. Finished, restarted
   and removed jobs also left their cursor keys and checkpoint file behind for good. Each job now
   drops those. A cancelled worker no longer writes its row back.
4. **Creator avatars on lockup cards.** `lockupViewModel` cards now read the creator picture
   from `lockupMetadataViewModel.image.decoratedAvatarViewModel` (or plain `avatarViewModel`).
   They show the channel picture instead of the coloured initial. The avatar stays out of the video
   artwork list.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Rust format | `cargo fmt --all -- --check` | pass | — |
| Rust lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` | pass | — |
| Rust tests | `cargo test --workspace --locked` | 416 passed, 0 failed, 31 opt-in ignored | fixture/unit |
| Lockup avatar parser | `lockup_card_keeps_its_creator_avatar_apart_from_the_artwork` (synthetic JSON) | pass | fixture |
| Retry-list policy | `PreparationPolicyTest` (3 new JVM cases) | written; its logic was run with plain `javac`/`java` against the policy class, Gradle not run | unit (partial) |
| Kotlin/Java compile, JVM tests, lint | `:app:compileDebugKotlin :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest :app:lintDebug` | not run: no Android SDK; installing one was not permitted in this session. Android CI is the first compile | — |
| Large-text regression | `CatalogUiTest.largeTextKeepsNavigationSearchAndSettingsReachable` (failed in nightly run 41817598431, shard 0) | expected to pass with this change; not run | — |

## Not verified and limits
- No Android build, emulator or device run here. The large-text fix follows from reading the
  Back handler: with the IME open, Back only hid the keyboard, the empty field kept focus, and
  `searchMode` kept the toolbar hidden.
- Dismissing the keyboard over the empty-field recent-search list now closes that list too.
  Reopen it by tapping the field.
- The lockup avatar path matches YouTube's current view-model layout but was not checked against
  a live response. Cards without that slot keep the initial, and the watch-page owner avatar is
  still not parsed.
- A Restart pressed while a preparation is still running starts again from the full playlist,
  because the retry list only exists once a pass has finished. URL-keyed downloads still deduplicate.
- Not addressed here: the indexed cold-start budget miss in the same nightly run
  (`IndexedPlaybackTest`, 2052 ms vs 1000 ms; draft PR #48 targets it), the nav-bar band (device
  only), queue drag reorder, process-death queue rebuild, HLS/MP3/FLAC export and lyric sidecars.

## For the plan and parity tables
- android/PARITY.md, Playlist preparation: "Restart now retries only failed (non-live) entries
  and drops the old row, cursor and checkpoint; finished/removed jobs clean their progress state
  (unit/fixture verified, device pending)."
- android/PARITY.md, Images/avatars: "Lockup-style cards carry creator avatars (fixture verified;
  live JSON shape and watch-page owner avatar pending)."
- VALIDATION: "Large-text Settings reachability: an empty search session ends when the keyboard is
  dismissed; awaiting the nightly emulator rerun."
