package dev.ytui.android

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import org.json.JSONObject

/** The button a transfer offers, matching what `download_toggle` actually does for it. */
internal class TransferAction(
    val glyph: Glyph,
    /** Short visible label. */
    val label: String,
    /** Spoken label, kept identical to the earlier icon-only button. */
    val description: String,
    /** True when the action stops a running transfer, so "Pause all" may include it. */
    val pauses: Boolean,
)

internal fun transferAction(item: JSONObject): TransferAction {
    val state = item.optString("state")
    return when {
        item.optString("id").startsWith("job:") ->
            TransferAction(Glyph.Repeat, "Restart", "Restart preparation", false)
        state.contains("failed", ignoreCase = true) ->
            TransferAction(Glyph.Repeat, "Retry", "Retry download", false)
        state == "Paused" -> TransferAction(Glyph.Play, "Resume", "Resume download", false)
        else -> TransferAction(Glyph.Pause, "Pause", "Pause download", true)
    }
}

/** Playlist preparation and exports have a status line but no byte counts to show. */
private fun isByteTransfer(id: String) = !id.startsWith("export:") && !id.startsWith("job:")

internal fun transferRowTone(item: JSONObject): TransferTone {
    val state = item.optString("state")
    return if (isByteTransfer(item.optString("id"))) transferTone(state)
    else if (
        state.startsWith("Preparing", ignoreCase = true) ||
            state.startsWith("Exporting", ignoreCase = true)
    )
        TransferTone.Active
    else TransferTone.Waiting
}

/** Longest gap between byte updates before the last measured speed stops being believable. */
private const val STALL_MS = 5_000L

/**
 * Smoothed bytes per second from how many bytes arrived between snapshots; 0 when unknown,
 * when nothing arrived for a while, or when the transfer is not running.
 */
@Composable
private fun rememberTransferSpeed(id: String, bytes: Long, running: Boolean): Double {
    var last by remember(id) { mutableStateOf(bytes to android.os.SystemClock.elapsedRealtime()) }
    var speed by remember(id) { mutableDoubleStateOf(0.0) }
    LaunchedEffect(bytes) {
        val now = android.os.SystemClock.elapsedRealtime()
        val (before, at) = last
        if (bytes > before && now > at) {
            val sample = (bytes - before) * 1000.0 / (now - at)
            speed = if (speed == 0.0) sample else speed * 0.7 + sample * 0.3
        }
        last = bytes to now
        // Cancelled and restarted by the next byte update; if none comes, drop the stale number.
        delay(STALL_MS)
        speed = 0.0
    }
    return if (running) speed else 0.0
}

/** What the Transfers list holds, plus "Pause all" when more than one transfer is running. */
@Composable
internal fun TransfersHeader(summary: String, canPauseAll: Boolean, onPauseAll: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            summary,
            Modifier.weight(1f).testTag("transfers-summary"),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (canPauseAll) QuietAction("Pause all", Modifier.testTag("transfers-pause-all"), action = onPauseAll)
    }
}

/**
 * One transfer as a card. The title, status and every number get their own full-width line,
 * and the actions are labelled buttons, so nothing is squeezed out by controls or truncated.
 */
@Composable
internal fun TransferCard(
    item: JSONObject,
    thumbnails: Boolean,
    onToggle: () -> Unit,
    onRemove: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = MaterialTheme.colorScheme
    val id = item.optString("id")
    val title = item.optString("title")
    val state = item.optString("state")
    val byteTransfer = isByteTransfer(id)
    val tone = transferRowTone(item)
    val bytes = item.optLong("bytes")
    val total = item.optLong("total")
    val progress =
        item.optDouble("progress").toFloat().takeIf { it.isFinite() }?.coerceIn(0f, 1f) ?: 0f
    val speed = rememberTransferSpeed(id, bytes, byteTransfer && tone == TransferTone.Active)
    val media = item.optJSONObject("media")?.let { MediaCard(it) }
    val square = item.optBoolean("audio")
    val action = transferAction(item)
    val surface = colors.surfaceContainer
    val ink: Color =
        when (tone) {
            TransferTone.Failed -> mediaReadableText(colors.error, surface)
            TransferTone.Active -> mediaReadableText(colors.primary, surface)
            else -> colors.onSurfaceVariant
        }
    Column(
        modifier
            .padding(vertical = 4.dp)
            .fillMaxWidth()
            .testTag("transfer-$id")
            .clip(MediaTokens.card)
            .background(surface)
            .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            val art =
                Modifier.width(if (square) 72.dp else 120.dp)
                    .aspectRatio(if (square) 1f else 16f / 9f)
                    .clip(MediaTokens.small)
            if (media != null) Artwork(media, art, thumbnails)
            else
                GlyphTile(
                    when {
                        id.startsWith("job:") -> Glyph.Queue
                        id.startsWith("export:") -> Glyph.ArrowRight
                        else -> Glyph.Download
                    },
                    art,
                )
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(
                    title,
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 3,
                    overflow = TextOverflow.Ellipsis,
                )
                media
                    ?.creator
                    ?.takeIf { it.isNotBlank() }
                    ?.let {
                        Text(
                            it,
                            style = MaterialTheme.typography.bodySmall,
                            color = colors.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Box(Modifier.size(8.dp).background(ink, CircleShape))
                    Text(
                        state,
                        style = MaterialTheme.typography.labelLarge,
                        color = ink,
                        maxLines = 3,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
        if (byteTransfer)
            MediaProgressBar(
                progress,
                Modifier.semantics {
                    contentDescription = "Download progress"
                    progressBarRangeInfo = ProgressBarRangeInfo(progress, 0f..1f)
                },
                color =
                    when (tone) {
                        TransferTone.Active -> colors.primary
                        TransferTone.Failed -> colors.error
                        else -> colors.onSurfaceVariant
                    },
                thickness = 6.dp,
            )
        else if (tone == TransferTone.Active) MediaProgressBar(null, thickness = 6.dp)
        // Before anything has arrived a queued transfer has no numbers worth a line.
        if (byteTransfer && (progress > 0f || bytes > 0 || tone == TransferTone.Active))
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(16.dp),
                verticalAlignment = Alignment.Top,
            ) {
                Column(Modifier.weight(1f)) {
                    Text(percentLabel(progress), style = MaterialTheme.typography.titleLarge)
                    downloadedLabel(bytes, total)?.let {
                        Text(
                            it,
                            style = MaterialTheme.typography.bodyMedium,
                            color = colors.onSurfaceVariant,
                        )
                    }
                }
                if (tone == TransferTone.Active)
                    Column(horizontalAlignment = Alignment.End) {
                        val rate = speedLabel(speed)
                        Text(
                            rate ?: "Measuring speed…",
                            style =
                                if (rate != null) MaterialTheme.typography.titleSmall
                                else MaterialTheme.typography.bodyMedium,
                            color = if (rate != null) colors.onSurface else colors.onSurfaceVariant,
                            textAlign = TextAlign.End,
                        )
                        secondsLeft(bytes, total, speed)?.let {
                            Text(
                                timeLeftLabel(it),
                                style = MaterialTheme.typography.bodyMedium,
                                color = colors.onSurfaceVariant,
                                textAlign = TextAlign.End,
                            )
                        }
                    }
            }
        FlowRow(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            if (!id.startsWith("export:"))
                Pill(
                    action.label,
                    Modifier.testTag("transfer-toggle-$id").semantics {
                        contentDescription = action.description
                    },
                    style = PillStyle.Tonal,
                    glyph = action.glyph,
                    onClick = onToggle,
                )
            QuietAction(
                "Remove",
                Modifier.testTag("transfer-remove-$id").semantics {
                    contentDescription = "Remove $title"
                },
                action = onRemove,
            )
        }
    }
}
