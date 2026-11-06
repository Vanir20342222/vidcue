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

  @Test
  public void retriesStopAfterTheLastAllowedRun() {
    int runs = 0;
    while (PreparationPolicy.retryAllowed(runs)) runs++;
    // Run attempt counts start at 0, so the last allowed run is MAX_RUNS - 1.
    assertEquals(PreparationPolicy.MAX_RUNS - 1, runs);
    assertFalse(PreparationPolicy.retryAllowed(Integer.MAX_VALUE - 1));
  }

  @Test
  public void aRetriedJobKeepsItsWorkNameAndProgressKeys() {
    String key = "8f0c2a4e-0000-4000-8000-000000000001";
    assertEquals(PreparationPolicy.uniqueWork(key), PreparationPolicy.uniqueWork(key));
    assertNotEquals(PreparationPolicy.uniqueWork(key), PreparationPolicy.uniqueWork(key + "x"));
    assertArrayEquals(
        new String[] {key, key + "-failed", key + "-retry"}, PreparationPolicy.progressKeys(key));
  }

  @Test
  public void nestedCollectionsAreSkippedNotResolved() {
    for (String kind : new String[] {"playlist", "album", "artist", "channel"})
      assertTrue(kind, PreparationPolicy.collection(kind));
    for (String kind : new String[] {"video", "song", "station", ""})
      assertFalse(kind, PreparationPolicy.collection(kind));
  }
}
