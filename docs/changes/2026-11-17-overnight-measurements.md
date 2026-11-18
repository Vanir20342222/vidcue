# M9 overnight measurement record

Protocol written 17 Nov 2026 CET before this session's timings. Stop before 08:00 CET on 18 Nov. No implementation, push or merge. PR #172 is open and draft at 024e8ec; Android app, Android gate, Rust lint/test/media/gate, hygiene and secrets passed. Hosted APK/device and release-build jobs skipped. The earlier failed navigation job is superseded. A local b2dfe01 and dirty worktree differ from the published PR. Use archived candidate APKs, not its current build outputs.

## Remaining gaps (one list)

1. Video below 500 ms: fresh emulator cold and prepared visible-card timings and Tab S9 live tap-to-visible results needed. Earlier physical prepared 328–392 ms, cold 1802–2438 ms. Movie-sheet source checks precede the separately timed activity; require full tap, failover, visible frame and continuing playback.
2. Feed below 1000 ms: need candidate process-start-to-populated-draw on emulator and tablet. Public Music Home previously 1175 ms first; repeat and bare browse-RPC time are distinct. Record empty/error as missing content.
3. Music around 100 ms: need emulator and physical first/warm output measurements, especially live sources; earlier physical synthetic playout 137–148 ms warm, 301 ms first. Keep audio output and callback observation separate.
4. Download throughput: need current emulator integrity and concurrent-range throughput, device and representative live-CDN app transfers with integrity, resume and playback coexistence. Prior loopback results cannot qualify live speed.

## Diff review against six findings

Reviewed the published merge-base-to-024e8ec diff. The navigation test correction is included in this head. [PR #172](https://github.com/Vanir20342222/vidcue/pull/172)

| Finding | What the draft changes | What its diff and CI leave open |
|---|---|---|
| F1 · HTTP transport | Enables HTTP/2 and compressed text responses, protecting identity and range semantics | Negotiated protocol on live origins, actual film starts and latency under load |
| F2 · Transfer contention | Separates metadata and transfer executors (three and four workers) | Verified live throughput, resume and foreground playback during transfers |
| F3 · Process startup | Moves native initialization and normal offline storage setup off the UI thread behind a readiness signal | Service-only cold storage, first streaming-cache open, real Keystore and process-to-populated-draw timing |
| F4 · Resolution cancellation | Retains started resolution flights under a four-permit bound | Foreground taps during prefetch, particularly with all permits occupied, on a device |
| F5 · Artwork writes | Bounds queued disk writes and handles cache clearing | First artwork, memory and scrolling frame times on hardware |
| F6 · Navigation accounting | Caches immutable JSON size estimates and incrementally accounts for page history | Real navigation timing in a large library |

Catalog source checks, Cinecat multi-stream checks, duplicate media reads and timing from the movie sheet's Play action all fall outside this diff.

## Predeclared protocol

Use the idle disposable x86_64 vidcue_perf_m9_20261115 (emulator-5554) for diagnostics. Use the previously paired SM-X710 only when connected and explicitly identified, with Preview only. No stable install or account access. Install both archived APKs on the emulator. Do not reinstall on a tablet with an incompatible signature. Each instrument invocation starts a new test process. No account sign-in, cache erasure, UI tuning or test-budget edits. Fresh process does not imply fresh DNS/CDN/OS caches. Take three independent cold/prepared fixture video rounds, one music invocation (three cold and three prepared selections), one throughput invocation (three repeats for each transport/concurrency pair = 24 CRC-checked 8 MiB transfers), three public browse rounds, and one live video/song/film corpus sweep on each available device. Retrieve exact file outputs and isolate append-only reports by pre-run byte offset. No retry of valid failures; report sample counts including failed or censored launches. Stop an invocation at 600 seconds or earlier at the overnight cutoff.

For process-start feed draw look for FeedStartup first_populated_draw_ms for the tested process, with source; use three cold normal launches for physical qualification. Diagnostic provider-RPC numbers exclude rendering. For films baseline the activity-level test separately; qualification needs three real sheet taps per source on device, including a broken first source; no instrumented sheet-tap timing currently exists. For live downloads qualification requires three identical public-object downloads per configuration on the tablet, exact verified bytes, pause/resume and concurrent playback. Existing range fixture is executable; there is no equivalent live benchmark identified. No absolute throughput target was specified: report MiB/s and compare medians among integrity-passing configurations, without declaring maximum live speed.

Acceptance: every valid cold and prepared video/film sample under 500 ms with visible frame and continued playback; every populated feed draw under 1000 ms; operational music threshold 100 ms or less for output and uninterrupted progression. Missing output or empty page fails qualification. For live media use the existing three-second continuation check. Report medians and worst where meaningful; never pool cold and prepared. Any omitted endpoint stays open.

Exact emulator commands (bash, project root). Instrumentation invokes the project's own runners. Keep raw logs local. Repeat on the paired tablet only after verifying the model.
~~~bash
set -eu
s=emulator-5554
pkg=dev.vidcue.android.preview
out=/tmp/vidcue-night-024e8ec
art=/home/vanir/.t3/worktrees/vidcue/t3-52e56a8a-ff9f451046ecc8bf/target/pr172-validation/candidate-024e8ec
mkdir -p "$out"
sha256sum "$art"/*.apk > "$out/apk-sha256"
for apk in app-debug.apk app-debug-androidTest.apk; do timeout 60 adb -s "$s" install -r -t "$art/$apk"; done
remote=/sdcard/Android/data/$pkg/files
run() {
  name=$1; cls=$2; file=$3; shift 3
  start=$(date +%s)
  offset=0
  if [ "$file" = live-media-timings.txt ]; then offset=$(timeout 10 adb -s "$s" shell "wc -c < $remote/$file" 2>/dev/null | tr -cd '0-9') || true; fi
  timeout 600 adb -s "$s" shell am instrument -w -r -e class "dev.ytui.android.$cls" "$@" "$pkg.test/androidx.test.runner.AndroidJUnitRunner" > "$out/$name.log" 2>&1 || true
  timeout 15 adb -s "$s" exec-out cat "$remote/$file" > "$out/$name.raw" || true
  tail -c "+$((${offset:-0}+1))" "$out/$name.raw" > "$out/$name.txt"
  timeout 10 adb -s "$s" logcat -d -T "$(date -d "@$start" '+%m-%d %T.000')" -s FeedStartup:I > "$out/$name.draw" || true
}
for i in 1 2 3; do run video$i 'StartupUiTest#coldAndPrefetchedCardTapsHaveActualPixelsUnderOneSecond' startup-subsecond-timings.txt; done
run music MusicLatencyTest music-latency-timings.txt -e vidcue_music_latency enforce
run download DownloadThroughputTest download-throughput.txt -e vidcue_download_throughput measure
for i in 1 2 3; do run feed$i 'LiveMediaLoadingTest#liveFeedPagesLoadWithinOneSecond' live-media-timings.txt -e vidcue_live measure; done
for pair in 'films liveFilmsStartWithinHalfASecond' 'streams liveYoutubeVideosStartWithinHalfASecond' 'songs liveSongsStartWithinAHundredMilliseconds'; do
  read -r name method <<< "$pair"
  run "$name" "LiveMediaLoadingTest#$method" live-media-timings.txt -e vidcue_live measure
done
~~~

Supplemental protocol for the gaps above: one measurement-only delegate may prepare a temporary instrumentation APK against 024e8ec, with no production changes. Record its exact class selectors, build, invocation, and fixture/transfer choices in a local addendum before timing. On the same emulator after the first batch releases it, measure three independent process-cold normal provider feed launches (existing first-populated-draw clock), three UI Play taps each for one openly licensed HLS film and one Archive MP4 plus one broken-first-source failover case. Report timeout at 45 seconds/sample and continued playback at three seconds. Measure three real, bounded, publicly licensed identical objects through the actual app download path and validate content length/hash against an independent reference, reporting MiB/s and any control/resume/coexistence gaps. A stream or host-only curl is not an app download. If unavailable, record the exact blocker. No tablet result inferred. Stop by 07:55 CET to leave time for handback.

Supplemental run commands. The temporary Kotlin driver uses the unmodified candidate app, the real movie-sheet Play semantics action, and the movie_download JNI operation. The failover title is Stream Lab S2E1. The three empty-cache network file copies use the Blender Bunny trailer served by W3C. The installed production APK is the archived PR APK throughout. No preexisting library content is removed.
~~~bash
set -eu
root=/tmp/vidcue-night-harness/apps/android
out=/tmp/vidcue-night-024e8ec
apk="$root/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk"
art=/home/vanir/.t3/worktrees/vidcue/t3-52e56a8a-ff9f451046ecc8bf/target/pr172-validation/candidate-024e8ec
pkg=dev.vidcue.android.preview
s=emulator-5554
cli=/home/vanir/.t3/userdata/device/targets/t3-81de92ffffd37727ed83264d/agent-device
runner="$pkg.test/androidx.test.runner.AndroidJUnitRunner"
timeout 1800 env YTUI_ANDROID_REVISION=024e8ec946f99c6bba8572f53e68055f8d9abd44 "$root/gradlew" -p "$root" --offline --no-daemon :app:assembleDebugAndroidTest > "$out/harness-build.log" 2>&1
curl -fLsS --max-filesize 16777216 --max-time 60 'https://media.w3.org/2010/05/bunny/trailer.mp4' -o "$out/license-film-trailer.mp4"
expected=$(sha256sum "$out/license-film-trailer.mp4" | cut -d' ' -f1)
sha256sum "$apk" "$out/license-film-trailer.mp4" > "$out/supplement-sha256"
timeout 60 adb -s "$s" install -r -t "$apk"
trap 'timeout 60 adb -s "$s" install -r -t "$art/app-debug-androidTest.apk"' EXIT
invoke() {
  name=$1; method=$2; shift 2
  timeout 130 adb -s "$s" shell am instrument -w -r -e class "dev.ytui.android.NightMeasurements#$method" "$@" "$runner" > "$out/$name.log" 2>&1 || true
}
invoke provision prepareReturningUser
for i in 1 2 3; do
  timeout 35 "$cli" open "$pkg" --relaunch > "$out/launch$i.log" 2>&1 || break
  pid=$(timeout 10 adb -s "$s" shell pidof "$pkg" | tr -d '\r')
  if [ -n "$pid" ]; then timeout 45 adb -s "$s" logcat --pid="$pid" -d -s FeedStartup:I > "$out/launch$i.draw"
    if ! grep -q first_populated_draw "$out/launch$i.draw"; then
      timeout 45 bash -c 'adb -s "$1" logcat --pid="$2" -s FeedStartup:I | grep --line-buffered -m1 first_populated_draw' _ "$s" "$pid" >> "$out/launch$i.draw" || true
    fi
    echo "pid=$pid" >> "$out/launch$i.draw"
  fi
done
for i in 1 2 3; do invoke "cdn$i" liveFile -e expected "$expected"; done
for i in 1 2 3; do
  for id in tears-of-steel sintel stream-lab; do invoke "tap-$id-$i" moviePlay -e pick "$id"; done
done
timeout 20 adb -s "$s" pull "/sdcard/Android/data/$pkg/files/night-observations.txt" "$out/observations.txt"
~~~

## Candidate measurements

The archived candidate and test APKs were installed together on API 35 AVD emulator-5554. The test package targets dev.vidcue.android.preview. The reported handset refuses its old endpoint and is absent from wireless discovery. Pre-existing device evidence belongs to a different build. Raw text and SHA-256 manifests for this run are under /tmp/vidcue-night-024e8ec.

* Fixture video: one cold selection reached usable playback at 5094 ms, with pixels confirmed at 6101 ms. Two other invocations failed to inject the tap. The first test stopped at its cold failure and no invocation reached its prepared selection. The timed cold result missed 500 ms. The failed input injections supply no latency values. Software decoder/virtual display timings cannot establish tablet startup.
* Feed: first public Music Home RPC took 1266, 1133 and 1143 ms (median 1143, worst 1266). Repeats took 248, 260 and 323 ms. First search RPC took 703, 474 and 440 ms. Each Music Home result had cards. StartupUiTest also logged three synthetic populated draws, but the fixture inserts cards after setup. Those logs cannot stand in for a normal cold feed.
* Fixture music: selection to observed active AudioTrack advancement took 1785, 955 and 774 ms cold (median 955), and 679, 601 and 728 ms prepared (median 679). All six exceeded 100 ms and progressed after start. The internal AudioTrack signals were 1782/948/772 and 678/600/721 ms respectively. These clocks measure Android software signals.
* Range downloads: 24 verified 8 MiB loopback transfers. Three-run medians at 1/2/4/8 connections on current transport were 11.58/22.51/37.21/53.53 MiB/s; pooled transport 12.16/22.93/37.67/38.61 MiB/s. These characterize the fixture. They do not establish a production concurrency choice.

Before supplemental measurements, the movie-sheet driver also records the first MoviePlayerActivity lifecycle PRE_ON_CREATE callback. Its delta from Play includes source selection, app handoff and activity dispatch. The remainder includes player setup, media access, decode and presentation. The lifecycle observer runs only in the test APK.

The archived live corpus produced the following results on the emulator.

* The six movie-player launches reached both a frame and active playback in 8.393–14.184 s and each played at least three seconds. Tears of Steel took 14.184 s, Big Buck Bunny 8.393 s, Sintel 12.229 s, Elephants Dream 9.611 s, Cosmos Laundromat 13.069 s and Stream Lab S1E1 8.890 s. This benchmark supplies sources directly to the activity and excludes the movie-sheet check. Player launch alone already missed the target for every title.
* The two live YouTube selections took 4.711 and 5.350 s cold and 3.708 and 1.811 s prepared. The two live songs took 3.473 and 2.300 s cold and 0.924 and 0.577 s prepared. All four preparations hit the resolver cache.
* In the single valid fixture video trace, resolve/handoff occurred at 526/653 ms, the decoder signal at 3052 ms and frame at 5094 ms. Codec initialization duration was 1218 ms. These trace fields have different time origins and overlap. This points to additional player/decoder cost on this software-rendered host.
* For interpretation, Android distinguishes initial display from useful, interactive content, and Media3 exposes timestamped player events for investigating buffering and progress. See the [Android startup guide](https://developer.android.com/topic/performance/issues/launch-time) and [Media3 analytics guide](https://developer.android.com/media/media3/exoplayer/analytics).

Three normal returning-user Preview cold launches produced populated provider draws at 20.205, 16.503 and 15.476 s (median 16.503 s), with distinct process IDs. The clock starts at Android process creation and stops on the first fully visible drawn card grid, without waiting for artwork decoding. The first process also logged a 352-frame skip and a 5.429 s HWUI frame. These are debug-APK measurements on software-rendered Android. The separate RPC test uses an explicit Music Home request, so subtracting its median from the launch median would not isolate UI cost.

### Additional live download protocol, written after the three movie saves failed

All three native movie_download requests returned “Atomic offline-bundle publication currently requires Linux”. The host reference contains 11,053,871 bytes. The Rust publish function in crates/vidcue-providers/src/cinecat/transfer.rs is compiled for target_os=linux and returns this error on Android. Failure to finalize the bundle means no verified completed movie downloads and no valid movie download throughput. Do not retry this path with the same APK.

The separate Android Media3 offline library uses a different downloader. On the same emulator after all movie-sheet runs finish, queue the identical public trailer through OfflineStore.add with production network requirements, an unused cache key per invocation and folder auto-export off only for the test. Test three baseline saves, a fourth during playback of Tears of Steel, and a fifth where the app is asked to pause after bytes arrive and then resume. Each run gets 90 s from queue to completion. Verify the full cached object via the app's cache-only source, comparing both length and SHA-256. Exclude hashing from elapsed time. For coexistence, record playback position over the transfer and at least 3 s of subsequent progress; position advancement alone does not prove uninterrupted delivery. For resume, require positive bytes before pause, a stopped download and subsequent completion with matching bytes. If too fast to pause in flight, report the missed observation. This comparison uses the app's normal defaults in all runs. No production edits, retries of valid failures or claims about optimal parallelism.
~~~bash
set -eu
root=/tmp/vidcue-night-harness/apps/android
out=/tmp/vidcue-night-024e8ec
s=emulator-5554
pkg=dev.vidcue.android.preview
art=/home/vanir/.t3/worktrees/vidcue/t3-52e56a8a-ff9f451046ecc8bf/target/pr172-validation/candidate-024e8ec
apk="$root/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk"
timeout 600 env YTUI_ANDROID_REVISION=024e8ec946f99c6bba8572f53e68055f8d9abd44 "$root/gradlew" -p "$root" --offline --no-daemon :app:assembleDebugAndroidTest > "$out/library-build.log" 2>&1
sha256sum "$apk" > "$out/library-sha256"
expected=$(sha256sum "$out/license-film-trailer.mp4" | cut -d' ' -f1)
size=$(stat -c%s "$out/license-film-trailer.mp4")
timeout 60 adb -s "$s" install -r -t "$apk"
trap 'timeout 60 adb -s "$s" install -r -t "$art/app-debug-androidTest.apk"' EXIT
for mode in base1 base2 base3 playing resume; do
  timeout 200 adb -s "$s" shell am instrument -w -r -e class 'dev.ytui.android.NightLibrary#download' -e mode "$mode" -e expected "$expected" -e bytes "$size" "$pkg.test/androidx.test.runner.AndroidJUnitRunner" > "$out/library-$mode.log" 2>&1 || true
done
timeout 20 adb -s "$s" pull "/sdcard/Android/data/$pkg/files/night-library.txt" "$out/library.txt"
~~~

Host-only source inspection for the Sintel progressive file: read top-level MP4 atom headers using bounded 4096-byte ranges, stopping at the first movie index (moov). Run `python3 target/android-performance-evidence/overnight-024e8ec/mp4-layout.py`. This determines whether a player may need a second request far into the file before decoding. It supplies no Android latency measurements.

Sintel's MP4 stores its 597,191-byte movie index at byte 310,076,814, following the entire media-data box. The catalog source check reads the prefix and cannot reach that index. Media3's MP4 extractor requests a seek past a large media-data box to parse the index, then seeks back to samples. This file layout adds a concrete reason to investigate remote range reads after the player starts. The actual time per range was not collected in the app. [Media3 1.8.0 MP4 extractor](https://github.com/androidx/media/blob/1.8.0/libraries/extractor/src/main/java/androidx/media3/extractor/mp4/Mp4Extractor.java#L576-L605)

## Movie-sheet results

All nine Play actions reached a rendered frame, active playback and at least three seconds of position. The clock starts at invocation of the real Compose Play handler. Touch dispatch, opening the sheet and waiting for its details are outside the clock. Values below are milliseconds. The two stage columns are from the actual run with the median total for each title.

| Title | Play to usable playback, three runs | Median total | Before activity creation | Activity creation to usable playback |
|---|---|---:|---:|---:|
| Tears of Steel (HLS) | 7818, 6865, 7856 | 7818 | 1785 | 6033 |
| Sintel (Archive MP4) | 14191, 16526, 14847 | 14847 | 4447 | 10400 |
| Stream Lab S2E1 (broken first HLS source) | 8905, 8463, 7713 | 8463 | 2676 | 5787 |

Every pre-activity interval independently exceeded 500 ms. Catalog Play waits for Rust to probe sources. The HLS probe reads up to two segments per selected video/audio track and the progressive probe reads roughly the first MiB. The Activity then builds a new ExoPlayer and passes the URL to a different HTTP loader; the probe body is not handed to the player. This establishes repeated retrieval work, though the number of duplicate bytes was not traced. The largest measured interval was after activity creation for each title. Neither the resolver nor decoder alone explains the full delay. Cinecat additionally verifies up to three segments per returned HLS stream before accepting a provider; its elapsed contribution remains unmeasured.

Source at the tested revision: [catalog Play](https://github.com/Vanir20342222/vidcue/blob/024e8ec946f99c6bba8572f53e68055f8d9abd44/apps/android/app/src/main/java/dev/ytui/android/MoviesPage.kt#L807-L818), [Rust probes](https://github.com/Vanir20342222/vidcue/blob/024e8ec946f99c6bba8572f53e68055f8d9abd44/crates/vidcue-providers/src/cinecat/transfer.rs#L248-L359), [activity player creation](https://github.com/Vanir20342222/vidcue/blob/024e8ec946f99c6bba8572f53e68055f8d9abd44/apps/android/app/src/main/java/dev/ytui/android/MoviePlayerActivity.kt#L90-L146).

The activity-only corpus and full Play-handler runs used different processes, network conditions and app state at the time of launch. The paired lifecycle timestamps within a single Play-handler run are the basis for the stage split. The elapsed differences between the two suites do not measure probe cost.

## Download results and handback

The Android Media3 offline library completed all five live trailer transfers. Every cache-only read yielded 11,053,871 bytes and matched the independent SHA-256. The three baseline queue-to-completion times were 4626, 3953 and 4029 ms, giving 2.28, 2.67 and 2.62 MiB/s (median 2.62). During a playing HLS movie the same transfer took 11.250 s (0.94 MiB/s). Playback position advanced 8.810 s during that transfer and another 3.324 s afterwards. This single coexistence observation does not establish stall-free playback. The resume run stopped at 178,912 bytes, resumed and produced the verified complete object in 4.797 s including the pause. Downloads used a direct media URL at the queue boundary. Source discovery and export to a user folder fall outside the clock. Results describe one public origin; a claim of maximum throughput requires other networks/origins and hardware. The separate native movie download path failed to publish the file in all three attempts.

The original candidate application and instrumentation APKs were verified on the emulator after restoring the test package. Only disposable Preview was used. The Galaxy Tab S9 was unavailable over wireless ADB, so all four hardware targets remain open. There was no new delegate, production edit, push or merge. The published draft remained at 024e8ec with ten checks passed and three skipped. Stopped testing at 23:36 CET on 17 November.

Local evidence is under [target/android-performance-evidence/overnight-024e8ec](../../target/android-performance-evidence/overnight-024e8ec/), including per-run [movie stages](../../target/android-performance-evidence/overnight-024e8ec/observations.txt), [live downloads](../../target/android-performance-evidence/overnight-024e8ec/library.txt), native [publication failures](../../target/android-performance-evidence/overnight-024e8ec/movie-download-errors.txt), instrumentation sources and bash scripts. To reconstruct the temporary test source tree, extract apps/android from git archive at the full PR revision above, expose the candidate's existing native/assets build inputs at target and vendor at the repository root, and place NightMeasurements.kt under app/src/androidTest/java/dev/ytui/android for the sheet run. Add NightLibrary.kt for the library run. The unmodified application APK is the archived debug candidate throughout.

## Owner continuation, 17 November 23:43 CET

### Open findings updated with run 51 (23:43–23:55 CET)

Evidence paths below are relative to [the overnight evidence directory](../../target/android-performance-evidence/overnight-024e8ec/).

1. **Measured — movie startup.** All five new Sintel starts missed 500 ms (13.043–13.989 s). In the median run, Play to activity took 4.024 s, followed by 9.635 s to a rendered frame and active playback (13.659 s total). All reached three seconds of playback. Evidence: `continuation/film1.txt` through `continuation/film5.txt`; [median observation](../../target/android-performance-evidence/overnight-024e8ec/continuation/film1.txt).
2. **Hypothesised — Sintel index retrieval contributes to delay after activity creation.** Earlier byte inspection located the 597,191-byte index at byte 310,076,814 of a 310,674,005-byte file. Run 51 did not time the player's range requests, so the amount of latency attributable to that layout remains unknown. Evidence: [mp4-layout.txt](../../target/android-performance-evidence/overnight-024e8ec/mp4-layout.txt) and [film1.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/film1.txt).
3. **Measured — Music Home first load.** The five first Music Home RPCs took 1.155–1.268 s (median 1.209 s) before rendering, while repeat calls took 0.250–0.371 s (median 0.294 s). Every first call returned cards and exceeded the 1 s target. Evidence: `continuation/feed1.txt` through `continuation/feed5.txt`; [paired observations from run three](../../target/android-performance-evidence/overnight-024e8ec/continuation/feed3.txt).
4. **Blocked — native offline-bundle publication on Android.** The same seven fixture tests passed on Linux. Android passed four and failed all three successful directory-publication cases (progressive MP4, encrypted HLS and subtitle bundles), each at the explicit Linux-only error. This reproduced earlier installed-app failures without a live CDN. Evidence: [linux.log](../../target/android-performance-evidence/overnight-024e8ec/continuation/linux.log), [android-native.log](../../target/android-performance-evidence/overnight-024e8ec/continuation/android-native.log), [movie-download-errors.txt](../../target/android-performance-evidence/overnight-024e8ec/movie-download-errors.txt).

Work for this continuation is confined to measurements and coverage. Keep the published candidate fixed at 024e8ec. Check test entry points and CI runner platforms, and repeat isolated samples without any changes to production code, media layout, prefetch or publish behavior.

Predeclared repeat protocol. Run `bash target/android-performance-evidence/overnight-024e8ec/continuation/repeat.sh` on API 35 emulator-5554 using the SHA-checked app and archived sheet instrumentation. Alternate five first/repeat public Music Home browse invocations and five Sintel Play invocations, each in a new instrumentation process. Same thresholds as above (Music Home under 1000 ms, playable movie under 500 ms with three seconds of progress). Record every attempt, per-run frame/playing and pre-activity times, sample counts, medians and ranges. Keep failed attempts visible and measure only new report bytes. No parallel build or benchmark on this device during timings. This repeat deliberately preserves shared OS/network caches and real provider variability.

Publication coverage protocol. After the five paired rounds, run `bash target/android-performance-evidence/overnight-024e8ec/continuation/platform-tests.sh`. Compile unmodified provider tests from a git archive of 024e8ec for Linux x86_64 and Android x86_64 (NDK 27, API 26 compiler, API 35 runtime). Execute the same seven named tests on both, with synthetic loopback sources and fresh temporary output directories. Cases cover successful encrypted HLS, a 50-byte progressive file, a multi-file subtitle bundle, single-subtitle save, interrupted body, cancellation and rejected range/budget/invalid responses. Count only tests actually executed. This is a provider-library coverage audit. The earlier installed-app results separately cover JNI and Android app storage.

### Publication coverage at the draft head

* The native Rust tests assert successful file, HLS and subtitle-directory publication. They also check cancellation and rollback. The HLS test saves once and then tries to save again to an existing directory. The second attempt can fail on the initial existence check; that does not exercise a competing publisher creating the destination between staging and commit. [Provider tests](https://github.com/Vanir20342222/vidcue/blob/024e8ec946f99c6bba8572f53e68055f8d9abd44/crates/vidcue-providers/src/cinecat/tests.rs#L657-L712)
* The implementation stages in the destination parent, writes a receipt and calls the same directory-publication helper for both movie bundles and subtitle-only bundles. Rust sets target_os to android for the emulator ABI. The Linux branch uses a no-replace rename; Android and other target OS values take the explicit error branch. Single subtitle *files* use a different path. [Bundle publication code](https://github.com/Vanir20342222/vidcue/blob/024e8ec946f99c6bba8572f53e68055f8d9abd44/crates/vidcue-providers/src/cinecat/transfer.rs#L800-L826) · [Rust compiler target settings](https://doc.rust-lang.org/reference/conditional-compilation.html#target_os)
* The PR Rust test job executes the workspace tests on Ubuntu. The Android PR job compiles instrumentation and executes JVM tests and lint. Its device job only runs on schedule or manual dispatch. The JVM MovieCatalogTest checks saved metadata parsing. The mobile Rust input-validation test rejects a relative output path before transfer. The checked-in Android instrumentation exercises other download paths; no call to the native movie_download operation or movie-sheet save flow was found there. The Windows build workflow builds a desktop artifact and does not execute these provider tests. No macOS provider-test job was found. [Rust CI](https://github.com/Vanir20342222/vidcue/blob/024e8ec946f99c6bba8572f53e68055f8d9abd44/.github/workflows/rust.yml#L52-L77) · [Android CI](https://github.com/Vanir20342222/vidcue/blob/024e8ec946f99c6bba8572f53e68055f8d9abd44/.github/workflows/android.yml#L169-L175) · [Windows workflow](https://github.com/Vanir20342222/vidcue/blob/024e8ec946f99c6bba8572f53e68055f8d9abd44/.github/workflows/windows-release.yml#L117)

The repeat benchmark asks YouTube Home twice before the first Music Home call. “First” means the first Music Home RPC in that process. The production browser keeps distinct Music and YouTube bootstrap contexts. A cold Music Home fetch downloads and parses the Music HTML and can use its embedded cards or request the browse API. Repeat Home requests remain live and can reuse the Music context. These branches explain why first and repeat calls can differ, but the per-RPC timings contain no per-request trace of HTML, API, or parse time. [Bootstrap code](https://github.com/Vanir20342222/vidcue/blob/024e8ec946f99c6bba8572f53e68055f8d9abd44/crates/vidcue-providers/src/browse.rs#L182-L231) · [Home fetch](https://github.com/Vanir20342222/vidcue/blob/024e8ec946f99c6bba8572f53e68055f8d9abd44/crates/vidcue-providers/src/browse.rs#L591-L609)

### Continuation results

All ten Android instrumentation invocations finished. Values are milliseconds. Each film observation reached at least three seconds of playback. The interval after activity creation includes buffering, decoding and presentation.

| Run | Music Home first | Music Home repeat | Sintel Play to activity | Activity to usable playback | Total | Confirmed position |
|---|---:|---:|---:|---:|---:|---:|
|1|1240|250|4024|9635|13659|3029|
|2|1268|371|3191|10578|13769|3112|
|3|1209|250|3298|9786|13084|3060|
|4|1209|303|3062|10927|13989|3015|
|5|1155|294|4164|8879|13043|3050|

The new Sintel median was 13.659 s. One actual run at that median split into 4.024 s to activity and 9.635 s afterwards. The modal opening, physical touch dispatch and per-request index timing are outside the scope of these clocks. All eight Sintel runs across the two sessions missed 500 ms. Music Home first-request median was 1.209 s and repeat median was 0.294 s. All eight first Music Home requests across both sessions missed 1 s. The separate earlier process-to-populated-feed median was 16.503 s.

The identical suite ran seven tests on each platform. Linux passed all seven. Android passed four and failed three: encrypted HLS publication, progressive MP4 publication and subtitle-directory publication. Each failed with the same Linux-only error during the success assertion. Single subtitle files, cancellation cleanup, interrupted-body cleanup and the rejected-range/budget case passed on both. This distinction matters because tests that expect no completed bundle can succeed on Android while successful publication is broken. Windows and macOS execution remain unverified. [Linux output](../../target/android-performance-evidence/overnight-024e8ec/continuation/linux.log) · [Android output](../../target/android-performance-evidence/overnight-024e8ec/continuation/android-native.log)

The earlier live music-output observations (2.300–3.473 s cold, 0.577–0.924 s prepared) and Media3 live download baseline (2.62 MiB/s median, SHA-256 verified) carry forward. Music playback and throughput were outside this repeat protocol. The Tab S9 remained unavailable over wireless ADB. The owner decision is whether the first implementation task should restore atomic movie saves on Android or prioritize movie startup. Resolving that priority does not require selecting a syscall, cache setting or range size. A reachable tablet is needed for hardware acceptance. PR #172 was still open and draft at 024e8ec at handback, with ten checks passed and three skipped. All work stayed local.

Stopped at 23:55 CET on 17 November. Both installed APK digests matched their archived originals after the test package was restored.

## Run 52, 18 November

Before timing, the open-findings list above was updated with run 51's results and evidence paths. For an independent next measurement, repeat the earlier live song and YouTube video tests, which had only one cold/prepared pair per title. Run `bash target/android-performance-evidence/overnight-024e8ec/run52/repeat-playback.sh` on API 35 emulator-5554. Verify both installed Preview APK digests against the archived PR head, then execute three rounds for each of the two default public videos and two songs, once cold and once prepared in each round (24 selections). Alternate suite order by round. The published tests clear the resolver cache before selection and wait for prepared entries; system and media caches can persist. Timing starts at selection, after prefetch when enabled. Record tap to first rendered frame and playing for video (<500 ms) or playing with advancing AudioTrack for music (around 100 ms), with three seconds of subsequent progress. Retain false starts and deadlines as failures. Separate each round's appended media timings and capture the per-process startup-stage report when new observations exist. No app, transport, prefetch or content changes.

### Run 52 measured playback results

| Run 52 selection | Observations | Median | Range | Met target | Status | Raw evidence |
|---|---:|---:|---:|---:|---|---|
| Video, cold | 6 | 5320.5 ms | 4237–8571 ms | 0/6 | Measured | [video1.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/video1.txt), [video2.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/video2.txt), [video3.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/video3.txt) |
| Video, prepared | 6 | 1644 ms | 1284–2847 ms | 0/6 | Measured | [video1.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/video1.txt), [video2.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/video2.txt), [video3.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/video3.txt) |
| Song, cold | 6 | 3002.5 ms | 2128–4501 ms | 0/6 | Measured | [song1.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/song1.txt), [song2.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/song2.txt), [song3.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/song3.txt) |
| Song, prepared | 6 | 714 ms | 456–913 ms | 0/6 | Measured | [song1.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/song1.txt), [song2.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/song2.txt), [song3.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/song3.txt) |

## 04:18 CET recovery and handback, 18 November

Runs 50 and 51 completed. Run 52 wrote the playback plan and launched its script before the owner-reported usage-limit interruption at 00:14 CET. The shell continued independently. All six invocations finished by 00:19:44 CET. Each log reports one completed instrumentation test, each result file has four positive startup observations and all 24 selections continued for three seconds. The runner used measurement mode, so its successful exit does not indicate a latency pass. Runs 53, 54 and 55 ended unsuccessfully at 00:42, 03:40 and 03:42 CET respectively during the reported usage-limit period. No additional test results were found for those attempts. Run 56 inspected the saved outputs after the expected reset. No repeat was necessary. [Start/end times](../../target/android-performance-evidence/overnight-024e8ec/run52/times.txt) · [Plan and command](../../target/android-performance-evidence/overnight-024e8ec/run52/repeat-playback.sh)



Each group covers two public titles over three rounds. Music uses advancing Android AudioTrack output. Raw observations are in run52/video1.txt through video3.txt and song1.txt through song3.txt. [Playback evidence](../../target/android-performance-evidence/overnight-024e8ec/run52/)

Sintel's run 51 median remains 13.659 s (4.024 s before activity, 9.635 s after). The time attributable to its end-of-file index remains unknown. Music Home's first request took 1.209 s median; the separate normal cold launch took 16.503 s median to draw populated content. The verified Media3 live download baseline remains 2.62 MiB/s. Native Android movie saves remain blocked at directory publication. Seven Linux fixture tests passed; on Android the MP4, encrypted HLS and subtitle-bundle publication cases failed and the four controls passed.

Only the emulator was connected at handback. Physical-device timing and maximum live throughput remain unqualified. The owner decision is which implementation to prioritize when changes resume. Restore native movie saves first to address a reproducible loss of functionality; a hardware movie-start trace would inform subsequent latency work. PR #172 remained open and draft at 024e8ec, with ten checks passed and three skipped. Git showed only the three pre-existing untracked local notes, including this updated one. No push, merge or production change.

## Run 56 recovery and evidence verified in run 57

Run 52 generated the playback samples on the API 35 emulator. Run 56 recovered and verified the samples after the usage-limit stops. Earlier movie, feed and download measurements below retain their original provenance.

* **Measured — live video, run 52.** Median cold startup was 5320.5 ms; median prepared startup was 1644 ms. All twelve selections exceeded 500 ms. [run52/video1.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/video1.txt), [run52/video2.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/video2.txt), [run52/video3.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/video3.txt).
* **Measured — live music, run 52.** Median cold startup was 3002.5 ms; median prepared startup was 714 ms. All twelve selections exceeded 100 ms. [run52/song1.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/song1.txt), [run52/song2.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/song2.txt), [run52/song3.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/song3.txt).
* **Measured — completion, verified in run 56.** Six invocations finished by 00:19:44 CET and produced 24 positive observations. The usage-limit interruptions ended agent turns; the previously launched measurement script completed. Run 56 supplied an audit, with zero new playback samples. [run52/times.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/times.txt); [run52/video1.log](../../target/android-performance-evidence/overnight-024e8ec/run52/video1.log), [run52/video2.log](../../target/android-performance-evidence/overnight-024e8ec/run52/video2.log), [run52/video3.log](../../target/android-performance-evidence/overnight-024e8ec/run52/video3.log); [run52/song1.log](../../target/android-performance-evidence/overnight-024e8ec/run52/song1.log), [run52/song2.log](../../target/android-performance-evidence/overnight-024e8ec/run52/song2.log), [run52/song3.log](../../target/android-performance-evidence/overnight-024e8ec/run52/song3.log).
* **Measured — movie startup, run 51.** Five Sintel starts had a 13.659 s median. The median observation spent 4.024 s before player activity creation and 9.635 s after it. [continuation/film1.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/film1.txt), [continuation/film2.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/film2.txt), [continuation/film3.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/film3.txt), [continuation/film4.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/film4.txt), [continuation/film5.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/film5.txt).
* **Hypothesised — Sintel index retrieval.** The end-of-file index location is verified. Its contribution to startup time has not been isolated. [mp4-layout.txt](../../target/android-performance-evidence/overnight-024e8ec/mp4-layout.txt).
* **Measured — feed.** The three earlier cold launches took 16.503 s median to draw provider cards. The five run 51 requests for Music Home took 1.209 s median on first request and 0.294 s on repeat, excluding rendering. [launch1.draw](../../target/android-performance-evidence/overnight-024e8ec/launch1.draw), [launch2.draw](../../target/android-performance-evidence/overnight-024e8ec/launch2.draw), [launch3.draw](../../target/android-performance-evidence/overnight-024e8ec/launch3.draw); [continuation/feed1.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/feed1.txt), [continuation/feed2.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/feed2.txt), [continuation/feed3.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/feed3.txt), [continuation/feed4.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/feed4.txt), [continuation/feed5.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/feed5.txt).
* **Measured — live download throughput.** The separate Media3 offline library delivered 2.62 MiB/s median over three hash-verified baseline downloads from one origin. Maximum throughput remains undetermined. [library.txt](../../target/android-performance-evidence/overnight-024e8ec/library.txt).
* **Blocked — native Android offline-bundle publication.** Encrypted HLS, progressive MP4 and subtitle-directory publication failed. Android passed the other four tests; all seven passed on Linux. Three earlier app-level movie saves also failed at publication. [continuation/android-native.log](../../target/android-performance-evidence/overnight-024e8ec/continuation/android-native.log); [continuation/linux.log](../../target/android-performance-evidence/overnight-024e8ec/continuation/linux.log); [movie-download-errors.txt](../../target/android-performance-evidence/overnight-024e8ec/movie-download-errors.txt).

### Owner decisions

1. Choose the first implementation priority. I recommend restoring Android movie saves first because the failure is reproducible without a live CDN. The alternative priority is reducing movie startup latency.
2. Identify when the Galaxy Tab S9 can be made available for wireless debugging on the test network. Hardware measurements are needed to qualify playback and feed timing.

At 04:43 CET the published PR was still open and draft at 024e8ec, with ten successful checks and three skipped. The separate scheduled Android run on main at 06d0479 passed one emulator shard and failed three. It ran a different revision and does not establish coverage of native movie publication on the PR. [Scheduled run on main](https://github.com/Vanir20342222/vidcue/actions/runs/44078110166). All changes remained local to the evidence note.

## Run 57 result and owner decisions

Run 57 audited the run 52 measurements and run 56 recovery, checked CI, and recorded the decisions for the owner. It produced zero new device samples. Its evidence file is the [archived text handback](../../target/android-performance-evidence/overnight-024e8ec/run57-summary.txt), which names the original timing files and test logs. The HTML preview failure is excluded from sample counts and conclusions.

| Finding | Status | Run 57 audit of existing results | Evidence file(s) |
|---|---|---|---|
| Run 57 audit | Measured | Six completed instrumentation invocations and 24 playback observations verified. Run 57 collected zero new device samples. | [run57-summary.txt](../../target/android-performance-evidence/overnight-024e8ec/run57-summary.txt), [times.txt](../../target/android-performance-evidence/overnight-024e8ec/run52/times.txt) |
| Movie startup (run 51) | Measured | Sintel median 13.659 s. In the median observation, 4.024 s elapsed before activity creation and 9.635 s afterwards. | [film1.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/film1.txt), [film2.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/film2.txt), [film3.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/film3.txt), [film4.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/film4.txt), [film5.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/film5.txt) |
| Sintel MP4 layout | Measured | The movie index begins at byte 310,076,814. | [mp4-layout.txt](../../target/android-performance-evidence/overnight-024e8ec/mp4-layout.txt) |
| Index retrieval as a cause of startup delay | Hypothesised | The player’s time spent retrieving the index was not isolated. | [mp4-layout.txt](../../target/android-performance-evidence/overnight-024e8ec/mp4-layout.txt) |
| Populated feed on cold launch | Measured | Median 16.503 s across three process launches. | [launch1.draw](../../target/android-performance-evidence/overnight-024e8ec/launch1.draw), [launch2.draw](../../target/android-performance-evidence/overnight-024e8ec/launch2.draw), [launch3.draw](../../target/android-performance-evidence/overnight-024e8ec/launch3.draw) |
| First Music Home provider request (run 51) | Measured | Median 1.209 s before rendering. | [feed1.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/feed1.txt), [feed2.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/feed2.txt), [feed3.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/feed3.txt), [feed4.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/feed4.txt), [feed5.txt](../../target/android-performance-evidence/overnight-024e8ec/continuation/feed5.txt) |
| Media3 live download throughput | Measured | Median 2.62 MiB/s over three hash-verified transfers from one origin. | [library.txt](../../target/android-performance-evidence/overnight-024e8ec/library.txt) |
| Native offline-bundle publication | Blocked | Android failed all three directory-publication cases. Linux passed all seven selected tests. | [android-native.log](../../target/android-performance-evidence/overnight-024e8ec/continuation/android-native.log), [linux.log](../../target/android-performance-evidence/overnight-024e8ec/continuation/linux.log), [movie-download-errors.txt](../../target/android-performance-evidence/overnight-024e8ec/movie-download-errors.txt) |
| Hardware qualification | Blocked | The Galaxy Tab S9 was unavailable for the recorded tests. | [run57-summary.txt](../../target/android-performance-evidence/overnight-024e8ec/run57-summary.txt) |

Owner decisions are implementation priority (restore native movie saves first, as recommended, or prioritize movie startup) and tablet availability for wireless testing. At the 05:13 CET check, PR #172 was open and draft at 024e8ec with ten successful checks and three skipped. No production changes, push or merge.

05:43 CET verification: each measured finding in the run 52 results and run 57 audit links to a local evidence file. Both implementation priority and tablet availability remain owner decisions. Draft PR status was reconfirmed at 024e8ec.

## Run 59 result (05:43 CET), archived at 06:13 CET

Run 59 verified the evidence links, separated run 52’s measurements from run 57’s audit, and confirmed the owner decision list. It added zero device samples. The [run 59 text evidence file](../../target/android-performance-evidence/overnight-024e8ec/run59-summary.txt) preserves both sets of findings and the source-file hashes checked at archival time.

| Status | Finding verified from earlier runs |
|---|---|
| Measured | Video target <500 ms. Live medians were 5320.5 ms cold and 1644 ms prepared. Sintel took 13.659 s in its median run, including 4.024 s before the player activity began. |
| Measured | Feed target <1 s. Cold launch to populated cards took 16.503 s median. The first Music Home RPC alone took 1.209 s median. |
| Measured | Music target about 100 ms. Median time to advancing AudioTrack was 3002.5 ms cold and 714 ms prepared. |
| Measured | Media3 download throughput was 2.62 MiB/s median across three verified downloads from one live origin. The maximum achievable rate remains unknown. |
| Measured | Sintel’s MP4 index begins at byte 310,076,814. |
| Hypothesised | Reading that end-of-file index contributes to the movie delay. No timing isolated this operation. |
| Blocked | Native Android offline-bundle publication failed for MP4, encrypted HLS and subtitle directories. The same seven tests passed on Linux. |
| Blocked | The unavailable Galaxy Tab S9 leaves physical-device acceptance open. |

Owner decisions: prioritize restoring movie saves (recommended because the failure reproduces with a local fixture) or reducing startup latency; identify when the tablet can be connected for hardware testing. PR #172 remained open and draft at 024e8ec at 06:13 CET, with ten checks passed and the APK, device and release-build jobs skipped. No fix, push or merge was performed.

## Run 60 evidence audit and handback

Run 60 completed at 06:13 CET and archived 26 source-file hashes for run 59. It collected zero new device samples. All 26 hashes matched the archived manifest at the 06:43 CET recheck. The [run 60 text record](../../target/android-performance-evidence/overnight-024e8ec/run60-summary.txt) identifies the manifest and preserves the classified findings.

| Status | Finding |
|---|---|
| Measured | Video under 500 ms: live medians 5.321 s cold and 1.644 s prepared. Median Sintel startup was 13.659 s, including 4.024 s before player activity creation. |
| Measured | Feed under 1 s: median 16.503 s to draw populated cards. The separate first Music Home request took 1.209 s median before rendering. |
| Measured | Music around 100 ms: median 3.003 s cold and 714 ms prepared to advancing Android audio output. |
| Measured | Media3 downloads: 2.62 MiB/s median on one origin, with file integrity verified. Maximum throughput remains undetermined. |
| Measured | Sintel's 597,191-byte index begins at byte 310,076,814. |
| Hypothesised | Fetching that index contributes to movie startup latency. Its individual duration remains unknown. |
| Blocked | Successful native publication of MP4, encrypted HLS and subtitle directories fails on Android. Four control tests passed there. All seven fixtures passed on Linux. |
| Blocked | Physical-device qualification awaits access to the Galaxy Tab S9. |

Owner decisions are the first implementation priority and the tablet testing window. Restoring movie saves is recommended first because the native failure reproduces with a local fixture. At 06:43 CET PR #172 was open and draft at 024e8ec. Ten checks passed; the APK, device and release-build jobs were skipped. No implementation, push or merge occurred.

## Run 61 result and owner decisions

Run 61 verified 26 archived hashes and recorded the run 60 handback. It collected zero additional device samples. The [run 61 text evidence](../../target/android-performance-evidence/overnight-024e8ec/run61-summary.txt) identifies the source records and preserves the findings below.

| Status | Finding |
|---|---|
| Measured | Video under 500 ms: live medians 5.321 s cold and 1.644 s prepared. Median Sintel startup was 13.659 s, including 4.024 s before player activity creation. |
| Measured | Feed under 1 s: median 16.503 s to draw populated cards. The separate first Music Home request took 1.209 s median before rendering. |
| Measured | Music around 100 ms: median 3.003 s cold and 714 ms prepared to advancing Android audio output. |
| Measured | Media3 downloads: 2.62 MiB/s median on one origin, with file integrity verified. Maximum throughput remains undetermined. |
| Measured | Sintel's 597,191-byte index begins at byte 310,076,814. |
| Hypothesised | Fetching that index contributes to movie startup latency. Its individual duration remains unknown. |
| Blocked | Successful native publication of MP4, encrypted HLS and subtitle directories fails on Android. Four control tests passed there. All seven fixtures passed on Linux. |
| Blocked | Physical-device qualification awaits access to the Galaxy Tab S9. |

**Owner decisions.** Choose whether to prioritize restoring movie saves or reducing startup latency. Restoring saves is recommended first because the failure reproduces with a local fixture. Identify a wireless testing window for the Galaxy Tab S9 so the emulator results can be checked on hardware.

07:13 CET check: the [Android test log](../../target/android-performance-evidence/overnight-024e8ec/continuation/android-native.log) records four passes and three failures; the [Linux log](../../target/android-performance-evidence/overnight-024e8ec/continuation/linux.log) records seven passes. PR #172 was open and draft at 024e8ec with ten successful checks and three skipped. All work stayed local.

## Final overnight summary — 07:43 CET, 18 November

Run 62 closed at 07:13 CET after documenting run 61 and checking the 26-file evidence manifest, publication logs and PR status. It added zero device samples. [Run 62 evidence file](../../target/android-performance-evidence/overnight-024e8ec/run62-summary.txt).

| Status | Finding |
|---|---|
| Measured | Video under 500 ms: live medians 5.321 s cold and 1.644 s prepared. Median Sintel startup was 13.659 s, including 4.024 s before player activity creation. |
| Measured | Feed under 1 s: median 16.503 s to draw populated cards. The separate first Music Home request took 1.209 s median before rendering. |
| Measured | Music around 100 ms: median 3.003 s cold and 714 ms prepared to advancing Android audio output. |
| Measured | Media3 downloads: 2.62 MiB/s median on one origin, with file integrity verified. Maximum throughput remains undetermined. |
| Measured | Sintel's 597,191-byte index begins at byte 310,076,814. |
| Hypothesised | Fetching that index contributes to movie startup latency. Its individual duration remains unknown. |
| Blocked | Successful native publication of MP4, encrypted HLS and subtitle directories fails on Android. Four control tests passed there. All seven fixtures passed on Linux. |
| Blocked | Physical-device qualification awaits access to the Galaxy Tab S9. |

**Owner decisions.** Choose the first implementation priority: restoring movie saves is recommended because a local fixture reproduces the failure. Provide a Galaxy Tab S9 wireless testing window for hardware acceptance.

At handback no overnight test runner was active. No fix or device run was started or resumed. PR #172 remained open and draft at 024e8ec. Nothing was pushed or merged.
