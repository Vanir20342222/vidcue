package dev.ytui.android;

/** Pure platform retry/checkpoint policy, kept testable without a device. */
final class PreparationPolicy {
  static boolean rateLimited(String error) {
    String lower = error.toLowerCase(java.util.Locale.ROOT);
    return lower.contains("429") || lower.contains("rate limit") || lower.contains("cooldown");
  }

  /** Runs of one preparation job before it stops and waits for a manual retry. */
  static final int MAX_RUNS = 8;

  /** Whether the run numbered [runAttemptCount] (0 for the first) may be retried. */
  static boolean retryAllowed(int runAttemptCount) {
    return runAttemptCount + 1 < MAX_RUNS;
  }

  /** Unique WorkManager name of preparation job [key]; a retry replaces that work. */
  static String uniqueWork(String key) {
    return "ytui-prepare-" + key;
  }

  /** "preparations-progress" entries owned by job [key]. */
  static String[] progressKeys(String key) {
    return new String[] {key, key + "-failed"};
  }

  static int checkedCursor(int cursor, int count) {
    if (count < 0 || count > 2000 || cursor < 0 || cursor > count)
      throw new IllegalArgumentException("Invalid playlist checkpoint");
    return cursor;
  }

  /** Nested playlist/album/artist/channel entries cannot be resolved as one video. */
  static boolean collection(String kind) {
    switch (kind) {
      case "playlist":
      case "album":
      case "artist":
      case "channel":
        return true;
      default:
        return false;
    }
  }

  private PreparationPolicy() {}
}
