# Android channel pictures load from the provider (M9)

- **Date:** 2026-11-04
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android (shared model and provider parser; desktop rendering unchanged)
- **Status:** implemented; Rust verified locally, Kotlin not compiled or run here (no Android SDK)

## What it does
Creator avatars were a coloured initial only, because the data model had no avatar URL.
This change carries the provider's channel picture through to the Android UI.

- `ytui-core::Media` gains `channel_thumbnail` (serialized only when non-empty, so existing
  JSON and saved state are unchanged).
- `ytui-providers` fills it from the channel-thumbnail slot that classic renderers carry
  (`channelThumbnailSupportedRenderers.channelThumbnailWithLinkRenderer.thumbnail`, with a
  `channelThumbnail` fallback). The `thumbnails()` walker now also skips
  `channelThumbnailSupportedRenderers`, so avatar sizes can never become video artwork.
- Android `ChannelAvatar` draws the picture over the initial. The initial stays visible until
  the picture loads, and it remains the only content when there is no URL, when the
  "Thumbnails" setting is off, or when loading fails after three attempts.
- Pictures use the existing `ArtworkCache`, via a synthetic artwork-only card keyed by the avatar
  URL. They share the memory cache and the concurrency limit with other artwork, but never a
  cache entry with video art.
- Call sites: feed and bleed rows (`MediaRow`), wide search rows (`WideMediaRow`), and the
  focused player's channel row for video.

This supersedes the earlier "initial only, no network requests" decision in
`docs/android/CONTENT_FIRST_RESTYLE.md` and the "no avatar URL" note in `docs/android/UX_OVERHAUL.md`.

## Limits
- Only the classic renderer avatar slot is parsed. Lockup-style cards (`lockupViewModel`) and
  the watch-page owner renderer are not yet covered, because their live JSON shape has not been
  verified. Those cards keep the initial.
- The focused player uses the media that was queued. Playback started from a source with no
  avatar keeps the initial.
- Live YouTube responses were not fetched. The parser tests use synthetic fixtures.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Avatar parsed; never becomes video artwork | `cargo test -p ytui-providers channel_avatar_is_kept` | passed | unit |
| Missing avatar stays absent from JSON | `cargo test -p ytui-providers missing_channel_avatar` | passed | unit |
| Affected crates (core, providers, mobile) | `cargo test -p ytui-core -p ytui-providers -p ytui-mobile` | all passed | unit |
| Format and lint | `cargo fmt --all -- --check`; `cargo clippy -p ytui-core -p ytui-providers --all-targets -D warnings` | clean | static |
| Avatar card is artwork-only and keyed apart from video | `ChannelAvatarTest` (Robolectric) | added, not run (no Android SDK here) | unit |
| Kotlin compile and existing Android unit tests | Gradle `testDebugUnitTest` in Android CI | pending | CI |
| Picture visible on a device, with network and the Thumbnails setting toggled | Galaxy Tab S9 / emulator, live feed | pending | device |
