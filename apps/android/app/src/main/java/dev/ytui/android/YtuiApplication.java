package dev.ytui.android;

import android.app.Application;
import android.util.Log;

public final class YtuiApplication extends Application {
  @Override
  public void onCreate() {
    super.onCreate();
    String failure =
        NativeBridge.init(getFilesDir().getAbsolutePath(), getCacheDir().getAbsolutePath());
    // Already sanitized by the engine. The UI keeps it as the status and shows it once.
    EngineStartup.current.record(failure);
    if (EngineStartup.current.error() != null) Log.e("Vidcue", "Engine start failed: " + failure);
    MediaBridge.attach(this);
  }
}
