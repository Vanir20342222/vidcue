package dev.ytui.android;

import static org.junit.Assert.*;

import android.content.Context;
import android.content.SharedPreferences;
import androidx.media3.common.C;
import androidx.media3.common.Player;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;

@RunWith(RobolectricTestRunner.class)
public final class PlaybackLifecycleTest {
  private static final int[] STATES = {
    Player.STATE_IDLE, Player.STATE_BUFFERING, Player.STATE_READY, Player.STATE_ENDED
  };

  @Test
  public void tickerRunsOnlyWhilePlaybackIsWantedAndLoadingOrPlaying() {
    for (int state : STATES) assertFalse(PlaybackLifecycle.tickActive(false, state));
    assertTrue(PlaybackLifecycle.tickActive(true, Player.STATE_READY));
    assertTrue(PlaybackLifecycle.tickActive(true, Player.STATE_BUFFERING));
    // Ended or failed/stopped playback has no skips, saves or progress to report.
    assertFalse(PlaybackLifecycle.tickActive(true, Player.STATE_ENDED));
    assertFalse(PlaybackLifecycle.tickActive(true, Player.STATE_IDLE));
  }

  @Test
  public void pausedPlayerStopsTickingUnlessASleepCountdownIsDue() {
    long now = 1_000_000;
    assertEquals(PlaybackLifecycle.TICK_MS, PlaybackLifecycle.tickDelay(true, 0, now));
    assertEquals(PlaybackLifecycle.TICK_MS, PlaybackLifecycle.tickDelay(true, now + 60_000, now));
    assertEquals(-1, PlaybackLifecycle.tickDelay(false, 0, now));
    // A paused countdown ticks once, at its deadline, so it cannot fire late after a resume.
    assertEquals(60_000, PlaybackLifecycle.tickDelay(false, now + 60_000, now));
    assertEquals(PlaybackLifecycle.TICK_MS, PlaybackLifecycle.tickDelay(false, now - 5, now));
  }

  @Test
  public void resumePositionsAreSavedOnlyForRealProgressOfOrdinaryMedia() {
    assertTrue(
        PlaybackLifecycle.savesPosition(true, false, false, false, Player.STATE_READY, 42_000));
    // Paused, buffering or failed players still have a meaningful position.
    for (int state : new int[] {Player.STATE_IDLE, Player.STATE_BUFFERING})
      assertTrue(PlaybackLifecycle.savesPosition(true, false, false, false, state, 42_000));
    assertFalse("history off", PlaybackLifecycle.savesPosition(
        false, false, false, false, Player.STATE_READY, 42_000));
    assertFalse("preview", PlaybackLifecycle.savesPosition(
        true, true, false, false, Player.STATE_READY, 42_000));
    assertFalse("live", PlaybackLifecycle.savesPosition(
        true, false, true, false, Player.STATE_READY, 42_000));
    assertFalse("lookup in flight", PlaybackLifecycle.savesPosition(
        true, false, false, true, Player.STATE_READY, 1_200));
    assertFalse("ended items were reset", PlaybackLifecycle.savesPosition(
        true, false, false, false, Player.STATE_ENDED, 300_000));
    assertFalse("start", PlaybackLifecycle.savesPosition(
        true, false, false, false, Player.STATE_READY, 0));
  }

  @Test
  public void resumeKeepsDesktopRulesForAutomaticAndExplicitResume() {
    assertEquals(-1, PlaybackLifecycle.resumeTarget(-1, false));
    assertEquals(-1, PlaybackLifecycle.resumeTarget(-1, true));
    assertEquals(0, PlaybackLifecycle.resumeTarget(0, false));
    assertEquals(3_000, PlaybackLifecycle.resumeTarget(3_000, false));
    assertEquals(-1, PlaybackLifecycle.resumeTarget(3_000, true));
    assertEquals(
        PlaybackService.AUTO_RESUME_MIN_MS,
        PlaybackLifecycle.resumeTarget(PlaybackService.AUTO_RESUME_MIN_MS, true));
  }

  @Test
  public void videoUsesMovieAndAudioOnlyUsesMusicAttributes() {
    assertEquals(C.AUDIO_CONTENT_TYPE_MOVIE, PlaybackLifecycle.contentType(false));
    assertEquals(C.AUDIO_CONTENT_TYPE_MUSIC, PlaybackLifecycle.contentType(true));
  }

  @Test
  public void downloadedYouTubeMediaStillLooksUpSponsorSegments() {
    String video = "https://www.youtube.com/watch?v=abcdefghijk";
    assertTrue(PlaybackLifecycle.fetchesSponsorMarks(false, false, video));
    assertTrue(PlaybackLifecycle.fetchesSponsorMarks(true, false, video));
    assertTrue(PlaybackLifecycle.fetchesSponsorMarks(true, false, "https://youtu.be/abcdefghijk"));
    assertTrue(PlaybackLifecycle.fetchesSponsorMarks(
        true, false, "https://music.youtube.com/watch?v=abcdefghijk"));
    assertTrue(PlaybackLifecycle.fetchesSponsorMarks(false, true, video));
    // Other local files have no YouTube ID; online media keeps its previous behaviour.
    assertFalse(PlaybackLifecycle.fetchesSponsorMarks(false, true, "content://media/file/12"));
    assertFalse(PlaybackLifecycle.fetchesSponsorMarks(true, false, "https://example.com/a.mp4"));
    assertFalse(PlaybackLifecycle.fetchesSponsorMarks(true, false, "https://notyoutube.com/x"));
    assertFalse(PlaybackLifecycle.fetchesSponsorMarks(true, true, ""));
    assertFalse(PlaybackLifecycle.fetchesSponsorMarks(true, true, null));
    assertFalse(PlaybackLifecycle.fetchesSponsorMarks(true, true, "not a url"));
    assertTrue(PlaybackLifecycle.fetchesSponsorMarks(false, false, "https://example.com/a.mp4"));
  }

  @Test
  public void previewRestoreContinuesWithoutResumeLookupOrNewStart() throws Exception {
    JSONObject media = new JSONObject().put("url", "https://www.youtube.com/watch?v=abcdefghijk");
    JSONObject interrupted =
        new JSONObject()
            .put("media", media)
            .put("stream", new JSONObject().put("url", "https://example.invalid/v"))
            .put("resume", true)
            .put("auto_resume", true)
            .put("quality", "720")
            .put("context_id", "ctx");
    JSONObject restore = PlaybackLifecycle.previewRestoreRequest(interrupted);
    assertFalse(restore.getBoolean("resume"));
    assertFalse(restore.getBoolean("auto_resume"));
    // play() keeps the caller's pause intent and recorded start for reconnect requests.
    assertTrue(restore.getBoolean("reconnect"));
    assertEquals("720", restore.getString("quality"));
    assertEquals("ctx", restore.getString("context_id"));
    assertEquals(media.toString(), restore.getJSONObject("media").toString());
    // The interrupted request itself is not changed.
    assertTrue(interrupted.getBoolean("resume"));
    assertFalse(interrupted.has("reconnect"));
  }

  @Test
  public void fullGainIsTheDefaultButASavedVolumeIsKept() {
    SharedPreferences prefs =
        RuntimeEnvironment.getApplication()
            .getSharedPreferences("player-volume-test", Context.MODE_PRIVATE);
    prefs.edit().clear().commit();
    assertEquals(1f, PlaybackLifecycle.savedVolume(prefs), 0f);
    prefs.edit().putFloat("volume", 0.7f).commit();
    assertEquals(0.7f, PlaybackLifecycle.savedVolume(prefs), 0f);
    prefs.edit().putFloat("volume", 0.25f).commit();
    assertEquals(0.25f, PlaybackLifecycle.savedVolume(prefs), 0f);
  }
}
