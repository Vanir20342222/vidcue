# Android link sharing and fullscreen review fixes (M9)

- **Date:** 2026-11-05
- **PR:** none yet
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified

Fixes AR-07, AR-08, AR-10 and AR-11 from
[docs/audit/ANDROID_REVIEW_2026-11-05.md](../audit/ANDROID_REVIEW_2026-11-05.md) (fix group D).

## What it does
- **AR-07:** A pasted, shared or opened link gets its media kind from the URL, not from the
  current tab. `music.youtube.com` links play as songs (audio). `youtube.com`, `m.youtube.com`,
  `youtu.be` and `youtube-nocookie.com` links are videos, including `/shorts/ID`, `/live/ID` and
  `/embed/ID`, on every tab. `/playlist?list=ID` stays a playlist. A `/watch` URL that has
  `list=` but no `v=` now opens as that playlist; with `v=` it still plays the video, as before.
  Only a non-YouTube HTTP(S) link still depends on the tab: on Radio it is a live station,
  elsewhere a video (it was audio-only on Music before). The rules are in the pure
  `SharedLinks.classify` (`SharedLinks.kt`); `YtuiViewModel.linked` calls it.
- **AR-08:** Shared text such as "Title https://youtu.be/ID" plays the first YouTube media link
  it contains (video, Short, live stream, playlist or YouTube Music song) instead of running a
  text search. A new `SharedLinkPlayer` composable plays it once the engine reports ready and
  waits while first-run setup is shown. Text without a YouTube media link still goes into the
  search box, as before. Typed search text containing such a link plays it too. `RustActivity` now
  has an `ACTION_VIEW` filter (`DEFAULT` + `BROWSABLE`, `http`/`https`, hosts `youtube.com`,
  `www.youtube.com`, `m.youtube.com`, `music.youtube.com` with `/watch`, `/playlist`, `/shorts/`,
  `/live/`, and `youtu.be` with `/ID`). Opened links take the same path as shares. A null or blank
  `EXTRA_TEXT` is ignored. The launch intent is not handled again when the activity is recreated
  from saved state or relaunched from Recents.
- **AR-10:** `FullscreenActivity` is no longer forced to `sensorLandscape`. It requests
  `SENSOR_PORTRAIT` for a video taller than it is wide (pixel aspect ratio applied) and
  `SENSOR_LANDSCAPE` otherwise. It updates on `onVideoSizeChanged` from the bound player. An
  unknown size keeps the current request, and the first request without a size is landscape.
  `configChanges` is unchanged, so rotation does not recreate the activity.
- **AR-11:** The fullscreen player is bound in `onStart` and unbound in `onStop`, using the same
  `VisibleFrameTracker.bind` as before. A split-screen neighbour taking focus therefore no
  longer blanks the video. `FLAG_KEEP_SCREEN_ON` is gone: the player view keeps the screen on
  only while `playWhenReady` is true and the player is neither idle nor ended, following
  `onPlayWhenReadyChanged` and `onPlaybackStateChanged`.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Kotlin/Java compile, test APK sources, JVM tests | `flock /tmp/vidcue-build.lock apps/android/gradlew -p apps/android --no-daemon :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest` | BUILD SUCCESSFUL; 178 JVM tests, 0 failures (baseline 168 + 10 new) | fixture/unit verified |
| URL classification and extraction | `SharedLinksTest` (7 tests): every YouTube host and path form on Radio and non-Radio tabs, Music songs, playlists, list-only watch URLs, look-alike hosts, rejected URLs, URL extraction with punctuation and parentheses, first-media-link choice | Passed | fixture/unit verified |
| Orientation and screen-on choice | `FullscreenPolicyTest` (3 tests): landscape, portrait, square, anamorphic, unknown size, play/pause/ended/idle | Passed | fixture/unit verified |
| Android lint | `flock /tmp/vidcue-build.lock apps/android/gradlew -p apps/android --no-daemon :app:lintDebug` | BUILD SUCCESSFUL; two new `AppLinkWarning` warnings for the VIEW filters (no `autoVerify`, by design) | implemented |

The existing instrumented checks were reviewed and still compile. `NativeUiTest.pastedPlaylistAndMusicUrlsKeepTheirProviderIntent`
asserts behaviour this change keeps. `MediaRuntimeTest` expects fullscreen to be landscape for
the 320x180 fixture, which the size-based choice still gives. Neither was run here.

## Not verified and limits
- No emulator or physical device was run. Sharing from another app, opening a YouTube link from a
  browser or chat app, rotation into portrait for a Short, split-screen video and screen timeout
  in fullscreen are not verified on Android.
- The VIEW filters are not verified App Links (no `android:autoVerify`). On Android 12 and later,
  a tapped YouTube link opens in Vidcue only if the user turns on these links under the app's
  "Open by default" settings or picks Vidcue in a chooser. Lint reports `AppLinkWarning` for this.
- Only YouTube and YouTube Music links play directly from a share. Shared links to other sites go
  to the search box, where Enter still opens them as a link.
- A share that arrives just before Android kills the process, and has not yet been handled, is
  not restored after recreation.
- On large screens and in multi-window mode Android may ignore the requested orientation.

## For the plan and parity tables
- docs/android/PARITY.md: "Links shared from other apps or opened from YouTube URLs (VIEW intent
  filter, not verified App Links) play directly; link kind follows the URL, not the tab. Unit
  verified 2026-11-05; device sharing not verified."
- docs/android/PARITY.md: "Fullscreen follows the video's orientation (portrait for Shorts and
  vertical videos), stays bound while visible in split screen, and lets the screen sleep while
  paused. Unit verified 2026-11-05; emulator/device not verified."
- docs/audit/ANDROID_REVIEW_2026-11-05.md: AR-07, AR-08, AR-10, AR-11 status -> fixed (see this note).
