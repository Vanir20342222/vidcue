package dev.ytui.android

import android.graphics.ColorMatrix
import android.graphics.ColorMatrixColorFilter
import android.graphics.Paint
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.TextureView
import android.view.View
import androidx.media3.ui.PlayerView
import dev.ytui.android.ColorGradeMath.Grade
import dev.ytui.android.ColorGradeMath.Stats

/**
 * Colour grading for the video player (Settings › Playback, onboarding). The grade is a GPU
 * colour filter on the TextureView's layer paint, so decoding and ExoPlayer are untouched.
 * `TextureView.getBitmap` returns the ungraded frame, so auto grade never measures its own
 * correction. Both settings are off by default; with both off the view is never changed.
 */
@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
object ColorGrade {
    private const val TICK_MS = 400L
    private const val FIRST_LOOK_MS = 3_000L
    private const val INTERVAL_MS = 30_000L
    private const val RETRY_MS = 60_000L
    private const val WINDOW_MS = 2_500L
    private const val SAMPLES = 6
    private const val SNAPSHOT_W = 48
    private const val SNAPSHOT_H = 27

    /** Starts grading [view] while it is attached to a window. Safe to call repeatedly. */
    @JvmStatic
    fun attach(view: PlayerView) {
        if (view.getTag(R.id.ytui_color_grade) != null) return
        val controller = Controller(view)
        view.setTag(R.id.ytui_color_grade, controller)
        view.addOnAttachStateChangeListener(controller)
        if (view.isAttachedToWindow) controller.onViewAttachedToWindow(view)
    }

    private class Controller(private val view: PlayerView) :
        View.OnAttachStateChangeListener, Runnable {
        private val handler = Handler(Looper.getMainLooper())
        private var applied: Grade? = null
        private var appliedTo: TextureView? = null
        private var auto: Grade? = null
        private var media: String? = null
        private var nextLook = 0L
        private var probeSince = 0L
        private val samples = ArrayList<Stats>()

        override fun onViewAttachedToWindow(v: View) {
            handler.removeCallbacks(this)
            handler.post(this)
        }

        override fun onViewDetachedFromWindow(v: View) {
            handler.removeCallbacks(this)
        }

        override fun run() {
            try {
                tick()
            } catch (_: RuntimeException) {
                // A failed snapshot or filter update leaves playback as it is.
            }
            if (view.isAttachedToWindow) handler.postDelayed(this, TICK_MS)
        }

        private fun tick() {
            val texture = view.videoSurfaceView as? TextureView ?: return
            val now = SystemClock.elapsedRealtime()
            val preset = MobileSettings.text("color_grade")
            val autoOn = MobileSettings.enabled("auto_color_grade")
            val player = view.player
            val id = player?.currentMediaItem?.mediaId
            if (id != media) {
                media = id
                auto = null
                samples.clear()
                probeSince = 0
                nextLook = now + FIRST_LOOK_MS
            }
            if (!autoOn) {
                auto = null
                samples.clear()
                probeSince = 0
                nextLook = now + FIRST_LOOK_MS
            } else if (probeSince != 0L) {
                if (player?.isPlaying == true && texture.isAvailable) {
                    texture.getBitmap(SNAPSHOT_W, SNAPSHOT_H)?.let { bitmap ->
                        val pixels = IntArray(SNAPSHOT_W * SNAPSHOT_H)
                        bitmap.getPixels(pixels, 0, SNAPSHOT_W, 0, 0, SNAPSHOT_W, SNAPSHOT_H)
                        bitmap.recycle()
                        ColorGradeMath.stats(pixels)
                            ?.takeIf { it != samples.lastOrNull() }
                            ?.let(samples::add)
                    }
                }
                if (samples.size >= SAMPLES || now - probeSince >= WINDOW_MS ||
                    player?.isPlaying != true) {
                    if (samples.isNotEmpty()) {
                        val target = ColorGradeMath.auto(ColorGradeMath.average(samples))
                        auto = auto?.blend(target, 0.5) ?: target
                    }
                    nextLook = now + if (samples.isEmpty() && auto == null) RETRY_MS else INTERVAL_MS
                    samples.clear()
                    probeSince = 0
                }
            } else if (player?.isPlaying == true && now >= nextLook) {
                probeSince = now
            }
            val wanted = (ColorGradeMath.preset(preset) + (auto ?: Grade())).rounded()
            if (texture === appliedTo && wanted == (applied ?: Grade())) return
            if (texture !== appliedTo && applied == null && wanted.neutral) return
            texture.setLayerPaint(
                if (wanted.neutral) null
                else
                    Paint().apply {
                        colorFilter = ColorMatrixColorFilter(ColorMatrix(ColorGradeMath.matrix(wanted)))
                    }
            )
            applied = wanted
            appliedTo = texture
        }
    }
}
