# Android latency targets and first-content scheduling (M9)

- **Date:** 2026-11-15
- **Milestone:** M9
- **Platforms:** Android
- **Status:** physical-device and live verified on one Galaxy Tab S9; video (prefetched) and feed targets met, music and film targets not met (see "Physical device and live results")

The owner requested video usable startup below 500 ms, first populated feed below
1000 ms, music around 100 ms and maximum practical download throughput. Cold,
resolved, byte-warmed and offline starts must remain separately identifiable.
These are targets, not achieved performance claims.

The theme picker catalog no longer gates first-feed scheduling. Active palette,
account, settings and watched-history initialization remain prerequisites. The
startup regression holds theme names pending until the selected Music feed is
populated, then verifies their eventual publication. This Kotlin change awaits
Gradle execution.

The bounded local playback timing report now marks video starts against 500 ms
and audio against 100 ms. Audio measures the player signal, explicitly not
acoustic output. Existing instrumentation gates were not weakened or changed.

Environment setup is in progress outside Git: Java 21, Rust 1.99, FFmpeg,
Android SDK/NDK and an API 35 x86_64 emulator image. The host exposes KVM; the
current user received access to that device for the authorized emulator work.
T3 device integration reports disabled; local adb/emulator tooling is the path.

Checks: direct javac/JUnit execution of StartupTimingsTest passed all seven
tests; git diff --check passed. No APK, emulator, physical, live-service, feed latency,
audible-start or throughput success has yet been established by this change.

An opt-in `MusicLatencyTest` now collects three byte-cold and three byte-warm
synthetic audio starts, retains the timing report even on failure and requires
all six selection-callback-to-playing results below 100 ms. It also checks
continued playback. Run with instrumentation argument `vidcue_music_latency=enforce`.
This is additional measurement coverage, not compiled/runtime qualification yet;
it neither counts as an actual touch measurement nor an acoustic-output test.
The downloaded emulator's `-accel-check` confirms KVM is installed and usable.

Shared-engine check: `RUSTUP_TOOLCHAIN=1.99.0 cargo test -p vidcue-mobile --lib
--locked` completed with 51 passed, 0 failed and 2 explicit ignores. This is
offline backend evidence, not an Android timing result. Android SDK Build Tools
35 installed successfully; platform, system image and NDK preparation continues.

A new identifier-free `FeedStartup` log records process-start-to-first populated
grid draw once per process. Loading/empty grids and the grid's own entry fade
cannot complete it. It records synthetic/provider scope and explicitly excludes
artwork decode; window/compositor visibility still needs device observation.
The process-start clock includes engine initialization. The eight pure Java timing
checks pass together. The new Compose hook still awaits Android compilation.
Synthetic MP4, audio, radio, HLS and indexed-DASH device fixtures were generated
successfully using `tools/android/test-fixtures.sh` (outputs remain under target).

Gradle completed `compileDebugKotlin`, `compileDebugJavaWithJavac`,
`compileDebugUnitTestKotlin` and `compileDebugUnitTestJavaWithJavac` successfully,
including the feed draw hook and startup changes. Test execution/dependency
resolution remains in progress. The music instrumentation source is not yet
compiled by those tasks.

Host qualification caveat: during setup the system load reached 49 with other
build activity present. Device latency measurements must record host load and
separate contended runs from representative qualification. Setup processes remain
live and their downloads continue; no timing success is inferred from waiting.

Music measurement now additionally requires Media3's AudioTrack position-
advancement callback and active playback. The original active-playback timings
remain separately available. Callback observations use elapsed realtime and reject
replaced timelines; no physical/acoustic output claim follows. The updated pure
Java timing tests pass 9/9. Android recompilation/runtime qualification is pending.

The API 35 Google APIs image download failed with a connection reset after about
699 MiB. SDK Manager removed the partial archive. A direct resumable download of
the same official archive is now in progress outside Git; repository metadata will
be checked before use. The NDK and Gradle dependencies remain live. Releasing this
task's idle Kotlin compiler daemon reduced observed host memory pressure.
The official Google image repository identifies the replacement archive as
`x86_64-35_r09.zip`, 1738815903 bytes, SHA-1
`0103e6dab21290c4b9d16550a3ce99476f884eef`; validate before extraction/use.

Resource recovery: the first Gradle test invocation retained about 1.3 GiB while
host memory/full I/O pressure remained severe and no test worker/result appeared.
The invocation is being cancelled and repeated with lower JVM memory limits,
retaining compilation/dependency caches and the unchanged test. This superseded
invocation is not recorded as passing.

NDK 27.2.12479018 installation completed. An x86_64-only native Rust/JNI build
and notices generation is now running with one worker for the disposable emulator.
This is a test-APK configuration, not both-ABI or release qualification. The
replacement Gradle invocation uses a 768 MiB heap, one worker and in-process Kotlin
compilation. The latest pure timing tests (including the audio-output target flag)
pass 9/9. APK packaging and device performance remain unverified.

The existing actual-window-pixel/active-playback cases now accept
`vidcue_video_latency=enforce` to enforce 500 ms, and log their exact budget.
The previous 1000 ms/advisory modes retain their thresholds. This mode is not yet
compiled or run. The complete official emulator archive downloaded successfully;
its size/SHA-1 verification is running before extraction. Shared agent cgroup
memory.high is approximately 5 GiB, explaining the observed reclaim/I/O stalls
when simultaneous builds exceed it; no shared memory limits were changed.

The tool runtime became unavailable during setup. After recovery, kernel journal
confirmed a global OOM kill of Gradle PID 96299 at 15:19:14; the replacement feed
regression invocation therefore failed without a test result. It is not counted
as passing. JNI Cargo compilation survived and continues. Further Gradle work
will be serialized with JNI compilation to limit memory. The earlier image-hash
process is gone with its tool output unavailable; verification is repeated into a
durable local checksum log before extraction, without restarting the download.

The official image passed size and SHA-1 verification (durable checksum output:
`/tmp/vidcue-image.sha1`) and extraction completed. A disposable Pixel 7 API 35
AVD is being created to match the CI device profile. It will not boot alongside
memory-intensive compilation. This remains setup evidence, not device latency.
AVD creation succeeded: `vidcue_perf_m9_20261115`, standard Pixel 7, API 35,
x86_64, 1080x2400 at 420 dpi, two guest cores and default 1536 MiB guest RAM.
No frame-size or quality reduction was made to satisfy latency. It has not yet
booted or run the app. Native build remains live.

After the runtime handoff, both the original Cargo PID and all rustc children
were absent, its log had no success/footer, and no JNI library existed. The
incremental x86_64 build is resumed into a new durable log and exit-status file;
this does not count the interrupted attempt as passing. Verified image and AVD
configuration remain present. No emulator app measurement has run yet.

Continuation recovered the prior performance thread and preserved its working
changes. The first combined Gradle JVM/instrumentation compile attempt in the
continuation was cancelled (exit 130) after repeated tool-runtime replacements
and severe memory pressure; it reached resource processing but produced no test
result. The surviving JNI build also disappeared without a library or success
footer. Neither attempt passes. Native compilation is now running alone via
`YTUI_ANDROID_ABIS=x86_64 YTUI_BUILD_JOBS=1 YTUI_ANDROID_SKIP_GRADLE=1
 tools/android/build.sh`, with durable output at
`/tmp/vidcue-performance-native.log`. Gradle and the performance AVD will follow
sequentially. Another UI thread's emulator use was coordinated explicitly.
Existing downloads already use four bounded parallel CDN range connections;
throughput must be measured before changing concurrency or claiming improvement.

Continuation verification: direct javac/JUnit re-execution of both timing classes
passes 9/9. `run-device-tests.sh` now forwards opt-in
`YTUI_ANDROID_VIDEO_LATENCY=enforce` and `YTUI_ANDROID_MUSIC_LATENCY=enforce`
to the matching instrumentation arguments in both APK and Gradle paths, and
collects `music-latency-timings.txt` on exit. Defaults retain existing regression
behavior. `bash -n tools/android/run-device-tests.sh` and `git diff --check`
pass. These runner changes have not yet run against a device.

The serialized native build completed successfully (exit 0), producing the
x86_64 JNI library and dependency notices. Notices generation reports 797
inventories, including 98 with SPDX metadata only requiring release review;
this is a disposable debug build, not release qualification. Full debug app and
instrumentation APK assembly, JVM tests and lint are now running sequentially
after native compilation, with one worker and a 768 MiB Gradle heap.

Both debug APKs assembled successfully, including music instrumentation. All
296 JVM tests passed with zero failures/errors/skips. The combined build was
cancelled during lint after JVM heap/metaspace pressure; lint alone then passed
in a fresh 1536 MiB heap / 768 MiB metaspace JVM (`BUILD SUCCESSFUL`, 1m27s).
Device qualification now starts on disposable AVD `vidcue_perf_m9_20261115`,
explicit adb serial `emulator-5580`. The other agent continues its UI task;
shared edits remain preserved and heavy resource use is coordinated.

First device baseline: strict MusicLatencyTest ran all six cases and failed the
100 ms deadline. StartupUiTest ran four cases with two failures (strict target
and an existing cold two-second gate); these are retained under
`target/android-performance-evidence/`. The cold provider-feed launch produced
no populated-feed draw record, so activity launch cannot qualify the feed target.
No performance target is declared achieved.

The trace identified offline lookup on the selection path. `OfflineStore.find`
now reuses the download index and parses the folder index at most once per
selection, avoiding repeated missing-record exceptions. Complete cached media
retain priority, incomplete paired downloads can still use folder exports,
cache-open failures retain folder fallback, and malformed candidates do not
hide later matches. No persistent negative cache is introduced. Debug APKs and
all JVM tests pass after this change; device comparison is running. Lint passed
before this lookup change and needs renewal for the final candidate.

The real `MediaRuntimeTest.actualVideoPauseVolumeSwitchDownloadAndOfflinePlayback`
regression passes (92.712 s) on the lookup candidate. This checks actual download
completion and cache-only decoding but is not a maximum-throughput benchmark.
Two video comparison attempts still fail the existing two-second cold threshold;
neither demonstrates a latency win. The next diagnostic adds `offline_lookup_ms`
inside the IO lookup, separate from the existing scheduling-inclusive
`offline_ms`. Updated APK/JVM/lint validation is running alone, with the emulator
stopped, into `/tmp/vidcue-performance-lookup-split-build.log` (exit file of the
same basename). No budgets, media qualities or existing tests were weakened.

The timing-split APK/JVM/lint run passed. Its device trace measured 10 ms
inside offline lookup versus 255 ms for the scheduling-inclusive offline stage,
while cold video still failed at 3458 ms with costly decoder initialization.
This changes the next optimization: disk lookup and native resolution now stay
in one IO coroutine instead of resuming the UI thread between them. Local
preference and explicit offline recovery retain precedence; scope cancellation
still reaches RustRpc, and account/network epochs are checked before handoff.
The overall resolved/usable clocks still include UI scheduling. `offline_ms`
now ends immediately after lookup on IO (including entry dispatch); it no longer
includes the removed UI return. No before/after claim may compare this stage
alone as a playback speedup. APK/JVM validation of this change is running into
`/tmp/vidcue-performance-resolution-build.log`.
The combined IO lookup/resolution candidate builds successfully; all 296 JVM
tests pass with zero failures/errors/skips. Lint has not yet been rerun after
this coroutine edit. The disposable emulator is restarting for the same strict
video case before any improvement claim.

The combined-worker device case still fails (3174 ms total usable start); its
offline stage is 15 ms and actual lookup 9 ms, but total resolution still includes
return-to-main delay. A separate atrace run also fails. The trace is retained at
`target/android-performance-evidence/video-atrace.txt` and shows long render
synchronization waits affecting both the app and System UI. EGL inspection reports
llvmpipe only, including after granting render-node access; no hardware GPU path
was observed. Emulator resolution, quality and deadlines remain unchanged.

Feed diagnosis correction: the earlier normal launch was on first-run onboarding,
not a provider-feed benchmark. Completing onboarding on this disposable install
then exposed disconnected emulator Wi-Fi. Connecting `AndroidWifi` restored
networking. The next normal launch reached YouTube, which returned no anonymous
Home recommendations (`feed-connected.png` retains the empty-state explanation).
No populated feed timing is inferred. Personalized Home needs suitable live
account evidence; public browse/fixture measurements remain independent work.
No real browser cookies or account credentials were read. The tool runtime was
replaced again after this diagnosis; prior command completion was inspected
before repeating this note write.

Added opt-in `DownloadThroughputTest` with an 8 MiB deterministic loopback range
fixture, concurrent handlers, fixed per-request latency/per-connection pacing,
three repetitions and 1/2/4/8 connections. Each transfer checks length and CRC32;
fixture bytes are generated before timing. It compares current download HTTP and
pooled playback HTTP without changing production concurrency. Instrumentation
APK assembly passes. `YTUI_ANDROID_DOWNLOAD_THROUGHPUT=measure` forwards the
opt-in in both runner paths and captures `download-throughput.txt`; shell syntax
and diff checks pass. Device run pending; this cannot establish real CDN rates.

The download benchmark passed on the disposable emulator (24 verified transfers,
three samples per transport/concurrency, instrumentation elapsed 11.765 s).
`target/android-performance-evidence/download-throughput.txt` retains every
measurement. The current HTTP implementation at four connections records
33.25/34.15/34.99 MiB/s in this controlled fixture; eight connections records
42.84/54.92/30.28 MiB/s. Pooled HTTP does not consistently improve this fixture.
These are synthetic, paced loopback measurements, not YouTube rates, WAN capacity
or a maximum-throughput claim. Production transport/concurrency stays unchanged
until evidence justifies a change. The fixture closes sockets and does not
measure TLS/HTTP2 connection reuse. It preserves the existing 8 MiB pending-chunk
budget across connection counts; transport/internal and currently-consumed
buffers are additional memory, not covered by that budget.

Inspection of the pinned Media3 1.8 API confirms source preloading exists,
whereas PlaybackCache currently warms only byte ranges. Preparing sample queues
is the next playback candidate; service warming already exists and should not
be duplicated. This is a proposed next step, not an implemented feature.

M9 source-preparation candidate: the existing renderer factory and load-control
budgets now feed Media3's shared DefaultPreloadManager/ExoPlayer builder. After
byte warming, one eligible item can prepare 250 ms of sample queues using cache
reads only. Inline manifests/data sources remain supported. Account/network,
quality, audio/video mode, media metadata and selected descriptor/caption identity
must match; the candidate expires after 30 s. A failed/incomplete preparation
falls back to ordinary foreground source creation. Captions use an isolated map
until handoff. No speculative history/queue/notification action is introduced.

PreparedPlaybackDataSource upgrades a paused cache-only loader at its current
byte offset after promotion, allowing future network/cache reads without replaying
bytes. Cancellation/promotion gates cannot revive an already-cancelled socket.
Unit tests cover scope changes, JSON ordering, diagnostic-field independence,
cache-only misses, promotion and offset continuity; APK assembly and JVM tests
pass on the first candidate. A new real audio device regression requires both
sample preparation and actual foreground reuse/continued playback. It has not yet
run. A separate source-reuse timing flag is added; rebuilding current sources
into `/tmp/vidcue-performance-source-final-build.log` before the device run.

The refreshed source-preparation candidate passes all 302 JVM tests and both APK
assemblies. Its first device case passes (23.437 s): byte warming and Media3
sample preparation complete, mismatched quality/mode are rejected, foreground
playback increments the actual source-use counter, and audio continues past one
second. No 100 ms claim follows. The instrumentation is now expanded to video
with decoder-frame advancement and to retain separate audio/video timing reports;
its APK is rebuilding. The first successful case did not persist a timing report.
Current source preparation has not yet passed final lint or broader player tests.

Both source-preparation device cases pass (29.953 s), including actual wrapper
reuse, mismatch rejection, continued audio and video decoder advancement. Separate
reports retain `source_preloaded_reused=true`. Prepared audio reaches active
playback in 128 ms and AudioTrack advancement in 345 ms in this sample; prepared
video remains far over target. This is not a controlled before/after speedup
claim (player/codec warmness differs), and none of the requested targets closes.
The broad real playback/switch/download/offline regression is running again on
the shared-preload-manager architecture; final candidate lint remains pending.
The broad playback/switch/download/cache-only regression passes on the source
preparation architecture (91.146 s). Existing strict StartupUiTest and six-sample
MusicLatencyTest are now rerunning with the unchanged 500/100 ms owner targets;
source reuse functionality does not replace those gates.

Strict owner-target recheck ran five existing cases: one passes, four fail on
latency (strict video, legacy cold-video/HLS windows, and music). Raw output is
`source-strict-latency.txt`; no threshold was weakened. The original cold-service
music cases explicitly report `source_preloaded_reused=false`: preparing bytes
before a service exists cannot prepare that service's sample queues. Source-
prepared warm-player coverage stays separately identified, not substituted for
those cold cases. The emulator is stopped and final candidate lint is running
in `/tmp/vidcue-performance-source-lint.log`. Device runner evidence collection
now includes both source-preparation timing files.

Final source-candidate lint passed (2m31s) before this next optimization. Pinned
Media3 bytecode confirms preload-manager construction creates another track
selector/capability list. Manager construction is now deferred until an eligible
byte-warmed candidate exists, retaining the shared player builder. Idle/cold
playback therefore avoids unused speculative setup. The device reuse test now
requires lazy initialization and actual cancellation/replacement before playback.
A new `service_setup_ms` duration records service onCreate work separately from
selection clocks; for a warm service it happened before the selection. Latest
APK/JVM validation is running in `/tmp/vidcue-performance-lazy-source-build.log`.

The lazy-manager APKs build and all 302 JVM tests pass. Both strengthened device
cases also pass (30.943 s): the idle service has no preload manager, byte warming
creates it, cancellation discards prepared sample queues, replacement succeeds,
and foreground audio/video really reuse the replacement and continue playing.
Timing reports are retained as `lazy-source-{audio,video}-timings.txt`. Cold-service
strict music measurement is rerunning with the new service-setup duration before
any latency-improvement claim. Latest source still needs renewed final lint.

Cold music still fails the strict 100 ms check after lazy setup. Later cold
service constructions measure 23–25 ms in the saved trace; remaining latency
cannot be attributed just to manager creation. Added an opt-in controlled
`SourcePreparationBenchmarkTest`: same warm player/quality/byte readiness,
interleaved ABBA source-prepared versus bytes-only starts, four warm-up and sixteen
measured selections. It asserts actual path use and continuing playback and
retains active-playback/AudioTrack-observation clocks separately. This tests
whether retaining the extra preparation is supported by comparable evidence;
it cannot replace cold, live, video or feed acceptance. Instrumentation APK build
is running in `/tmp/vidcue-performance-source-comparison-build.log`.

Controlled warm-player comparison passes (60.314 s), verifying all 20 intended
path selections. After the four warm-up samples, eight bytes-only starts have
median active playback 285 ms and AudioTrack callback observation 646.5 ms;
eight source-prepared starts have medians 211.5 ms and 621.5 ms. These interleaved
synthetic observations support retaining source preparation for active startup,
with a much smaller callback-observation change. They do not qualify the 100 ms
target, cold-service performance, physical/acoustic output, video or live content.
Raw per-selection data and startup reports remain in
`target/android-performance-evidence/source-preparation-comparison.txt`.
The source-comparison chart uses those exact medians. The runner now forwards
`YTUI_ANDROID_SOURCE_BENCHMARK=measure` and collects the comparison report.

## Physical device and live results, Galaxy Tab S9 (2026-11-16)

Device: SM-X710, Android 16, de_DE, speaker output, home Wi-Fi, separate Preview install
(`dev.vidcue.android.preview`). Levels: physical device; "live" rows used public YouTube
and openly licensed film hosts, no account.

### Music start (target 100 ms)
- `setEnableOnAudioPositionAdvancingFix(true)`: without it Media3 1.8 reported audio
  advancing early. Honest baseline with the fix: 380-550 ms tap to AudioTrack advancing.
- Low-latency AudioTrack performance mode: no measurable change (370-550 ms); reverted.
- Measured raw AudioTrack start: idle output 95-447 ms, output kept running by another
  stream 27-46 ms. Added `AudioOutputWarmth`: an inaudible silent stream while Music or
  Radio is open in the foreground and nothing plays (no audio focus, stops on pause, tab
  change, playback, or after 2 minutes).
- Added `QuickStartAudioTrackProvider`: start threshold ~40 ms instead of the full
  ~250 ms buffer.
- The playout start Media3 reports (wall clock) is mapped to the monotonic clock and
  bounded by the event time.
- Result, `MusicLatencyTest` (synthetic, Music page, service cold each sample): playout
  137-148 ms after warm-up (301 ms first sample); test signal 201-230 ms. **Target not met.**
  ExoPlayer reaches "playing" in ~80-95 ms; the rest is the output pipeline.

### Video start (target 500 ms), live YouTube
| Video | Cold (caches cleared) | Prefetched (resolved + first bytes, as a visible card) |
| --- | --- | --- |
| Big Buck Bunny 4K (aqz-KE-bpKQ) | 1802 ms | 392 ms |
| Sintel (eRsGyueVLvQ) | 2438 ms | 328 ms |

Cold breakdown (song example): extraction ~920-1040 ms, first media bytes ~570-670 ms,
audio output ~350 ms. The 500 ms target is met only after prefetch.

### Feed (target 1000 ms), live, anonymous
Music home 1175 ms first / 910 ms repeat (after the separate EU-consent fix); search
940-1148 ms first / 2 ms repeat. Anonymous YouTube home is empty without an account.

### Films, live (openly licensed / public test streams only)
HLS: Tears of Steel 1577 ms, Big Buck Bunny 1700 ms, Stream Lab bip-bop 4214 ms.
Internet Archive files: Elephants Dream 6496 ms, Sintel 12645 ms, Cosmos Laundromat
12461 ms (archive.org redirects and first-byte time dominate). **Target not met.**

### New opt-in live test
`LiveMediaLoadingTest` (`-e vidcue_live measure|enforce`, or `YTUI_ANDROID_LIVE` in
`tools/android/run-device-tests.sh`) times feed pages, public YouTube videos and songs
(cold and prefetched) and every non-Cinecat catalog film, requires three seconds of
continued playback per sample, and writes `live-media-timings.txt`. No account, no stream
URLs in reports. Default URL lists can be overridden with `vidcue_live_videos`,
`vidcue_live_songs`, `vidcue_live_query`.

### Not verified
Bluetooth output, other devices, acoustic output (all audio numbers are AudioTrack
signals), downloads on live CDNs, battery cost of the warm output. Prefetched song starts
were not yet measured live.
