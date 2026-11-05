# Android downloads use bounded CDN chunks (M9)

- **Date:** 2026-11-05
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented (unit test written; not run locally, see below)

## What it does
Offline downloads were sent to Media3's progressive downloader as one open-ended HTTP
request. googlevideo.com paces such requests at roughly the media bitrate, so a 2 MB
audio track downloaded at about 35 KB/s on a 19 Mbps connection (owner report,
2026-11-05). Downloads now go through `ChunkedRangeDataSource`. It splits a YouTube CDN read into
consecutive bounded requests of at most 10 MiB, the same size yt-dlp uses
(`http_chunk_size`). It uses `clen` for the total length and sends each chunk through
the existing `IndexedRangeDataSource` query-range/Range-header path. Cache keys, cache
positions and the reported URI stay those of the original request, so existing partial
and completed downloads remain valid. Non-CDN URLs, URLs without `clen`, signed or
pre-ranged URLs and reads of 10 MiB or less pass through unchanged. A chunk that ends
early raises `EOFException`, so a truncated file never completes.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Chunk boundaries, pass-through cases, truncation | `ChunkedRangeDataSourceTest` (Robolectric) via `:app:testDebugUnitTest` | Not run in the authoring container (no Android SDK); left to Android CI | unit |

## Not verified and limits
- No emulator, physical-device or live download throughput measurement yet. The speed
  claim rests on YouTube's documented pacing behaviour and yt-dlp practice, not on a
  measured run.
- Chunks within one stream are sequential. Video and audio parts still download in
  parallel up to `download_parallel`.
- CDN URLs without `clen` keep the single-request path.

## For the plan and parity tables
- Android VALIDATION: "Offline downloads request YouTube CDN media in bounded 10 MiB
  chunks (unit verified); live throughput on device pending."
