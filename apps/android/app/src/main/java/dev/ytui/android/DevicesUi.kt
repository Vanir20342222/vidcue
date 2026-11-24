package dev.ytui.android

import androidx.compose.foundation.layout.*
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONArray
import org.json.JSONObject

/** Rows for the player's options sheet: this account's devices and the Jam. */
@Composable
internal fun SyncPlayerEntries(vm: YtuiViewModel, done: () -> Unit) {
    val status by vm.sync.status.collectAsState()
    if (!status.optBoolean("signed_in")) return
    val features = status.optJSONArray("features").strings()
    if ("realtime" in features)
        ValueRow("Devices", Modifier.testTag("player-devices"), glyph = Glyph.Devices) {
            done()
            vm.sync.open(SyncSheet.Devices)
        }
    if ("jam" in features)
        ValueRow("Jam", Modifier.testTag("player-jam"), glyph = Glyph.Jam) {
            done()
            vm.sync.open(SyncSheet.Jam)
        }
}

/** Other devices of this account with what they play; tap one to hand off or control it. */
@Composable
internal fun DevicesSheet(vm: YtuiViewModel, close: () -> Unit) {
    val devices by vm.sync.presence.collectAsState()
    var selected by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) { runCatching { vm.sync.refreshPresence() } }
    val device = devices.firstOrNull { it.text("device_id") == selected }
    if (device != null) {
        RemotePad(vm, device) { selected = null }
        return
    }
    Panel("Devices", close) {
        if (devices.isEmpty()) EmptyMediaState(Glyph.Devices, "No other devices online", "")
        for (entry in devices) {
            val state = entry.optJSONObject("state") ?: JSONObject()
            val media = state.optJSONObject("media")
            MediaListRow(
                entry.text("device_name").ifBlank { "Device" },
                media?.text("title").orEmpty(),
                Modifier.testTag("device-${entry.text("device_id")}"),
                artwork = { modifier ->
                    if (media != null) Artwork(MediaCard(media), modifier, thumbnailsOn(vm)) else GlyphTile(Glyph.Devices, modifier)
                },
                square = true,
                onClick = { selected = entry.text("device_id") },
                trailing = {
                    if (state.optBoolean("playing")) MediaIcon(Glyph.Play, Modifier.size(18.dp), SyncColors.online)
                },
            )
        }
    }
}

/** What a remote device is doing now, estimated from its last presence update. */
internal fun remotePositionMs(state: JSONObject, serverAt: Long, now: Long): Long {
    val position = state.optLong("position_ms").coerceAtLeast(0)
    if (!state.optBoolean("playing") || serverAt <= 0) return position
    val rate = state.optDouble("rate", 1.0).takeIf { it.isFinite() && it > 0 } ?: 1.0
    val moved = position + ((now - serverAt).coerceAtLeast(0) * rate).toLong()
    val duration = state.optLong("duration_ms")
    return if (duration > 0) moved.coerceAtMost(duration) else moved
}

@Composable
private fun RemotePad(vm: YtuiViewModel, device: JSONObject, back: () -> Unit) {
    val runner = rememberSyncRunner()
    val player by vm.player.collectAsState()
    val id = device.text("device_id")
    val state = device.optJSONObject("state") ?: JSONObject()
    val media = state.optJSONObject("media")?.let(::MediaCard)
    val can = state.optJSONArray("can").strings()
    val controllable = "remote" in can
    fun send(command: String, vararg extra: Pair<String, Any?>) =
        runner.run { vm.sync.obj("remote_send", "to_device" to id, "command" to command, *extra) }
    Panel(device.text("device_name").ifBlank { "Device" }, back) {
        if (media != null) {
            Artwork(media, Modifier.fillMaxWidth().aspectRatio(if (media.audio) 1f else 16f / 9f), thumbnailsOn(vm), MediaTokens.card)
            Text(media.title, style = MaterialTheme.typography.titleLarge, maxLines = 2, overflow = TextOverflow.Ellipsis)
        } else GlyphBadge(Glyph.Devices, 88.dp)
        if (controllable) {
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterHorizontally),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                RoundIconButton(Glyph.Previous, "Previous", Modifier.testTag("remote-prev"), enabled = media != null) { send("prev") }
                RoundIconButton(Glyph.Back, "Back 10 seconds", Modifier.testTag("remote-back"), enabled = media != null) {
                    val at = remotePositionMs(state, device.optLong("server_at"), System.currentTimeMillis())
                    send("seek", "position_ms" to (at - 10_000).coerceAtLeast(0))
                }
                RoundIconButton(
                    if (state.optBoolean("playing")) Glyph.Pause else Glyph.Play,
                    if (state.optBoolean("playing")) "Pause" else "Play",
                    Modifier.testTag("remote-toggle"),
                    size = 64.dp,
                    iconSize = 30.dp,
                    container = MaterialTheme.colorScheme.onBackground,
                    tint = MaterialTheme.colorScheme.background,
                    enabled = media != null,
                ) {
                    send("toggle")
                }
                RoundIconButton(Glyph.Forward, "Forward 10 seconds", Modifier.testTag("remote-forward"), enabled = media != null) {
                    val at = remotePositionMs(state, device.optLong("server_at"), System.currentTimeMillis())
                    send("seek", "position_ms" to at + 10_000)
                }
                RoundIconButton(Glyph.Next, "Next", Modifier.testTag("remote-next"), enabled = media != null) { send("next") }
            }
            var volume by remember(id) { mutableFloatStateOf(state.optInt("volume", 80).coerceIn(0, 100).toFloat()) }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                MediaIcon(Glyph.Volume, Modifier.size(20.dp), MaterialTheme.colorScheme.onSurfaceVariant)
                Slider(
                    volume,
                    { volume = it },
                    Modifier.weight(1f).testTag("remote-volume").semantics { contentDescription = "Volume" },
                    valueRange = 0f..100f,
                    onValueChangeFinished = { send("volume", "volume" to volume.toInt()) },
                )
            }
        }
        val local = player.media
        if ("handoff" in can && local != null && isOnlineMedia(local))
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                Pill("Hand off", Modifier.testTag("remote-handoff"), PillStyle.Inverse, enabled = !runner.busy, glyph = Glyph.Devices) {
                    runner.run {
                        val queue = vm.sync.list("queue").filter(::isOnlineMedia).take(25)
                        vm.sync.obj(
                            "handoff_send",
                            "to_device" to id,
                            "media" to local,
                            "position_ms" to (player.position * 1000).toLong(),
                            "playing" to player.playing,
                            "queue" to JSONArray(queue),
                        )
                        // The other device takes over; this one stops making sound.
                        if (vm.player.value.playing) vm.command("pause")
                        Notices.show(Notice(device.text("device_name").take(40).ifBlank { "Sent" }, Glyph.Devices))
                        back()
                    }
                }
            }
        ErrorLine(runner.error)
    }
}
