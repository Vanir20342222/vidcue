package dev.ytui.android

import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.net.Uri
import android.os.Bundle
import android.view.View
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.ViewCompositionStrategy
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.Tracks
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.PlayerView
import java.io.File
import java.util.Locale
import kotlinx.coroutines.delay
import org.json.JSONArray

/**
 * Full-screen player for Movies (alpha). It uses the same native video chrome as YouTube playback
 * (the Media3 controller layout, gestures, colour grading, captions button and glass options
 * panel) around its own player: it plays the checked source first and fails over to the next one
 * when a source errors mid-film, keeping the position. Quality, audio, subtitle and speed options
 * read the stream's own tracks; the position is saved for Resume.
 */
@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
class MoviePlayerActivity : ComponentActivity() {
    private var player: ExoPlayer? = null
    private lateinit var key: String
    private var sources: List<MovieSource> = emptyList()
    private var sourceIndex = mutableIntStateOf(0)
    private var offline: String? = null
    private val optionsOpen = mutableStateOf(false)

    /** HTTP loader for Cinecat streams: per-source headers are applied on every load.
     * The UA matches the shared transfer code, which some providers expect. */
    private val httpFactory =
        DefaultHttpDataSource.Factory().setAllowCrossProtocolRedirects(true).setUserAgent("Vidcue-Cinecat/0.1")

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        MovieLibrary.init(this)
        key = intent.getStringExtra(EXTRA_KEY).orEmpty()
        val title = intent.getStringExtra(EXTRA_TITLE).orEmpty()
        offline = intent.getStringExtra(EXTRA_OFFLINE)
        sources =
            runCatching {
                    val array = JSONArray(intent.getStringExtra(EXTRA_SOURCES) ?: "[]")
                    (0 until array.length()).mapNotNull { i -> array.optJSONObject(i)?.let(::movieSourceFromJson) }
                }
                .getOrDefault(emptyList())
        if (key.isBlank() || (offline == null && sources.isEmpty())) {
            finish()
            return
        }
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowCompat.getInsetsController(window, window.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
        // Seek steps follow the same setting as the YouTube player's buttons and double taps.
        val step = MobileSettings.integer("seek_seconds") * 1000L
        val exo =
            ExoPlayer.Builder(this)
                .setMediaSourceFactory(DefaultMediaSourceFactory(DefaultDataSource.Factory(this, httpFactory)))
                .setSeekBackIncrementMs(step)
                .setSeekForwardIncrementMs(step)
                .build()
        player = exo
        exo.addListener(
            object : Player.Listener {
                override fun onIsPlayingChanged(playing: Boolean) {
                    if (playing) window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                    else window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                }

                override fun onPlaybackStateChanged(state: Int) {
                    if (state == Player.STATE_ENDED) MovieLibrary.clearPosition(key)
                }
            }
        )
        load(sourceIndex.intValue, MovieLibrary.startPosition(key))

        val view = videoView(exo, title)
        val root = FrameLayout(this)
        root.setBackgroundColor(android.graphics.Color.BLACK)
        root.addView(view, matchParent())
        setContentView(root)
        // Failure, failover notices and the options panel sit above the video in this window,
        // as in the YouTube fullscreen player, so system bars stay hidden.
        root.addView(
            ComposeView(this).apply {
                setViewCompositionStrategy(ViewCompositionStrategy.DisposeOnViewTreeLifecycleDestroyed)
                setContent {
                    val scheme = ComposeHost.colorScheme ?: darkColorScheme()
                    val reduced = AuthUi.reducedMotion || !android.animation.ValueAnimator.areAnimatorsEnabled()
                    MaterialTheme(colorScheme = scheme, typography = ComposeHost.typography) {
                        CompositionLocalProvider(
                            LocalContentColor provides scheme.onSurface,
                            LocalReducedMotion provides reduced,
                            LocalMotion provides Motion(reduced = reduced),
                            androidx.compose.foundation.LocalIndication provides SoftIndication,
                        ) {
                            MovieOverlay(
                                exo,
                                view,
                                sources,
                                offline != null,
                                sourceIndex,
                                optionsOpen,
                                onFailover = ::failover,
                                onRetry = { load(0, exo.currentPosition) },
                                onSwitch = { load(it, exo.currentPosition) },
                                onSave = ::savePosition,
                                onClose = ::finish,
                            )
                        }
                    }
                }
            },
            matchParent(),
        )
        ViewCompat.setOnApplyWindowInsetsListener(root) { _, insets ->
            val cutout = insets.getInsets(WindowInsetsCompat.Type.displayCutout())
            PlayerChrome.fitControls(view, cutout.left, cutout.top, cutout.right)
            insets
        }
    }

    /** The YouTube player's native view and chrome, bound to this activity's own player. */
    private fun videoView(exo: ExoPlayer, title: String): PlayerView {
        val reduced = AuthUi.reducedMotion
        val view = layoutInflater.inflate(R.layout.ytui_player, null) as PlayerView
        view.setBackgroundColor(android.graphics.Color.BLACK)
        // Gestures and the captions button act on this player, never the YouTube session.
        PlayerChrome.markStandalone(view)
        view.useController = true
        ComposeHost.colorScheme?.let { PlayerChrome.rememberAccent(it.primary.toArgb()) }
        PlayerChrome.apply(view)
        view.setControllerAnimationEnabled(!reduced && android.animation.ValueAnimator.areAnimatorsEnabled())
        PlayerChrome.reducedMotion = reduced
        PlayerChrome.bindCaptions(view)
        PlayerChrome.softenPresses(view)
        // One film per screen: no queue, shuffle or fullscreen toggle.
        view.setShowShuffleButton(false)
        view.setShowPreviousButton(false)
        view.setShowNextButton(false)
        view.findViewById<View>(R.id.ytui_collapse)?.apply {
            setOnClickListener { finish() }
            contentDescription = getString(R.string.ytui_player_close)
        }
        view.findViewById<View>(R.id.ytui_heading)?.visibility = View.VISIBLE
        view.findViewById<TextView>(R.id.ytui_title)?.text = title
        view.findViewById<View>(R.id.ytui_settings)?.apply {
            visibility = View.VISIBLE
            setOnClickListener {
                view.hideController()
                optionsOpen.value = true
            }
        }
        VideoGestures.attach(view)
        ColorGrade.attach(view)
        PlayerChrome.trackSponsorMarks(view)
        PlayerChrome.trackLive(view)
        view.player = exo
        return view
    }

    /** Prepares source [index] (or the offline copy) at [positionMs]. */
    private fun load(index: Int, positionMs: Long) {
        val exo = player ?: return
        sourceIndex.intValue = index
        val item =
            offline?.let { path ->
                val builder = MediaItem.Builder().setUri(Uri.fromFile(File(path)))
                if (path.endsWith(".m3u8")) builder.setMimeType(MimeTypes.APPLICATION_M3U8)
                builder.build()
            }
                ?: sources[index].let { source ->
                    // Cinecat providers demand Origin/Referer; plain catalog URLs carry none.
                    httpFactory.setDefaultRequestProperties(HashMap(source.headers))
                    MediaItem.Builder()
                        .setUri(source.url)
                        .apply { if (source.kind == "hls") setMimeType(MimeTypes.APPLICATION_M3U8) }
                        .setSubtitleConfigurations(sidecarSubtitles(source.subtitles))
                        .build()
                }
        exo.setMediaItem(item, positionMs)
        exo.prepare()
        exo.playWhenReady = true
    }

    /** Sidecar captions the engine resolved; VTT and SRT are supported, other formats are skipped. */
    private fun sidecarSubtitles(subs: List<MovieSubtitle>): List<MediaItem.SubtitleConfiguration> =
        subs.mapNotNull { sub ->
            val mime =
                when (sub.format.lowercase(Locale.ROOT)) {
                    "vtt" -> MimeTypes.TEXT_VTT
                    "srt" -> MimeTypes.APPLICATION_SUBRIP
                    else -> return@mapNotNull null
                }
            MediaItem.SubtitleConfiguration.Builder(Uri.parse(sub.url))
                .setMimeType(mime)
                .setLanguage(sub.language.takeIf { it.isNotBlank() })
                .setLabel(sub.display.takeIf { it.isNotBlank() })
                .build()
        }

    /** Moves to the next source after an error; false when none is left. */
    private fun failover(): Boolean {
        val exo = player ?: return false
        if (offline != null || sourceIndex.intValue + 1 >= sources.size) return false
        load(sourceIndex.intValue + 1, exo.currentPosition)
        return true
    }

    private fun savePosition() {
        val exo = player ?: return
        if (exo.playbackState == Player.STATE_ENDED) return
        val duration = exo.duration.takeIf { it != C.TIME_UNSET } ?: 0
        MovieLibrary.savePosition(key, exo.currentPosition, duration)
    }

    override fun onStop() {
        super.onStop()
        savePosition()
        player?.pause()
    }

    override fun onDestroy() {
        player?.release()
        player = null
        super.onDestroy()
    }

    companion object {
        private const val EXTRA_KEY = "key"
        private const val EXTRA_TITLE = "title"
        private const val EXTRA_SOURCES = "sources"
        private const val EXTRA_OFFLINE = "offline"

        private fun matchParent() =
            FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT)

        /** Streams [sources] in order (the checked one first). */
        internal fun stream(context: Context, key: String, title: String, sources: List<MovieSource>) {
            context.startActivity(
                Intent(context, MoviePlayerActivity::class.java)
                    .putExtra(EXTRA_KEY, key)
                    .putExtra(EXTRA_TITLE, title)
                    .putExtra(EXTRA_SOURCES, JSONArray(sources.map { it.json() }).toString())
            )
        }

        /** Plays a finished download from app storage. */
        internal fun offline(context: Context, download: MovieDownload) {
            context.startActivity(
                Intent(context, MoviePlayerActivity::class.java)
                    .putExtra(EXTRA_KEY, download.key)
                    .putExtra(EXTRA_TITLE, download.title)
                    .putExtra(EXTRA_OFFLINE, download.entrypoint)
            )
        }
    }
}

/** One choice in a track menu. */
private data class TrackChoice(val label: String, val selected: Boolean, val apply: () -> Unit)

private val SPEEDS = listOf(0.5f, 0.75f, 1f, 1.25f, 1.5f, 2f)

/**
 * Everything the movie player adds above the shared video chrome: the loading spinner, failover
 * notices, the "Playback stopped" card and the playback options panel.
 */
@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
@Composable
private fun MovieOverlay(
    player: ExoPlayer,
    view: PlayerView,
    sources: List<MovieSource>,
    offline: Boolean,
    sourceIndex: State<Int>,
    optionsOpen: MutableState<Boolean>,
    onFailover: () -> Boolean,
    onRetry: () -> Unit,
    onSwitch: (Int) -> Unit,
    onSave: () -> Unit,
    onClose: () -> Unit,
) {
    var buffering by remember { mutableStateOf(true) }
    var tracks by remember { mutableStateOf(player.currentTracks) }
    var speed by remember { mutableFloatStateOf(player.playbackParameters.speed) }
    var notice by remember { mutableStateOf("") }
    var failure by remember { mutableStateOf("") }

    DisposableEffect(player) {
        val listener =
            object : Player.Listener {
                override fun onPlaybackStateChanged(state: Int) {
                    buffering = state == Player.STATE_BUFFERING
                    if (state == Player.STATE_READY) failure = ""
                }

                override fun onTracksChanged(value: Tracks) {
                    tracks = value
                    PlayerChrome.showCaptions(view)
                }

                override fun onPlaybackParametersChanged(parameters: PlaybackParameters) {
                    speed = parameters.speed
                }

                override fun onPlayerError(error: PlaybackException) {
                    val failed = sources.getOrNull(sourceIndex.value)?.label ?: "This copy"
                    val reason = error.errorCodeName.removePrefix("ERROR_CODE_").lowercase(Locale.ROOT).replace('_', ' ')
                    if (onFailover()) {
                        val next = sources.getOrNull(sourceIndex.value)?.label.orEmpty()
                        notice = "$failed stopped ($reason). Switched to $next."
                    } else {
                        failure = "$failed stopped ($reason)."
                        view.hideController()
                    }
                }
            }
        player.addListener(listener)
        onDispose { player.removeListener(listener) }
    }
    // Save the position every five seconds of playback for Resume.
    LaunchedEffect(player) {
        while (true) {
            delay(5_000)
            if (player.isPlaying) onSave()
        }
    }
    LaunchedEffect(notice) {
        if (notice.isNotEmpty()) {
            delay(5_000)
            notice = ""
        }
    }
    val sourceLabel = if (offline) "Downloaded copy" else sources.getOrNull(sourceIndex.value)?.label.orEmpty()
    SideEffect {
        view.findViewById<TextView>(R.id.ytui_subtitle)?.apply {
            text = sourceLabel
            visibility = if (sourceLabel.isBlank()) View.GONE else View.VISIBLE
        }
    }

    Box(Modifier.fillMaxSize()) {
        if (rememberStalled(buffering && failure.isEmpty())) LoadingSpinner(Modifier.align(Alignment.Center))
        AnimatedVisibility(notice.isNotEmpty(), Modifier.align(Alignment.TopCenter), enter = fadeIn(), exit = fadeOut()) {
            Text(
                notice,
                Modifier.windowInsetsPadding(WindowInsets.safeDrawing)
                    .padding(top = 64.dp)
                    .clip(MediaTokens.pill)
                    .background(Color.Black.copy(alpha = 0.8f))
                    .padding(horizontal = 16.dp, vertical = 10.dp),
                style = MaterialTheme.typography.bodyMedium,
                color = Color.White,
            )
        }
        if (failure.isNotEmpty())
            Box(
                Modifier.fillMaxSize()
                    .background(Color.Black.copy(alpha = 0.6f))
                    .pointerInput(Unit) { detectTapGestures {} },
                contentAlignment = Alignment.Center,
            ) {
                Column(
                    Modifier.widthIn(max = 420.dp)
                        .clip(MediaTokens.dialog)
                        .background(Color.Black.copy(alpha = 0.75f))
                        .padding(20.dp)
                        .testTag("movie-failure"),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    CompositionLocalProvider(LocalContentColor provides Color.White) {
                        Text("Playback stopped", style = MaterialTheme.typography.titleMedium, color = Color.White)
                        Text(
                            failure + if (!offline && sources.size > 1) " Every source was tried." else "",
                            style = MaterialTheme.typography.bodyMedium,
                            color = Color.White.copy(alpha = 0.8f),
                        )
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            QuietAction("Close", action = onClose)
                            Pill("Try again", style = PillStyle.Primary, glyph = Glyph.Refresh) {
                                failure = ""
                                onRetry()
                            }
                        }
                    }
                }
            }
        MovieOptions(player, view, tracks, speed, sources, offline, sourceIndex, optionsOpen) {
            failure = ""
            onSwitch(it)
        }
    }
}

private fun speedLabel(speed: Float): String =
    if (speed == 1f) "Normal" else String.format(Locale.ROOT, "%sx", speed.toString().removeSuffix(".0"))

/**
 * The movie's playback options in the same frosted side panel as the YouTube fullscreen player:
 * quality, audio, subtitles, speed and (when several copies were found) the source.
 */
@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
@Composable
private fun MovieOptions(
    player: ExoPlayer,
    view: PlayerView,
    tracks: Tracks,
    speed: Float,
    sources: List<MovieSource>,
    offline: Boolean,
    sourceIndex: State<Int>,
    open: MutableState<Boolean>,
    onSwitch: (Int) -> Unit,
) {
    val visible = open.value
    val reduced = LocalReducedMotion.current
    val scheme = MaterialTheme.colorScheme
    var page by remember { mutableStateOf("") }
    fun close() {
        open.value = false
        page = ""
    }
    BackHandler(visible) { if (page.isNotEmpty()) page = "" else close() }
    val blurSupported = android.os.Build.VERSION.SDK_INT >= 31
    val blurTarget = with(LocalDensity.current) { if (visible) 28.dp.toPx() else 0f }
    val blur by
        animateFloatAsState(
            blurTarget,
            if (reduced) androidx.compose.animation.core.snap() else tween(220),
            label = "movie-video-blur",
        )
    val content = view.findViewById<View>(androidx.media3.ui.R.id.exo_content_frame)
    SideEffect { blurVideo(content, blur) }
    DisposableEffect(content) { onDispose { blurVideo(content, 0f) } }

    val sections =
        buildList {
            add(Triple("quality", "Quality", qualityChoices(player, tracks)))
            add(Triple("audio", "Audio", audioChoices(player, tracks)))
            add(Triple("subtitles", "Subtitles", subtitleChoices(player, tracks)))
            add(
                Triple(
                    "speed",
                    "Playback speed",
                    SPEEDS.map { s -> TrackChoice(speedLabel(s), s == speed) { player.setPlaybackSpeed(s) } },
                )
            )
            if (!offline && sources.size > 1)
                add(
                    Triple(
                        "source",
                        "Source",
                        sources.mapIndexed { i, s ->
                            TrackChoice(s.label, i == sourceIndex.value) { if (i != sourceIndex.value) onSwitch(i) }
                        },
                    )
                )
        }
    val current = sections.firstOrNull { it.first == page }

    BoxWithConstraints(Modifier.fillMaxSize()) {
        val wide = maxWidth > maxHeight
        val sheetWidth = if (wide) minOf(420.dp, maxWidth * 0.5f) else maxWidth
        AnimatedVisibility(
            visible,
            enter = if (reduced) EnterTransition.None else fadeIn(tween(160)),
            exit = if (reduced) ExitTransition.None else fadeOut(tween(120)),
        ) {
            // Tapping the dimmed video closes the panel; it never reaches the player.
            Box(
                Modifier.fillMaxSize()
                    .background(Color.Black.copy(alpha = if (blurSupported) 0.28f else 0.5f))
                    .testTag("movie-options-scrim")
                    .pointerInput(Unit) { detectTapGestures { close() } }
            )
        }
        AnimatedVisibility(
            visible,
            Modifier.align(Alignment.CenterEnd),
            enter =
                if (reduced) EnterTransition.None
                else slideInHorizontally(tween(220)) { it / 3 } + fadeIn(tween(160)),
            exit =
                if (reduced) ExitTransition.None
                else slideOutHorizontally(tween(160)) { it / 3 } + fadeOut(tween(120)),
        ) {
            Surface(
                Modifier.width(sheetWidth)
                    .fillMaxHeight()
                    .testTag("movie-menu")
                    .semantics { paneTitle = "Playback options" },
                shape = RoundedCornerShape(topStart = 28.dp, bottomStart = 28.dp),
                color = scheme.surfaceContainerLow.copy(alpha = if (blurSupported) 0.72f else 0.92f),
                border = BorderStroke(1.dp, MediaTokens.videoGlassEdge.copy(alpha = 0.18f)),
            ) {
                Column(
                    Modifier.fillMaxSize()
                        .background(
                            Brush.verticalGradient(listOf(MediaTokens.videoGlassSheen, Color.Transparent), endY = 360f)
                        )
                        .windowInsetsPadding(
                            WindowInsets.displayCutout.only(WindowInsetsSides.End + WindowInsetsSides.Vertical)
                        )
                        .padding(start = 8.dp, end = 8.dp, top = 8.dp)
                ) {
                    Row(Modifier.fillMaxWidth().heightIn(min = 56.dp), verticalAlignment = Alignment.CenterVertically) {
                        if (current != null)
                            MediaIconButton(Glyph.Back, "Back to playback options", "movie-options-back") { page = "" }
                        Text(
                            current?.second ?: "Playback options",
                            Modifier.weight(1f).padding(horizontal = 8.dp),
                            style = MaterialTheme.typography.titleLarge,
                            fontWeight = FontWeight.Medium,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        MediaIconButton(Glyph.Close, "Close playback options", "movie-options-close") { close() }
                    }
                    HorizontalDivider(color = scheme.outlineVariant)
                    Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(bottom = 16.dp)) {
                        if (current == null)
                            for ((id, label, choices) in sections)
                                SheetAction(
                                    text = {
                                        Column {
                                            Text(label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                            Text(
                                                choices.firstOrNull { it.selected }?.label
                                                    ?: if (choices.isEmpty()) "None in this stream" else "Off",
                                                style = MaterialTheme.typography.bodySmall,
                                                color = scheme.onSurfaceVariant,
                                                maxLines = 1,
                                                overflow = TextOverflow.Ellipsis,
                                            )
                                        }
                                    },
                                    onClick = { page = id },
                                    modifier = Modifier.testTag("movie-option-$id"),
                                    leadingIcon = {
                                        MediaIcon(
                                            when (id) {
                                                "quality" -> Glyph.Settings
                                                "audio" -> Glyph.Volume
                                                "subtitles" -> Glyph.Captions
                                                "speed" -> Glyph.Speed
                                                else -> Glyph.Link
                                            },
                                            Modifier.size(22.dp),
                                            tint = scheme.onSurface,
                                        )
                                    },
                                )
                        else if (current.third.isEmpty())
                            Text(
                                "This stream offers no choices here.",
                                Modifier.padding(16.dp),
                                style = MaterialTheme.typography.bodyMedium,
                                color = scheme.onSurfaceVariant,
                            )
                        else
                            for (choice in current.third)
                                ChoiceRow(choice.label, choice.selected) {
                                    choice.apply()
                                    PlayerChrome.showCaptions(view)
                                    close()
                                }
                    }
                }
            }
        }
    }
}

/** "Auto" plus one entry per video height, highest first. */
@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
private fun qualityChoices(player: ExoPlayer, tracks: Tracks): List<TrackChoice> {
    val params = player.trackSelectionParameters
    val overridden = params.overrides.values.firstOrNull { it.type == C.TRACK_TYPE_VIDEO }
    val options =
        tracks.groups
            .filter { it.type == C.TRACK_TYPE_VIDEO }
            .flatMap { group ->
                (0 until group.length)
                    .filter { group.isTrackSupported(it) }
                    .map { i -> Triple(group, i, group.getTrackFormat(i)) }
            }
            .filter { it.third.height > 0 }
            .groupBy { it.third.height }
            .map { (_, same) -> same.maxBy { it.third.bitrate } }
            .sortedByDescending { it.third.height }
    val playing = player.videoFormat?.height ?: 0
    return listOf(
        TrackChoice(if (overridden == null && playing > 0) "Auto (${playing}p)" else "Auto", overridden == null) {
            player.trackSelectionParameters = params.buildUpon().clearOverridesOfType(C.TRACK_TYPE_VIDEO).build()
        }
    ) +
        options.map { (group, index, format) ->
            TrackChoice(
                "${format.height}p" + if (format.bitrate > 0) " · ${format.bitrate / 1000} kb/s" else "",
                overridden?.mediaTrackGroup == group.mediaTrackGroup && index in overridden.trackIndices,
            ) {
                player.trackSelectionParameters =
                    params.buildUpon().setOverrideForType(TrackSelectionOverride(group.mediaTrackGroup, index)).build()
            }
        }
}

@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
private fun audioChoices(player: ExoPlayer, tracks: Tracks): List<TrackChoice> =
    tracks.groups
        .filter { it.type == C.TRACK_TYPE_AUDIO }
        .flatMap { group -> (0 until group.length).filter { group.isTrackSupported(it) }.map { group to it } }
        .map { (group, index) ->
            val format = group.getTrackFormat(index)
            TrackChoice(
                trackName(format.label, format.language, "Audio ${index + 1}") +
                    if (format.channelCount > 2) " · ${format.channelCount} ch" else "",
                group.isTrackSelected(index),
            ) {
                player.trackSelectionParameters =
                    player.trackSelectionParameters
                        .buildUpon()
                        .setOverrideForType(TrackSelectionOverride(group.mediaTrackGroup, index))
                        .build()
            }
        }

@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
private fun subtitleChoices(player: ExoPlayer, tracks: Tracks): List<TrackChoice> {
    val text = tracks.groups.filter { it.type == C.TRACK_TYPE_TEXT }
    if (text.isEmpty()) return emptyList()
    val off = player.trackSelectionParameters.disabledTrackTypes.contains(C.TRACK_TYPE_TEXT) || text.none { it.isSelected }
    return listOf(
        TrackChoice("Off", off) {
            player.trackSelectionParameters =
                player.trackSelectionParameters.buildUpon().setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true).build()
        }
    ) +
        text.flatMap { group -> (0 until group.length).map { group to it } }.mapIndexed { n, (group, index) ->
            val format = group.getTrackFormat(index)
            TrackChoice(trackName(format.label, format.language, "Subtitles ${n + 1}"), !off && group.isTrackSelected(index)) {
                player.trackSelectionParameters =
                    player.trackSelectionParameters
                        .buildUpon()
                        .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, false)
                        .setOverrideForType(TrackSelectionOverride(group.mediaTrackGroup, index))
                        .build()
            }
        }
}

/** A track's label, else its language's display name, else [fallback]. */
internal fun trackName(label: String?, language: String?, fallback: String): String {
    val named = label?.takeIf { it.isNotBlank() && !it.startsWith("stream_") }
    val lang =
        language
            ?.takeIf { it.isNotBlank() && it != "und" }
            ?.let { Locale.forLanguageTag(it).getDisplayName(Locale.getDefault()).ifBlank { it } }
    return when {
        named != null && lang != null && !named.equals(lang, ignoreCase = true) -> "$named ($lang)"
        named != null -> named
        lang != null -> lang
        else -> fallback
    }
}
