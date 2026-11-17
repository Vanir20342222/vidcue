package dev.ytui.android;

import java.util.concurrent.atomic.AtomicBoolean;

/**
 * Keeps the sanitized reason {@link NativeBridge#init} returned, so the UI can explain why every
 * engine request fails instead of showing only "Engine is not initialized".
 */
final class EngineStartup {
  static final EngineStartup current = new EngineStartup();
  static final int LIMIT = 240;

  private volatile String error;
  private final java.util.concurrent.CompletableFuture<Void> ready = new java.util.concurrent.CompletableFuture<>();

  /** Only blocking workers may wait; activity creation never waits on this gate. */
  void await() throws Exception {
    ready.get();
    if (error != null) throw new IllegalStateException(error);
  }
  private final AtomicBoolean announced = new AtomicBoolean();

  /** Records the value {@code NativeBridge.init} returned; empty or null means it started. */
  synchronized void record(String raw) {
    if (ready.isDone()) return;
    error = message(raw);
    ready.complete(null);
  }

  void whenFinished(Runnable action) { ready.thenRun(action); }

  boolean isReady() { return ready.isDone() && error == null; }

  /** The user-facing start error, or null when the engine started. */
  String error() {
    return error;
  }

  /** Returns the start error the first time it is asked for in this process, then null. */
  String claimNotice() {
    String value = error;
    return value != null && announced.compareAndSet(false, true) ? value : null;
  }

  static String message(String raw) {
    if (raw == null) return null;
    String text = raw.trim().replaceAll("\\s+", " ");
    if (text.isEmpty()) return null;
    if (text.length() > LIMIT) text = text.substring(0, LIMIT - 1).trim() + "…";
    return "Vidcue engine could not start: " + text;
  }
}
