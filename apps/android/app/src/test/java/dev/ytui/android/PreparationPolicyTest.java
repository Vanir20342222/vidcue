package dev.ytui.android;

import static org.junit.Assert.*;

import org.junit.Test;

public final class PreparationPolicyTest {
  @Test
  public void serviceCooldownNeverBecomesAnEntryFailure() {
    assertTrue(PreparationPolicy.rateLimited("HTTP 429"));
    assertTrue(PreparationPolicy.rateLimited("RATE LIMIT"));
    assertTrue(PreparationPolicy.rateLimited("Service cooldown"));
    assertFalse(PreparationPolicy.rateLimited("Video unavailable"));
  }

  @Test
  public void checkpointResumesEveryEntryThroughTheLast() {
    int cursor = 0;
    int count = 42;
    while (cursor < count) {
      cursor = PreparationPolicy.checkedCursor(cursor + 1, count);
    }
    assertEquals(count, cursor);
    assertEquals(0, PreparationPolicy.checkedCursor(0, 0));
  }

  @Test
  public void restartCarriesOnlyTheEntriesThatFailed() {
    String retry = "";
    retry = PreparationPolicy.withFailure(retry, 2);
    retry = PreparationPolicy.withFailure(retry, 5);
    assertArrayEquals(new int[] {2, 5}, PreparationPolicy.failures(retry, 6));
    assertArrayEquals(new int[0], PreparationPolicy.failures("", 6));
    assertArrayEquals(new int[0], PreparationPolicy.failures(null, 0));
  }

  @Test
  public void damagedRetryListIsRejected() {
    for (String bad : new String[] {"5,2", "2,2", "6", "x", ",", "-1", "1,"}) {
      try {
        PreparationPolicy.failures(bad, 6);
        fail(bad);
      } catch (IllegalArgumentException expected) {
      }
    }
  }

  @Test
  public void finishedRowSaysWhatRestartWillDo() {
    assertEquals("Playlist queued: 4 entries", PreparationPolicy.finished(4, 0, 0));
    assertEquals(
        "Playlist queued with 1 failure; Restart retries 1 entry",
        PreparationPolicy.finished(4, 1, 1));
    assertEquals(
        "Playlist queued with 3 failures; Restart retries 2 entries",
        PreparationPolicy.finished(4, 3, 2));
    // Skipped live entries count as failures but cannot be retried.
    assertEquals(
        "Playlist queued with 1 failure; nothing to retry", PreparationPolicy.finished(4, 1, 0));
  }

  @Test
  public void invalidCheckpointIsRejected() {
    for (int[] row : new int[][] {{-1, 2}, {3, 2}, {0, 2001}}) {
      try {
        PreparationPolicy.checkedCursor(row[0], row[1]);
        fail();
      } catch (IllegalArgumentException expected) {
      }
    }
  }
}
