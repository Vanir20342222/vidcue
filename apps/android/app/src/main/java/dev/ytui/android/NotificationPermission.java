package dev.ytui.android;

import android.app.Activity;
import android.content.pm.PackageManager;
import android.os.Build;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * Asks for the Android 13+ notification permission at most once per app launch, and only after
 * first-run setup has finished or been skipped (AR-23).
 */
final class NotificationPermission {
  static final String PERMISSION = "android.permission.POST_NOTIFICATIONS";
  static final int REQUEST = 21;
  private static final AtomicBoolean asked = new AtomicBoolean();

  /** Pure decision: Android 13+, not yet granted, and not yet asked in this launch. */
  static boolean shouldAsk(int sdk, boolean granted, boolean alreadyAsked) {
    return sdk >= 33 && !granted && !alreadyAsked;
  }

  static void requestOnce(Activity activity) {
    if (activity == null || activity.isFinishing()) return;
    boolean granted =
        Build.VERSION.SDK_INT < 33
            || activity.checkSelfPermission(PERMISSION) == PackageManager.PERMISSION_GRANTED;
    if (!shouldAsk(Build.VERSION.SDK_INT, granted, asked.get())) return;
    if (asked.compareAndSet(false, true))
      activity.requestPermissions(new String[] {PERMISSION}, REQUEST);
  }

  private NotificationPermission() {}
}
