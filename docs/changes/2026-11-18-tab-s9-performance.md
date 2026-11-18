# M9 Galaxy Tab S9 performance
Use the dedicated Preview package on Android 16, archived PR 024e8ec ARM64 app. Run three fixture video attempts; three cold and prepared music starts; 24 loopback downloads; three public feed RPC rounds; the film, song and YouTube corpus. Supplement with three normal cold feed launches, nine movie-sheet Play actions, three native movie saves, three verified Media3 saves plus resume and playback coexistence. Compare visible video under 500 ms, populated feed under 1000 ms, advancing music output around 100 ms; report verified throughput. Preserve the stable package.

## Measured baseline on the tablet

The runs used wireless ADB, the SM-X710 on API 36, and the archived arm64 Preview APK at 024e8ec. The evidence directory is `target/android-performance-evidence/tablet-024e8ec/`. Times below include visible video and active playback, advancing audio output, or populated cards as indicated.

| Target | Physical result | Evidence in that directory |
| --- | --- | --- |
| Video under 500 ms | Two prefetched YouTube videos passed at 271 and 281 ms. Cold starts took 2416 and 1526 ms, including 1182 and 898 ms for resolution. All six directly launched movies missed at 673–6819 ms. Three UI fixture runs failed to register the card selection and yielded no timing. | `streams.txt`, `video-stages.txt`, `films.txt`, `video{1,2,3}.log` |
| Populated feed under 1 s | Three process-cold normal launches produced no populated-draw signal within 45 s. Music Home returned cards in 2001, 1697 and 1827 ms on first request, then 734, 977 and 683 ms on repeat. These RPC timings exclude rendering. All six YouTube search requests failed with an HTTP/2 send error. The error's causal relationship to the cold Home failure is unverified. | `launch{1,2,3}.draw`, `feed{1,2,3}.txt` |
| Music around 100 ms | Live songs took 1758 and 1097 ms cold, 377 and 472 ms prefetched. On prefetched songs the resolver took 2 ms, but advancing audio output followed player-start by 213 and 352 ms. No acoustic measurement. | `songs.txt`, `song-stages.txt`, `music.txt` |
| Download throughput | Three SHA-256-verified 11,053,871-byte Media3 downloads took 15.069, 10.275 and 9.096 s (median 1.03 MiB/s). Downloading alongside playback took 40.889 s (0.26 MiB/s). Pause/resume preserved the bytes. All 24 synthetic range transfers passed. Native offline movie saves failed at publication on all three baseline attempts. | `library.txt`, `download.txt`, `cdn{1,2,3}.log` |

## Movie sheet experiment

Three sheet taps per film measured from Play to both first frame and active playback, with at least three seconds of continued playback. Removing the automatic catalog source check reduced median Tears of Steel startup from 6904 to 1037 ms and Sintel from 10102 to 6026 ms. The activity started within 12–22 ms on the candidate. The broken-first-source fixture regressed from 3183 to 5918 ms median. Its follow-up candidate skips Media3 retry delays on HTTP 404/410 when a backup source exists. Evidence is in `observations.txt`, `patched/movies.txt`, and the follow-up `final/` directory.

For source diagnostics, issue anonymous bounded requests from the tablet's system curl. On Sintel compare the catalog URL with its own resolved redirect URL for a 4 KiB prefix and the 597,191-byte MP4 index. Take three samples of each, discard bodies, and retain status, redirect count, bytes, first-byte and total seconds. Check the missing Stream Lab manifest's HTTP status and whether a signed-out YouTube homepage responds over HTTP/1.1. These timings isolate the network path and exclude playback. Run outside the live movie batch.

For the feed transport diagnosis, compare the PR's signed-out Browser using its default HTTP client and an otherwise matched HTTP/1.1-only client. Run each twice with fresh clients and compare Home and Search cards, elapsed time and sanitized reason. This standalone test runs as Android shell and does not qualify app rendering or per-app network routing.

**Measured.** The patched arm64 JNI movie downloader completed all three live saves with the reference hash, in 7.211, 12.888 and 10.167 s (median 1.04 MiB/s). See `final/publication.txt`. The test APK used the verified Preview signer and the exact rebuilt shared library.

**Measured.** Standalone Android HTTP range reads returned 206 and the expected lengths in all 12 cases. On the Archive catalog URL, the median prefix and tail-index times were 1.916 and 2.468 s. Using its resolved CDN URL gave 0.851 and 1.619 s. The missing Stream Lab manifest returned 404 in 424 ms. These are network diagnostics, not a movie-start benchmark. See `final/probe.txt`.

**Measured.** In the signed-out standalone Rust provider probe, both negotiation modes returned an empty Home with a service notice on both attempts. Both returned Search cards on both attempts. HTTP/1.1-only did not recover Home. The installed-app Search errors were not reproduced by this shell process. See `final/feed-probe.txt`.

**Measured.** The arm64 Cinecat provider suite passed on the Tab S9 (31 passed, 0 failed, 3 ignored). Tests covered encrypted HLS and subtitle bundle publication as well as collisions and concurrent writers. Two opt-in public-provider checks and the external FFmpeg/ffprobe decoding check were skipped. See `final/native-android.txt`.

After the build host reboot, rebuild the archived PR with the two pending movie UI edits and the recovered arm64 library. Compare three Play taps each for the failed-first-source fixture, Tears of Steel and Sintel. Then test a separate candidate that reuses Archive's selected HTTPS CDN for extractor range reads within a single progressive load. Compare three Sintel sheet starts, verify at least three seconds of playback each, and retain the change only if its median improves. Keep both candidates' evidence separate.

## Recovered build and final tablet comparisons

The pre-restart native suite and live saves had finished. The interrupted Gradle build had produced no new APK, and no runner survived the host restart. Rebuilt the 024e8ec Preview candidate from the archived source and recovered arm64 library, then resumed with the same tablet and existing evidence. The missing-first-source case took 2152, 1475 and 1643 ms (median 1643 ms, down from 5918 ms with default retries and 3183 ms on the original PR). Tears of Steel took 3296, 1420 and 2079 ms (median 2079 ms, original PR 6904 ms). Sintel took 5867, 7477 and 6201 ms (median 6201 ms). Every sample played at least three seconds. The normal Android JVM suite completed with 319 passes and two skips. See `final/observations.txt` and `final/restart-build.log`.

**Measured.** Reusing the selected Archive CDN for range reads delivered Sintel in 4417, 4199 and 4899 ms (median 4417 ms, 29% below the preceding median of 6201 ms). Every sample showed a frame and advanced at least three seconds. The original PR median was 10102 ms. The local change retains the catalog URL as a fallback and scopes redirect reuse to a single data source. The 500 ms cold target remains open. See `final/archive/observations.txt`.
