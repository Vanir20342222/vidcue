# Faster downloads: parallel bounded CDN chunks (M9)

- **Date:** 2026-11-05
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android, desktop
- **Status:** implemented; desktop args unit verified; Android unit test written, not run locally

## What it does
**Android.** Offline downloads were sent to Media3's progressive downloader as one
open-ended HTTP request. googlevideo.com paces such requests at roughly the media
bitrate: on 2026-11-05 the owner saw a 2 MB audio track download at 35 KB/s on a
19 Mbps connection, and asked for every download to be as fast as possible. Downloads
now go through `ChunkedRangeDataSource`. It reads a YouTube CDN resource as bounded
requests over 4 parallel connections per stream, using `clen` for the total length.
Chunks are `ceil(size / 4)`, kept between 512 KiB and 2 MiB, so a short track also
spreads across connections. Buffered memory stays at about 8 MiB per stream.
- Each chunk uses the existing `IndexedRangeDataSource` query-range/Range-header path.
- Chunks are buffered and handed to the cache strictly in order. Cache keys, positions
  and the reported URI are unchanged, so existing partial and completed downloads stay
  valid.
- A dropped or timed-out connection is retried twice per chunk. HTTP status errors
  (403, 429, ...) are not retried.
- A short chunk raises `EOFException`, so a truncated file never completes.
- Pause or removal aborts in-flight sockets.
- These pass through unchanged: non-CDN URLs, URLs without `clen`, and signed or
  pre-ranged URLs.

**Desktop.** The native extractor and yt-dlp already download YouTube in 10 MiB chunks.
External yt-dlp now also gets `--concurrent-fragments 4` for HLS/DASH, but only when
no bandwidth cap is set, because yt-dlp applies `--limit-rate` per fragment thread.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| yt-dlp args include/omit concurrent fragments | `cargo test -p vidcue-services --locked --lib download` | 5 passed | unit |
| Lints | `cargo clippy -p vidcue-services --all-targets --locked -- -D warnings`, `cargo fmt --all -- --check` | clean | unit |
| Parallelism, ordering, retry, truncation, pass-through | `ChunkedRangeDataSourceTest` (Robolectric) via `:app:testDebugUnitTest` | Not run in the authoring container (no Android SDK); left to Android CI | unit |

## Not verified and limits
- No emulator, physical-device or live throughput measurement yet. The expected
  speed-up rests on YouTube's documented pacing and yt-dlp practice.
- CDN URLs without `clen` keep the single-request path.
- The native desktop downloader (yt-dlp-rs) fetches its 10 MiB chunks one at a time.

## For the plan and parity tables
- Android VALIDATION: "Offline downloads fetch YouTube CDN media as parallel bounded
  chunks (4 connections, <= 2 MiB each; unit verified); live throughput on device pending."
