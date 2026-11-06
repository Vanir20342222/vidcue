# M9: Android playlist download expands nested playlists

## Problem

Downloading a collection on Android (for example an artist or channel page,
or a playlist whose catalog lists other playlists) queued nested playlist
entries as if they were videos. Resolving one failed with
`Playlist entry failed: <title> · playlist result requires extract_result or
extract_result_with_format`.

## Change

- `ytui_providers::watch::collect_playable` collects the collection, expands
  each nested playlist/album one level into its videos, skips artist/channel
  entries and drops duplicate videos. Existing 2,000-entry, page, memory and
  cancellation limits still apply; at most 50 nested playlists are expanded.
- The mobile `collection`/`enqueue_collection` operations use it.
- `PrepareWorker` skips nested collection entries still present in older
  checkpoints instead of reporting them as failures.

## Evidence

- `cargo +1.99 test -p ytui-providers collection`: 7 passed, including the new
  `playable_collection_expands_nested_playlists_into_single_videos` loopback test.
- `cargo +1.99 clippy -p ytui-providers -p ytui-mobile --all-targets -D warnings`
  and `cargo fmt --check`: clean.
- `PreparationPolicy.collection` compiled and checked with plain `javac`.
  The Gradle unit test (`PreparationPolicyTest`) was added but not run: this
  container has no Android SDK.

## Not verified

No live YouTube download or physical-device run of the reported playlist.
