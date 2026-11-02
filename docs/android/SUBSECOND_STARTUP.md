# Android subsecond playback work (M9)

Target: less than 1000 ms from a video tap to both visible video and active
playback, retaining the selected quality, native Rust extraction and existing
account/proxy/data-usage preferences. Cached and unprepared starts must be
reported separately. This is a performance candidate, not a universal latency
promise: live-service and physical-device measurements remain necessary.

Base: released main `2baa9b6`; separate from the unmerged UI candidates in PR31
and `claude/app-ui-ux-redesign-739q3a`. Owner's instruction remains PR only,
**do not merge**. Desktop and durable offline state must stay compatible.

## Evidence and likely costs

The owner's mobile.8 Best-quality Tab S9 sample recorded five cold usable starts
at 1115–1752 ms and one prepared start at 632 ms. Resolution took 363–568 ms in
those cold attempts, while first network bytes and decoder/surface startup used
the remaining budget. Existing speculative resolution helps only settled rows;
byte warming currently handles one large video prefix before its paired audio,
and does not explicitly prioritize remote initialization/index ranges.

## Work and acceptance

1. Use a shared modern HTTP transport for media reads/prefetch, preserving proxy
   selection, timeout/redirect rules, bounded cache and request-specific headers.
2. Warm indexed initialization/index ranges and audio/video startup bytes with
   bounded parallelism; retain immediate foreground priority, cancellation,
   account/network/quality scoping and metered-network restrictions.
3. Measure codec initialization durations separately from tap-relative events;
   reduce initial buffer delay only while retaining rebuffer protection.
4. Verify actual byte reuse, HTTP connection behavior, proxy routing, scoped
   cancellation and real first-frame/playing output. Keep existing 2000 ms
   regression checks and add an explicit 1000 ms performance qualification;
   retain failures rather than changing clocks, quality, frames or thresholds.
5. Publish only an unmerged PR. A physical Tab S9 report is needed to qualify
   real YouTube under 1000 ms; fixtures do not establish that result.

## Research

- [Media3 network stacks](https://developer.android.com/media/media3/exoplayer/network-stacks):
  built-in HTTP is usually HTTP/1.1; HTTP/2 or HTTP/3 and sharing one client/engine
  can improve adaptive CDN streaming. OkHttp supports HTTP/2 with a small APK
  footprint and explicit redirect/proxy controls; HttpEngine supports HTTP/3 on
  recent Android but requires matching the app's routing/redirect policy.
- [Media3 preload manager](https://developer.android.com/media/media3/exoplayer/preloading-media/preloadmanager):
  prioritizes likely next items and can prepare sample queues before selection.
  Byte caching and a prepared decoder are different states; report them honestly.
- [Media3 downloading/caching](https://developer.android.com/media/media3/exoplayer/downloading-media):
  foreground and preload must share cache identity and avoid duplicate downloads.

Implementation and exact benchmark results will be added as they are verified.

## Candidate implementation

Media reads and byte prefetch share OkHttp's HTTP/2-capable connection pool.
Client selection happens on each new request, so a factory retained by a playing
media source still honors a subsequent proxy/account epoch. Headers remain
request-local; automatic cookies and protocol-changing redirects are disabled.
The existing eight-second connect and twelve-second read timeouts remain.
Optional client/TLS initialization runs off the UI thread before the first tap.

The two selected audio/video tracks warm concurrently, with at most two byte
workers. Indexed initialization and index ranges precede bulk prefixes, including
indices outside the file prefix. Metadata counts toward the existing indexed
byte budgets (1 MiB video, 192 KiB paired audio, 384 KiB audio-only). Only selected
resource URLs can enter the plan. HLS keeps its existing bounded playlist/segment
path. Completion verifies actual cached spans; a successful network read alone
cannot claim byte readiness. Foreground taps cancel both CacheWriter work and
its active HTTP calls, while retaining a shared in-flight Rust resolution.

Visible-row preparation can hand its already-resolved descriptor directly to
byte warming, guarded by account/network epochs. Quality selection, lease TTLs,
resolution ownership, history and queue policy are unchanged. Initial playback
buffer decreases from 250 to 100 ms; rebuffer threshold 750 ms and 5–20 second ongoing
buffer policy remain. No quality downgrade or invisible frame counts as success.

The local report now targets **<1000 ms usable start**, distinguishes resolved
metadata from `warmup_bytes_at_tap`, and includes actual codec initialization
durations alongside the older tap-relative codec events. New native tests use
real loopback sockets to check pool/header isolation, proxy changes, cancellation,
parallel paired warming, tail-index cache reuse and rejected stale descriptors.
A separate prepared-card test includes the actual card tap, Rust lease lookup,
player/service/UI handoff, playing state and window-pixel readback under 1000 ms.
The pre-existing 2000 ms cold/indexed/HLS regression gates stay unchanged.

Local debug/instrumentation APK builds, 21 JVM tests, lint and diff checks pass.
The exact-source emulator run is pending. No physical subsecond result is asserted.
