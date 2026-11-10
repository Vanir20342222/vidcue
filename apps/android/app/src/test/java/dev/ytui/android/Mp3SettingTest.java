package dev.ytui.android;

import static org.junit.Assert.*;

import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.annotation.Config;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 35)
public class Mp3SettingTest {
  @Test
  public void automaticConversionIsOffByDefaultAndTheChoiceIsRemembered() throws Exception {
    android.app.Application previous = MediaBridge.context;
    MediaBridge.context = RuntimeEnvironment.getApplication();
    try {
      assertEquals(Boolean.FALSE, MobileSettings.BOOLEANS.get("auto_mp3"));
      assertFalse(MobileSettings.enabled("auto_mp3"));
      assertFalse(MobileSettings.snapshot().getBoolean("auto_mp3"));
      MobileSettings.prefs().edit().putBoolean("auto_mp3", true).commit();
      assertTrue(MobileSettings.enabled("auto_mp3"));
      assertTrue(MobileSettings.snapshot().getBoolean("auto_mp3"));
      MobileSettings.prefs().edit().putBoolean("auto_mp3", false).commit();
      assertFalse(MobileSettings.enabled("auto_mp3"));
    } finally {
      MediaBridge.context = previous;
    }
  }
}
