package dev.ytui.android;

import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import org.json.JSONObject;

/**
 * Tells the sync engine whether the app is in the foreground, so it keeps its WebSocket open
 * while visible and closes it a minute after the app leaves the screen (unless audio plays or a
 * Jam is running). The engine call runs off the main thread, in call order.
 */
final class SyncLifecycle {
  private static final ExecutorService worker =
      Executors.newSingleThreadExecutor(
          task -> {
            Thread thread = new Thread(task, "vidcue-sync-lifecycle");
            thread.setDaemon(true);
            return thread;
          });

  private SyncLifecycle() {}

  /** Call on the main thread (activity start/stop): it reads the player there. */
  static void foreground(boolean visible) {
    boolean playing = false;
    PlaybackService service = PlaybackService.current;
    if (service != null && service.player != null) playing = service.player.isPlaying();
    send(visible, playing);
  }

  /**
   * The play state changed while the UI is hidden (notification, headset, end of queue): the
   * engine keeps the socket while audio plays and closes it a minute after playback stops.
   */
  static void playing(boolean playing) {
    send(null, playing);
  }

  private static void send(Boolean visible, boolean playing) {
    worker.execute(
        () -> {
          try {
            EngineStartup.current.await();
            JSONObject request =
                new JSONObject().put("op", "sync_foreground").put("playing", playing);
            if (visible != null) request.put("visible", visible.booleanValue());
            NativeBridge.request(request.toString());
          } catch (Exception | LinkageError ignored) {
            // No engine (start failed) or no sync support: nothing to keep open.
          }
        });
  }
}
