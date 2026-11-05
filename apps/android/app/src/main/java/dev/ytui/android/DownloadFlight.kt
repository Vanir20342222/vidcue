package dev.ytui.android

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay

/**
 * A queued download's cover flies from where it was saved into the Downloads tab, instead of a
 * confirmation glyph popping up at the bottom of the screen. Positions are root coordinates.
 *
 * When the navigation is covered (the full-screen player or a panel is open) or not laid out
 * (the keyboard hides the phone dock), the cover would otherwise fly towards a tab the user
 * cannot see. It lands on a temporary Downloads shortcut instead, which stays for
 * [SHORTCUT_MILLIS] after the landing so it can be tapped, then fades away.
 */
internal object DownloadFlight {
    /** How long the Downloads shortcut stays after a cover lands on it. */
    const val SHORTCUT_MILLIS = 3_000L

    data class Flight(val id: Long, val media: MediaCard, val from: Offset?, val toShortcut: Boolean)

    /** Centre of the Downloads destination in the navigation while it is laid out, else null. */
    var target by mutableStateOf<Offset?>(null)
    var flight by mutableStateOf<Flight?>(null)
        private set

    /**
     * Whether something covers the navigation right now. The host keeps this current; it is
     * read only when a flight starts, so it needs no snapshot state.
     */
    var navigationHidden = false

    /** Id of the flight whose Downloads shortcut is showing, or null. */
    var shortcut by mutableStateOf<Long?>(null)
        private set

    /** Increments when a cover lands in the navigation, so the Downloads icon can bounce. */
    var landed by mutableLongStateOf(0L)
        private set

    /** Increments when a cover lands on the shortcut, so the shortcut can bounce instead. */
    var shortcutLanded by mutableLongStateOf(0L)
        private set

    private var ids = 0L

    /** True when the Downloads tab cannot be seen, so a flight needs the shortcut. */
    fun needsShortcut(navigationHidden: Boolean, target: Offset?): Boolean =
        navigationHidden || target == null

    fun launch(media: MediaCard, from: Offset?) {
        val id = ++ids
        val toShortcut = needsShortcut(navigationHidden, target)
        flight = Flight(id, media, from, toShortcut)
        if (toShortcut) shortcut = id
    }

    fun land(id: Long) {
        val current = flight ?: return
        if (current.id != id) return
        flight = null
        if (current.toShortcut) shortcutLanded++ else landed++
    }

    /** Hides the shortcut for [id], unless a newer download has taken it over. */
    fun dismissShortcut(id: Long) {
        if (shortcut == id) shortcut = null
    }

    /** Clears everything; for tests. */
    internal fun reset() {
        target = null
        flight = null
        navigationHidden = false
        shortcut = null
        landed = 0L
        shortcutLanded = 0L
    }
}

/** Records the centre of this element, in root coordinates, into [into]. */
internal fun Modifier.trackCentre(into: (Offset) -> Unit): Modifier =
    onGloballyPositioned { into(it.boundsInRoot().center) }

/**
 * Draws the flying cover and the temporary Downloads shortcut over everything. Place once,
 * filling the root. [navigationHidden] says whether the Downloads tab is covered;
 * [openDownloads] runs when the shortcut is tapped.
 */
@Composable
internal fun DownloadFlightHost(
    thumbnails: Boolean,
    navigationHidden: Boolean,
    downloads: kotlinx.coroutines.flow.StateFlow<List<org.json.JSONObject>>,
    openDownloads: () -> Unit,
) {
    SideEffect { DownloadFlight.navigationHidden = navigationHidden }
    val flight = DownloadFlight.flight
    val shortcut = DownloadFlight.shortcut
    val motion = LocalMotion.current
    // Root-coordinate centre of the shortcut, so a cover can aim at it.
    var shortcutCentre by remember { mutableStateOf<Offset?>(null) }
    Box(Modifier.fillMaxSize()) {
        // What the shortcut shows: the item whose flight claimed it.
        val shortcutMedia = remember { arrayOfNulls<MediaCard>(1) }
        if (flight != null && flight.toShortcut && flight.id == shortcut) shortcutMedia[0] = flight.media
        DownloadsShortcut(
            shortcut,
            shortcutMedia[0].takeIf { shortcut != null },
            downloads,
            thumbnails,
            { shortcutCentre = it },
        ) {
            shortcut?.let(DownloadFlight::dismissShortcut)
            openDownloads()
        }
        if (flight != null)
            key(flight.id) {
                val progress = remember { Animatable(0f) }
                LaunchedEffect(Unit) {
                    if (motion.animate)
                        progress.animateTo(1f, tween(motion.millis(720), easing = FastOutSlowInEasing))
                    DownloadFlight.land(flight.id)
                }
                if (motion.animate)
                    FlyingCover(
                        flight,
                        { progress.value },
                        thumbnails,
                        if (flight.toShortcut) shortcutCentre else DownloadFlight.target,
                    )
            }
    }
}

/**
 * The download that was just saved, with its real progress, in the bottom-right corner. It
 * appears as the cover sets off, catches it, follows the actual transfer (queued, percent,
 * finished), stays [DownloadFlight.SHORTCUT_MILLIS] after the landing, then slips away. Tapping
 * it opens Downloads.
 */
@Composable
private fun DownloadsShortcut(
    id: Long?,
    media: MediaCard?,
    downloads: kotlinx.coroutines.flow.StateFlow<List<org.json.JSONObject>>,
    thumbnails: Boolean,
    onCentre: (Offset) -> Unit,
    open: () -> Unit,
) {
    val motion = LocalMotion.current
    val colors = MaterialTheme.colorScheme
    if (id != null)
        LaunchedEffect(id) {
            // Wait for the cover to land (immediate with reduced motion), then give the user
            // time to tap. A newer download restarts this with its own id.
            while (DownloadFlight.flight?.let { it.id == id } == true) delay(50)
            delay(DownloadFlight.SHORTCUT_MILLIS)
            DownloadFlight.dismissShortcut(id)
        }
    val bounce = remember { Animatable(1f) }
    LaunchedEffect(DownloadFlight.shortcutLanded) {
        if (DownloadFlight.shortcutLanded > 0 && motion.animate) {
            bounce.snapTo(1.08f)
            bounce.animateTo(1f, spring(0.45f, Spring.StiffnessMediumLow))
        }
    }
    // Keep showing the last item while the card slides away (a holder, not state: a new
    // item always arrives through a recomposition of this function).
    val last = remember { arrayOfNulls<MediaCard>(1) }
    if (media != null) last[0] = media
    val shown = last[0]
    Box(
        Modifier.fillMaxSize()
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .padding(end = 16.dp, bottom = 24.dp),
        contentAlignment = Alignment.BottomEnd,
    ) {
        AnimatedVisibility(
            visible = id != null,
            enter =
                if (motion.animate)
                    fadeIn(motion.fade(160)) +
                        scaleIn(motion.spring(), 0.6f, TransformOrigin(1f, 1f)) +
                        slideInHorizontally(motion.enter(320)) { it / 3 }
                else EnterTransition.None,
            exit =
                if (motion.animate)
                    fadeOut(motion.fade(320)) +
                        slideOutHorizontally(motion.exit(360)) { it / 2 } +
                        scaleOut(motion.exit(360), 0.9f, TransformOrigin(1f, 1f))
                else ExitTransition.None,
        ) {
            val list by downloads.collectAsState()
            val item = shown?.let { card -> downloadFor(list, card) }
            val state = item?.optString("state").orEmpty()
            val tone = item?.let(::transferRowTone)
            val progress =
                item?.optDouble("progress")?.toFloat()?.takeIf { it.isFinite() }?.coerceIn(0f, 1f)
            val complete = state == "Complete"
            val glide by
                androidx.compose.animation.core.animateFloatAsState(
                    if (complete) 1f else progress ?: 0f,
                    if (motion.animate) tween(520, easing = androidx.compose.animation.core.LinearEasing)
                    else androidx.compose.animation.core.snap(),
                    label = "shortcut-progress",
                )
            val status =
                when {
                    item == null -> "Queued"
                    complete -> "Saved offline"
                    tone == TransferTone.Failed -> state
                    progress != null && progress > 0f -> "${state.ifBlank { "Downloading" }} · ${percentLabel(progress)}"
                    else -> state.ifBlank { "Queued" }
                }
            val shape = RoundedCornerShape(22.dp)
            Column(
                Modifier.graphicsLayer {
                        scaleX = bounce.value
                        scaleY = bounce.value
                        transformOrigin = TransformOrigin(1f, 1f)
                    }
                    .widthIn(min = 220.dp, max = 300.dp)
                    .shadow(18.dp, shape, ambientColor = Color.Black, spotColor = Color.Black)
                    .clip(shape)
                    .background(colors.surfaceContainerHigh)
                    .border(1.dp, colors.outlineVariant.copy(alpha = 0.6f), shape)
                    .clickable(role = Role.Button, onClick = open)
                    .padding(start = 10.dp, end = 14.dp, top = 10.dp, bottom = 12.dp)
                    .testTag("download-shortcut")
                    .semantics {
                        contentDescription = "Download ${status.lowercase()}. Open Downloads"
                        liveRegion = LiveRegionMode.Polite
                    },
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // The flying cover lands exactly on this tile.
                    Box(Modifier.size(40.dp).trackCentre(onCentre)) {
                        val card = shown
                        if (card != null)
                            Artwork(card, Modifier.fillMaxSize(), thumbnails, shape = RoundedCornerShape(10.dp))
                        else GlyphTile(Glyph.Download, Modifier.fillMaxSize().clip(RoundedCornerShape(10.dp)))
                    }
                    Column(Modifier.weight(1f, fill = false), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text(
                            shown?.title?.ifBlank { null } ?: "Download",
                            style = MaterialTheme.typography.labelLarge,
                            fontWeight = FontWeight.SemiBold,
                            color = colors.onSurface,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            status,
                            style = MaterialTheme.typography.labelMedium,
                            color =
                                if (tone == TransferTone.Failed) colors.error
                                else colors.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    MediaIcon(
                        if (complete) Glyph.Check else Glyph.ArrowRight,
                        Modifier.size(18.dp),
                        colors.onSurfaceVariant,
                    )
                }
                // The real transfer: indeterminate until the first bytes arrive.
                MediaProgressBar(
                    if (item == null || (progress ?: 0f) <= 0f && !complete) null else glide,
                    Modifier.fillMaxWidth().testTag("download-shortcut-progress"),
                    color = if (tone == TransferTone.Failed) colors.error else colors.primary,
                    thickness = 4.dp,
                )
            }
        }
    }
}

/** The transfer saving [card], newest first, or null until the engine has queued it. */
internal fun downloadFor(downloads: List<org.json.JSONObject>, card: MediaCard): org.json.JSONObject? =
    downloads.lastOrNull { item ->
        val media = item.optJSONObject("media")
        (card.url.isNotBlank() && media?.optString("url") == card.url) ||
            (media == null && card.title.isNotBlank() && item.optString("title") == card.title)
    }

@Composable
private fun FlyingCover(
    flight: DownloadFlight.Flight,
    progress: () -> Float,
    thumbnails: Boolean,
    target: Offset?,
) {
    val density = LocalDensity.current
    var hostSize by remember { mutableStateOf(androidx.compose.ui.geometry.Size.Zero) }
    Box(
        Modifier.fillMaxSize().onGloballyPositioned {
            hostSize = androidx.compose.ui.geometry.Size(it.size.width.toFloat(), it.size.height.toFloat())
        }
    ) {
        if (hostSize != androidx.compose.ui.geometry.Size.Zero) {
            val start = flight.from ?: Offset(hostSize.width / 2, hostSize.height * 0.55f)
            // Until the shortcut has been laid out, aim at where it will appear.
            val end =
                target
                    ?: with(density) {
                        Offset(hostSize.width - 260.dp.toPx(), hostSize.height - 72.dp.toPx())
                    }
            // An arc: up and over, then down into the tab.
            val lift = with(density) { 140.dp.toPx() }
            val control = Offset((start.x + end.x) / 2, minOf(start.y, end.y) - lift)
            val cover = 64.dp
            val half = with(density) { cover.toPx() / 2 }
            Box(
                Modifier.size(cover)
                    .graphicsLayer {
                        val t = progress()
                        val u = 1f - t
                        val x = u * u * start.x + 2 * u * t * control.x + t * t * end.x
                        val y = u * u * start.y + 2 * u * t * control.y + t * t * end.y
                        translationX = x - half
                        translationY = y - half
                        // Shrinks onto the 40dp tile of the shortcut, or into the tab icon.
                        val scale = 1f - (if (flight.toShortcut) 0.375f else 0.7f) * t
                        scaleX = scale
                        scaleY = scale
                        rotationZ = if (flight.toShortcut) -12f * t * (1f - t) * 4f else -12f * t
                        alpha = if (t < 0.85f || flight.toShortcut) 1f else (1f - t) / 0.15f
                    }
                    .shadow(12.dp, RoundedCornerShape(14.dp), ambientColor = Color.Black, spotColor = Color.Black)
                    .testTag("download-flight")
                    .semantics {
                        contentDescription = "Download queued"
                        if (!flight.toShortcut) liveRegion = LiveRegionMode.Polite
                    }
            ) {
                Artwork(flight.media, Modifier.fillMaxSize(), thumbnails, shape = RoundedCornerShape(14.dp))
            }
        }
    }
}
