# Channel pictures on modern feed cards (M9)

- **Date:** 2026-11-05
- **Milestone:** M9
- **Platforms:** shared engine (Android consumes it)
- **Status:** fixture/unit verified

## What it does
Android showed only the coloured initial in the channel avatar on most Home,
Subscriptions and related cards. The provider read the creator picture only from
classic `videoRenderer` cards, while YouTube's current feeds use `lockupViewModel`
cards, which carry it under `metadata.lockupMetadataViewModel.image`. The parser now
reads that slot (single `decoratedAvatarViewModel` / `avatarViewModel`, or the first
creator of an `avatarStackViewModel` collaboration). Protocol-relative image URLs
(`//yt3...`, common for channel pictures and channel search results) are now accepted
as HTTPS instead of being dropped. Avatars still never enter the video artwork list,
and the initial remains the fallback when no picture is advertised or it fails to load.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Lockup avatars, collaboration stack, missing avatar, artwork isolation | `cargo test -p vidcue-providers lockup_cards_keep_their_creator_avatar` | pass | fixture |
| Protocol-relative channel picture | `cargo test -p vidcue-providers channel_search_results_accept_protocol_relative_pictures` | pass | fixture |
| Full provider suite | `cargo test -p vidcue-providers` | 88 passed | unit |
| Lint | `cargo clippy -p vidcue-providers --all-targets -- -D warnings`, `cargo fmt --check` | clean | static |

## Not verified and limits
- Fixtures are synthetic shapes modelled on current YouTube lockups; no live account
  feed, APK build, emulator or physical-device check was run in this session.
- Cards whose response carries no avatar at all (some shelves, the music app) still
  show the initial.

## For the plan and parity tables
- Android: creator pictures on modern lockup feed cards and protocol-relative channel
  pictures — fixture verified (2026-11-05); live/device acceptance pending.
