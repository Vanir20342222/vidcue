package dev.ytui.android;

import android.os.Bundle;
import androidx.test.platform.app.InstrumentationRegistry;

/**
 * The tap-to-visible-frame budget the startup tests assert.
 *
 * <p>Hosted CI runs a software-rendered emulator whose frame timing jitters by tens to hundreds of
 * milliseconds, so a hard 2000 ms assertion failed pull requests at random ("Indexed pixels: 2001").
 * Pull requests and the merge queue therefore assert only a ceiling that catches a hung or broken
 * start, while the measured times are still recorded in the evidence files. The nightly run, and any
 * physical-device run, passes {@code -e ytui_timing enforce} (or {@code YTUI_ANDROID_TIMING=enforce}
 * with {@code tools/android/run-device-tests.sh}) to restore the 2000 ms gate. Enforcing the 1000 ms
 * subsecond target ({@code ytui_subsecond enforce}) implies the 2000 ms gate.
 */
final class StartupBudget {
  static final long ENFORCED_MS = 2000;
  /** Advisory ceiling: a start this slow is a defect, not emulator jitter. */
  static final long ADVISORY_MS = 6000;

  private StartupBudget() {}

  static boolean enforced() {
    Bundle arguments = InstrumentationRegistry.getArguments();
    return "enforce".equals(arguments.getString("ytui_timing"))
        || "enforce".equals(arguments.getString("ytui_subsecond"));
  }

  /** Milliseconds from the tap within which visible output must arrive. */
  static long ms() {
    return enforced() ? ENFORCED_MS : ADVISORY_MS;
  }
}
