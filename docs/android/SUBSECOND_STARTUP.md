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
A separate cold/prepared-card test includes the actual card tap, Rust lease lookup,
player/service/UI handoff, playing state and window-pixel readback under 1000 ms.
The pre-existing 2000 ms cold/indexed/HLS regression gates stay unchanged.

Local debug/instrumentation APK builds, 21 JVM tests, lint and diff checks pass.
The first exact-source emulator run is recorded below. No physical subsecond
result is asserted.

## Revision 1 qualification and follow-up

Source `91b9a06`, workflow merge `1b4704a`: Android run
[41463430970](https://github.com/Vanir20342222/ytui/actions/runs/41463430970)
has **43 passing, one failing and one explicitly skipped public-YouTube test**
(JUnit XML counts). The six new socket/cache tests passed. Rust formatting,
Clippy, workspace tests and real-media checks passed in run `41463430996`;
secret scans passed. The APK and native evidence are retained as workflow
artifacts `13151050462` and `13153490015`.

The new prepared-card case had actual visible/playing startup at **734 ms**, but
window pixel confirmation at **1020 ms** failed the strict <1000 ms gate. Original
2000 ms tests passed: progressive cold usable786/window1028 ms, indexed cold
usable1164/window1450 ms, HLS cold usable703/window976 ms. Indexed startup remains
above the new goal. These are loopback media, not live YouTube measurements.

The observer waited for AndroidX ActivityScenario UI idleness after the tap:
`onActivity` calls `Instrumentation.waitForIdleSync` before scheduling work.
The follow-up binds the observer before tapping and posts PixelCopy requests
directly to the main thread while frames render. It retains the original tap
clock, actual Window (including overlays), focus/bounds checks, RGB-frame check,
playing requirement and unchanged <1000 ms deadline. Both cold and prefetched
cards run this gate, including offline lookup. Existing 2000 ms observers remain
unchanged. This corrects observer scheduling; it does not qualify faster app
playback until a fresh run passes.

Readiness now also rechecks cached ranges after LRU eviction, including the
notification status and duplicate-prefetch path. Publication is guarded against
an old cancelled preload. The paired-stream socket test removes its own cached
ranges and checks that readiness becomes false; quality mismatches also reject
readiness. The streaming cache still has bounded LRU storage: bytes are not
pinned and only the first settled visible candidate receives byte warming.

Follow-up local debug/instrumentation APKs, 21 JVM tests and lint pass. Fresh
exact-source emulator qualification is pending.


## Paired indexed and transport-control follow-up

A separate strict one-second test warms both real indexed MP4 tracks, then checks
active playback and actual window pixels from player handoff, verifies that no
additional startup requests are made, and exercises continued playback/seek.
This specifically qualifies prepared source handoff; the separate progressive
card test includes the UI tap and Rust lookup. Neither replaces live acceptance.
The loopback CDN now declares `Connection: close`, matching its actual one-request
per-socket behavior rather than advertising an implicit persistent connection.

Next/Previous selection now releases speculative byte sockets before offline
lookup/extraction, as direct card taps already do. A shared in-flight Rust
resolution remains joinable and completed cache spans remain available. Empty
selection does not interrupt warming. Full native qualification follows.

Resolved descriptors retain their producer account/network epochs through JSON
copying and worker submission; a concurrent change cannot relabel stale work
with the new scope. Native stale-descriptor coverage includes both epochs.

The optional public workflow now accepts an explicit quality and 1000/2000 ms
budget (defaults: Best and 1000 ms). Its cold native extraction and visible-view/
active-playback timings are retained even on failure, without media identifiers
or stream URLs. It also checks three seconds of continued decode. This check
uses real service traffic but callback-based view evidence; the loopback RGB
tests and physical acceptance remain distinct. Raw instrumentation without the
new arguments retains the original 720p/2000 ms regression mode.


## Revision 2 retained results and foreground preparation

Source `6bbdb8e`, PR merge `78ef261`: run
[41467639893](https://github.com/Vanir20342222/ytui/actions/runs/41467639893)
failed three cases (41 pass, three fail, one public skip). Indexed cold usable
startup was 4013 ms; strict cold card usable973/window1002 ms missed the actual
pixel deadline; HLS usable1885/window2005 ms missed its original 2000 ms window
deadline. All failures remain valid; no timeout/clock/quality waiver is applied.
Artifact `13155320671` retains JUnit, timing files, pixels and logs. Logs contain
substantial frame stalls; these results are not a universal speed qualification.

The separate same-source public run
[41467956874](https://github.com/Vanir20342222/ytui/actions/runs/41467956874)
had 43 passes/two failures. Strict loopback cold usable573/window915 ms and
prefetched usable433/window524 ms passed. Live extraction failed DNS lookup,
and a catalog thumbnail-pixel wait timed out. Artifact `13155110536` retains
these failures; passing synthetic timings cannot replace the blocked live result.
Rust checks passed in run `41467640173`.

The next candidate initializes the idle Media3 service while the activity is
foreground. No media is prepared, played or added to history/queue, and the warm
command does not register a session/notification. The ticker starts only with
actual playback. Leaving the foreground releases an empty service using the
start-ID guard, preserving an actual later play command and active media.
`player_at_tap` records initialized/cold infrastructure separately from resolution
and byte-cache readiness. The strict cold-card test still forces a fresh service;
it is not changed into a prewarmed-service test. A separate lifecycle test checks
idle preparation, absent playback/notification and release on activity close.
This requires new native qualification and a physical Best-quality report.
