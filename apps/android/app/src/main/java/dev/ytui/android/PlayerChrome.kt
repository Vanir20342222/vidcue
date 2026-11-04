package dev.ytui.android

import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.InsetDrawable
import android.graphics.drawable.LayerDrawable
import android.widget.ImageButton
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

    /** Refresh the marks and the captions toggle whenever the controller becomes visible. */
    @JvmStatic
    fun trackSponsorMarks(view: PlayerView) {
        view.setControllerVisibilityListener(
            PlayerView.ControllerVisibilityListener { visibility ->
                if (visibility == View.VISIBLE) {
                    showSponsorMarks(view)
                    showCaptions(view)
                }
            }
        )
        showSponsorMarks(view)
    }

    /**
     * One captions button that switches captions on or off. Media3's own button opened a list of
     * every caption track, which on YouTube means well over a hundred machine translations; the
     * choice of language lives in the playback options instead. Switching on without a choice
     * picks the most useful track (see Captions.best).
     */
    @JvmStatic
    fun bindCaptions(view: PlayerView) {
        val button = view.findViewById<ImageButton>(R.id.ytui_captions) ?: return
        button.setOnClickListener {
            MediaBridge.execute(org.json.JSONObject().put("op", "captions_toggle").toString())
            // Track selection applies on the next main-looper turn; read it back after that.
            view.postDelayed({ showCaptions(view) }, 250)
        }
        showCaptions(view)
    }

    /** Show the captions button only when the media has captions, with its current state. */
    @JvmStatic
    fun showCaptions(view: PlayerView) {
        val button = view.findViewById<ImageButton>(R.id.ytui_captions) ?: return
        val player = view.player
        val groups = player?.currentTracks?.groups ?: emptyList()
        val available = groups.any { it.type == androidx.media3.common.C.TRACK_TYPE_TEXT && it.length > 0 }
        val on =
            available &&
                player?.trackSelectionParameters?.disabledTrackTypes?.contains(
                    androidx.media3.common.C.TRACK_TYPE_TEXT
                ) != true &&
                groups.any { it.type == androidx.media3.common.C.TRACK_TYPE_TEXT && it.isSelected }
        button.visibility = if (available) View.VISIBLE else View.GONE
        button.setImageResource(if (on) R.drawable.ytui_captions_on else R.drawable.ytui_captions_off)
        button.contentDescription =
            view.resources.getString(
                if (on) R.string.ytui_player_captions_off else R.string.ytui_player_captions_on
            )
    }

    /**
     * The inline portrait frame is too narrow for five discs: prev, rewind, play, forward and
     * next crowd the picture and clip at the edge. Below [COMPACT_WIDTH_DP] keep prev/play/next
     * and use a smaller play disc; double-tap seeking (VideoGestures) still covers rewind and
     * forward.
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
                view.findViewById<View>(androidx.media3.ui.R.id.exo_play_pause)?.apply {
                    val density = resources.displayMetrics.density
                    val size = ((if (wide) 64 else 56) * density).toInt()
                    val pad = ((if (wide) 19 else 16) * density).toInt()
                    layoutParams = layoutParams.apply {
                        width = size
                        height = size
                    }
                    setPadding(pad, pad, pad, pad)
                }
            }
        }
    }

    private const val COMPACT_WIDTH_DP = 560f

    /**
     * Flat dark disc behind a transport: no glass edge, so the controls sit quietly on the
     * picture. [insetDp] draws it slightly inside a 48dp touch target; the disc still reaches the
     * point ContrastUiTest samples (0.85 of the radius), so white glyphs keep 3:1 on white video.
     */
    private fun disc(resources: android.content.res.Resources, insetDp: Float = 0f): Drawable {
        val shape =
            GradientDrawable().apply {
                shape = GradientDrawable.OVAL
                setColor(MediaTokens.videoChromeDisc.toArgb())
            }
        val inset = (insetDp * resources.displayMetrics.density).toInt()
        return if (inset == 0) shape else InsetDrawable(shape, inset)
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
            view.findViewById<View>(id)?.apply {
                background = disc(resources, if (id == androidx.media3.ui.R.id.exo_play_pause) 0f else 2f)
            }
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
                val disc = disc(resources, 2f)
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
