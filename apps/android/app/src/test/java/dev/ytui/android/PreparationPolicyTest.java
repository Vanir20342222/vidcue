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
        new String[] {key, key + "-failed"}, PreparationPolicy.progressKeys(key));
  }

  @Test
  public void nestedCollectionsAreSkippedNotResolved() {
    for (String kind : new String[] {"playlist", "album", "artist", "channel"})
      assertTrue(kind, PreparationPolicy.collection(kind));
    for (String kind : new String[] {"video", "song", "station", ""})
      assertFalse(kind, PreparationPolicy.collection(kind));
  }
}
