package dev.ytui.android

import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.net.Uri
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.Tracks
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import java.io.File
import java.util.Locale
import kotlinx.coroutines.delay
import org.json.JSONArray

/**
 * Full-screen player for Movies (alpha). Plays the checked source first and fails over to the next
 * one when a source errors mid-film, keeping the position. Quality, audio, subtitle and speed menus
 * read the stream's own tracks; the position is saved for Resume.
 */
@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
class MoviePlayerActivity : ComponentActivity() {
    private var player: ExoPlayer? = null
    private lateinit var key: String
    private var sources: List<MovieSource> = emptyList()
    private var sourceIndex = mutableIntStateOf(0)
    private var offline: String? = null

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
        val exo =
            ExoPlayer.Builder(this)
                .setMediaSourceFactory(DefaultMediaSourceFactory(DefaultDataSource.Factory(this, httpFactory)))
                .setSeekBackIncrementMs(10_000)
                .setSeekForwardIncrementMs(10_000)
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
        setContent {
            MaterialTheme(colorScheme = ComposeHost.colorScheme ?: darkColorScheme(), typography = ComposeHost.typography) {
                CompositionLocalProvider(
                    LocalContentColor provides Color.White,
                    androidx.compose.foundation.LocalIndication provides SoftIndication,
                ) {
                    MoviePlayerScreen(
                        exo,
                        title,
                        sources,
                        offline != null,
                        sourceIndex,
                        onFailover = ::failover,
                        onRetry = { load(0, exo.currentPosition) },
                        onSwitch = { load(it, exo.currentPosition) },
                        onSave = ::savePosition,
                        onClose = ::finish,
                    )
                }
            }
        }
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

@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
@Composable
private fun MoviePlayerScreen(
    player: ExoPlayer,
    title: String,
    sources: List<MovieSource>,
    offline: Boolean,
    sourceIndex: State<Int>,
    onFailover: () -> Boolean,
    onRetry: () -> Unit,
    onSwitch: (Int) -> Unit,
    onSave: () -> Unit,
    onClose: () -> Unit,
) {
    var playing by remember { mutableStateOf(player.isPlaying) }
    var buffering by remember { mutableStateOf(true) }
    var position by remember { mutableLongStateOf(0L) }
    var duration by remember { mutableLongStateOf(0L) }
    var tracks by remember { mutableStateOf(player.currentTracks) }
    var controls by remember { mutableStateOf(true) }
    var menu by remember { mutableStateOf("") }
    var notice by remember { mutableStateOf("") }
    var failure by remember { mutableStateOf("") }
    var speed by remember { mutableFloatStateOf(1f) }
    var scrubbing by remember { mutableStateOf<Float?>(null) }
    var lastTouch by remember { mutableLongStateOf(System.currentTimeMillis()) }

    DisposableEffect(player) {
        val listener =
            object : Player.Listener {
                override fun onIsPlayingChanged(isPlaying: Boolean) {
                    playing = isPlaying
                }

                override fun onPlaybackStateChanged(state: Int) {
                    buffering = state == Player.STATE_BUFFERING
                    if (state == Player.STATE_READY) failure = ""
                    if (state == Player.STATE_ENDED) controls = true
                }

                override fun onTracksChanged(value: Tracks) {
                    tracks = value
                }

                override fun onPlayerError(error: PlaybackException) {
                    val failed = sources.getOrNull(sourceIndex.value)?.label ?: "This copy"
                    val reason = error.errorCodeName.removePrefix("ERROR_CODE_").lowercase(Locale.ROOT).replace('_', ' ')
                    if (onFailover()) {
                        val next = sources.getOrNull(sourceIndex.value)?.label.orEmpty()
                        notice = "$failed stopped ($reason). Switched to $next."
                    } else {
                        failure = "$failed stopped ($reason)."
                        controls = true
                    }
                }
            }
        player.addListener(listener)
        onDispose { player.removeListener(listener) }
    }
    LaunchedEffect(player) {
        var ticks = 0
        while (true) {
            position = player.currentPosition
            duration = player.duration.takeIf { it != C.TIME_UNSET && it > 0 } ?: 0
            if (++ticks % 10 == 0 && player.isPlaying) onSave()
            delay(500)
        }
    }
    // Controls fade after three quiet seconds of playback.
    LaunchedEffect(controls, playing, lastTouch, menu) {
        if (controls && playing && menu.isEmpty()) {
            delay(3_000)
            controls = false
        }
    }
    LaunchedEffect(notice) {
        if (notice.isNotEmpty()) {
            delay(5_000)
            notice = ""
        }
    }
    BackHandler { if (menu.isNotEmpty()) menu = "" else onClose() }

    Box(Modifier.fillMaxSize().background(Color.Black)) {
        AndroidView(
            factory = { context ->
                PlayerView(context).apply {
                    useController = false
                    resizeMode = AspectRatioFrameLayout.RESIZE_MODE_FIT
                    setShutterBackgroundColor(android.graphics.Color.BLACK)
                    this.player = player
                }
            },
            modifier = Modifier.fillMaxSize().testTag("movie-surface"),
        )
        // Tap anywhere to show or hide the controls.
        Box(
            Modifier.fillMaxSize().clickable(remember { MutableInteractionSource() }, null) {
                controls = !controls
                lastTouch = System.currentTimeMillis()
            }
        )
        if (buffering && failure.isEmpty()) MediaSpinner(Modifier.align(Alignment.Center).size(48.dp), Color.White)
        AnimatedVisibility(controls || failure.isNotEmpty(), enter = fadeIn(), exit = fadeOut()) {
            Box(
                Modifier.fillMaxSize()
                    .background(
                        Brush.verticalGradient(
                            0f to Color.Black.copy(alpha = 0.7f),
                            0.3f to Color.Transparent,
                            0.65f to Color.Transparent,
                            1f to Color.Black.copy(alpha = 0.8f),
                        )
                    )
                    .windowInsetsPadding(WindowInsets.safeDrawing)
                    .padding(horizontal = 16.dp, vertical = 8.dp)
            ) {
                Row(Modifier.fillMaxWidth().align(Alignment.TopStart), verticalAlignment = Alignment.CenterVertically) {
                    RoundIconButton(Glyph.Back, "Close player", tint = Color.White, action = onClose)
                    Column(Modifier.weight(1f).padding(start = 8.dp)) {
                        Text(
                            title,
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.SemiBold,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            color = Color.White,
                        )
                        Text(
                            if (offline) "Downloaded copy"
                            else sources.getOrNull(sourceIndex.value)?.label.orEmpty(),
                            style = MaterialTheme.typography.bodySmall,
                            color = Color.White.copy(alpha = 0.7f),
                            maxLines = 1,
                        )
                    }
                    RoundIconButton(Glyph.Settings, "Quality", tint = Color.White) { menu = "quality" }
                    RoundIconButton(Glyph.Volume, "Audio", tint = Color.White) { menu = "audio" }
                    RoundIconButton(Glyph.Captions, "Subtitles", tint = Color.White) { menu = "subtitles" }
                    RoundIconButton(Glyph.Speed, "Speed", tint = Color.White) { menu = "speed" }
                    if (!offline && sources.size > 1)
                        RoundIconButton(Glyph.Link, "Source", tint = Color.White) { menu = "source" }
                }
                if (failure.isNotEmpty())
                    Column(
                        Modifier.align(Alignment.Center)
                            .widthIn(max = 420.dp)
                            .clip(MediaTokens.dialog)
                            .background(Color.Black.copy(alpha = 0.75f))
                            .padding(20.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
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
                else
                    Row(
                        Modifier.align(Alignment.Center),
                        horizontalArrangement = Arrangement.spacedBy(32.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RoundIconButton(Glyph.Previous, "Back 10 seconds", size = 56.dp, tint = Color.White) {
                            player.seekBack()
                            lastTouch = System.currentTimeMillis()
                        }
                        RoundIconButton(
                            if (playing) Glyph.Pause else Glyph.Play,
                            if (playing) "Pause" else "Play",
                            Modifier.testTag("movie-play"),
                            size = 72.dp,
                            iconSize = 36.dp,
                            container = Color.White.copy(alpha = 0.16f),
                            tint = Color.White,
                        ) {
                            if (player.playbackState == Player.STATE_ENDED) player.seekTo(0)
                            if (player.isPlaying) player.pause() else player.play()
                            lastTouch = System.currentTimeMillis()
                        }
                        RoundIconButton(Glyph.Next, "Forward 10 seconds", size = 56.dp, tint = Color.White) {
                            player.seekForward()
                            lastTouch = System.currentTimeMillis()
                        }
                    }
                Column(Modifier.fillMaxWidth().align(Alignment.BottomCenter)) {
                    val shown = scrubbing ?: if (duration > 0) position.toFloat() / duration else 0f
                    Slider(
                        shown.coerceIn(0f, 1f),
                        onValueChange = {
                            scrubbing = it
                            lastTouch = System.currentTimeMillis()
                        },
                        onValueChangeFinished = {
                            scrubbing?.let { player.seekTo((it * duration).toLong()) }
                            scrubbing = null
                        },
                        enabled = duration > 0,
                        colors =
                            SliderDefaults.colors(
                                thumbColor = Color.White,
                                activeTrackColor = MaterialTheme.colorScheme.primary,
                                inactiveTrackColor = Color.White.copy(alpha = 0.3f),
                            ),
                        modifier = Modifier.testTag("movie-seek"),
                    )
                    Row(Modifier.fillMaxWidth()) {
                        Text(
                            time(((scrubbing?.times(duration))?.toLong() ?: position) / 1000f),
                            style = MaterialTheme.typography.labelMedium,
                            color = Color.White,
                        )
                        Spacer(Modifier.weight(1f))
                        if (speed != 1f)
                            Text(
                                "${speedLabel(speed)} · ",
                                style = MaterialTheme.typography.labelMedium,
                                color = Color.White.copy(alpha = 0.7f),
                            )
                        Text(time(duration / 1000f), style = MaterialTheme.typography.labelMedium, color = Color.White)
                    }
                }
            }
        }
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
        if (menu.isNotEmpty()) {
            val choices =
                when (menu) {
                    "quality" -> qualityChoices(player, tracks)
                    "audio" -> audioChoices(player, tracks)
                    "subtitles" -> subtitleChoices(player, tracks)
                    "speed" ->
                        SPEEDS.map { s ->
                            TrackChoice(speedLabel(s), s == speed) {
                                speed = s
                                player.setPlaybackSpeed(s)
                            }
                        }
                    else ->
                        sources.mapIndexed { i, s ->
                            TrackChoice(s.label, i == sourceIndex.value) {
                                if (i != sourceIndex.value) {
                                    failure = ""
                                    onSwitch(i)
                                }
                            }
                        }
                }
            TrackMenu(
                when (menu) {
                    "quality" -> "Quality"
                    "audio" -> "Audio"
                    "subtitles" -> "Subtitles"
                    "speed" -> "Playback speed"
                    else -> "Source"
                },
                choices,
                onDismiss = { menu = "" },
            )
        }
    }
}

private fun speedLabel(speed: Float): String =
    if (speed == 1f) "Normal" else String.format(Locale.ROOT, "%sx", speed.toString().removeSuffix(".0"))

@Composable
private fun TrackMenu(title: String, choices: List<TrackChoice>, onDismiss: () -> Unit) {
    Box(
        Modifier.fillMaxSize()
            .background(Color.Black.copy(alpha = 0.5f))
            .clickable(remember { MutableInteractionSource() }, null, onClick = onDismiss),
        contentAlignment = Alignment.CenterEnd,
    ) {
        Column(
            Modifier.fillMaxHeight()
                .widthIn(max = 360.dp)
                .fillMaxWidth(0.45f)
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                .clickable(remember { MutableInteractionSource() }, null) {}
                .windowInsetsPadding(WindowInsets.safeDrawing)
                .verticalScroll(rememberScrollState())
                .padding(12.dp)
                .testTag("movie-menu"),
        ) {
            CompositionLocalProvider(LocalContentColor provides MaterialTheme.colorScheme.onSurface) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(title, Modifier.weight(1f).padding(start = 8.dp), style = MaterialTheme.typography.titleMedium)
                    SheetClose(onDismiss)
                }
                if (choices.isEmpty())
                    Text(
                        "This stream offers no choices here.",
                        Modifier.padding(8.dp),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                for (choice in choices)
                    SheetAction(
                        text = { Text(choice.label) },
                        onClick = {
                            choice.apply()
                            onDismiss()
                        },
                        trailing = { ChoiceMark(choice.selected) },
                    )
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
