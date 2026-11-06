package dev.ytui.android;

/**
 * Sleep timer state for PlaybackService: either a wall-clock countdown or "end of this track",
 * which pauses when the playing item ends instead of advancing to the next one. Times are
 * SystemClock.elapsedRealtime milliseconds. Not thread-safe; the service uses it on the main
 * thread only.
 */
final class SleepTimer {
  private long deadline, total;
  private boolean endOfTrack;

  /** Starts a countdown of {@code seconds} (at least one) from {@code now}. */
  void countdown(long now, long seconds) {
    total = Math.max(1, seconds) * 1000;
    deadline = now + total;
    endOfTrack = false;
  }

  /** Pauses when the current item ends, however long that takes (pause, seek, speed). */
  void endOfTrack() {
    deadline = 0;
    total = 0;
    endOfTrack = true;
  }

  void cancel() {
    deadline = 0;
    total = 0;
    endOfTrack = false;
  }

  boolean active() {
    return deadline > 0 || endOfTrack;
  }

  /** Elapsed-realtime deadline of a countdown, 0 when none runs. */
  long deadline() {
    return deadline;
  }

  /** Length of the running countdown in ms, 0 when none runs. */
  long total() {
    return total;
  }

  boolean pausesAtEndOfTrack() {
    return endOfTrack;
  }

  /** True once when the countdown has run out; the timer is then off. */
  boolean expire(long now) {
    if (deadline <= 0 || now < deadline) return false;
    deadline = 0;
    total = 0;
    return true;
  }

  /**
   * Called when the playing item reached its end. True once when playback must pause there
   * rather than continue; the timer is then off.
   */
  boolean itemEnded() {
    if (!endOfTrack) return false;
    endOfTrack = false;
    return true;
  }
}
