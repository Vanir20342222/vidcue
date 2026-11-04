package dev.ytui.android

import android.content.Intent
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.Canvas
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.basicMarquee
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsDraggedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.Player
import kotlin.math.abs
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.launch

@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
@Composable
internal fun VideoSurface(
    modifier: Modifier = Modifier,
    onCollapse: (() -> Unit)? = null,
    playbackPlayer: Player? = PlaybackService.current?.controlsPlayer,
    covered: Boolean = false,
    timingToken: Long = PlaybackService.current?.startupToken ?: 0,
    onSettings: (() -> Unit)? = null,
    onPanelDrag: ((Float) -> Unit)? = null,
    onPanelEnd: ((Float, Float) -> Unit)? = null,
) {
    val collapse by rememberUpdatedState(onCollapse)
    val panelDrag by rememberUpdatedState(onPanelDrag)
    val panelEnd by rememberUpdatedState(onPanelEnd)
    val settings by rememberUpdatedState(onSettings)
    val accent = MaterialTheme.colorScheme.primary.toArgb()
    val reduced by rememberUpdatedState(LocalReducedMotion.current)
    AndroidView(
        factory = { context ->
            val inflateAt = android.os.SystemClock.elapsedRealtime()
            VideoViewPreparation.take(context)
                .also {
                    StartupTimings.stage(
                        timingToken,
                        "view_inflate",
                        android.os.SystemClock.elapsedRealtime() - inflateAt,
                    )
                }
                .apply {
                    useController = true
                    setControllerAnimationEnabled(
                        !reduced && android.animation.ValueAnimator.areAnimatorsEnabled()
                    )
                    // Captions use ytui's own on/off button (PlayerChrome.bindCaptions).
                    // Queue modes stay in the app's transport; the video chrome is for watching.
                    setShowShuffleButton(false)
                    setFullscreenButtonClickListener {
                        context.startActivity(
                            Intent(context, FullscreenActivity::class.java)
                                .putExtra("reduced_motion", reduced)
                        )
                    }
                    tag = VisibleFrameTracker(this)
                    VideoGestures.attach(this)
                    PlayerChrome.trackSponsorMarks(this)
                    PlayerChrome.fitTransport(this)
                    PlayerChrome.bindCaptions(this)
                    VideoGestures.setPanelSwipe(
                        this,
                        object : VideoGestures.PanelSwipe {
                            override fun enabled() = panelDrag != null
                            override fun onDrag(dy: Float) {
                                panelDrag?.invoke(dy)
                            }
                            override fun onEnd(dy: Float, velocity: Float) {
                                panelEnd?.invoke(dy, velocity)
                            }
                        },
                    )
                }
        },
        modifier = modifier.background(Color.Black).testTag("native-video-surface"),
        update = { view ->
            view.setControllerAnimationEnabled(
                !reduced && android.animation.ValueAnimator.areAnimatorsEnabled()
            )
            PlayerChrome.rememberAccent(accent)
            PlayerChrome.apply(view)
            PlayerChrome.showSponsorMarks(view)
            PlayerChrome.showCaptions(view)
            view.findViewById<android.view.View>(R.id.ytui_collapse)?.apply {
                tag = "panel-close" // Native View tag; Compose tag remains on music/modal chrome.
                visibility =
                    if (collapse == null) android.view.View.GONE else android.view.View.VISIBLE
                setOnClickListener { collapse?.invoke() }
            }
            // The app's own options (quality, speed, volume, tracks, sleep, segments)
            // replace Media3's reduced settings popup.
            view.findViewById<android.view.View>(R.id.ytui_settings)?.apply {
                visibility =
                    if (settings == null) android.view.View.GONE else android.view.View.VISIBLE
                setOnClickListener {
                    view.hideController()
                    settings?.invoke()
                }
            }
            val actual = playbackPlayer
            (view.tag as VisibleFrameTracker).setPresentable(!covered)
            (view.tag as VisibleFrameTracker).bind(actual)
            view.keepScreenOn = actual?.playWhenReady == true
        },
        onRelease = {
            it.keepScreenOn = false
            (it.tag as VisibleFrameTracker).bind(null)
        },
    )
}

/** A bounded screen: advanced controls open separately, never below a long cover. */
@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
internal fun FocusedPlayer(
    vm: YtuiViewModel,
    allowVideoSurface: Boolean,
    minimize: () -> Unit,
    onQueue: () -> Unit,
    onMore: () -> Unit,
    onLyrics: () -> Unit = {},
    onSleep: () -> Unit = {},
) {
    val player by vm.player.collectAsState()
    val config by vm.config.collectAsState()
    val mobile by vm.mobile.collectAsState()
    val queue by vm.queue.collectAsState()
    val upNext by vm.upNext.collectAsState()
    val context = LocalContext.current
    val mediaUrl = player.media?.optString("url")
    // Keep the inline Up next list current as playback advances.
    LaunchedEffect(mediaUrl) { vm.loadUpNext() }
    val threshold = with(LocalDensity.current) { 72.dp.toPx() }
    val motion = LocalMotion.current
    val panelDrag = LocalPanelDrag.current
    val restingDrag = remember { Animatable(0f) }
    // The whole panel follows the finger, music and video alike.
    val dismiss =
        Modifier.swipeAxis(
            enabled = mobile.gesture("gesture_swipe_dismiss"),
            horizontal = false,
            offset = panelDrag?.offset ?: restingDrag,
            threshold = threshold,
            follow = panelDrag != null,
            allow = { it > 0 },
            onSwipe = { direction ->
                if (panelDrag != null) panelDrag.dismiss(this, direction) else minimize()
            },
        )
    // Horizontal swipe on music artwork changes track, carousel style.
    val trackShift = remember { Animatable(0f) }
    val haptic = rememberGestureHaptic()
    val trackWidth =
        with(LocalDensity.current) { LocalConfiguration.current.screenWidthDp.dp.toPx() }
    val trackSwipe =
        Modifier.swipeAxis(
            enabled = !player.video && mobile.gesture("gesture_swipe_tracks"),
            horizontal = true,
            offset = trackShift,
            threshold = with(LocalDensity.current) { 80.dp.toPx() },
            onSwipe = { direction ->
                val out = direction * trackWidth * 0.6f
                trackShift.animateTo(out, motion.exit(200))
                vm.command(if (direction < 0) "next" else "previous")
                trackShift.snapTo(-out)
                trackShift.animateTo(0f, motion.spring())
            },
        )
    val artTaps =
        Modifier.pointerInput(mobile.gesture("gesture_double_tap_seek")) {
            if (mobile.gesture("gesture_double_tap_seek"))
                detectTapGestures(
                    onDoubleTap = {
                        haptic()
                        vm.command("pause")
                    }
                )
        }
    val scope = rememberCoroutineScope()
    val openFullscreen = {
        context.startActivity(
            Intent(context, FullscreenActivity::class.java)
                .putExtra("reduced_motion", config.optBoolean("reduced_motion"))
        )
    }
    val swipeGestures = mobile.gesture("gesture_swipe_dismiss")
    val flingSpeed = with(LocalDensity.current) { 900.dp.toPx() }
    val fullscreenThreshold = with(LocalDensity.current) { 56.dp.toPx() }
    // YouTube-style: swipe the video down to minimize, up for fullscreen. The panel tracks the
    // finger on the way down; an upward swipe springs back as the fullscreen activity opens.
    val videoDrag: ((Float) -> Unit)? =
        if (!swipeGestures || panelDrag == null) null
        else
            ({ dy: Float ->
                if (motion.animate)
                    scope.launch(start = CoroutineStart.UNDISPATCHED) {
                        panelDrag.offset.snapTo(dy.coerceAtLeast(0f))
                    }
            })
    val videoEnd: ((Float, Float) -> Unit)? =
        if (!swipeGestures || panelDrag == null) null
        else
            ({ dy: Float, velocity: Float ->
                scope.launch {
                    when {
                        dy > threshold || (velocity > flingSpeed && dy > threshold / 3) -> {
                            haptic()
                            panelDrag.dismiss(this, 1)
                        }
                        dy < -fullscreenThreshold ||
                            (velocity < -flingSpeed && dy < -fullscreenThreshold / 3) -> {
                            haptic()
                            openFullscreen()
                            panelDrag.offset.animateTo(0f, motion.spring())
                        }
                        else -> panelDrag.offset.animateTo(0f, motion.spring())
                    }
                }
                Unit
            })
    BoxWithConstraints(Modifier.fillMaxSize().testTag("focused-player")) {
        val landscape = maxWidth > maxHeight * 1.25f
        val portraitVideoHeight =
            minOf(maxWidth * (9f / 16f), (maxHeight - 60.dp).coerceAtLeast(1.dp))
        @Composable
        fun Controls(modifier: Modifier) {
            val preparing = player.preparing
            if (preparing != null) {
                Column(
                    modifier
                        .padding(horizontal = MediaTokens.textInset)
                        .heightIn(min = 48.dp)
                        .testTag("panel-header")
                        .then(dismiss)
                ) {
                    Text(
                        preparing.media.title,
                        style = MaterialTheme.typography.titleLarge,
                        fontWeight = FontWeight.Bold,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (preparing.media.creator.isNotBlank())
                        Text(
                            preparing.media.creator,
                            maxLines = 1,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                }
                return
            }
            @Composable
            fun ActionRow() {
                // Labelled pill actions under the title (YouTube watch-page pattern), with
                // More pinned to the trailing edge. Every entry maps to an existing action;
                // fullscreen lives on the video itself. Narrow columns scroll the pills.
                val media = player.media
                val canSave =
                    media != null &&
                        !player.value.optBoolean("offline") &&
                        !media.optBoolean("live") &&
                        !player.preview
                Row(
                    Modifier.fillMaxWidth()
                        .then(
                            // Music: one soft tray, the same shape language as the dock, instead
                            // of loose icons floating above the gesture bar.
                            if (player.video) Modifier
                            else
                                Modifier.clip(MediaTokens.card)
                                    .background(LocalContentColor.current.copy(alpha = 0.07f))
                                    .padding(horizontal = 4.dp, vertical = 2.dp)
                        )
                        .testTag("player-actions"),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Row(
                        Modifier.weight(1f)
                            .then(
                                if (player.video) Modifier.horizontalScroll(rememberScrollState())
                                else Modifier
                            ),
                        horizontalArrangement =
                            if (player.video) Arrangement.spacedBy(8.dp)
                            else Arrangement.SpaceEvenly,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        val compact = !player.video
                        PlayerAction(Glyph.Queue, "Queue", "Playback queue", compact = compact, action = onQueue)
                        if (!player.video)
                            PlayerAction(Glyph.Lyrics, "Lyrics", "Lyrics", compact = compact, action = onLyrics)
                        if (canSave && media != null) {
                            var saveCentre by remember { mutableStateOf<androidx.compose.ui.geometry.Offset?>(null) }
                            PlayerAction(
                                Glyph.Download,
                                "Save",
                                "Save offline",
                                "player-download",
                                compact,
                                modifier = Modifier.trackCentre { saveCentre = it },
                            ) {
                                val card = MediaCard(media)
                                DownloadFlight.launch(card, saveCentre)
                                vm.play(card, download = true)
                            }
                        }
                        val sleepLeft = rememberSleepLeft(player)
                        PlayerAction(
                            Glyph.Sleep,
                            if (sleepLeft > 0) sleepLabel(sleepLeft) else "Sleep",
                            if (sleepLeft > 0) "Sleep timer, ${sleepLabel(sleepLeft)} left" else "Sleep timer",
                            compact = compact,
                            active = sleepLeft > 0,
                            action = onSleep,
                        )
                    }
                    Spacer(Modifier.width(4.dp))
                    MediaIconButton(
                        Glyph.More,
                        "Playback options",
                        "player-options",
                        action = onMore,
                    )
                }
            }
            Column(
                modifier
                    .padding(horizontal = MediaTokens.textInset)
                    .then(
                        if (player.video || LocalDensity.current.fontScale > 1.3f)
                            Modifier.verticalScroll(rememberScrollState())
                        else Modifier
                    ),
                verticalArrangement = Arrangement.spacedBy(if (landscape) 4.dp else 8.dp),
            ) {
                Column(
                    Modifier.fillMaxWidth()
                        .then(
                            if (player.video)
                                Modifier.heightIn(min = 48.dp).testTag("panel-header").then(dismiss)
                            else Modifier
                        ),
                    verticalArrangement = Arrangement.spacedBy(if (landscape) 4.dp else 8.dp),
                ) {
                    // Music keeps one line (long titles scroll) so the controls below never move
                    // when the next track has a longer or not-yet-known title.
                    Text(
                        player.title.ifBlank { " " },
                        if (player.video) Modifier
                        else Modifier.fillMaxWidth().basicMarquee(iterations = Int.MAX_VALUE, initialDelayMillis = 1800),
                        style =
                            if (player.video) MaterialTheme.typography.titleLarge
                            else MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.Bold),
                        maxLines = if (player.video) 2 else 1,
                        overflow = if (player.video) TextOverflow.Ellipsis else TextOverflow.Clip,
                    )
                    // Video: views and age under the title, then the channel row (watch-page
                    // order). Music keeps the artist directly under the title.
                    if (player.video) player.media?.let { MediaInformation(it, mobile) }
                    val creator =
                        player.value.optString("station").ifBlank {
                            player.media?.optString("creator") ?: ""
                        }
                    val showCreator =
                        creator.isNotBlank() &&
                            (mobile.optBoolean("show_channel", true) ||
                                player.media?.optBoolean("live") == true)
                    if (!player.video && !showCreator)
                        // Hold the artist line's height while the next track loads.
                        Text(" ", style = MaterialTheme.typography.bodyLarge, maxLines = 1)
                    if (showCreator)
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            if (player.video)
                                ChannelAvatar(
                                    creator,
                                    32.dp,
                                    imageUrl = player.media?.optString("channel_thumbnail").orEmpty(),
                                    loadImage = config.optBoolean("thumbnails", true),
                                )
                            Text(
                                creator,
                                style =
                                    if (player.video) MaterialTheme.typography.titleSmall
                                    else MaterialTheme.typography.bodyLarge,
                                fontWeight = if (player.video) FontWeight.Medium else null,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                                color =
                                    if (player.video) MaterialTheme.colorScheme.onSurface
                                    else MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                }
                player.value
                    .optString("error")
                    .takeUnless { it.isBlank() || it == "null" }
                    ?.let { Text(it, color = MaterialTheme.colorScheme.error, maxLines = 2) }
                if (player.video) ActionRow()
                if (!player.video)
                    // Fixed height: a track change (no duration yet) or a live station must not
                    // shrink this slot and shift the title above it.
                    Box(Modifier.fillMaxWidth().height(72.dp), contentAlignment = Alignment.CenterStart) {
                        PlayerSeek(vm, player)
                    }
                if (!player.video)
                    Row(
                        Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceEvenly,
                    ) {
                        MediaModeButton(
                            Glyph.Shuffle,
                            "Shuffle",
                            "player-shuffle",
                            player.value.optBoolean("shuffle"),
                        ) {
                            vm.command("shuffle")
                        }
                        TransportButton(Glyph.Previous, "Previous track") { vm.command("previous") }
                        PlayDisc(player.playing, Modifier.testTag("player-pause")) { vm.command("pause") }
                        TransportButton(Glyph.Next, "Next track", Modifier.testTag("player-next")) {
                            vm.command("next")
                        }
                        MediaModeButton(
                            Glyph.Repeat,
                            "Repeat one",
                            "player-repeat",
                            player.value.optBoolean("repeat_one"),
                        ) {
                            vm.command("repeat")
                        }
                    }
                if (!player.video) {
                    Spacer(Modifier.height(6.dp))
                    ActionRow()
                }
            }
        }
        @Composable
        fun Art(modifier: Modifier) {
            BoxWithConstraints(
                modifier.then(
                    if (player.video) Modifier else dismiss.then(trackSwipe).then(artTaps)
                ),
                // Music art shares spare height above and below instead of
                // leaving one gap between the cover and its title.
                contentAlignment = if (player.video) Alignment.TopCenter else Alignment.Center,
            ) {
                if (player.video && allowVideoSurface) {
                    Box(
                        Modifier.width(minOf(maxWidth, maxHeight * (16f / 9f)))
                            .aspectRatio(16f / 9f)
                    ) {
                        VideoSurface(
                            Modifier.fillMaxSize(),
                            onCollapse = minimize,
                            onSettings = onMore,
                            onPanelDrag = videoDrag,
                            onPanelEnd = videoEnd,
                            playbackPlayer = PlaybackService.current?.controlsPlayer,
                            covered = player.preparing != null,
                            timingToken =
                                player.preparing?.token ?: player.value.optLong("startup_token"),
                        )
                        player.preparing?.let { preparing ->
                            Box(
                                Modifier.fillMaxSize()
                                    .background(Color.Black)
                                    .testTag("preparing-video")
                                    .semantics { contentDescription = "Preparing video" },
                                contentAlignment = Alignment.Center,
                            ) {
                                Box(
                                    Modifier.matchParentSize().pointerInput(preparing.token) {
                                        awaitPointerEventScope {
                                            while (true) awaitPointerEvent().changes.forEach {
                                                it.consume()
                                            }
                                        }
                                    }
                                )
                                MediaSpinner(Modifier.size(36.dp), MediaTokens.videoInk)
                                Box(Modifier.align(Alignment.TopStart).padding(4.dp)) {
                                    CompositionLocalProvider(
                                        LocalContentColor provides MediaTokens.videoInk
                                    ) {
                                        MediaIconButton(
                                            Glyph.Down,
                                            "Minimize player",
                                            "panel-close",
                                            minimize,
                                        )
                                    }
                                }
                            }
                        }
                    }
                } else if (player.video) {
                    // The root's distinct mode flow can lag the player snapshot
                    // by one frame. Never create a native view in the music animation.
                    Spacer(Modifier.fillMaxSize().background(Color.Black))
                } else {
                    val ratio = visualizerRatio(androidPlayerStyle(config))
                    // The cover eases back while paused and steps forward when playing.
                    val rest by
                        animateFloatAsState(
                            if (player.playing || !motion.animate) 1f else 0.86f,
                            motion.spring(),
                            label = "cover-rest",
                        )
                    Visualizer(
                        player,
                        config,
                        Modifier.width(minOf(maxWidth, maxHeight * ratio))
                            .aspectRatio(ratio)
                            .graphicsLayer {
                                scaleX = rest
                                scaleY = rest
                                translationX = trackShift.value
                                val away = (abs(trackShift.value) / trackWidth).coerceIn(0f, 1f)
                                alpha = 1f - away * 0.7f
                                rotationZ = trackShift.value / trackWidth * 6f
                            },
                    )
                }
            }
        }
        if (player.video) {
            // Watch page: edge-to-edge 16:9 video (capped so details stay usable in
            // short windows), details directly beneath, then the real queue.
            if (landscape) {
                Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                    Art(Modifier.weight(2f).fillMaxHeight())
                    Controls(Modifier.weight(1f).fillMaxHeight())
                }
            } else {
                Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Art(Modifier.fillMaxWidth().height(portraitVideoHeight))
                    Controls(Modifier.fillMaxWidth())
                    UpNext(
                        queue,
                        upNext,
                        mediaUrl,
                        player.value.optBoolean("shuffle"),
                        config.optBoolean("thumbnails", true),
                        mobile.optBoolean("show_channel", true),
                        Modifier.weight(1f),
                        onQueue,
                        play = vm::playUpNext,
                    )
                }
            }
        } else if (landscape)
            Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                Art(Modifier.weight(1f).fillMaxHeight())
                Controls(Modifier.weight(1f).align(Alignment.CenterVertically))
            }
        else
            Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Art(Modifier.weight(1f).fillMaxWidth().padding(horizontal = 20.dp, vertical = 4.dp))
                Controls(Modifier.fillMaxWidth().padding(bottom = 12.dp))
            }
    }
}

/**
 * Inline Up next under the video and in the fullscreen options panel, in the order Next takes
 * them: the manual queue, then the rest of the playlist or related videos. Full queue management
 * stays in the queue sheet (`onManage == null` hides that entry).
 */
@Composable
internal fun UpNext(
    queue: List<MediaCard>,
    state: UpNextState,
    playingUrl: String?,
    shuffle: Boolean,
    thumbnails: Boolean,
    showCreator: Boolean,
    modifier: Modifier,
    onManage: (() -> Unit)?,
    showHeader: Boolean = true,
    play: (MediaCard, Boolean) -> Unit,
) {
    val upcoming = queue.filter { it.url.isBlank() || it.url != playingUrl }
    // The item Next/autoplay starts, unless shuffle makes the choice random.
    val next =
        if (shuffle) null
        else
            upcoming.firstOrNull()
                ?: state.context.firstOrNull { it.url != playingUrl }.takeIf { state.autoplay }
    val contextLabel = if (state.related) "Recommended" else "From this playlist"
    Column(modifier.fillMaxWidth().testTag("up-next")) {
        if (showHeader)
            Row(
                Modifier.fillMaxWidth().padding(start = MediaTokens.textInset, end = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text("Up next", style = MaterialTheme.typography.titleLarge)
                // With no manual queue the context list is all there is: name its source in
                // the header instead of stacking a second heading above the first row.
                Text(
                    if (queue.isEmpty() && state.context.isNotEmpty()) contextLabel else "",
                    Modifier.weight(1f).padding(start = 8.dp),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (queue.isNotEmpty() && onManage != null)
                    Pill("Edit queue", style = PillStyle.Quiet, onClick = onManage)
            }
        if (queue.isEmpty() && state.context.isEmpty())
            Text(
                if (state.loading) "Finding what plays next…"
                else "Nothing queued. Add videos with Add to queue in any ⋮ menu.",
                Modifier.padding(horizontal = MediaTokens.textInset, vertical = 8.dp),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        else
            androidx.compose.foundation.lazy.LazyColumn(
                Modifier.weight(1f).fillMaxWidth(),
                contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                @Composable
                fun Entry(media: MediaCard, fromQueue: Boolean) {
                    val current = media.url.isNotBlank() && media.url == playingUrl
                    val label =
                        when {
                            current -> "Playing"
                            media === next -> "Plays next"
                            else -> ""
                        }
                    val creator = if (showCreator) media.creator else ""
                    MediaListRow(
                        media.title,
                        listOf(label, creator).filter { it.isNotBlank() }.joinToString(" · "),
                        Modifier.padding(horizontal = 8.dp),
                        artwork = { Artwork(media, it, thumbnails) },
                        square = media.audio,
                        highlighted = current,
                        onClick = { play(media, fromQueue) },
                    )
                }
                items(queue.size, key = { "${queue[it].key}#$it" }) { Entry(queue[it], true) }
                if (state.context.isNotEmpty()) {
                    if (queue.isNotEmpty() || !showHeader)
                        item(key = "context-header") {
                            SectionHeader(
                                contextLabel,
                                Modifier.padding(start = 4.dp).testTag("up-next-context"),
                            )
                        }
                    items(state.context.size, key = { "context:${state.context[it].key}#$it" }) {
                        Entry(state.context[it], false)
                    }
                    if (state.more)
                        item(key = "context-more") {
                            Text(
                                "More after these",
                                Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                }
            }
    }
}

/** Labelled pill for the player action row; [description] stays the accessible name. */
@Composable
internal fun PlayerAction(
    glyph: Glyph,
    label: String,
    description: String,
    tag: String = "",
    compact: Boolean = false,
    active: Boolean = false,
    modifier: Modifier = Modifier,
    action: () -> Unit,
) {
    val source = remember { MutableInteractionSource() }
    val accent = mediaReadableText(MaterialTheme.colorScheme.primary, MaterialTheme.colorScheme.background)
    if (compact) {
        // Music: an evenly spaced icon row with small captions, like a transport's second line.
        Column(
            modifier.testTag(tag)
                .pressScale(source, 0.9f)
                .widthIn(min = 56.dp)
                .heightIn(min = 56.dp)
                .clip(MediaTokens.medium)
                .clickable(source, SoftIndication, role = Role.Button, onClick = action)
                .semantics { contentDescription = description }
                .padding(horizontal = 6.dp, vertical = 6.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            MediaIcon(glyph, Modifier.size(22.dp), if (active) accent else LocalContentColor.current, filled = active)
            Text(
                label,
                Modifier.clearAndSetSemantics {},
                style =
                    MaterialTheme.typography.labelSmall.merge(
                        androidx.compose.ui.text.TextStyle(fontFeatureSettings = "tnum")
                    ),
                color = if (active) accent else MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
            )
        }
        return
    }
    Row(
        modifier.testTag(tag)
            .pressScale(source, 0.94f)
            .heightIn(min = 48.dp)
            .clip(MediaTokens.pill)
            .clickable(source, SoftIndication, role = Role.Button, onClick = action)
            .semantics { contentDescription = description },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(
            // The visible label repeats the description; keep one accessible name.
            Modifier.clip(MediaTokens.pill)
                .background(LocalContentColor.current.copy(alpha = 0.09f))
                .padding(start = 12.dp, end = 16.dp)
                .heightIn(min = 38.dp)
                .clearAndSetSemantics {},
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            MediaIcon(glyph, Modifier.size(18.dp), if (active) accent else LocalContentColor.current, filled = active)
            Text(label, style = MaterialTheme.typography.labelLarge, maxLines = 1, color = if (active) accent else Color.Unspecified)
        }
    }
}

/** The main play/pause control: a large ink disc whose glyph morphs between states. */
@Composable
internal fun PlayDisc(playing: Boolean, modifier: Modifier = Modifier, size: androidx.compose.ui.unit.Dp = 72.dp, action: () -> Unit) {
    val source = remember { MutableInteractionSource() }
    val ink = MaterialTheme.colorScheme.onBackground
    Box(
        modifier
            .size(size)
            .pressScale(source, 0.9f)
            .shadow(10.dp, CircleShape, ambientColor = Color.Black, spotColor = Color.Black)
            .clip(CircleShape)
            .background(ink)
            .clickable(source, null, role = Role.Button, onClick = action)
            .semantics { contentDescription = if (playing) "Pause" else "Play" },
        contentAlignment = Alignment.Center,
    ) {
        PlayPauseIcon(playing, Modifier.size(size * 0.42f), MaterialTheme.colorScheme.background)
    }
}

/** Previous/next: larger glyphs than toolbar icons, the same soft press. */
@Composable
internal fun TransportButton(glyph: Glyph, label: String, modifier: Modifier = Modifier, action: () -> Unit) =
    RoundIconButton(glyph, label, modifier, size = 60.dp, iconSize = 30.dp, action = action)

@Composable
internal fun PlayerSeek(vm: YtuiViewModel, player: PlayerState) {
    val live = player.media?.optBoolean("live") == true
    var drag by remember(player.media?.optString("url")) { mutableStateOf<Float?>(null) }
    if (live) {
        // The same live marker as on station artwork, instead of bare capitals.
        ArtworkBadge(true, "Live", Modifier.heightIn(min = 36.dp).testTag("player-live"))
    } else if (player.duration <= 0f) {
        Text(
            if (player.value.optBoolean("buffering")) "Buffering…" else "",
            Modifier.heightIn(min = 36.dp),
            color = MaterialTheme.colorScheme.primary,
            style = MaterialTheme.typography.labelMedium,
        )
    } else {
        Column {
            MediaSeekSlider(
                (drag ?: player.position).coerceIn(0f, player.duration),
                { drag = it },
                valueRange = 0f..player.duration,
                onValueChangeFinished = {
                    drag?.let { vm.command("seek", "seconds" to it) }
                    drag = null
                },
                modifier = Modifier.fillMaxWidth().testTag("seek-slider"),
                marks = player.sponsorMarks(),
            )
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                val numbers =
                    MaterialTheme.typography.labelMedium.merge(
                        androidx.compose.ui.text.TextStyle(fontFeatureSettings = "tnum")
                    )
                Text(time(drag ?: player.position), style = numbers, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(
                    "-" + time((player.duration - (drag ?: player.position)).coerceAtLeast(0f)),
                    style = numbers,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

/** SponsorBlock segments as (start, end) seconds for drawing on the seek timeline. */
internal fun PlayerState.sponsorMarks(): List<Pair<Float, Float>> =
    value.optJSONArray("segments")?.objects()?.mapNotNull { mark ->
        val start = mark.optDouble("start").toFloat()
        val end = mark.optDouble("end").toFloat()
        if (start.isFinite() && end.isFinite() && end > start) start to end else null
    } ?: emptyList()

/**
 * Keep Material's seek semantics/drag handling; draw a slim line and persistent round playhead.
 * [marks] are (start, end) values in [valueRange] highlighted on the track, e.g. sponsor segments.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun MediaSeekSlider(
    value: Float,
    onValueChange: (Float) -> Unit,
    valueRange: ClosedFloatingPointRange<Float>,
    onValueChangeFinished: () -> Unit,
    modifier: Modifier = Modifier,
    marks: List<Pair<Float, Float>> = emptyList(),
) {
    val accent = MaterialTheme.colorScheme.onBackground
    val track = MaterialTheme.colorScheme.onBackground.copy(alpha = 0.16f)
    val markColor = MediaTokens.sponsorMark
    val source = remember { MutableInteractionSource() }
    val dragging by source.collectIsDraggedAsState()
    val pressing by source.collectIsPressedAsState()
    val active = dragging || pressing
    val motion = LocalMotion.current
    val thickness by animateDpAsState(if (active) 8.dp else 4.dp, motion.spring(), label = "seek-track")
    val knob by animateDpAsState(if (active) 18.dp else 12.dp, motion.spring(), label = "seek-knob")
    Slider(
        value,
        onValueChange,
        modifier.heightIn(min = 48.dp),
        valueRange = valueRange,
        onValueChangeFinished = onValueChangeFinished,
        interactionSource = source,
        thumb = {
            Box(Modifier.size(18.dp), contentAlignment = Alignment.Center) {
                Box(Modifier.size(knob).shadow(3.dp, CircleShape).clip(CircleShape).background(accent))
            }
        },
        track = { state ->
            Canvas(Modifier.fillMaxWidth().height(thickness)) {
                val fraction =
                    mediaProgress(
                        state.value - state.valueRange.start,
                        state.valueRange.endInclusive - state.valueRange.start,
                    )
                val radius = androidx.compose.ui.geometry.CornerRadius(size.height / 2)
                drawRoundRect(track, cornerRadius = radius)
                drawRoundRect(
                    accent,
                    size = androidx.compose.ui.geometry.Size(size.width * fraction, size.height),
                    cornerRadius = radius,
                )
                val span = state.valueRange.endInclusive - state.valueRange.start
                if (span > 0f) {
                    marks.forEach { (start, end) ->
                        val from = ((start - state.valueRange.start) / span).coerceIn(0f, 1f)
                        val to = ((end - state.valueRange.start) / span).coerceIn(from, 1f)
                        if (to > from) {
                            val y = size.height / 2
                            drawLine(
                                markColor,
                                androidx.compose.ui.geometry.Offset(size.width * from, y),
                                androidx.compose.ui.geometry.Offset(size.width * to, y),
                                size.height,
                            )
                        }
                    }
                }
            }
        },
    )
}

/** Labelled seek jump ("−10" / "+10") with the same round press as other transport keys. */
@Composable
internal fun SeekJump(label: String, description: String, action: () -> Unit) {
    val source = remember { MutableInteractionSource() }
    Box(
        Modifier.size(52.dp)
            .pressScale(source, 0.9f)
            .clip(CircleShape)
            .clickable(source, SoftIndication, role = Role.Button, onClick = action)
            .semantics { contentDescription = description },
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label,
            style =
                MaterialTheme.typography.titleSmall.merge(
                    androidx.compose.ui.text.TextStyle(fontFeatureSettings = "tnum")
                ),
            modifier = Modifier.clearAndSetSemantics {},
        )
    }
}
