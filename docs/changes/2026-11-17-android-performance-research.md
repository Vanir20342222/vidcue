# M9 Android performance investigation

Date: 2026-11-17. Research and implementation brief, not a completion claim.

The owner requested a broad investigation followed by fixes delegated to exactly
one GPT-6.1 Sol agent with high reasoning effort. The targets remain tap to
visible video below 500 ms, process start to populated feed below 1000 ms,
music output around 100 ms, and maximum practical download throughput without
corruption, playback regressions, or reduced media quality.

## Scope and evidence

Inspected application/JNI startup, feed bootstrap/cache/preloading, Compose
navigation and images, native stream resolution/cancellation, Media3 service,
audio processors/output warming, streaming caches, offline download executors,
chunk transport, and build/measurement configuration. Read existing emulator
and physical evidence plus Android, Media3 1.8.0, reqwest 0.12.28 and Tokio
primary documentation/source.

Initial checkout was `4ca8522c680733b89a4df8a83fe7cafc88bad44a`. Fetched main and
fast-forwarded to `06d0479189e720cdda095fd90fa63deae64ce801` (merged UI PR #170),
then checked affected UI call sites again. Findings below apply to that base.
Open PR #146 changes agent rules only; no open source PR overlapped at inspection.
The earlier untracked merged-main validation note was preserved.

Evidence levels below distinguish code-confirmed mechanisms from measurements
and hypotheses. No new physical latency, acoustic-output, or app throughput
measurement was made during this investigation. Previous tablet wireless ADB
refused connections. Its return is required for final acceptance, but it does
not block the source fixes and deterministic regressions described here.

Prior physical results remain relevant diagnostics, not proof about the new UI
base: cold YouTube video exceeded the target while two fully prefetched cases
met it; music output and first Music Home remained above target; public film
startup was dominated by redirects/first bytes. See
[previous measurements](2026-11-15-android-performance-targets.md) and
[merged-main qualification limits](2026-11-16-merged-main-performance-validation.md).

## Findings to fix

### F1 — Missing response compression and HTTP/2 capability (high priority)

**Confirmed build configuration.** Workspace reqwest disables default features;
Android dependencies add TLS but do not re-enable `http2`, `gzip`, `brotli`,
`deflate`, or `zstd`. Verified effective features, including transitive feature
unification, with:

```sh
cargo tree -p vidcue-mobile --target aarch64-linux-android -e features -i reqwest --locked
```

The resulting reqwest 0.12.28 features include TLS, blocking, cookies, JSON,
SOCKS and stream, but neither HTTP/2 nor decompression. This affects provider
HTML/JSON and the Rust extractor, not the separate Android OkHttp media client.
Browser bootstrap reads up to 8 MiB HTML; API responses allow up to 16 MiB.

[reqwest's versioned feature documentation](https://docs.rs/reqwest/0.12.28/reqwest/)
and [manifest](https://github.com/seanmonstar/reqwest/blob/v0.12.28/Cargo.toml)
confirm these capabilities are feature controlled. Connection pooling already
exists through retained clients; constructing a new pool per request is not the
problem here.

**Host-only public probe:** four sequential fresh curl processes requested
anonymous Music Home with the public reject-optional-cookies marker, HTTP/1.1,
and identity/gzip/gzip/identity encoding. No browser session, cookies from disk,
response body, or signed URL was retained. All responses were HTTP 200.

| Order | Requested encoding | Wire body bytes | Total seconds |
| --- | --- | ---: | ---: |
| 1 | identity | 355517 | 0.544534 |
| 2 | gzip | 72513 | 0.554683 |
| 3 | gzip | 48909 | 0.574145 |
| 4 | identity | 341615 | 0.472079 |

This confirms substantially smaller transfers are available. It does **not**
demonstrate faster wall time: gzip was not faster in these four dynamic public
responses, whose contents can differ. Host DNS/TLS, server timing, and small
sample size prevent an app-speed inference. Raw sanitized timing data:
`/tmp/vidcue-public-home-transport-research.json` (local artifact).

**Fix:** enable appropriate text-response decompression and HTTP/2 support while
preserving HTTP/1.1 fallback, proxy/account isolation and redirect rules. Verify
decompressed-body limits, compressed malformed/oversized responses, synthetic
authenticated requests and byte-range media semantics. Cargo feature unification
can affect the extractor too; inspect it explicitly. Do not indiscriminately
compress media range requests or upgrade dependencies as a substitute for a fix.

### F2 — Download transfers can starve library operations (high priority)

**Confirmed scheduling mechanism.** `OfflineStore.java:14` creates one fixed
three-thread `IO` executor. `manager()` passes it to `DefaultDownloaderFactory`
(around line 95); `changedSnapshot()` queues library enumeration onto the same
executor (line 398). Offline play/export preparation and completion bookkeeping
also use it. Default maximum parallel downloads is three, configurable to four.

Media3 1.8.0's
[ProgressiveDownloader](https://github.com/androidx/media/blob/1.8.0/libraries/exoplayer/src/main/java/androidx/media3/exoplayer/offline/ProgressiveDownloader.java)
submits the entire cache-writing transfer to that executor and waits for it.
Three long transfers can occupy every worker until one finishes. Metadata work
then waits behind transfers; selecting four downloads does not provide four
independent progressive transfer workers. This is an availability/latency issue
even when individual range transfers are fast. No new end-to-end device
reproduction has yet been run.

**Fix:** separate bounded transfer execution from metadata/control work, keeping
actual concurrency consistent with the setting and avoiding unbounded thread
growth. Add a deterministic blocked-transfer regression proving metadata and
offline operations progress while transfers remain active. Retain cancellation,
resume, paired audio, range integrity and requirements behavior. Do not merely
increase CDN connections: existing 4-versus-8 tests did not justify that.

### F3 — Application startup does blocking initialization before UI (high priority)

**Confirmed call chain, unmeasured current-device duration.**
`YtuiApplication.onCreate()` synchronously loads JNI and calls `NativeBridge.init`.
`Engine::new` (`crates/vidcue-mobile/src/lib.rs:67`) reads/parses config, library,
account and lyrics, cleans transient files and constructs the HTTP client.
Account opening can invoke Android Keystore. `MediaBridge.attach()` then makes
another synchronous settings RPC and posts `OfflineStore.manager()` to main
before first content is guaranteed. The manager constructs a disk-backed cache.
First-use streaming cache setup is likewise synchronous in `PlaybackCache.cache()`.

[Android startup guidance](https://developer.android.com/topic/performance/appstartup/analysis-optimization)
recommends tracing main-thread I/O, binder calls and work that can wait until
after first display. Moving a call to a coroutine is insufficient if the main
thread subsequently waits for its lock/result.

**Fix:** establish a single explicit engine-readiness/error gate; move eligible
initialization off main while preserving account/config/history prerequisites,
deep-link/service startup, proxy application before requests, and failure UI.
Separate disk/cache preparation from main-looper manager commands. Media3's
[DownloadManager threading contract](https://github.com/androidx/media/blob/1.8.0/libraries/exoplayer/src/main/java/androidx/media3/exoplayer/offline/DownloadManager.java)
must be retained; blindly constructing/using it on arbitrary IO threads is not a
safe fix. Add startup trace stages and lifecycle/error regressions. Assess local
settings/theme/account/history RPC batching after the readiness gate is sound.

### F4 — Tapping an in-progress visible-card preload can discard its work (high priority)

**Code-supported cancellation race; deterministic reproduction required.**
`ComposeHost.kt:1307` keys the visible-card effect on `focused`; it waits 400 ms
then calls `prepareVisible`. `prepareVideoSurface()` sets `playerOpen=true`,
which cancels that effect. Coroutine cancellation calls `NativeBridge.cancel`.
Engine request cancellation drops `resolve` while its `spawn_blocking` extraction
may already be running. The resolver flight lock and cache insertion belong to
the dropped outer future, not to the blocking worker.

[Tokio documents](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)
that started blocking tasks cannot be aborted. Consequently, the old worker can
continue consuming a reserved extraction slot while the foreground request must
resolve again and the old result never enters the lease cache. Completed
prefetch timing tests do not cover a tap halfway through extraction.

**Fix:** reproduce using a controlled blocking resolver, then retain/promote a
same-key flight so a tap can use its result. Bound detached work and preserve
epoch invalidation, account/quality/language keys, errors, stale-result rejection
and foreground capacity. Cancellation must still end superseded UI requests and
must never commit history or queues merely because a preload finishes.

### F5 — Artwork waits for optional disk persistence (medium/high priority)

**Confirmed critical path.** `images.rs:215–235` decodes an image and inserts it
into memory, but then awaits PNG encoding plus `atomic_write` before returning
the image to JNI/Compose. `atomic_write` fsyncs the file and its parent directory.
The first network image, and every 32nd subsequently, also awaits a directory
scan/prune. Thus a valid bitmap cannot appear until optional cache maintenance
finishes, and its fetch permit remains occupied. Existing size bounds and
off-main decode are good, but do not remove this wait.

**Fix:** publish decoded images before bounded asynchronous cache persistence
and pruning. Preserve memory/disk bounds, atomic replacement, deduplication and
cache-clear semantics: an old delayed write must not refill a freshly cleared
cache. Test with an intentionally blocked writer and a clear/write race. Avoid
an unbounded task per thumbnail or removing durability from user data.

### F6 — Navigation serializes entire history on the main thread (medium priority)

**Confirmed repeated work.** `RustRepository.kt:691–696` repeatedly computes
every retained card's JSON length to enforce the navigation-history byte budget.
It repeats that scan while evicting entries. `FeedPageCache` also computes JSON
size when inserting results on its main-confined coroutine. With large pages,
this produces allocations and UI-thread work directly on category navigation.

**Fix:** compute/cache immutable page byte estimates once, maintain incremental
history totals and preserve the existing entry/byte limits. Avoid memoizing a
mutable JSONObject without an invalidation rule. Add deterministic large-page
tests and verify account clearing/back navigation/eviction. Android's
[Compose guidance](https://developer.android.com/develop/ui/compose/performance/bestpractices)
supports removing repeated expensive calculations from UI paths; it does not
justify indiscriminate `remember` or `@Stable` annotations.

## Additional contributors requiring measurement or careful qualification

1. **Anonymous first feed has a conditional extra request.** The default tab is
   Videos/Home. If anonymous Home is empty, `ComposeHost.kt:1184` waits for that
   result and then navigates to the existing public topic source. That causes an
   extra network request and a Compose handoff before populated content. Measure
   this branch and consider moving the established fallback into the repository
   request flow. Do not silently replace authenticated recommendations or assume
   every anonymous Home response is always empty. An empty response is not
   successful populated-feed timing.
2. **Speculation competes with interactive loading.** Category preparation starts
   immediately after a page load and can visit up to 30 targets with concurrency
   two. Visible-card preparation then resolves up to three candidates serially
   but byte-prefetches only the first. Existing foreground extraction slots help;
   they do not reserve radio bandwidth. Measure tap-during-prefetch and throttle
   speculation during playback start. The actual ViewModel cache overrides the
   helper defaults to **64 entries/8 MiB**; a claim that 30 requests necessarily
   overflow a 12-entry cache would be incorrect. Keep network/metered preferences.
3. **Cold live playback remains network limited.** DNS, TCP/TLS, bootstrap/player
   API requests, challenge solving, manifests, CDN first byte and film redirects
   are separate costs. Existing pooling, leases, byte ranges and prepared sample
   queues already help. Add missing stage evidence before replacing transports.
   A source server that takes seconds to return the first byte cannot satisfy
   a universal 500 ms cold request through a UI-only change.
4. **Music output is not just ExoPlayer's playing state.** Current load control
   requires 100 ms of buffered media to start; AudioTrack requests roughly 40 ms
   start threshold, and the device output may need to wake. These are media-buffer
   quantities, not fixed additive wall-clock delays. The
   [Media3 load-control source](https://github.com/androidx/media/blob/1.8.0/libraries/exoplayer/src/main/java/androidx/media3/exoplayer/DefaultLoadControl.java)
   and [AudioTrack API](https://developer.android.com/reference/android/media/AudioTrack#setStartThresholdInFrames(int))
   explain those gates. Lowering them blindly risks underruns. Validate audible/
   playout start, continued playback, underruns and route changes on hardware.
5. **Audio warming and processing have costs.** `AudioOutputWarmth` holds a silent
   stream for up to two minutes while browsing music/radio. Battery/thermal cost
   has not been qualified. EqualizerProcessor and AudioTap still copy PCM buffers
   when their optional processing is disabled. Measure before changing activation
   because runtime EQ/visualizer toggles and sink reconfiguration must still work.
6. **Image memory spans Rust and Java.** Sized image clones, JNI pixel arrays,
   bitmap creation, and the two memory caches can create transient allocation
   pressure; four fetch/decode slots and explicit limits already exist. Profile
   allocations/GC with large artwork and fast scroll; do not infer a leak from
   cache size alone. Hardware video decode/surface cost needs a real device;
   SwiftShader emulator timings cannot qualify it.
7. **Production-like measurement is missing.** Current release has R8 disabled;
   there is no app-owned baseline/startup profile or Macrobenchmark module found.
   Debug Compose measurements are useful diagnostics but not final release
   performance. Android recommends a
   [non-debuggable, production-like benchmark variant](https://developer.android.com/topic/performance/benchmarking/macrobenchmark-overview)
   and supports [profiles](https://developer.android.com/topic/performance/baselineprofiles/overview)
   to precompile relevant paths. Add an isolated disposable benchmark variant if
   useful; do not modify stable signing/versions or enable untested minification
   across JNI/reflection. Do not hand-author a profile and call it measured.

## Implementation and acceptance brief

One requested Sol/high agent should implement F1–F6, first reproducing the
cancellation/scheduling issues and validating threading assumptions. It should
investigate the additional items and fix those supported by tests/traces, while
recording any remaining hardware-dependent experiments. Keep source changes
focused; do not redesign the UI from merged #170. Avoid global buffer reductions,
quality reductions, blanket retries, unbounded warm-up work or fabricated caches.

For each fix record the exact revision, affected phase, regression test, and
evidence level. Required verification includes relevant deterministic Rust and
JVM regressions, Kotlin/Java and instrumentation compilation, lint, and fresh JNI
builds for changed Rust before device claims. Follow repository PR check rules.
Run only one Gradle build at a time. Use a separate disposable Preview/benchmark
install; preserve real accounts, downloads and the stable installation.

Physical acceptance must cover true process-cold/empty-cache, warm process,
resolved-only, byte-prefetched and sample-prepared cases separately, with repeated
samples and median/tail values, not only the fastest case. Exercise taps during
prefetch, multiple active downloads, large saved libraries, scrolling, lifecycle
and account/network changes. Measure populated draw separately from artwork
completion; visible video separately from decoder-ready; audio playout separately
from playing state. Downloads need integrity, cancellation/resume, aggregate
throughput and playback coexistence, including representative live CDNs.

This investigation finds actionable implementation costs and unverified risks.
It does not establish that fixing them will meet every target, and it does not
change any latency assertion or test budget.
