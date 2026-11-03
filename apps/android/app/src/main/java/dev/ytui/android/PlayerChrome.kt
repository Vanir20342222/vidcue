package dev.ytui.android

import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.LayerDrawable
import android.view.Gravity
import android.view.View
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.toArgb
import androidx.core.view.ViewCompat
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

    /**
     * Show SponsorBlock segments on the video controller's timebar. Media3 draws extra ad-group
     * markers as ticks, so each segment contributes a start and (when distinct) an end tick.
     * Main thread only; the service updates its marks on the main looper.
     */
    @JvmStatic
    fun showSponsorMarks(view: PlayerView) {
        val marks = PlaybackService.current?.sponsors?.objects() ?: emptyList()
        val times =
            marks
                .flatMap { mark ->
                    val start = mark.optDouble("start")
                    val end = mark.optDouble("end")
                    if (start.isFinite() && end.isFinite() && end > start)
                        listOf((start * 1000).toLong(), (end * 1000).toLong())
                    else emptyList()
                }
                .filter { it >= 0 }
                .distinct()
                .sorted()
                .toLongArray()
        val applied = view.getTag(R.id.ytui_sponsor_marks) as? LongArray
        if (applied?.contentEquals(times) == true) return
        view.setTag(R.id.ytui_sponsor_marks, times)
        view.setExtraAdGroupMarkers(times, BooleanArray(times.size))
    }

    /** Refresh the marks whenever the controller (and so its timebar) becomes visible. */
    @JvmStatic
    fun trackSponsorMarks(view: PlayerView) {
        view.setControllerVisibilityListener(
            PlayerView.ControllerVisibilityListener { visibility ->
                if (visibility == View.VISIBLE) showSponsorMarks(view)
            }
        )
        showSponsorMarks(view)
    }

    /**
     * The inline portrait frame is too narrow for five discs: prev, rewind, play, forward and
     * next crowd the picture and clip at the edge. Below [COMPACT_WIDTH_DP] keep prev/play/next;
     * double-tap seeking (VideoGestures) still covers rewind and forward.
     */
    @JvmStatic
    fun fitTransport(view: PlayerView) {
        // Prepared views can be reused; attach the listener once.
        if (view.getTag(R.id.ytui_transport_wide) != null) return
        view.setTag(R.id.ytui_transport_wide, "pending")
        view.addOnLayoutChangeListener { v, left, _, right, _, oldLeft, _, oldRight, _ ->
            if (right - left == oldRight - oldLeft) return@addOnLayoutChangeListener
            val wide = (right - left) / v.resources.displayMetrics.density >= COMPACT_WIDTH_DP
            if (v.getTag(R.id.ytui_transport_wide) == wide) return@addOnLayoutChangeListener
            v.setTag(R.id.ytui_transport_wide, wide)
            // Post: changing button visibility inside a layout pass would be dropped.
            v.post {
                view.setShowRewindButton(wide)
                view.setShowFastForwardButton(wide)
            }
        }
    }

    private const val COMPACT_WIDTH_DP = 560f

    /** Frosted-glass disc: tinted translucent fill with a lit hairline edge. */
    private fun glassDisc(resources: android.content.res.Resources) =
        GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(MediaTokens.videoDisc.toArgb())
            setStroke(
                maxOf(1, resources.displayMetrics.density.toInt()),
                MediaTokens.videoGlassEdge.toArgb(),
            )
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
            view.findViewById<View>(id)?.apply { background = glassDisc(resources) }
        }
        // API 23+ layouts merge these Buttons straight into the shared transport
        // strip. Their arrows are foregrounds; painting their parent creates a
        // huge oval behind all controls. Back each Button, retaining glyph/text.
        for (id in
            listOf(
                androidx.media3.ui.R.id.exo_rew_with_amount,
                androidx.media3.ui.R.id.exo_ffwd_with_amount,
            )) {
            view.findViewById<View>(id)?.apply {
                val disc = glassDisc(resources)
                val glyph =
                    (background as? LayerDrawable)?.findDrawableByLayerId(android.R.id.icon)
                        ?: background
                ViewCompat.setBackgroundTintList(this, null)
                background =
                    if (foreground != null || glyph == null) disc
                    else LayerDrawable(arrayOf(disc, glyph)).apply { setId(1, android.R.id.icon) }
            }
        }
        view.findViewById<DefaultTimeBar>(androidx.media3.ui.R.id.exo_progress)?.apply {
            setPlayedColor(accent)
            setScrubberColor(accent)
            // SponsorBlock ticks: a fixed warm marker stays distinct from any accent.
            setAdMarkerColor(MediaTokens.sponsorMark.toArgb())
            setPlayedAdMarkerColor(MediaTokens.sponsorMark.toArgb())
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
