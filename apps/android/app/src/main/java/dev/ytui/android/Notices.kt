package dev.ytui.android

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay

/** A transient app message with an optional action, shown above the dock. */
internal data class Notice(
    val text: String,
    val glyph: Glyph = Glyph.Info,
    val action: String = "",
    val onAction: () -> Unit = {},
    val error: Boolean = false,
)

/** UI-side notices (undo offers and similar) that are not view-model errors. */
internal object Notices {
    val current = mutableStateOf<Notice?>(null)
    private var serial by mutableLongStateOf(0L)
    val sequence: Long
        get() = serial

    fun show(notice: Notice) {
        serial++
        current.value = notice
    }
}

private val CONFIRMATIONS =
    mapOf(
        "queue" to (Glyph.Queue to "Added to queue"),
        "download" to (Glyph.Download to "Download queued"),
        "saved" to (Glyph.Save to "Saved locally"),
        "unsaved" to (Glyph.Save to "Removed from saved items"),
        "lyrics" to (Glyph.Lyrics to "Lyrics saved locally"),
        "palette" to (Glyph.Palette to "Palette applied"),
        "tips" to (Glyph.Info to "Gesture tips will show again"),
    )

/**
 * Where the app talks back. Errors appear as a card with Dismiss and stay until dismissed or
 * replaced, for up to ten seconds. Completed actions get a wordless check: a glyph disc that pops
 * and leaves, announced to screen readers through its description.
 */
@Composable
internal fun NoticeHost(vm: YtuiViewModel, bottom: Dp, modifier: Modifier = Modifier) {
    val error by vm.actionError.collectAsState()
    val confirmed by vm.confirmed.collectAsState()
    val notice by Notices.current
    val sequence = Notices.sequence
    var check by remember { mutableStateOf<Pair<Glyph, String>?>(null) }
    LaunchedEffect(confirmed.first) {
        if (confirmed.first == 0L) return@LaunchedEffect
        check = CONFIRMATIONS[confirmed.second] ?: return@LaunchedEffect
        delay(1400)
        check = null
    }
    LaunchedEffect(error) {
        if (error == null) return@LaunchedEffect
        delay(10_000)
        if (vm.actionError.value == error) vm.actionError.value = null
    }
    LaunchedEffect(sequence) {
        if (notice == null) return@LaunchedEffect
        delay(6_000)
        Notices.current.value = null
    }
    val motion = LocalMotion.current
    Box(modifier.fillMaxSize().padding(bottom = bottom), contentAlignment = Alignment.BottomCenter) {
        val card = error?.let { Notice(it, Glyph.Info, "Dismiss", { vm.actionError.value = null }, true) } ?: notice
        AnimatedVisibility(
            card != null,
            enter =
                if (motion.animate) fadeIn(motion.fade(180)) + slideInVertically(motion.spring()) { it / 2 }
                else fadeIn(motion.fade(0)),
            exit =
                if (motion.animate) fadeOut(motion.fade(160)) + slideOutVertically(motion.exit(200)) { it / 2 }
                else fadeOut(motion.fade(0)),
        ) {
            // Keep the last card while it animates out.
            var shown by remember { mutableStateOf(card) }
            if (card != null) shown = card
            shown?.let { NoticeCard(it) }
        }
        AnimatedVisibility(
            check != null && card == null,
            enter = if (motion.animate) fadeIn(motion.fade(120)) + scaleIn(motion.spring(), 0.5f) else fadeIn(motion.fade(0)),
            exit = if (motion.animate) fadeOut(motion.fade(220)) + scaleOut(motion.exit(220), 0.8f) else fadeOut(motion.fade(0)),
        ) {
            var shown by remember { mutableStateOf(check) }
            if (check != null) shown = check
            shown?.let { (glyph, text) -> ConfirmationDisc(glyph, text) }
        }
    }
}

@Composable
private fun NoticeCard(notice: Notice) {
    val colors = MaterialTheme.colorScheme
    val ink = colors.onSurface
    CompositionLocalProvider(LocalContentColor provides ink) {
        Row(
            Modifier.padding(horizontal = 16.dp)
                .widthIn(max = 560.dp)
                .fillMaxWidth()
                .shadow(16.dp, MediaTokens.card, ambientColor = Color.Black, spotColor = Color.Black)
                .clip(MediaTokens.card)
                .background(colors.surfaceContainerHigh)
                .border(1.dp, colors.outlineVariant, MediaTokens.card)
                .testTag(if (notice.error) "error-notice" else "notice")
                .semantics { liveRegion = LiveRegionMode.Polite }
                .padding(start = 14.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Box(
                Modifier.size(30.dp)
                    .background(
                        (if (notice.error) colors.error else colors.onSurface).copy(alpha = 0.14f),
                        CircleShape,
                    ),
                contentAlignment = Alignment.Center,
            ) {
                MediaIcon(
                    notice.glyph,
                    Modifier.size(16.dp),
                    if (notice.error) mediaReadableText(colors.error, colors.surfaceContainerHigh) else ink,
                )
            }
            Text(
                notice.text,
                Modifier.weight(1f).padding(vertical = 10.dp),
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
            )
            if (notice.action.isNotBlank())
                QuietAction(notice.action) {
                    notice.onAction()
                    if (!notice.error) Notices.current.value = null
                }
        }
    }
}

@Composable
private fun ConfirmationDisc(glyph: Glyph, text: String) {
    val colors = MaterialTheme.colorScheme
    val motion = LocalMotion.current
    val tick = remember { Animatable(if (motion.animate) 0f else 1f) }
    // Replay the badge draw whenever the message changes, including a second confirmation that
    // arrives while this disc is still visible; otherwise its check would stay frozen.
    LaunchedEffect(text) {
        tick.snapTo(if (motion.animate) 0f else 1f)
        tick.animateTo(1f, motion.enter(360, delay = 120))
    }
    val ink = colors.background
    Box(
        Modifier.size(56.dp)
            .shadow(14.dp, CircleShape, ambientColor = Color.Black, spotColor = Color.Black)
            .background(colors.onBackground, CircleShape)
            .testTag("confirmation")
            .semantics {
                contentDescription = text
                liveRegion = LiveRegionMode.Polite
            },
        contentAlignment = Alignment.Center,
    ) {
        MediaIcon(glyph, Modifier.size(22.dp), ink)
        // A check badge draws itself on in the corner.
        Canvas(Modifier.align(Alignment.TopEnd).offset(x = 2.dp, y = (-2).dp).size(22.dp)) {
            val w = size.width
            // A background-colored ring separates the badge from the light disc it sits on, the
            // same cutout treatment the glyph uses, so it no longer blends into a smudge.
            val ring = 2.dp.toPx()
            drawCircle(colors.background, radius = w / 2f)
            drawCircle(colors.primary, radius = w / 2f - ring)
            val c = mediaOnAccent(colors.primary)
            val a = Offset(w * 0.28f, w * 0.52f)
            val b = Offset(w * 0.44f, w * 0.68f)
            val e = Offset(w * 0.74f, w * 0.36f)
            val p = tick.value
            val first = (p / 0.4f).coerceIn(0f, 1f)
            val second = ((p - 0.4f) / 0.6f).coerceIn(0f, 1f)
            val stroke = 2.2.dp.toPx()
            drawLine(c, a, a + (b - a) * first, stroke, StrokeCap.Round)
            if (second > 0f) drawLine(c, b, b + (e - b) * second, stroke, StrokeCap.Round)
        }
    }
}

/** Remaining sleep-timer milliseconds from the published deadline, ticking once a second. */
@Composable
internal fun rememberSleepLeft(player: PlayerState): Long {
    val deadline = player.value.optLong("sleep_deadline")
    var now by remember { mutableLongStateOf(android.os.SystemClock.elapsedRealtime()) }
    LaunchedEffect(deadline) {
        while (deadline > 0) {
            now = android.os.SystemClock.elapsedRealtime()
            delay(1000)
        }
    }
    return if (deadline <= 0) 0 else (deadline - now).coerceAtLeast(0)
}

internal fun sleepLabel(ms: Long): String {
    val minutes = (ms + 59_999) / 60_000
    return if (minutes >= 60) "${minutes / 60}:%02d".format(minutes % 60) else "$minutes min"
}

/** Small moon-and-countdown chip shown wherever playback is controlled while a timer runs. */
@Composable
internal fun SleepBadge(player: PlayerState, modifier: Modifier = Modifier, onClick: (() -> Unit)? = null) {
    val left = rememberSleepLeft(player)
    AnimatedVisibility(left > 0, modifier, enter = fadeIn() + scaleIn(initialScale = 0.7f), exit = fadeOut() + scaleOut(targetScale = 0.7f)) {
        val ink = LocalContentColor.current
        Row(
            Modifier.clip(MediaTokens.pill)
                .background(ink.copy(alpha = 0.12f))
                .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
                .testTag("sleep-badge")
                .semantics { contentDescription = "Sleep timer, ${sleepLabel(left)} left" }
                .padding(horizontal = 8.dp, vertical = 3.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            MediaIcon(Glyph.Sleep, Modifier.size(12.dp), ink, filled = true)
            Text(
                sleepLabel(left),
                style =
                    MaterialTheme.typography.labelSmall.merge(
                        androidx.compose.ui.text.TextStyle(fontFeatureSettings = "tnum")
                    ),
                color = ink,
            )
        }
    }
}

/** Sleep timer choices shared by the main sheet and the fullscreen options panel. */
@Composable
internal fun SleepOptions(vm: YtuiViewModel, done: () -> Unit) {
    val player by vm.player.collectAsState()
    val left = rememberSleepLeft(player)
    val colors = MaterialTheme.colorScheme
    if (left > 0)
        Column(
            Modifier.fillMaxWidth()
                .clip(MediaTokens.card)
                .background(colors.surfaceContainerHigh)
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                MediaIcon(Glyph.Sleep, Modifier.size(22.dp), colors.primary, filled = true)
                Column(Modifier.weight(1f)) {
                    Text(
                        sleepLabel(left),
                        style = MaterialTheme.typography.headlineSmall.merge(
                            androidx.compose.ui.text.TextStyle(fontFeatureSettings = "tnum")
                        ),
                        modifier = Modifier.testTag("sleep-remaining"),
                    )
                    Text("until playback pauses", style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
                }
                SecondaryAction("+15 min") {
                    vm.command("sleep", "seconds" to (left / 1000 + 15 * 60))
                }
            }
        }
    for (minutes in listOf(15, 30, 60, 120)) {
        SheetAction(
            text = { Text("$minutes minutes") },
            leadingIcon = { MediaIcon(Glyph.Sleep) },
            onClick = {
                vm.command("sleep", "seconds" to minutes * 60)
                done()
            },
        )
    }
    val remaining = (player.duration - player.position).toLong()
    if (player.media != null && player.duration > 0 && remaining > 1)
        SheetAction(
            text = {
                Column {
                    Text("End of this track")
                    Text(
                        "In ${time(remaining.toFloat())}",
                        style = MaterialTheme.typography.bodySmall,
                        color = colors.onSurfaceVariant,
                    )
                }
            },
            leadingIcon = { MediaIcon(Glyph.Stop) },
            onClick = {
                vm.command("sleep", "seconds" to remaining)
                done()
            },
        )
    SheetAction(
        text = { Text("Cancel timer") },
        leadingIcon = { MediaIcon(Glyph.Close) },
        onClick = {
            vm.command("sleep_cancel")
            done()
        },
    )
}
