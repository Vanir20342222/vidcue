# Android bug and feature review (M9), 2026-11-05

Source review of `apps/android` and `crates/vidcue-mobile` at `main` 61ef259 (after #107).
Findings were read from the code; none was reproduced on a device. No Android SDK, emulator or
device was available when the review was written. Each fix records its own evidence in a
`docs/changes/` note; this file only tracks the list.

Status values: **open**, **fixing** (assigned to a fix group), **fixed** (code changed, see note),
**deferred** (needs an owner decision or is a larger feature).

## Checks run during the review

| Check | Result |
| --- | --- |
| `cargo test -p vidcue-mobile` (Rust 1.99.1) | 70 passed |
| `cargo test --workspace` | 467 passed, 0 failed, 31 ignored (opt-in live/desktop/tool tests) |
| Scheduled Android CI on `main` adb4947 (run 42014977667) | **Failed**: emulator shard 2, see AR-24 |
| APK build, Gradle JVM tests, emulator, physical device | Not run during the review |

## Bugs

| ID | Severity | Area | Problem | Where | Fix group | Status |
| --- | --- | --- | --- | --- | --- | --- |
| AR-01 | High | Extraction | The isolated JS sandbox connection is created once and cached forever. If the sandbox process dies (WebView update, low-memory kill) or the first connection fails, every later challenge solve fails until the app process restarts. The error message also embeds a main-thread stack trace. | `ScriptRuntime.java:17`, `:74` | B runtime | fixed |
| AR-02 | High | Startup | `NativeBridge.init` returns an error string that is ignored, so a failed engine start shows only "Engine is not initialized" everywhere. | `YtuiApplication.java:9` | B runtime | fixed |
| AR-03 | Medium | Playback | Returning from a preview restarts the interrupted item through `play()`: it is recorded in history again, and its delayed resume seek can override the restored position. | `PlaybackService.finishPreview` | A playback | fixed |
| AR-04 | Medium | Battery | The 200 ms ticker reposts itself for the whole service lifetime, including while paused, stopped or in the background, and rebuilds the full status JSON each time. | `PlaybackService.ticker` | A playback | fixed |
| AR-05 | Low | Playback | Live reconnect re-resolves without the selected quality or audio language. | `PlaybackService.reconnect` | A playback | fixed |
| AR-06 | Medium | Sleep timer | "End of this track" is a wall-clock countdown; pausing, seeking or changing speed fires it at the wrong time. "Cancel timer" shows when no timer runs. | `Notices.kt:336`, `PlaybackService` | A playback | fixed |
| AR-07 | Medium | Links | Pasted/shared links take their media kind from the current tab: on Radio a YouTube video becomes a live station, on Music any video plays audio-only. | `RustRepository.linked` | D links/fullscreen | fixed |
| AR-08 | Medium | Sharing | Shared text only fills the search box; text such as "Title https://youtu.be/…" runs a text search instead of opening the link. YouTube links cannot be opened from other apps (no VIEW intent filter). | `RustActivity.onNewIntent`, `ComposeHost.kt` share effect, manifest | D links/fullscreen | fixed |
| AR-09 | Low | Audio | Player gain defaults to 0.7 on top of system volume, so new users hear everything quieter than other apps. | `PlaybackService.java:219` | A playback | fixed |
| AR-10 | Medium | Fullscreen | Fullscreen is always `sensorLandscape`; portrait videos and Shorts are letterboxed. | manifest, `FullscreenActivity` | D links/fullscreen | fixed |
| AR-11 | Low | Fullscreen | Video is unbound in `onPause` (black in split-screen while visible); the screen is kept on while paused. | `FullscreenActivity.java:41`, `:121` | D links/fullscreen | fixed |
| AR-12 | Low | Resume | Position is saved every 5 s only while playing, never on pause or stop. | `PlaybackService.ticker` | A playback | fixed |
| AR-13 | Low | Offline | Downloaded media never gets SponsorBlock marks (and has no captions). | `PlaybackService.java:805`, `:901` | A playback (SponsorBlock); captions deferred | SponsorBlock fixed; captions deferred (AF-13) |
| AR-14 | Low | Audio | All media, including video, uses `AUDIO_CONTENT_TYPE_MUSIC`. | `PlaybackService.java:211` | A playback | fixed |
| AR-15 | Medium | Downloads | `OfflineService.getScheduler()` returns null, so downloads interrupted by network loss do not resume until the app is opened. | `OfflineService.java:20` | C downloads | fixed (Android 8–11; Media3 already waits on 12+), see download note |
| AR-16 | Medium | Downloads | Signed stream URLs expire; paused or long-queued downloads fail with HTTP 403 and need a manual retry. No free-space check. | `OfflineStore` | C downloads | fixed |
| AR-17 | Medium | Downloads | Retrying a playlist job cancels it and enqueues a new job from entry 0; the old row stays. `preparations-progress` keys and `prepare-<id>.json` checkpoints are never cleaned. | `OfflineStore.toggle/remove`, `PrepareWorker` | C downloads | fixed |
| AR-18 | Medium | Downloads | Playlist preparation has no network constraint and retries forever on any exception. | `PrepareWorker.enqueue/doWork` | C downloads | fixed |
| AR-19 | Low | Storage | Exported files keep their internal cache copy, so saved media uses twice the space. Changing this alters offline playback; needs an owner decision. | `OfflineExport` | — | deferred |
| AR-20 | Low | Branding/UX | Notifications still say "ytui downloads" and "ytui: exporting offline media"; the download notification has no tap action or progress and counts paired audio parts. | `OfflineService.java:28`, `OfflineExport.java:439` | C downloads | fixed |
| AR-21 | Low | Size | The retired egui UI (`ui.rs`, `platform.rs`, eframe/glow/game-activity) and unused natives (`frameAvailable`, `uiGeneration`, `uiShutdowns`, `videoFrame`, `releaseVideo`) still ship in the APK. | `crates/vidcue-mobile`, `MediaBridge.java`, `NativeBridge.java`, Gradle | E cleanup | fixed |
| AR-22 | Low | Security | `usesCleartextTraffic="true"` app-wide instead of a scoped network security config. Radio stations still need HTTP, so the scope must remain media-only. | manifest | B runtime | fixed for Java HTTP; the Rust client is not covered |
| AR-23 | Low | UX | Notification permission is requested on first launch, before onboarding. | `RustActivity.onCreate` | B runtime | fixed |
| AR-24 | Medium | CI | Scheduled emulator shard 2 failed on `main`: `CatalogUiTest.musicTransitions…` found "Cancel timer" after closing the sleep-timer sheet; `tabletSearchUsesRows…` timed out waiting for covers. #105 passed, so this may be flaky, but the sheet failing to close may be a regression. | `CatalogUiTest.kt:1418`, `:1523` | A playback (investigate) | analysed: timing flake; sheet checks updated, see playback note |

## Missing features (backlog, not assigned)

| ID | Feature | Notes |
| --- | --- | --- |
| AF-01 | Picture-in-picture | No `supportsPictureInPicture` or PiP entry. |
| AF-02 | Auto-fullscreen on rotate | Fullscreen only from the button. |
| AF-03 | Chapters, comments, clickable timestamps/links in descriptions | Not implemented on Android. |
| AF-04 | Repeat whole queue | Only off / repeat-one; the repeat-all icon is unused. |
| AF-05 | Item actions: Play next, Go to channel, Share / Copy link, Start radio, Remove from history | Card sheet has Play, Queue, Download, Preview, Play all, Bookmark, Browse tracks. |
| AF-06 | Channel pages, subscribe, like | Read-only account; mutations are intentionally absent on desktop too. |
| AF-07 | Android Auto / Wear browsing (`MediaLibraryService`) | Only `MediaSessionService`. |
| AF-08 | Home-screen widget | None. |
| AF-09 | Localization | All Android strings are hard-coded English; desktop has German. |
| AF-10 | Wi-Fi-only downloads, mobile-data quality, storage cap / low-space warning | Part of AR-16 work adds Wi-Fi-only and a space check. |
| AF-11 | Failed-entry-only playlist retry | Open PR #78 covers part of it. |
| AF-12 | Remote YouTube watch history | Local history only. |
| AF-13 | Offline captions | Downloads drop the caption catalog. |

Documented open gates in `docs/android/PARITY.md` (live YouTube, physical device, audible output,
OEM battery, drag reorder, queue rebuild after process death, HLS/MP3/FLAC export) still apply.

## Fix groups

Each group owns separate files so fixes can be developed in parallel:

- **A playback**: `PlaybackService.java`, `Notices.kt` sleep options, related tests.
- **B runtime**: `ScriptRuntime.java`, `YtuiApplication.java`, `RustActivity.java` permission timing, manifest network security config.
- **C downloads**: `OfflineStore.java`, `OfflineService.java`, `PrepareWorker.java`, `OfflineExport.java` notification.
- **D links/fullscreen**: `RustRepository.linked/search`, share handling, `FullscreenActivity.java`, manifest intent filter and orientation.
- **E cleanup**: retired egui UI and unused JNI entry points.

## Fix results (2026-11-05)

Fixed on branch `ccr-d57b5bbb-h4akie`. Each group's evidence and limits are in its change note:
[playback](../changes/2026-11-05-android-playback-review-fixes.md),
[runtime](../changes/2026-11-05-android-runtime-review-fixes.md),
[downloads](../changes/2026-11-05-android-download-review-fixes.md),
[links/fullscreen](../changes/2026-11-05-android-links-fullscreen-review-fixes.md),
[legacy UI removal](../changes/2026-11-05-android-remove-legacy-egui.md).
All fixes are fixture/unit verified only: Android compile, lint and 218 JVM tests pass on the merged
branch. No emulator, physical device or live service was used.
