package dev.ytui.android

import kotlinx.coroutines.delay
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONObject

/** One call into the view model's player commands (`vm.command(op, key to value)`). */
internal data class PlayerOp(val op: String, val key: String? = null, val value: Double? = null)

/**
 * Executes what another device of the same account asked for: a handoff (start this media here)
 * or a remote command. Everything goes through the view model's existing player calls, so a
 * remote press behaves exactly like a local one.
 */
internal object RemoteControl {
    /** Maps a `remote_command` to player calls. [playing] is the local player's state. */
    fun ops(command: String, positionMs: Long?, volume: Int?, playing: Boolean, hasMedia: Boolean): List<PlayerOp> =
        when (command) {
            "play" -> if (hasMedia && !playing) listOf(PlayerOp("pause")) else emptyList()
            "pause" -> if (playing) listOf(PlayerOp("pause")) else emptyList()
            "toggle" -> if (hasMedia) listOf(PlayerOp("pause")) else emptyList()
            "seek" ->
                if (hasMedia && positionMs != null && positionMs >= 0)
                    listOf(PlayerOp("seek", "seconds", positionMs / 1000.0))
                else emptyList()
            "next" -> if (hasMedia) listOf(PlayerOp("next")) else emptyList()
            "prev" -> if (hasMedia) listOf(PlayerOp("previous")) else emptyList()
            "volume" ->
                if (volume != null && volume in 0..100) listOf(PlayerOp("volume", "value", volume / 100.0))
                else emptyList()
            else -> emptyList()
        }

    fun apply(vm: YtuiViewModel, ops: List<PlayerOp>) {
        for (op in ops)
            if (op.key == null || op.value == null) vm.command(op.op) else vm.command(op.op, op.key to op.value)
    }

    fun remote(vm: YtuiViewModel, payload: JSONObject) {
        val player = vm.player.value
        apply(
            vm,
            ops(
                payload.optString("command"),
                payload.takeIf { it.has("position_ms") && !it.isNull("position_ms") }?.optLong("position_ms"),
                payload.takeIf { it.has("volume") && !it.isNull("volume") }?.optInt("volume", -1),
                player.playing,
                player.media != null,
            ),
        )
    }

    /** `handoff_received`: start the media at its position, then take over the sender's queue. */
    fun handoff(sync: SyncController, vm: YtuiViewModel, payload: JSONObject) {
        val media = payload.optJSONObject("media")?.takeIf(::isOnlineMedia) ?: return
        val queue = payload.optJSONArray("queue")?.objects().orEmpty().filter(::isOnlineMedia).take(25)
        val from = payload.text("from_name").take(40)
        startAt(sync, vm, MediaCard(media), payload.optLong("position_ms").coerceAtLeast(0), payload.optBoolean("playing", true))
        sync.act {
            val current = sync.list("queue")
            for (index in current.indices.reversed())
                sync.obj("queue_remove", "index" to index, "url" to current[index].text("url"))
            for (item in queue) sync.obj("enqueue", "media" to item, "music" to MediaCard(item).musicSource)
            vm.loadQueue()
            vm.effect("queue_changed")
        }
        Notices.show(Notice(from.ifBlank { "Handoff" }, Glyph.Devices))
    }

    /**
     * Starts [media] at [atMs] through the existing playback path (the recovery start position),
     * and pauses it once it runs when [playing] is false.
     */
    fun startAt(sync: SyncController, vm: YtuiViewModel, media: MediaCard, atMs: Long, playing: Boolean) {
        vm.play(
            media,
            recovery =
                JSONObject()
                    .put("media", media.value)
                    .put("position", atMs.coerceAtLeast(0) / 1000.0)
                    .put("video", !media.audio)
                    .put(
                        "quality",
                        vm.config.value.optJSONObject("download")?.optString("quality", "1080") ?: "1080",
                    ),
        )
        if (playing) return
        val key = MediaKeys.of(media.value)
        sync.quiet {
            val started =
                withTimeoutOrNull(20_000) {
                    while (true) {
                        val player = vm.player.value
                        if (player.playing && player.media?.let(MediaKeys::of) == key) break
                        delay(200)
                    }
                    true
                }
            if (started == true) vm.command("pause")
        }
    }
}
