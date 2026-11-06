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
    return new String[] {key, key + "-failed", key + "-retry"};
  }

  static int checkedCursor(int cursor, int count) {
    if (count < 0 || count > 2000 || cursor < 0 || cursor > count)
      throw new IllegalArgumentException("Invalid playlist checkpoint");
    return cursor;
  }

  /**
   * Appends a failed entry to the comma-separated retry list kept with the checkpoint. Entries
   * arrive in playlist order, so the list stays ascending and duplicate-free.
   */
  static String withFailure(String encoded, int index) {
    if (index < 0) throw new IllegalArgumentException("Invalid playlist entry");
    return encoded == null || encoded.isEmpty() ? Integer.toString(index) : encoded + "," + index;
  }

  /**
   * Entries that a Restart should prepare again. Rejects anything that does not describe distinct,
   * ascending positions inside the checkpointed playlist.
   */
  static int[] failures(String encoded, int count) {
    checkedCursor(0, count);
    if (encoded == null || encoded.isEmpty()) return new int[0];
    String[] parts = encoded.split(",", -1);
    if (parts.length > count) throw new IllegalArgumentException("Invalid playlist retry list");
    int[] indices = new int[parts.length];
    int previous = -1;
    for (int i = 0; i < parts.length; i++) {
      int index;
      try {
        index = Integer.parseInt(parts[i]);
      } catch (NumberFormatException error) {
        throw new IllegalArgumentException("Invalid playlist retry list");
      }
      if (index <= previous || index >= count)
        throw new IllegalArgumentException("Invalid playlist retry list");
      indices[i] = previous = index;
    }
    return indices;
  }

  /** The row text once every entry has been tried. */
  static String finished(int count, int failed, int retryable) {
    if (failed == 0) return "Playlist queued: " + count + " entries";
    String text = "Playlist queued with " + failed + (failed == 1 ? " failure" : " failures");
    if (retryable == 0) return text + "; nothing to retry";
    return text
        + "; Restart retries "
        + (retryable == 1 ? "1 entry" : retryable + " entries");
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
