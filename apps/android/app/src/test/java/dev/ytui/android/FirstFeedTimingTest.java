package dev.ytui.android;

import static org.junit.Assert.*;
import org.junit.Test;

public final class FirstFeedTimingTest {
  @Test
  public void loadingAndTransparentGridsCannotCompleteTheProcessStartSample() {
    FirstFeedTiming timing = new FirstFeedTiming();
    assertEquals(-1, timing.drawn(100, true, true));
    timing.begin(100);
    assertEquals(-1, timing.drawn(99, true, true));
    assertEquals(-1, timing.drawn(200, false, true));
    assertEquals(-1, timing.drawn(300, true, false));
    assertEquals(450, timing.drawn(550, true, true));
    assertEquals(-1, timing.drawn(900, true, true));
  }
}
