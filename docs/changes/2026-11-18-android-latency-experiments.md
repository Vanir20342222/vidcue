# M9 Galaxy Tab S9 latency experiments

Use the disposable Preview package and the archived PR plus physically verified Archive redirect and missing-source fixes. Compare network-movie startup with Media3’s 1000 ms default and a 250 ms initial buffer. Keep rebuffer and steady-state thresholds at their defaults. Run three taps per build for the failed-first-source fixture, Tears of Steel HLS and Sintel MP4, recording Play to both frame and playing plus continued playback. Report medians and ranges and flag source variability. Verify audio and feed candidates separately from the movie results.

Audio experiment: compare control and candidate in A-B-B-A order. Use the identical test APK with Music visible, the same two public songs, a cold and a prefetched start for each, and verify advancing audio plus continued playback. Compare medians across matched song and cache states; collect actual warmer activity. Preserve the 100 ms target. Category experiment: publish two prepared 400-card fixture categories, alternate the cached categories 60 times, drop the first ten, require all cards synchronously available without fetching again. Measure only time inside navigate on the Android main thread. These fixture timings exclude network, rendering and Home startup.

Replay experiment: route only a matching built-in, single-source, header-free progressive Archive film without sidecars through Vidcue's existing bounded streaming cache. Use a dedicated public-movie cache key. No speculative requests. On the tablet run one first play with the new key, then five process-restarted Sintel plays. Verify each reaches a rendered frame, playing state and at least three seconds of progress. Report replay latency separately from first-play latency; compare against the three uncached repeats. Preserve the installed stable channel and account data.

After the app timers finish, compare single-stream and four concurrent HTTP ranges on the same tablet with system curl against the same 11,053,871-byte public W3C trailer, three rounds each in balanced order. Require HTTP 206 for every range and a complete matching SHA-256 for every sample. This diagnostic tests the live origin and connection; it does not measure the Vidcue queue or establish an optimal default.

Final confirmation: keep the prepared category representation and the public-file cache. Restore Media3's original streaming buffer and music output handoff. Inspect the cached byte spans for the public Sintel entry, clear only that benchmark entry before each of three cold trials, and follow each cold play immediately with a separate-process warm play. Keep the sheet trial, instrumented event timestamps and three-second playback requirement unchanged.

## Physical experiments

Evidence is local under `target/android-performance-evidence/tablet-experiments/`. All runs used the SM-X710, Android 16, and the Preview application. Warm video clocks stop only after both frame and playing. The movie harness separately checks the playhead at three seconds.

| Experiment | Control | Candidate | Decision |
| --- | --- | --- | --- |
| Initial movie buffer, three samples per title | 1000 ms buffer. Median Tears of Steel 1647 ms, Sintel 4625 ms, failed first source 1717 ms | 250 ms buffer. Medians 1323, 4443, 1883 ms respectively. Worst Sintel run reached 7069 ms | Keep the default. The small, variable sample did not justify a lower startup cushion. |
| Music output handoff, A-B-B-A, two songs each | Pooled median cold 1176.5 ms, prefetched 261.5 ms | Pooled median cold 1075 ms, prefetched 239.5 ms. One prefetched start took 330 ms | Keep the existing behavior. The small sample and inconsistent per-song results do not establish a repeatable gain, and all samples missed 100 ms. |
| Main-thread publication on a cached 400-card category | 50 samples, median 5.10 ms and p95 5.30 ms | 50 samples, median 3.62 ms and p95 3.84 ms | Keep prepared category cards. This saves main-thread work during navigation. Fresh Home delivery still needs its own populated-draw measurement. |
| Bounded streaming cache for public catalog MP4s | Three uncached Sintel starts at the same 250 ms buffer had a median of 4443 ms | First fill 7442 ms. Five subsequent process-restarted starts took 400, 447, 393, 417 and 401 ms | Retest with production buffering. Playback writes only the ranges actually read and uses the existing streaming-cache limit. |
| Live public-file transfer, standalone system curl | Single-stream median 1.108 MiB/s | Four ranges median 1.167 MiB/s. All six 11,053,871-byte copies verified | Keep the app transport policy. The ranges overlapped substantially in speed and a system curl run does not qualify app download scheduling. |

Raw files: `{control,lower,replay}/observations.txt`, `song-{A1,B1,B2,A2}.txt`, `track-{A1,B1,B2,A2}.txt`, `feed-{lower,feed}.ns`, `download-probe.txt`. The feed replay asserts synchronous population of all 400 cards and checks the browse call count on each build. Its timing excludes layout and artwork. All time and throughput comparisons are scoped to the fixture or public media and the network used during these runs.

Bracket the selected-build pairs with three more uncached original-buffer Sintel starts after qualification, then restore the selected Preview build. Compare the first-play timings to both controls to expose network drift.

## Retained build

With the original movie buffer and audio handoff restored, clearing only the Sintel cache entry gave first-play times of 4474, 4752 and 4600 ms. The immediate process-restarted repeats took 399, 443 and 408 ms (median 408 ms). All three repeats reached both frame and playing within 500 ms and advanced past three seconds. Each inspected cache contained the 597,191-byte MP4 index at offset 310,076,814 as well as bytes near the beginning. Original uncached control medians at the start and end of testing were 4625 and 7735 ms respectively. Network variation prevents attributing the cold difference to the cache. The verified gain is repeat startup with the relevant spans still resident.

The retained Preview is based on PR snapshot 024e8ec plus local improvements. Its JVM suite ran 322 tests with 320 passes and two skips. Installed APK SHA-256 matched the selected local APK after restoring the regular test package. See `selected/timings.txt`, `selected/spans-{1,2,3}.txt`, `end-control/observations.txt`, `selected/installed-sha256` and `selected-build.log`.
