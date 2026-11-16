package dev.ytui.android;

/** One process-start-to-populated-draw sample. Contains no feed or account identifiers. */
final class FirstFeedTiming {
  static final FirstFeedTiming current = new FirstFeedTiming();
  private long start = -1;
  private boolean recorded;

  synchronized void begin(long processStart) {
    start = processStart;
    recorded = false;
  }

  /** Returns a sample only once, after populated content is drawn at full opacity. */
  synchronized long drawn(long now, boolean populated, boolean fullyVisible) {
    if (recorded || start < 0 || now < start || !populated || !fullyVisible) return -1;
    recorded = true;
    return now - start;
  }
}
