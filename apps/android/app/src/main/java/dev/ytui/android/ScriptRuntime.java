package dev.ytui.android;

import android.os.ParcelFileDescriptor;
import android.util.Log;
import androidx.javascriptengine.*;
import com.google.common.util.concurrent.ListenableFuture;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.*;

/** Runs only offline player challenge batches, isolated from cookies, UI and network. */
public final class ScriptRuntime {
  private static final String TAG = "VidcueSolver";
  private static ListenableFuture<JavaScriptSandbox> connection;
  private static final Semaphore slots = new Semaphore(2);

  private static synchronized ListenableFuture<JavaScriptSandbox> connection() {
    // A failed connect (e.g. during warm-up) must not be cached for the process lifetime.
    if (connection != null && connection.isDone() && !connected(connection)) connection = null;
    if (connection == null)
      connection = JavaScriptSandbox.createConnectedInstanceAsync(MediaBridge.context);
    return connection;
  }

  private static boolean connected(ListenableFuture<JavaScriptSandbox> done) {
    if (done.isCancelled()) return false;
    try {
      done.get();
      return true;
    } catch (ExecutionException failed) {
      return false;
    } catch (InterruptedException interrupted) {
      // Interruption says nothing about the connection; keep it.
      Thread.currentThread().interrupt();
      return true;
    }
  }

  /**
   * Drops a dead or failed connection so the next call binds a fresh sandbox. Closing unbinds the
   * service, which the library requires before another connection can be made.
   */
  private static synchronized void discard(ListenableFuture<JavaScriptSandbox> stale) {
    if (connection != stale) return; // Another evaluation already replaced it.
    connection = null;
    if (!stale.isDone()) {
      stale.cancel(true);
      return;
    }
    try {
      if (connected(stale)) stale.get().close();
    } catch (Exception ignored) {
      /* already dead or never connected */
    }
  }

  static void warm() {
    // Connect only: no script evaluation, network requests or access to account data.
    try {
      connection();
    } catch (RuntimeException unavailable) {
      /* lazy solve reports support errors */
    }
  }

  public static String evaluate(String script) {
    final long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(15);
    boolean acquired = false;
    try {
      if (script.length() > 16 * 1024 * 1024) throw new SandboxRecovery.Refusal("JS input limit");
      acquired = slots.tryAcquire(remaining(deadline), TimeUnit.MILLISECONDS);
      if (!acquired) throw new TimeoutException();
      return solve(script, deadline);
    } catch (Exception error) {
      if (error instanceof InterruptedException) Thread.currentThread().interrupt();
      Log.w(TAG, "Isolated JavaScript evaluation failed", error);
      throw new IllegalStateException(SandboxRecovery.message(error), error);
    } finally {
      if (acquired) slots.release();
    }
  }

  /** One attempt, plus one on a fresh connection when the sandbox is dead or never connected. */
  private static String solve(String script, long deadline) throws Exception {
    for (int attempt = 0; ; attempt++) {
      ListenableFuture<JavaScriptSandbox> pending = connection();
      SandboxRecovery.Stage stage = SandboxRecovery.Stage.CONNECT;
      JavaScriptIsolate isolate = null;
      File file = null;
      try {
        JavaScriptSandbox sandbox = pending.get(remaining(deadline), TimeUnit.MILLISECONDS);
        stage = SandboxRecovery.Stage.ISOLATE;
        if (!sandbox.isFeatureSupported(JavaScriptSandbox.JS_FEATURE_EVALUATE_FROM_FD)
            || !sandbox.isFeatureSupported(JavaScriptSandbox.JS_FEATURE_ISOLATE_MAX_HEAP_SIZE))
          throw new SandboxRecovery.Refusal(
              "Update Android System WebView for isolated YouTube solving");
        IsolateStartupParameters limits = new IsolateStartupParameters();
        // The adjacent feature checks are required (and kept) for lint's RequiresFeature.
        if (sandbox.isFeatureSupported(JavaScriptSandbox.JS_FEATURE_ISOLATE_MAX_HEAP_SIZE))
          limits.setMaxHeapSizeBytes(128L * 1024 * 1024);
        if (sandbox.isFeatureSupported(
            JavaScriptSandbox.JS_FEATURE_EVALUATE_WITHOUT_TRANSACTION_LIMIT))
          limits.setMaxEvaluationReturnSizeBytes(1024 * 1024);
        isolate = sandbox.createIsolate(limits);
        stage = SandboxRecovery.Stage.EVALUATE;
        String executable =
            "(()=>{let result='';const console={log:(value)=>{result=String(value);}};\n"
                + script
                + "\nreturn result;})()";
        file = File.createTempFile("ytui-solver-", ".js", MediaBridge.context.getCacheDir());
        try (FileOutputStream out = new FileOutputStream(file)) {
          out.write(executable.getBytes(StandardCharsets.UTF_8));
        }
        try (ParcelFileDescriptor descriptor =
            ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)) {
          String result;
          if (sandbox.isFeatureSupported(JavaScriptSandbox.JS_FEATURE_EVALUATE_FROM_FD)) {
            result =
                isolate
                    .evaluateJavaScriptAsync(descriptor)
                    .get(remaining(deadline), TimeUnit.MILLISECONDS);
          } else throw new SandboxRecovery.Refusal("FD evaluation unavailable");
          if (result.length() > 1024 * 1024) throw new SandboxRecovery.Refusal("JS output limit");
          return result;
        }
      } catch (Exception error) {
        if (!SandboxRecovery.needsReconnect(stage, error)) throw error;
        discard(pending);
        long left = TimeUnit.NANOSECONDS.toMillis(deadline - System.nanoTime());
        if (!SandboxRecovery.mayRetry(attempt, left)) throw error;
        Log.w(TAG, "Isolated JavaScript sandbox unavailable at " + stage + "; reconnecting");
      } finally {
        if (isolate != null) isolate.close();
        if (file != null && !file.delete()) file.deleteOnExit();
      }
    }
  }

  private static long remaining(long deadline) throws TimeoutException {
    long ms = TimeUnit.NANOSECONDS.toMillis(deadline - System.nanoTime());
    if (ms <= 0) throw new TimeoutException();
    return ms;
  }

  private ScriptRuntime() {}
}
