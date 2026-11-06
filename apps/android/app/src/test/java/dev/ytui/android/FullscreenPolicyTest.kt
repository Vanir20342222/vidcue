package dev.ytui.android

import android.content.pm.ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE as LANDSCAPE
import android.content.pm.ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT as PORTRAIT
import android.content.pm.ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED as UNSPECIFIED
import androidx.media3.common.Player
import org.junit.Assert.*
import org.junit.Test

class FullscreenPolicyTest {
    @Test
    fun orientationFollowsTheVideoShape() {
        assertEquals(LANDSCAPE, FullscreenPolicy.orientation(1920, 1080, 1f, UNSPECIFIED))
        assertEquals(PORTRAIT, FullscreenPolicy.orientation(1080, 1920, 1f, UNSPECIFIED))
        assertEquals(PORTRAIT, FullscreenPolicy.orientation(1080, 1920, 1f, LANDSCAPE))
        assertEquals(LANDSCAPE, FullscreenPolicy.orientation(1920, 1080, 1f, PORTRAIT))
        // Square videos fit either way; landscape is the conventional fullscreen.
        assertEquals(LANDSCAPE, FullscreenPolicy.orientation(720, 720, 1f, PORTRAIT))
        // Anamorphic pixels: 720x960 stored, shown 1440 wide.
        assertEquals(LANDSCAPE, FullscreenPolicy.orientation(720, 960, 2f, UNSPECIFIED))
        assertEquals(PORTRAIT, FullscreenPolicy.orientation(720, 960, Float.NaN, UNSPECIFIED))
    }

    @Test
    fun unknownSizeStartsLandscapeAndThenKeepsTheCurrentChoice() {
        assertEquals(LANDSCAPE, FullscreenPolicy.orientation(0, 0, 1f, UNSPECIFIED))
        assertEquals(PORTRAIT, FullscreenPolicy.orientation(0, 0, 1f, PORTRAIT))
        assertEquals(LANDSCAPE, FullscreenPolicy.orientation(0, 1080, 1f, LANDSCAPE))
    }

    @Test
    fun screenStaysOnOnlyWhilePlaying() {
        assertTrue(FullscreenPolicy.keepScreenOn(true, Player.STATE_READY))
        assertTrue(FullscreenPolicy.keepScreenOn(true, Player.STATE_BUFFERING))
        assertFalse(FullscreenPolicy.keepScreenOn(false, Player.STATE_READY))
        assertFalse(FullscreenPolicy.keepScreenOn(false, Player.STATE_BUFFERING))
        assertFalse(FullscreenPolicy.keepScreenOn(true, Player.STATE_ENDED))
        assertFalse(FullscreenPolicy.keepScreenOn(true, Player.STATE_IDLE))
    }
}
