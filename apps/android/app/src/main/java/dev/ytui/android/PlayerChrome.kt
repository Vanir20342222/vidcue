package dev.ytui.android

import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.LayerDrawable
import android.view.Gravity
import android.view.View
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.toArgb
import androidx.media3.ui.DefaultTimeBar
import androidx.media3.ui.PlayerView

/** Presentation only: preserve Media3 commands, player binding, gestures and auto-hide. */
@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
internal object PlayerChrome {
    private var accent = MediaTokens.videoInk.toArgb()

    @JvmStatic
    fun rememberAccent(value: Int) {
        accent = value
    }

    @JvmStatic
    fun apply(view: PlayerView) {
        if (view.getTag(R.id.ytui_top_scrim) == accent) return
        view.setTag(R.id.ytui_top_scrim, accent)
        val clear = MediaTokens.transparent.toArgb()
        val scrim = MediaTokens.videoScrim.toArgb()
        view.findViewById<View>(R.id.ytui_top_scrim)?.background =
            GradientDrawable(GradientDrawable.Orientation.TOP_BOTTOM, intArrayOf(scrim, clear))
        view.findViewById<View>(R.id.ytui_bottom_scrim)?.background =
            GradientDrawable(GradientDrawable.Orientation.BOTTOM_TOP, intArrayOf(scrim, clear))
        // Keep every transport legible even over a uniformly bright frame.
        // Each view needs its own drawable; shared drawable bounds are mutable.
        for (id in
            listOf(
                androidx.media3.ui.R.id.exo_play_pause,
                androidx.media3.ui.R.id.exo_prev,
                androidx.media3.ui.R.id.exo_next,
            )) {
            view.findViewById<View>(id)?.background =
                GradientDrawable().apply {
                    shape = GradientDrawable.OVAL
                    setColor(MediaTokens.videoDisc.toArgb())
                }
        }
        // These are text Buttons whose backgrounds ARE the curved-arrow glyphs.
        // Put the disc on their wrappers so Media3's glyph and seek amount survive.
        for (id in
            listOf(
                androidx.media3.ui.R.id.exo_rew_with_amount,
                androidx.media3.ui.R.id.exo_ffwd_with_amount,
            )) {
            (view.findViewById<View>(id)?.parent as? View)?.background =
                GradientDrawable().apply {
                    shape = GradientDrawable.OVAL
                    setColor(MediaTokens.videoDisc.toArgb())
                }
        }
        view.findViewById<DefaultTimeBar>(androidx.media3.ui.R.id.exo_progress)?.apply {
            setPlayedColor(accent)
            setScrubberColor(accent)
            // Choose the narrow keyline from the actual accent: fixed dark
            // backing would hide dark accents from light/imported palettes.
            val backing = mediaOnAccent(Color(accent))
            val ink = mediaOnAccent(backing)
            setUnplayedColor(lerp(backing, ink, 0.45f).toArgb())
            setBufferedColor(lerp(backing, ink, 0.65f).toArgb())
            background =
                LayerDrawable(
                        arrayOf(
                            GradientDrawable().apply {
                                setColor(backing.toArgb())
                                cornerRadius = 4f * resources.displayMetrics.density
                            }
                        )
                    )
                    .apply {
                        setLayerHeight(0, (6f * resources.displayMetrics.density).toInt())
                        setLayerGravity(0, Gravity.CENTER_VERTICAL or Gravity.FILL_HORIZONTAL)
                    }
        }
    }
}
