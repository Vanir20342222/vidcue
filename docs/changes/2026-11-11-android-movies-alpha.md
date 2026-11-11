# Android Movies (alpha) page over the shared HLS engine (M9)

- **Date:** 2026-11-11
- **PR:** #148
- **Milestone:** M9
- **Platforms:** Android, shared mobile engine
- **Status:** fixture/unit verified; engine live-checked on this host; not run on a device or emulator

## What it does
A fifth navigation destination, **Movies**, mirrors the desktop Movies (alpha) page on Android:
a featured hero, poster rows, search, Continue watching with progress, Downloads, a "Play a
stream URL" panel and a detail sheet. The sheet has a season/episode picker, Play/Resume, Download
and a per-source check list. Playing first checks sources in order with real segment reads, then
opens a full-screen player that fails over to the next source if one stops mid-film. The player
has quality (Auto plus each height), audio, subtitle, speed and source menus, ±10 s and seek, and
saves the position for Resume. Downloads run one at a time into app-private storage through the
shared Cinecat transfer code, show bytes received, and play offline with their subtitles.

**Cinecat integration:** the page also carries a **Cinecat** section mirroring the standalone
CLI's curated TMDB pool (39 titles, two of them TV episodes). Playing or downloading one resolves
the stream through the Cinecat sources API (`movie_cinecat_resolve`: automatic provider failover
with verification, ZCDN web adapters first, then the API list), so the catalog carries a TMDB id
instead of a direct URL. Check all qualifies every provider (`movie_cinecat_check_all`) and shows
each provider's result, the way the desktop's provider check list does. Resolved streams can
require provider headers (Origin/Referer) and sidecar subtitles; `MovieSource` carries both, and
the player applies the headers to its HTTP loader and maps VTT/SRT sidecars onto
`MediaItem.SubtitleConfiguration`.

**Catalog sections:** openly licensed films (Blender open movies via Mux, Unified Streaming and
the Internet Archive), public Apple/Mux test streams, and the Cinecat titles above. Users can also
play any HLS or file URL they paste. The new engine operations in
`crates/vidcue-mobile/src/movies.rs` add `movie_cinecat_resolve`, `movie_cinecat_check_all` and
`movie_cinecat_providers` to the catalog-only `movie_check` and `movie_download`.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Engine unit tests | `cargo test -p vidcue-mobile --locked movies` | 8 passed, 1 ignored (live) | unit |
| Live failover | `cargo test -p vidcue-mobile movies -- --include-ignored` | missing Mux source fails, second source chosen | live (host) |
| Catalog sources probe | temporary ignored test running `movie_check` on every catalog URL | all 13 catalog URLs deliver segments (Cosmos Laundromat needs literal parentheses, as shipped) | live (host) |
| HLS offline bundle | temporary test: `movie_download` of Mux Tears of Steel (cap 360p → uncapped fallback) and Mux test 001 | 553 MB / 734 s with audio + 2 subtitle renditions; 65 MB / 510 s; `ffprobe` and a mid-film `ffmpeg` decode of the local `index.m3u8` succeed | live (host) |
| Rust lint | `cargo fmt --all -- --check`, `cargo clippy -p vidcue-mobile --all-targets --locked -D warnings` | clean | static |
| Android unit tests | `gradlew :app:testDebugUnitTest` (adds `MovieCatalogTest`, 8 tests) | 289 tests, 0 failures | unit |
| Android lint | `gradlew :app:lintDebug` | no errors; new warnings match existing code (`screenHeightDp`, prefs KTX) | static |
| Debug APK | `tools/android/build.sh` | `app-debug.apk` (51 MB) assembled with arm64/x86_64 engine and `MoviePlayerActivity` in the manifest; its bundled unit-test step was hit by a local Gradle socket fault and re-run separately (289 / 0) | build |
| Cinecat engine ops | `cargo test -p vidcue-mobile --locked` (adds `movie_cinecat_resolve`, `movie_cinecat_check_all`, `movie_cinecat_providers` + mapping/validation tests) | 49 + 17 + 0 tests pass; clippy clean | unit |
| Cinecat resolution | `cargo test -p vidcue-mobile movies -- --include-ignored` (adds `movie_cinecat_resolve` live) | ZCDN serves Dune (TMDB 438631) with failover attempts recorded | live (host) |
| Android Cinecat wiring | `gradlew :app:testDebugUnitTest` after the Movies UI gained the Cinecat sheet flow, provider reports, headers and sidecar subtitles | 289 tests, 0 failures (test now branches on `title.cinecat`) | unit |

## Review fixes (2026-11-11, same change)
A ten-agent review of the integration produced the following fixes, all in this change:

- **Engine (`crates/vidcue-mobile/src/movies.rs`)**: `movie_check` validates each source
  element separately — a malformed entry now fails that attempt (label from the raw JSON,
  empty otherwise) instead of failing the whole request. `movie_cinecat_*` media requests
  reject negative season/episode values instead of silently resolving as a movie. Provider
  labels use correct plural grammar ("1 subtitle", "N subtitles"). Module doc no longer
  claims the module never asks a provider API. New tests:
  `check_reports_a_malformed_source_without_failing_the_request`,
  `media_requests_reject_negative_seasons`; suite now 51 + 17 passing, clippy clean.
- **Catalog (`MovieCatalog.kt`)**: Cinecat TV ids drop the `:s:e` suffix (`cinecat-1396`),
  so `pick("cinecat-1396:1:1")` round-trips like any other series key; display labels are
  the bare title ("Breaking Bad", not "Breaking Bad S1E1").
- **Shared source parser**: `movieSourceFromJson` in `MovieCatalog.kt` replaces the
  duplicated inline parsing in `MoviesPage` and `MoviePlayerActivity.onCreate`.
- **Retry downloads survive the screen**: `downloadCinecat` now runs on `MovieLibrary`'s
  app-lifetime scope (`MovieLibrary.launch`) instead of a composition scope that dies with
  the transfer row.
- **Sheet state**: "Check all" clears stale provider reports before running; a successful
  resolve is reused instead of re-hitting the API on a second Play/Download tap; the
  Download pill is disabled while a check runs; the running check/resolve job is tracked and
  cancelled when the episode picker changes pick, so a stale resolution can no longer start
  playback of the previous episode; the Cinecat provider section shows a spinner while its
  first check runs instead of a blank body.
- **Player headers**: `MoviePlayerActivity`'s HTTP loader now sends the same
  `Vidcue-Cinecat/0.1` user agent as the shared transfer code.
- Re-run evidence: `cargo test -p vidcue-mobile --locked movies` 9 passed / 0 failed;
  `cargo fmt --all -- --check` clean; `gradlew :app:testDebugUnitTest :app:lintDebug`
  BUILD SUCCESSFUL — 289 unit tests, 0 failures, no lint errors.

## Not verified and limits
- No device or emulator run. Nothing in this change has been seen on screen, including the
  layout, navigation dock with five items, the bottom sheet, the player overlay, track menus,
  orientation, offline playback of a downloaded bundle in ExoPlayer, or Resume.
- Downloads have no foreground service: they run while the app process lives. A process death
  drops the job, and the next launch removes its staging folder. Progress is bytes received, not
  a percentage.
- Live-host checks depend on third-party test hosts (Mux, Unified Streaming, Apple, Internet
  Archive) staying online.
- The probe's `sustainable` flag reads false even with high headroom on fast hosts. The UI shows
  only height, throughput and time, not that flag.
- Not included: TMDB metadata, Cinecat providers, casting, or Picture-in-Picture.

## For the plan and parity tables
- Android PARITY: "Movies (alpha): open-film/test-stream catalog, source check + failover, quality/
  audio/subtitle/speed menus, resume, offline HLS/file downloads via shared transfer code —
  implemented, unit/fixture verified; device acceptance open. No Cinecat source resolution."
