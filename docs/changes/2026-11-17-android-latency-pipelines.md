# Android latency pipelines (M9)

- **Date:** 2026-11-17
- **Milestone:** M9
- **Platforms:** Android / shared engine
- **Status:** fixture/unit verified
- **Base:** `06d0479189e720cdda095fd90fa63deae64ce801`

## What it does

Implements the six source findings from the separately preserved
[research brief](2026-11-17-android-performance-research.md). This is one related
latency investigation across transport, startup, downloads, artwork and browsing;
The non-test scope exceeds 400 changed lines because these six requested fixes
cross JNI/Android startup, native extraction and shared transport/cache boundaries.
Keeping the readiness, looper and cancellation rules requires their service,
worker and sign-in entry points in the same reviewable change. It does not redesign the UI from #170. Open UI PR #171
was discovered during implementation; no edits to its UI files were made.

- **F1:** reqwest 0.12.28 gains HTTP/2 negotiation (ordinary HTTP/1 fallback),
  gzip, Brotli and deflate. No package versions were deliberately upgraded;
  Cargo.lock gains the compression dependencies. Feature unification includes
  the native extractor: its webpage/JSON/XML and YouTube API/iframe text reads
  now have decoded-byte limits, alongside the existing player-JS bounds.
  Provider HTML/API chunk limits remain in force after decompression. Proxy,
  cookie and redirect policies are unchanged. Range requests explicitly select
  identity encoding. The native extractor already explicitly requests identity;
  its shared text/media client now disables transparent decoding so unified
  compression features cannot hide Content-Encoding from media integrity checks.
  Cinecat's byte-reading client similarly keeps encoded bytes and response headers.
  Native extractor compression therefore remains a separate future optimization;
  it gains HTTP/2 capability without changing its existing identity policy.
- **F2:** four transfer workers are separate from three metadata workers.
  Media3's configured parallel-download limit still controls active downloads;
  this does not change per-transfer CDN connection counts or chunk logic.
- **F3:** Application creation attaches the platform context and starts native
  initialization on a worker. A single process readiness/error result gates
  RustRpc, ViewModel prerequisites, preparation workers and playback commands.
  Platform proxy settings are applied before readiness. Service commands arriving
  during initialization retain FIFO order and reject a destroyed service.
  Sign-in startup no longer reads settings synchronously. Offline disk preparation
  normally runs on metadata IO before posting manager construction to main;
  manager construction is explicitly restricted to main. Restored downloads
  resume after engine/network readiness. Trace sections cover engine initialization,
  platform network configuration and offline storage.
- **F4:** the resolver retains an owned same-key flight lock and lease insertion
  after a UI waiter cancels. A foreground waiter joins that result. Detached
  flights are limited to four per engine; existing one-background/two-total
  extractor slots remain. Pending lock waiters are cancellable, errors release
  ownership, and epoch checks still reject stale results. Resolution still does
  not commit queue/history state.
- **F5:** decoded artwork returns after memory publication. At most two optional
  persistence/prune jobs run; a busy writer skips optional persistence instead of
  queueing decoded images. Clear drains both fetches and maintenance before
  deleting owned files, so earlier delayed writes cannot refill the cleared cache.
  Proxy/client replacement preserves that shared maintenance ownership as well.
  Atomic writes and user-data durability are unchanged.
- **F6:** history keeps incremental byte totals and saved page weights. Immutable
  media JSON identities share weak size estimates; edits must use `copyJson`, so
  changed objects get a new estimate. Page-cache sizing and incoming card estimates
  run on a background dispatcher before publication. Cached page reconstruction
  can reuse these estimates. Entry/byte limits, account clearing, scroll/back
  restoration and the actual 64-entry/8-MiB ViewModel cache remain intact.

## Reproduction and regression evidence

The original resolver plus the new cancelled-waiter regression failed with two
loads instead of one (`/tmp/vidcue-f4-reproduction.log`). The strengthened final
case holds a `spawn_blocking` worker during cancellation. The original shared
three-worker arrangement is also reproduced in a deterministic JVM test: its
snapshot remains queued until held transfers release. The replacement test holds
four transfers, completes metadata work, and cancels one transfer through the
separate control executor while others remain held.

Other new regressions cover decoded gzip size limits/malformed data with synthetic
account headers, explicit identity ranges and encoded media header/byte preservation, a blocked
artwork writer plus concurrent clear, large-page history accounting, copied media
estimates, and startup wait/failure/single-result semantics. Existing assertions
are retained. Navigation tests inject their test dispatcher for new background
page processing; the startup fixture explicitly publishes synthetic readiness.

## Source revision and coverage

All six fixes and their regressions are in `191fe656786fd51133e670cf38eb8323d71f62c5`; the following evidence
commit changes documentation only.

| Finding | Affected phase | Deterministic coverage | Evidence level |
| --- | --- | --- | --- |
| F1 | text transport / media range integrity | provider compression tests; native HTTP budget/range tests; Cinecat ranged bundle; feature tree | fixture/unit + static |
| F2 | transfers / library metadata-control admission | OfflineExecutorsTest, plus unchanged range/promotion tests in the full JVM suite | fixture/unit |
| F3 | process startup / readiness / services | EngineStartupTest and FeedStartupTest; app/instrumentation-source compilation | fixture/unit, no lifecycle device claim |
| F4 | partial speculative extraction / foreground tap | cancelled_waiter_retains_started_extraction_for_foreground; existing invalidation/key/fresh tests | fixture/unit |
| F5 | decoded artwork publication / persistence / clear | blocked writer with replacement client and concurrent clear; existing dedup/restart/decode bounds | fixture/unit |
| F6 | page admission / navigation-history budgets | PageHistoryTest; FeedPageCacheTest; WatchedRecommendationsTest with unchanged navigation assertions and copied-media estimate case | fixture/unit |

## Checks

| Check | Result | Level |
| --- | --- | --- |
| `cargo fmt --all -- --check` | pass | static |
| `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --locked -- -D warnings` | pass | static |
| `CARGO_INCREMENTAL=0 cargo test --workspace --locked` | 449 passed, 0 failed, 26 explicit ignores | fixture/unit |
| `CARGO_INCREMENTAL=0 cargo test --manifest-path vendor/yt-dlp-rs/Cargo.toml --locked --lib http::tests --target-dir target` | 4 passed, including encoded-range preservation and text bounds | fixture/unit |
| `cargo tree -p vidcue-mobile --target aarch64-linux-android -e features -i reqwest --locked` | HTTP/2, gzip, Brotli and deflate present | static |
| `apps/android/gradlew -p apps/android :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest :app:lintDebug` | pass; 320 cases, 318 passed, 2 existing skips, 0 failures/errors; lint pass | fixture/unit |

Rust command logs: `/tmp/vidcue-workspace-complete.log`,
`/tmp/vidcue-clippy-complete.log`, `/tmp/vidcue-extractor-http-verified.log`.
Final Android log: `/tmp/vidcue-android-verified.log` (3m16s).
Incremental compilation was disabled on final Rust checks to fit the host disk;
assertions and the repository test selection were unchanged.

The first workspace run reached provider tests and failed the new range test:
this reqwest build advertises compression even with Range. This prompted explicit
identity headers on byte-oriented requests and disabling transparent decoding on
shared media clients, rather than assuming the documented automatic suppression.
The initial workspace Clippy run passed. A focused follow-up hit host disk
exhaustion; only this worktree's generated incremental/example caches were removed
and inactive generated test binaries had debug sections stripped before retrying.
No parent checkout, source assertion or test budget was changed.

An intermediate Android build captured the production source list before the new
MediaSizeEstimates file but compiled newer tests, producing unresolved references
(`/tmp/vidcue-android-final.log`). A complete subsequent run and the final frozen
source run both passed.

Initial Android run compiled app and instrumentation sources but failed three JVM
cases: missing synthetic readiness in FeedStartup, a real background dispatcher
outside the navigation test scheduler, and a startup test built before the
single-result guard was added. These failures are retained in
`/tmp/vidcue-android-check.log`; no assertion or latency budget was loosened.

## Remaining limits and acceptance

No new emulator, physical-device, live-account/CDN, audible-output or latency
qualification is claimed. JNI ABIs were not rebuilt for device execution in this
source-only investigation. Before device testing, rebuild both ABIs and use only
a disposable Preview installation. Existing AVD software graphics cannot qualify
hardware timings. The 500-ms visible-video, 1000-ms populated-feed and roughly
100-ms music-output targets remain open; maximum practical live download
throughput, integrity/resume and playback coexistence still need measurement.

Cold DownloadService creation can still synchronously prepare storage because
Media3 requires a synchronous manager from its service callback. Normal activity
startup prepares storage on IO; a service-only/process-restart trace is still
required. First-use streaming-cache setup also remains a possible synchronous
cost. Lifecycle/deep-link/service/Keystore acceptance requires actual Android
instrumentation beyond JVM gate tests. Compression's host research showed smaller
wire bodies, **not** improved total latency. HTTP/2 negotiation with real origins
and malformed Brotli/deflate responses need broader transport qualification.

Additional contributors were examined without unsupported tuning: authenticated
Home recommendations and the conditional anonymous topic fallback are unchanged;
category speculation still respects playback/mobile-data checks and can prepare
up to 30 targets. Its detached requests and bandwidth contention need traces
before changing scheduling policy. Audio start thresholds, silent warming,
disabled PCM copies, image/JNI allocation pressure, release R8 and profiles remain
measurement work. No generated or hand-written profile is claimed, no benchmark
variant or signing/version/workflow changes were made.

## For the plan and parity tables

M9: source fixes reduce avoidable transport, transfer-executor, startup, speculative
resolution, artwork-persistence and navigation-accounting costs. Deterministic
regressions and final local checks are listed here; physical/live latency targets,
service-only cold startup and release acceptance remain open.
