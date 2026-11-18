package dev.ytui.android;

import android.app.Application;
import android.util.Log;

public final class YtuiApplication extends Application {
  @Override
  public void onTrimMemory(int level) {
    super.onTrimMemory(level);
    if (level >= android.content.ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW)
      ComposeHost.trimMemory();
  }

  @Override
  public void onLowMemory() {
    super.onLowMemory();
    ComposeHost.trimMemory();
  }

  @Override
  public void onCreate() {
    super.onCreate();
    FirstFeedTiming.current.begin(android.os.Process.getStartElapsedRealtime());
    MediaBridge.attach(this);
    java.util.concurrent.CompletableFuture.runAsync(() -> {
      String failure;
      android.os.Trace.beginSection("Vidcue.engineInit");
      try {
        failure = NativeBridge.init(getFilesDir().getAbsolutePath(), getCacheDir().getAbsolutePath());
        if (failure == null || failure.isBlank()) MediaBridge.initializeNetwork();
      } catch (Exception | LinkageError error) {
        // Do not expose paths or account details from a linkage/platform failure.
        failure = "Native initialization failed (" + error.getClass().getSimpleName() + ")";
      } finally {
        android.os.Trace.endSection();
      }
      EngineStartup.current.record(failure);
      if (EngineStartup.current.error() != null) Log.e("Vidcue", EngineStartup.current.error());
      else OfflineStore.IO.execute(() -> {
        try {
          OfflineStore.prepareStorage();
          MediaBridge.main.post(OfflineStore::manager);
        } catch (Exception error) {
          MediaBridge.event("error", "Offline library could not be restored");
        }
      });
    });
  }
}
