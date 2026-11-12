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
     * Mark [view] as driving its own player (Movies) rather than the shared PlaybackService
     * session: gestures and the captions button then act on the view's player directly, and the
     * service's SponsorBlock marks are never drawn on it.
     */
    @JvmStatic
    fun markStandalone(view: PlayerView) {
        view.setTag(R.id.ytui_standalone, true)
    }

    @JvmStatic
    fun standalone(view: PlayerView): Boolean = view.getTag(R.id.ytui_standalone) == true

    /** Keep controls clear of cutouts and screen edges; scrims and video stay edge to edge. */
    @JvmStatic
    fun fitControls(view: PlayerView, cutoutLeft: Int, cutoutTop: Int, cutoutRight: Int) {
        val edge = Math.round(16 * view.resources.displayMetrics.density)
        for (id in
            intArrayOf(
                R.id.ytui_top_bar,
                androidx.media3.ui.R.id.exo_bottom_bar,
                androidx.media3.ui.R.id.exo_progress,
                androidx.media3.ui.R.id.exo_minimal_controls,
            )) {
            val child = view.findViewById<View>(id) ?: continue
            val params = child.layoutParams as? android.view.ViewGroup.MarginLayoutParams ?: continue
            params.leftMargin = maxOf(edge, cutoutLeft)
            params.rightMargin = maxOf(edge, cutoutRight)
            if (id == R.id.ytui_top_bar) params.topMargin = cutoutTop
            child.layoutParams = params
        }
    }

    /**
     * Show SponsorBlock segments on the video controller's timebar. Media3 draws extra ad-group
     * markers as ticks, so each segment contributes a start and (when distinct) an end tick.
     * Main thread only; the service updates its marks on the main looper.
     */
    @JvmStatic
    fun showSponsorMarks(view: PlayerView) {
        val marks =
            if (standalone(view)) emptyList()
            else PlaybackService.current?.sponsors?.objects() ?: emptyList()
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

    private const val LIVE_TICK_MS = 500L
    private val LIVE_RED = MediaTokens.live.toArgb() // ArtworkBadge's live colour.
    private val LIVE_BEHIND = android.graphics.Color.argb(0x99, 0xff, 0xff, 0xff)

    /**
     * Live streams (M9): Media3 prints a live window as an ordinary time ("1:00:28 / 1:00:00").
     * While the bound player's item is live, the time pill instead reads LIVE with a red dot at
     * the edge, or "−0:45 LIVE" with a grey dot when behind, and tapping it returns to the edge.
     * The timebar keeps scrubbing within the stream's DVR window. Attach once per view.
     */
    @JvmStatic
    fun trackLive(view: PlayerView) {
        if (view.getTag(R.id.ytui_live_tracker) != null) return
        val tick =
            object : Runnable {
                override fun run() {
                    showLive(view)
                    view.removeCallbacks(this)
                    if (view.isAttachedToWindow) view.postDelayed(this, LIVE_TICK_MS)
                }
            }
        view.setTag(R.id.ytui_live_tracker, tick)
        view.addOnAttachStateChangeListener(
            object : View.OnAttachStateChangeListener {
                override fun onViewAttachedToWindow(v: View) {
                    v.post(tick)
                }

                override fun onViewDetachedFromWindow(v: View) {
                    v.removeCallbacks(tick)
                }
            }
        )
        if (view.isAttachedToWindow) view.post(tick)
    }

    /** Switch the time pill between clock and live presentation for the bound player. */
    @JvmStatic
    fun showLive(view: PlayerView) {
        val pill = view.findViewById<View>(androidx.media3.ui.R.id.exo_time) ?: return
        val label = view.findViewById<android.widget.TextView>(R.id.ytui_live) ?: return
        val behindText = view.findViewById<android.widget.TextView>(R.id.ytui_live_behind) ?: return
        val player = view.player
        val live = LivePosition.live(player)
        val clock = if (live) View.GONE else View.VISIBLE
        for (id in
            intArrayOf(
                androidx.media3.ui.R.id.exo_position,
                R.id.ytui_time_separator,
                androidx.media3.ui.R.id.exo_duration,
            )) {
            view.findViewById<View>(id)?.let { if (it.visibility != clock) it.visibility = clock }
        }
        if (!live) {
            label.visibility = View.GONE
            behindText.visibility = View.GONE
            if (pill.hasOnClickListeners()) {
                pill.setOnClickListener(null)
                pill.isClickable = false
                pill.contentDescription = null
            }
            return
        }
        val behind = LivePosition.behindMs(player)
        val atEdge = LivePosition.atEdge(behind)
        val text = LivePosition.behindLabel(behind)
        label.visibility = View.VISIBLE
        behindText.visibility = if (atEdge) View.GONE else View.VISIBLE
        if (behindText.text.toString() != text) behindText.text = text
        val dotColor = if (atEdge) LIVE_RED else LIVE_BEHIND
        if (label.getTag(R.id.ytui_live_tracker) != dotColor) {
            label.setTag(R.id.ytui_live_tracker, dotColor)
            val size = (8 * view.resources.displayMetrics.density).toInt()
            label.setCompoundDrawablesRelative(
                GradientDrawable().apply {
                    shape = GradientDrawable.OVAL
                    setColor(dotColor)
                    setSize(size, size)
                    setBounds(0, 0, size, size)
                },
                null,
                null,
                null,
            )
        }
        pill.contentDescription =
            if (atEdge) view.resources.getString(R.string.ytui_player_live_at_edge)
            else view.resources.getString(R.string.ytui_player_go_live, text.removePrefix("\u2212"))
        if (!pill.hasOnClickListeners()) {
            pill.setOnClickListener {
                LivePosition.goLive(view.player)
                showLive(view)
            }
        }
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
            if (standalone(view)) view.player?.let(::toggleOwnCaptions)
            else MediaBridge.execute(org.json.JSONObject().put("op", "captions_toggle").toString())
            // Track selection applies on the next main-looper turn; read it back after that.
            view.postDelayed({ showCaptions(view) }, 250)
        }
        showCaptions(view)
    }

    /**
     * Captions on/off for a standalone player. Switching on keeps an earlier choice from the
     * options panel; without one it picks a track in the device language, else the first.
     */
    private fun toggleOwnCaptions(player: androidx.media3.common.Player) {
        val text = androidx.media3.common.C.TRACK_TYPE_TEXT
        val groups = player.currentTracks.groups.filter { it.type == text && it.length > 0 }
        if (groups.isEmpty()) return
        val params = player.trackSelectionParameters
        val on = !params.disabledTrackTypes.contains(text) && groups.any { it.isSelected }
        // An override names its track group; after a source switch the old group may be absent
        // here, and ExoPlayer would then select nothing. Keep only overrides this item has.
        val kept =
            params.overrides.values.filter { o ->
                o.type == text && groups.any { it.mediaTrackGroup == o.mediaTrackGroup }
            }
        val next = params.buildUpon().setTrackTypeDisabled(text, on).clearOverridesOfType(text)
        kept.forEach { next.addOverride(it) }
        if (!on && kept.isEmpty()) {
            val language = java.util.Locale.getDefault().language
            val group =
                groups.firstOrNull { group ->
                    group.getTrackFormat(0).language?.let {
                        java.util.Locale.forLanguageTag(it).language == language
                    } == true
                } ?: groups.first()
            next.setOverrideForType(androidx.media3.common.TrackSelectionOverride(group.mediaTrackGroup, 0))
        }
        player.trackSelectionParameters = next.build()
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
                    val pad = ((if (wide) 17 else 15) * density).toInt()
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

    /** Reduced motion (the app setting); system animator scale is checked per touch. */
    @JvmStatic var reducedMotion = false

    /**
     * Springy press feedback for the video controls, matching the app's Compose buttons: a
     * control sinks while held and springs back with a little overshoot when released. The
     * listener only observes touches; Media3 keeps handling the clicks.
     */
    @JvmStatic
    @android.annotation.SuppressLint("ClickableViewAccessibility")
    fun softenPresses(view: PlayerView) {
        val ids =
            listOf(
                androidx.media3.ui.R.id.exo_play_pause,
                androidx.media3.ui.R.id.exo_prev,
                androidx.media3.ui.R.id.exo_next,
                androidx.media3.ui.R.id.exo_rew_with_amount,
                androidx.media3.ui.R.id.exo_ffwd_with_amount,
                androidx.media3.ui.R.id.exo_fullscreen,
                androidx.media3.ui.R.id.exo_minimal_fullscreen,
                R.id.ytui_collapse,
                R.id.ytui_captions,
                R.id.ytui_settings,
            )
        for (id in ids) {
            val button = view.findViewById<View>(id) ?: continue
            button.setOnTouchListener { v, event ->
                if (reducedMotion || !android.animation.ValueAnimator.areAnimatorsEnabled())
                    return@setOnTouchListener false
                when (event.actionMasked) {
                    android.view.MotionEvent.ACTION_DOWN ->
                        v.animate()
                            .scaleX(PRESSED)
                            .scaleY(PRESSED)
                            .setDuration(110)
                            .setInterpolator(android.view.animation.DecelerateInterpolator())
                            .start()
                    android.view.MotionEvent.ACTION_UP,
                    android.view.MotionEvent.ACTION_CANCEL ->
                        v.animate()
                            .scaleX(1f)
                            .scaleY(1f)
                            .setDuration(360)
                            .setInterpolator(android.view.animation.OvershootInterpolator(2.4f))
                            .start()
                }
                false
            }
        }
    }

    private const val PRESSED = 0.84f

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
        // Ease out of the scrim: full at the edge, under half by the middle, clear at the end.
        val half = MediaTokens.videoScrim.copy(alpha = MediaTokens.videoScrim.alpha * 0.4f).toArgb()
        view.findViewById<View>(R.id.ytui_top_scrim)?.background =
            GradientDrawable(GradientDrawable.Orientation.TOP_BOTTOM, intArrayOf(scrim, half, clear))
        view.findViewById<View>(R.id.ytui_bottom_scrim)?.background =
            GradientDrawable(GradientDrawable.Orientation.BOTTOM_TOP, intArrayOf(scrim, half, clear))
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
                // Media3's own arrows are not centred on their ring, so the seek amount sat
                // off-centre. Use the app's symmetric ring (centred at 12,12) for both.
                val ring =
                    androidx.core.content.ContextCompat.getDrawable(
                        context,
                        if (id == androidx.media3.ui.R.id.exo_rew_with_amount) R.drawable.ytui_rewind
                        else R.drawable.ytui_forward,
                    )
                ViewCompat.setBackgroundTintList(this, null)
                foreground = null
                background =
                    if (ring == null) disc
                    else LayerDrawable(arrayOf(disc, ring)).apply { setId(1, android.R.id.icon) }
                // Media3 writes the seek amount ("10") as button text. Centre the digits
                // themselves: a centred text box puts them off by the font's spare ascent and
                // descent.
                (this as? android.widget.TextView)?.apply {
                    gravity = Gravity.CENTER
                    includeFontPadding = false
                    textSize = 10f
                    typeface =
                        androidx.core.content.res.ResourcesCompat.getFont(context, R.font.instrument_sans)
                            ?.let { android.graphics.Typeface.create(it, android.graphics.Typeface.BOLD) }
                    val digits = android.graphics.Rect()
                    paint.getTextBounds("0", 0, 1, digits)
                    val metrics = paint.fontMetrics
                    // How far the digits' middle sits below the text box's middle.
                    val below = (-metrics.ascent - metrics.descent - digits.height()) / 2f
                    val shift = kotlin.math.round(kotlin.math.abs(below) * 2).toInt()
                    if (below > 0) setPadding(0, 0, 0, shift) else setPadding(0, shift, 0, 0)
                }
            }
        }
        view.findViewById<DefaultTimeBar>(androidx.media3.ui.R.id.exo_progress)?.apply {
            setPlayedColor(accent)
            setScrubberColor(accent)
            // SponsorBlock ticks: a fixed warm marker stays distinct from any accent.
            setAdMarkerColor(MediaTokens.sponsorMark.toArgb())
            setPlayedAdMarkerColor(MediaTokens.sponsorMark.toArgb())
            // Choose the narrow keyline from the actual accent: dark over the scrim,
            // light only when a dark accent from a light/imported palette needs it.
            val backing = mediaTimeBarBacking(Color(accent))
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
