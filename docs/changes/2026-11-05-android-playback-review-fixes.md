# Android playback review fixes (M9)

- **Date:** 2026-11-05
- **PR:** none yet (local branch `fix/ar-playback`)
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified

## What it does
Fixes the playback findings of fix group A in `docs/audit/ANDROID_REVIEW_2026-11-05.md`:

- **AR-03** Returning from a preview continues the interrupted item where it was and as it was. It
  is no longer recorded in history or committed from the queue a second time, its saved-position
  lookup no longer runs (so it cannot jump back), and a paused item stays paused and silent
  (the pause intent is set before the handoff instead of pausing after starting).
- **AR-04** The 200 ms ticker runs only while the player wants to play and is buffering or ready
  (this covers SponsorBlock skips, position saves, previews and on-screen progress). Paused, ended,
  failed and stopped players stop ticking; a pending sleep countdown on a paused player ticks once,
  at its deadline. Play, seek, commands and player state changes restart it; the listeners still
  publish every state change. `stopAndClear` cancels it.
- **AR-05** Live reconnects re-resolve with the current quality and audio language.
- **AR-06** "End of this track" is a real end-of-track timer (`sleep` with `end_of_track: true`):
  it pauses when the item ends, before advancing (also on a repeat-one restart), whatever pauses,
  seeks or speed changes happened. The service publishes `sleep_end_of_track`; the badge, player
  button and options row show the track's remaining time at the current speed, the sheet says
  "until this track ends" and hides "+15 min" in that mode. "Cancel timer" shows only while a timer
  runs. Resuming after the pause replays the same item from the start.
- **AR-09** The default player gain is 1.0 (`PlaybackLifecycle.DEFAULT_VOLUME`) for listeners who
  never chose a volume; a saved volume is kept.
- **AR-12** The resume position is also stored when playback pauses (any cause), when another item
  replaces the current one, on `stopAndClear` and in `onDestroy` (history on, not previews, not
  live, never while the item's own saved-position lookup is in flight, never 0). Saves run in order
  on one executor that finishes queued writes after the service is destroyed.
- **AR-13** Downloads and local exports of YouTube videos look up SponsorBlock segments (a network
  request whose failure is ignored, as before). Other local files are still skipped. Offline
  captions stay deferred (AF-13).
- **AR-14** Video items use `AUDIO_CONTENT_TYPE_MOVIE`, audio-only items `AUDIO_CONTENT_TYPE_MUSIC`,
  set when an item starts (ExoPlayer still handles audio focus).

New logic lives in `PlaybackLifecycle.java` (pure decisions) and `SleepTimer.java`;
`PlaybackService.java` only wires them in.

### AR-24 investigation (scheduled CI, emulator shard 2)
Run 42014977667, job `device (2)` on a pixel_7 profile (1080x2400). The assertion at
`CatalogUiTest.kt:1418` found the "Cancel timer" row at top = 3455 px, more than 1000 px below the
2400 px display: the sheet had already slid off screen but was still composed. `Panel` dismisses a
sheet with `scope.launch { state.hide(); close() }`, so the row leaves composition only after the
hide animation finished and one more frame recomposed. This test drives the clock by hand
(`autoAdvance = false`, 38 frames for 600 ms) while a screen recording and the decoder load the
emulator, so the coroutine steps can land late in that window. There is only one "Close" node (the
sheet's own button) and the sheet does close, so this is a timing flake of the test, not a
user-visible bug; `ComposeHost.kt` is unchanged. #105 passed the same test.

Because "Cancel timer" now only shows while a timer runs, the two sheet checks changed:
`restyle…` now asserts "15 minutes" is shown and "Cancel timer" does **not** exist when no timer
runs (a stronger check), and `musicTransitions…` uses "15 minutes" as the sheet marker for both
the open and the closed assertion. No wait, frame count or budget was changed.

Observation for the owner (not changed): `musicTransitions…` has no `finally` that stops its
30 s `screenrecord` or restores the clock, so its failure leaves the recording running into the
next test. `tabletSearchUsesRowsWithoutChangingCardActions` failed 24 s later on the same shard
with a cover wait timeout, which fits that cascade. If the sheet flake recurs, stepping frames
until the sheet has left (with a bound) would fix it, but that lengthens a test wait and needs the
owner's decision.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Kotlin/Java compile, app and test APK sources | `apps/android/gradlew -p apps/android --no-daemon :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` | Passed | implemented |
| JVM unit tests | `apps/android/gradlew -p apps/android --no-daemon :app:testDebugUnitTest` | 183 passed, 0 failed (168 before + 15 new) | fixture/unit verified |
| New tests | `PlaybackLifecycleTest` (ticker policy, position-save rules, resume target, content type, SponsorBlock for downloads, preview restore request, default/saved volume), `SleepTimerTest` (countdown, end of track, cancel), `SleepTimerUiTest` (remaining time and share for countdown and end of track) | Passed | fixture/unit verified |
| CI log of run 42014977667 shard 2 | `gh run view 42014977667 --log --job 127206961965` | Read; analysis above | n/a |

## Not verified and limits
- No emulator, physical device or live service was run. The instrumentation changes in
  `CatalogUiTest.kt` were compiled but not executed.
- Preview restore, end-of-track pause (including repeat-one), ticker lapse while paused, pause and
  stop position saves, movie/music audio attributes and SponsorBlock on a downloaded video were not
  observed on a device.
- The settings sheet's volume slider in `ComposeHost.kt` still falls back to 0.7 when the status has
  no `volume`; the service always publishes it, so this only affects the empty snapshot.
- An end-of-track pause stays on the ended item; Play replays it rather than advancing.

## For the plan and parity tables
- docs/android/PARITY.md, playback: "Sleep timer offers a countdown and a real end-of-track mode
  (pauses at the item's end regardless of pause, seek or speed); fixture/unit verified, device
  pending."
- docs/android/VALIDATION.md: "AR-03/04/05/06/09/12/13/14 playback review fixes: JVM unit tests
  (183 passed) and app/test APK compile on 2026-11-05; no emulator or device run."
- Review list: AR-03, AR-04, AR-05, AR-06, AR-09, AR-12, AR-13 (SponsorBlock part), AR-14 fixed;
  AR-24 analysed as a test timing flake (no product change).
