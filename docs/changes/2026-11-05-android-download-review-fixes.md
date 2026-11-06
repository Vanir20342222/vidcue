# Android download review fixes AR-15 to AR-18 and AR-20 (M9)

- **Date:** 2026-11-05
- **PR:** none yet
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified

## What it does
Fixes the download findings AR-15, AR-16, AR-17, AR-18 and AR-20 from
`docs/audit/ANDROID_REVIEW_2026-11-05.md`. AR-19 (export cache retention) was deferred and is
unchanged.

- **AR-15:** `OfflineService.getScheduler()` now returns a Media3 `PlatformScheduler` with a
  fixed JobScheduler id, and the manifest declares
  `PlatformScheduler$PlatformSchedulerService` with `BIND_JOB_SERVICE` and `exported=false`.
  Media3 1.8 asks a foreground `DownloadService` for its scheduler only below Android 12 (the
  bytecode checks `SDK_INT < 31`). So the job restarts network-stopped downloads on Android 8–11.
  On Android 12 and later Media3 keeps the service in the foreground while it waits, which it did
  before this change too. `OfflineService` now also handles Android 15's dataSync timeout
  (`onTimeout`) by stopping itself, so the system does not crash the app after 6 hours a day.
- **AR-16:** when a download or its `-audio` part fails with HTTP 403 or 410, the app resolves
  the stream again once and re-adds the download. This uses the manual retry's path
  (`resolve`, then `add`). The re-added request carries an `auto_refreshed` flag, and a flagged
  request is never refreshed automatically again. A manual retry clears the flag. An in-memory
  set prevents duplicate refreshes when both parts fail together. A refresh does not bring back a
  download the user removed meanwhile.
  `OfflineStore.add` now checks free space before it queues a download. It uses
  `StorageManager.getAllocatableBytes`, falling back to `getUsableSpace`. The space needed is the
  stream's `clen`/`filesize` size, minus bytes already downloaded, plus 64 MB. If the size is
  unknown, it is a 200 MB floor. Without that space, the app reports "Not enough free space to
  download: needs about N MB, M MB free" and does not start the download. A playlist preparation
  stops with a status asking the user to free space. It keeps its checkpoint, so a retry resumes
  at that entry.
  The new Settings › Offline & storage toggle "Download on Wi-Fi only" (`download_unmetered`,
  default off) sets the DownloadManager requirement to `NETWORK_UNMETERED`. The setting applies
  when the manager is created and whenever settings are saved.
- **AR-17:** playlist preparation jobs now have a stable job key, stored in the work input. The
  downloads row (`job:<key>`), the `prepare-<key>.json` checkpoint and the
  `preparations-progress` cursor all use that key. The work is unique work
  (`ytui-prepare-<key>`). Retry replaces that work under the same key, so the row stays single
  and progress continues. Rows created before this change use their old work id as the key and
  keep working. Removing a job deletes its checkpoint and progress keys. Success and final
  failures also delete them, except retry exhaustion and low space, which keep them so a retry
  resumes. A stopped run whose row was removed meanwhile cleans up what it wrote, and stopped
  runs no longer write status rows. Failed-entry-only retry is not part of this change; PR #78
  covers it.
- **AR-18:** preparation work requires `NetworkType.CONNECTED`. Exceptions and rate limits
  retry with backoff for at most 8 runs (`PreparationPolicy.MAX_RUNS`). After that, the row shows
  "Playlist preparation stopped after 8 attempts; retry from downloads".
- **AR-20:** the download notification's title is "Vidcue downloads". Tapping it opens
  `RustActivity`. It counts each video once, so a video and its `-audio` part are one item. While
  parts download it shows aggregate progress: bytes when every part's length is known, otherwise
  the average percentage. It says when it is waiting for Wi-Fi or a network. The export
  notification's title is "Vidcue: exporting offline media". The notification channel ids stay
  unchanged so existing user channel settings are kept.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Compile app and instrumentation sources, JVM tests, lint | `flock /tmp/vidcue-build.lock apps/android/gradlew -p apps/android --no-daemon :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest :app:lintDebug` | BUILD SUCCESSFUL; 179 JVM tests, 0 failures (baseline 168); no new lint findings in the changed files | fixture/unit verified |
| 403/410 detection, refresh-once flag, space estimate, Wi-Fi requirement, notification counting/progress | `DownloadPolicyTest` (Robolectric) | 8 tests pass | fixture/unit verified |
| Retry bound, stable unique work name and progress keys | `PreparationPolicyTest` | 2 new tests pass | fixture/unit verified |
| Checkpoint and progress cleanup is limited to one job | `PrepareWorkerTest.purgeRemovesOnlyThatJobsCheckpointAndProgress` (Robolectric) | passes | fixture/unit verified |
| Media3 scheduler behaviour | `javap` of `DownloadService.onCreate` in media3-exoplayer 1.8.0 | `getScheduler()` is called only when there is no foreground notification or `SDK_INT < 31` | source inspection |

## Not verified and limits
- No emulator or physical device was run. JobScheduler restart, Wi-Fi-only switching, a real
  expired-URL refresh, the low-space refusal, notification taps and progress, and the Android 15
  dataSync timeout are not exercised on a device.
- The WorkManager unique-work replacement in `OfflineStore.toggle("job:…")` has no automated
  test. The JVM tests have no WorkManager test artifact, and running the worker would need the
  native engine. Only the key, name and cleanup helpers are unit tested.
- On Android 12 and later, a download whose process is killed while it waits for a network still
  resumes only when the app or a new download starts the service. Media3 does not use a scheduler
  there, and Android does not allow a dataSync foreground service to start from the background.
- After the Android 15 timeout stops the service, downloads continue only while the process
  lives. They resume when the app is next opened.
- The free-space check covers one download at a time. It does not reserve space for other queued
  downloads.
- The automatic refresh happens once per user-started stream. If the refreshed link fails again
  with 403 (for example, a format that needs a token), the download stays failed until a manual
  retry.

## For the plan and parity tables
- docs/android/PARITY.md (downloads): "Expired download links are re-resolved once
  automatically. Downloads check free space before they start. Optional Wi-Fi-only downloads.
  Playlist preparation needs a network, retries at most 8 runs, and resumes in place on retry.
  The download notification is Vidcue-branded and shows progress. Fixture/unit verified only;
  device check pending."
- docs/android/VALIDATION.md: "2026-11-05 AR-15–AR-18, AR-20 download fixes: JVM 179/179 and
  lintDebug pass; no emulator or device run."
