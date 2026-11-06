package dev.ytui.android

import android.content.pm.ActivityInfo
import androidx.media3.common.Player

/** Fullscreen window choices that follow the bound player (AR-10, AR-11). */
internal object FullscreenPolicy {
    /**
     * Portrait videos and Shorts fill a portrait screen instead of being letterboxed. [current]
     * is the activity's present request: an unknown size (between items, before the first
     * frame) keeps it, so fullscreen does not flip while the next video loads, and the first
     * request without a size is landscape.
     */
    @JvmStatic
    fun orientation(width: Int, height: Int, pixelRatio: Float, current: Int): Int {
        if (width <= 0 || height <= 0)
            return if (current == ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED)
                ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
            else current
        val shown = width * (if (pixelRatio > 0f && pixelRatio.isFinite()) pixelRatio else 1f)
        return if (height > shown) ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT
        else ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
    }

    /** The screen may sleep while playback is paused, stopped or finished. */
    @JvmStatic
    fun keepScreenOn(playWhenReady: Boolean, state: Int): Boolean =
        playWhenReady && state != Player.STATE_IDLE && state != Player.STATE_ENDED
}
