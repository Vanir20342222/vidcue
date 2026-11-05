package dev.ytui.android;

import static org.junit.Assert.*;

import org.junit.Test;

public final class PlaybackPolicyTest {
  @Test
  public void onlyRejectedOrExpiredUrlsTriggerAutomaticRefresh() {
    assertTrue(PlaybackPolicy.refreshableStatus(403));
    assertTrue(PlaybackPolicy.refreshableStatus(410));
    for (int status : new int[] {0, 200, 400, 401, 404, 429, 500, 503})
      assertFalse(PlaybackPolicy.refreshableStatus(status));
  }

  @Test
  public void rustSongAndStationQueueEntriesRemainAudioOnly() {
    assertTrue(PlaybackPolicy.audioOnly("song"));
    assertTrue(PlaybackPolicy.audioOnly("station"));
    for (String kind : new String[] {"video", "playlist", "album", "artist", "channel", ""})
      assertFalse(PlaybackPolicy.audioOnly(kind));
  }

  @Test
  public void ordinaryYouTubeVideosAndCollectionsKeepVideoWithEitherSavePreference() {
    for (boolean enabled : new boolean[] {false, true})
      for (String kind : new String[] {"video", "playlist", "album", "artist", "channel"})
        assertFalse(kind, PlaybackPolicy.audioForSave(kind, false, enabled));
  }

  @Test
  public void musicSongsLinksAndCollectionsFollowTheSavePreference() {
    for (boolean enabled : new boolean[] {false, true}) {
      // Songs remain Music even when downloaded from history or a saved list.
      assertEquals(enabled, PlaybackPolicy.audioForSave("song", false, enabled));
      for (String kind : new String[] {"song", "video", "playlist", "album", "artist", "channel"})
        assertEquals(kind, enabled, PlaybackPolicy.audioForSave(kind, true, enabled));
      // The download preference never changes normal song playback.
      assertTrue(PlaybackPolicy.audioOnly("song"));
    }
  }

  @Test
  public void radioRemainsAudioRegardlessOfTheMusicSavePreference() {
    for (boolean enabled : new boolean[] {false, true})
      assertTrue(PlaybackPolicy.audioForSave("station", false, enabled));
  }

  @Test
  public void liveMetadataIsBoundedWithoutControlCodesOrBrokenUnicode() {
    assertEquals("First second", PlaybackPolicy.liveTitle("First\nsecond\t\033"));
    String title = PlaybackPolicy.liveTitle("🎵".repeat(600));
    assertEquals(512, title.codePointCount(0, title.length()));
    assertFalse(Character.isHighSurrogate(title.charAt(title.length() - 1)));
    assertEquals("\ufffd", PlaybackPolicy.liveTitle("\ud800"));
    assertEquals("", PlaybackPolicy.liveTitle(null));
  }

  @Test
  public void exportNamesCannotBecomePathsOrTruncateUnicodeSurrogates() {
    assertEquals("a_b_c_.mp4", PlaybackPolicy.exportName("a/b\\c\n", "mp4"));
    String longName = PlaybackPolicy.exportName("🎵".repeat(120), "webm");
    assertEquals(100, longName.substring(0, longName.length() - 5).codePointCount(0, 200));
    assertEquals("vidcue-media.m4a", PlaybackPolicy.exportName(" ", "m4a"));
  }
}
