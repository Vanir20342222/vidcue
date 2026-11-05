package dev.ytui.android;

import androidx.javascriptengine.SandboxDeadException;
import java.io.IOException;
import java.util.concurrent.CancellationException;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeoutException;

/**
 * Pure decisions for {@link ScriptRuntime}: when the cached isolated JavaScript sandbox must be
 * dropped and reconnected, and which short message a failed solve reports.
 */
final class SandboxRecovery {
  /** Where an evaluation attempt failed. */
  enum Stage {
    /** Waiting for the cached {@code JavaScriptSandbox} connection future. */
    CONNECT,
    /** Feature checks and {@code createIsolate} on a connected sandbox. */
    ISOLATE,
    /** Writing the script and evaluating it in the isolate. */
    EVALUATE
  }

  /** A refusal whose message is safe and useful to show as is. */
  static final class Refusal extends IOException {
    Refusal(String message) {
      super(message);
    }
  }

  static final String UNAVAILABLE =
      "Isolated YouTube solver unavailable; update Android System WebView and try again";
  static final String TIMED_OUT = "Isolated YouTube solver timed out";

  /**
   * True when the cached connection is unusable: the connection future failed or was cancelled,
   * the sandbox was closed under {@code createIsolate}, or any stage reports a dead sandbox. Script
   * errors, isolate-only memory limits and timeouts keep the connection.
   */
  static boolean needsReconnect(Stage stage, Throwable error) {
    if (error == null) return false;
    if (find(error, SandboxDeadException.class) != null) return true;
    switch (stage) {
      case CONNECT:
        return error instanceof ExecutionException || error instanceof CancellationException;
      case ISOLATE:
        return error instanceof IllegalStateException;
      default:
        return false;
    }
  }

  /** One fresh connection is tried per evaluation, and only while the deadline has time left. */
  static boolean mayRetry(int attempt, long remainingMillis) {
    return attempt == 0 && remainingMillis > 0;
  }

  /** Short user-facing reason; details go to logcat, never into the message. */
  static String message(Throwable error) {
    if (find(error, TimeoutException.class) != null) return TIMED_OUT;
    Refusal refusal = find(error, Refusal.class);
    return refusal != null ? refusal.getMessage() : UNAVAILABLE;
  }

  private static <T extends Throwable> T find(Throwable error, Class<T> type) {
    for (int depth = 0; error != null && depth < 16; depth++, error = error.getCause())
      if (type.isInstance(error)) return type.cast(error);
    return null;
  }

  private SandboxRecovery() {}
}
