package dev.ytui.android;

import static org.junit.Assert.*;

import org.junit.Test;

public final class NotificationPermissionTest {
  @Test
  public void asksOnlyOnAndroid13AndNewer() {
    assertFalse(NotificationPermission.shouldAsk(32, false, false));
    assertTrue(NotificationPermission.shouldAsk(33, false, false));
    assertTrue(NotificationPermission.shouldAsk(35, false, false));
  }

  @Test
  public void neverAsksAgainWhenGrantedOrAlreadyAskedThisLaunch() {
    assertFalse(NotificationPermission.shouldAsk(35, true, false));
    assertFalse(NotificationPermission.shouldAsk(35, false, true));
  }
}
