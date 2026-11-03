# Android subsecond playback work (M9)

Target: less than 1000 ms from a video tap to both visible video and active
playback, retaining the selected quality, native Rust extraction and existing
account/proxy/data-usage preferences. Cached and unprepared starts must be
reported separately. This is a performance candidate, not a universal latency
promise: live-service and physical-device measurements remain necessary.

Initial base: released main `2baa9b6`. The accepted UI overhaul reached main
as `b6a76a2` through PR33 during this work; it is now integrated into the
performance feature branch, including its sharded native CI. PR32 itself must
remain unmerged, per the owner. Desktop and durable offline state must stay
compatible. Earlier validation below refers to the explicitly recorded old bases.

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


## Reused-connection recovery

A deterministic local socket probe reproduced a transport regression: with
connection recovery disabled, a server closing the second request on a reused
connection produced IOException after two requests; enabling OkHttp connection
recovery completed the same GET on a fresh connection (three total requests,
about 3 ms recovery in that host sample). HTTP429 with Retry-After still made
exactly one request. The candidate enables connection recovery and adds both
a JVM test against the production client builder and an Android Media3-adapter
case. This prevents passing stale-socket failures to Media3's slower load-retry
backoff; it does not retry HTTP429 or change extraction cooldowns.


## Revision 4 and indexed observer cost

Source `df39cbc`, PR merge `c31c62d`: run
[41473132687](https://github.com/Vanir20342222/ytui/actions/runs/41473132687)
has **46 pass/one fail/one optional public skip** out of 48 cases. Socket recovery,
HTTP429 single-request behavior, idle-player lifecycle and original regression
cases pass. Strict progressive card taps pass: cold usable696/window924 ms;
prefetched usable664/window884 ms. The sole failure is prepared indexed
usable823 ms but window confirmation did not complete by 1000 ms. This remains a
valid failure. Artifact `13157722013` retains all evidence; Rust checks and secret
scans pass. APK artifact `13156646982` has SHA256
`8cd3865a918bc004b08a6b514ab8abe8f8ab7e4f42df647eddfa62baaa94fbd5`
and passed signature, both-ABI, 16 KiB alignment and notices checks.

The indexed test previously requested Window PixelCopy/PNG reads throughout
decoder startup, including black frames. The follow-up waits for the existing
real output+playing signal before GPU readback and records request count/observer
time. It still requires actual RGB Window pixels, including overlays, before
1000 ms from the same original tap. It does not reset clocks, count decoder-only
frames or change quality. A late output signal still fails, and failure screenshots
are retained. This is a benchmark-observer correction, not a production speed
change; the cold/prepared progressive stress checks remain unchanged. A fresh
native run must qualify it. The physical Tab S9 request uses the same production
code from `df39cbc`; its live acceptance remains pending.

[Media3 surface guidance](https://developer.android.com/media/media3/ui/surface)
recommends SurfaceView for frame timing/power, describes Android14 attachment
lifecycle behavior and a Compose SDK34 synchronization workaround. No surface
switch is included here. Such a candidate would also require compositor-level
pixel verification because Window PixelCopy alone does not include a separate
SurfaceView layer, plus fullscreen/controls/gesture regression qualification.


## Revision 5: observer correction did not qualify the deadline

Source `7af124c`, merge `ea1791f`, Android run `41491374334` retains another
strict failure: prepared indexed usable677/window1006 ms, one readback request
with 328 ms observer time. Cold progressive usable924/window1003 ms also failed.
These are valid misses, not passes based on the earlier frame callback. Full
failure pixels show decoded video after the deadline. Artifact `13163869920`
retains the evidence. Physical YouTube under one second remains open.

## Early video-surface preparation (new candidate)

Video selection now publishes a scoped preparing intent and opens/binds its
real native surface while Rust resolves, overlapping layout/control/surface
creation with extraction. Intent and accepted native state share one PlayerState
emission, so handoff does not briefly recreate a video view in the music branch.
An old/empty native snapshot cannot erase the preparing selection. A black loading
overlay blocks old controls until the matching native source/token is accepted.
Covered native views cannot complete visible-frame timings; revealing an already
rendered current source schedules the normal view-frame guard. No placeholder or
old frame can satisfy a new timing token.

Cancellation/failure cleanup is token-scoped, preserves a newer selection and
restores the prior panel state without reopening a minimized panel. A bounded
five-second service-handoff wait reports an actual start failure rather than
leaving preparation stuck if the OS refuses the service. Audio, preview and
download paths keep their existing presentation. A new native functional case
blocks actual foreground resolution, checks prebinding/no frame acceptance,
minimizes/reselects and verifies view identity survives real playback handoff.
A separate case checks invalid resolution restores browsing. These deliberately
blocked cases are not latency qualification.

The prepared indexed gate now dispatches the same early-surface UI action before
its already-resolved stream. All preparation/dispatch still happens after the
original handoff timer begins, without ActivityScenario idle waits in that
interval. Actual Window RGB pixels and active playback must still complete
before 1000 ms. New native and physical qualification is required; no speed
claim follows from local compilation.

Cancelled Kotlin RPC futures are purged immediately from their bounded executor
queue. A cancelled queued selection must not exhaust admission capacity while
older native calls finish. The new preparation test makes twelve rapid selections
with both real foreground workers held, then requires the latest actual card
selection to resolve/play after release; no rejected stale queue entries may
erase that selection.

The accepted-source snapshot includes its startup token, so UI handoff does not
rely on a service still being alive when an already-captured snapshot is applied.
An explicit token-scoped start-failure event survives immediate service teardown;
only the matching preparing intent is cleared/restored. A native negative case
rejects a missing stream URL without network/decode and verifies error delivery
and UI cleanup. Newer selections ignore an old request's failure.

The latest playback intent is retained independently of a preparing panel that
may already have been cleared by an accepted snapshot. That preserves scoped
failure delivery during immediate service teardown, ignores old failure events
after a newer selection, and keeps current audio/preview start errors visible.
Account changes invalidate that intent. Foreground-service start rejection in
the platform bridge uses the same token-scoped failure path.

The underlying browse layer is transparent while an opaque full-screen video
panel covers it, retaining composition/scroll/artwork state while skipping
covered drawing. The full player remains a separate sibling layer. Music
transition behavior is unchanged; native/fullscreen/window-pixel regression
qualification must confirm the layer boundary.


## Integration with accepted UI main

Main advanced to `b6a76a2` (PR33) while this feature was running. Integration is
on the performance branch only; PR32 remains draft/unmerged. It preserves the
accepted watch-page layout, native collapse controls, expanded UI/contrast tests,
JSON snapshot deduplication and four-shard exact-APK device workflow. Pending
video intent is combined with deduplication atomically. A visible white minimize
action and title drag area remain available over the loading cover because the
accepted watch page puts its normal minimize control inside the native view.
Both direct and sharded public runners retain explicit Best/1000 ms arguments
and all timing/failure artifacts. New qualification is required against this
integrated source; previous main/branch results cannot qualify the merge.


## Integrated revision 7 and rejected foreground-service start

Source `2f5462a`, merge `1b0a69d`, run
[41500209162](https://github.com/Vanir20342222/ytui/actions/runs/41500209162)
built the exact app/test APKs and passed Rust checks and both secret scans.
Local integrated checks passed 23 JVM tests, lint, APK builds and six publication
boundary cases. APK `13167863626` has SHA256
`dc7d9d553f73e7487807c17b15bbef8549d3f6a251f055d970422e8cf6d7209d`.
It is retained evidence, not the recommended candidate after the crash below.

Shard 1 passed all nine cases, including proxy/header/account isolation, native
transport contrast and indexed cold playback. Shard 0 passed rapid cancellation,
early native-view creation and retained view identity through actual handoff.
Strict performance failures remain: paired indexed handoff had no visible/playing
completion by 1000 ms (handoff230, decoder676 ms after handoff); cold progressive
usable964/window1001 ms missed the one-second pixel deadline. No threshold or
clock changes are applied. Shards 0/1/3 artifacts are respectively `13169221249`,
`13169096406` and `13170000163`. Shard 2 (`13169541190`) also passed, including
the original two-second card checks and expanded catalog/theme UI coverage.
Raw runner results total 48 passes, two latency failures, one crash failure and
one opt-in public skip; five further declared cases did not run after the crash.
The 57-case suite therefore remains incomplete and failed.

The rejected-service functional test found an actual Android crash:
`ForegroundServiceDidNotStartInTimeException` on teardown. Calling stopSelf does
not discharge an outstanding startForegroundService obligation. The fix validates
required media/stream URLs before the bridge starts a service, without disturbing
current media. Service play also validates before mutating state. For requests
rejected inside the actual service, the exceptional path briefly fulfills the
foreground obligation, then removes the notification and stops using its start
ID. Successful playback retains Media3's notification lifecycle. Failure events
use the rejected request's token rather than a previous source token. The native
regression now exercises both bridge preflight and a direct malformed FGS intent,
requires clean UI/service teardown and checks JNI remains available. Fresh native
qualification is required; compilation alone does not qualify this fix.


## Idle native-view preparation candidate

The integrated failures also recorded 230–242 ms between resolution and media
handoff. A further candidate prepares one native PlayerView on an idle UI turn
after the first browsing frame. The browsing activity owns it; it remains
detached and unbound, creating no decoder surface or media/session/history work.
The first video view claims it once. Normal construction remains the fallback;
fullscreen cannot borrow another activity's view. Background/teardown and
configuration changes discard an unused view, preventing retained activity
state or stale dimensions/fonts. This prepares widgets, not decoded media.

The actual first view claim/construction duration is reported as
`view_inflate_ms`; the original tap, first genuine frame, active playback and
window-pixel deadlines are unchanged. Native lifecycle coverage requires no
parent, bound player, attached window, available TextureView surface or playback
notification while idle, followed by cleanup. The blocked-resolution UI test
requires the first actual card tap to reuse the activity's prepared native view,
then retains its existing rapid-selection/handoff checks. Local and fresh native
qualification must establish this candidate; it is not yet a speed result.


## Revision 8 qualification

Source `017c240`, merge `c394278`, run
[41502679721](https://github.com/Vanir20342222/ytui/actions/runs/41502679721)
finished all **57 declared cases: 52 pass, four fail, one optional public skip**.
The direct rejected-FGS and bridge-preflight checks pass without the prior crash;
all three preparation/cancellation/failure-cleanup tests pass, including the case
aborted by the previous crash. Rust checks, secret scans and APK inspection pass.

Strict misses remain: prepared indexed usable715/window1000 ms (one observer
request,271 ms), cold card usable799/window1001 ms. Original two-second gates
remain separate. Two catalog cases also failed the 15-second decoded-cover
check: supplemental sheets and music motion. Failure pixels show an actual
placeholder among decoded neighboring covers, not merely a screenshot timing
assumption. The cause is still open. Failure-only synthetic-server per-cover
request counts and a bounded native decode probe now retain further diagnostics;
they execute after the failed assertion and cannot qualify or retry it.

Artifacts for shards0–3 are `13170227451`, `13170326828`, `13169278343` and
`13169178642`; APK `13170925283`. The later idle-view candidate passed local APK
builds, lint and 23 JVM tests but still needs exact-source native and physical
qualification. No live subsecond claim follows from these results.
