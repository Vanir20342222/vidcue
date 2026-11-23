package dev.ytui.android

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONObject

/** The engine's `jam_target` reply: what the local player should do to match the room. */
internal data class JamTarget(
    val inRoom: Boolean,
    val action: String,
    val mediaKey: String,
    val media: JSONObject?,
    val playing: Boolean,
    val targetMs: Long,
    val seekMs: Long,
) {
    companion object {
        fun from(reply: JSONObject) =
            JamTarget(
                inRoom = reply.optBoolean("in_room"),
                action = reply.optString("action", "none"),
                mediaKey = reply.text("media_key"),
                media = reply.optJSONObject("media"),
                playing = reply.optBoolean("playing"),
                targetMs = reply.optLong("target_ms").coerceAtLeast(0),
                seekMs = reply.optLong("seek_ms").coerceAtLeast(0),
            )
    }
}

/** One change to the local player, through the view model's existing playback calls. */
internal sealed interface JamStep {
    /** Start the room's item at [atMs]; [playing] false pauses it once it runs. */
    data class Load(val media: JSONObject, val atMs: Long, val playing: Boolean) : JamStep

    /** The player's play/pause toggle. */
    data object Toggle : JamStep

    data class Seek(val ms: Long) : JamStep
}

/** What the local player state is, as far as the Jam drift rule needs it. */
internal data class JamPlayerView(
    val mediaKey: String?,
    val playing: Boolean,
    /** A start is in progress (resolving or preparing a video surface). */
    val busy: Boolean,
)

internal object JamSteps {
    /** A load of the same item is not repeated while it may still be starting. */
    const val LOAD_GRACE_MS = 12_000L

    /** The local player counts as at the end this close to the item's duration. */
    const val END_WINDOW_MS = 1_500L

    /**
     * Turns the engine's drift decision into at most one player step. The engine already applies
     * the 400 ms threshold, the seek lead and its seek backoff (§5.5); this side only avoids
     * acting while a start is in flight and never toggles towards a state the player is in.
     */
    fun decide(
        target: JamTarget,
        player: JamPlayerView,
        pendingLoad: Pair<String, Long>?,
        now: Long,
    ): JamStep? {
        if (!target.inRoom || player.busy) return null
        return when (target.action) {
            "load" -> {
                val media = target.media?.takeIf(::isOnlineMedia) ?: return null
                if (pendingLoad != null && pendingLoad.first == target.mediaKey && now - pendingLoad.second < LOAD_GRACE_MS)
                    null
                else JamStep.Load(media, target.targetMs, target.playing)
            }
            "play" -> if (!player.playing && player.mediaKey == target.mediaKey) JamStep.Toggle else null
            "pause" -> if (player.playing) JamStep.Toggle else null
            "seek" -> if (player.mediaKey == target.mediaKey) JamStep.Seek(target.seekMs) else null
            else -> null
        }
    }

    fun finished(positionMs: Long, durationMs: Long): Boolean =
        durationMs > 0 && positionMs >= durationMs - END_WINDOW_MS

    /** Members may steer playback when they host, or when the host allows guests to. */
    fun canControl(room: JSONObject?): Boolean =
        room != null && (room.optString("role") == "host" || room.optBoolean("guests_can_control"))
}

/**
 * Keeps the local player in step with the Jam room while this device is in one: on every
 * `jam_state` and every two seconds it asks the engine for `jam_target` and applies the answer.
 * Media is always resolved and streamed by this device itself; the room only coordinates.
 */
internal class JamController(
    private val sync: SyncController,
    private val vm: YtuiViewModel,
    private val scope: CoroutineScope,
    private val clock: () -> Long = System::currentTimeMillis,
) {
    val room = MutableStateFlow<JSONObject?>(null)
    val invites = MutableStateFlow<List<JSONObject>>(emptyList())
    val playback = MutableStateFlow<JSONObject?>(null)

    private var loop: Job? = null
    private val tickLock = Mutex()
    private var pendingLoad: Pair<String, Long>? = null
    private var endedSent: String? = null

    val roomId: String
        get() = room.value?.text("id").orEmpty()

    fun clear() {
        setRoom(null)
        invites.value = emptyList()
    }

    private fun setRoom(value: JSONObject?) {
        room.value = value
        playback.value = value?.optJSONObject("playback")
        if (value == null) {
            loop?.cancel()
            loop = null
            pendingLoad = null
        } else if (loop?.isActive != true)
            loop =
                scope.launch {
                    while (isActive) {
                        try {
                            tick()
                        } catch (cancelled: CancellationException) {
                            throw cancelled
                        } catch (unreachable: Exception) {
                            // Offline for a moment: the next tick tries again.
                        }
                        delay(2_000)
                    }
                }
    }

    fun onEvent(kind: String, text: String) {
        when (kind) {
            "jam_invite" -> {
                sync.quiet { refresh() }
                val invite = SyncEvents.payload(text) ?: return
                val from = invite.optJSONObject("from")?.text("username").orEmpty()
                if (room.value == null)
                    Notices.show(Notice(from.take(40).ifBlank { "Jam" }, Glyph.Jam, "Open", { sync.open(SyncSheet.Jam) }))
            }
            "jam_changed" -> if (text.isNotBlank() && text == roomId) sync.quiet { reload() }
            "jam_state" -> {
                val state = SyncEvents.payload(text) ?: return
                if (room.value == null) return
                playback.value = state
                sync.quiet { tick() }
            }
            "jam_ended" -> {
                val ended = SyncEvents.payload(text) ?: return
                if (ended.text("room_id") == roomId) {
                    setRoom(null)
                    Notices.show(Notice("Jam ended", Glyph.Jam))
                }
                invites.value = invites.value.filter { it.text("room_id") != ended.text("room_id") }
            }
        }
    }

    suspend fun refresh() {
        val reply = sync.obj("jam_current")
        invites.value = reply.optJSONArray("invites")?.objects().orEmpty()
        setRoom(reply.optJSONObject("current"))
    }

    suspend fun reload() {
        val id = roomId.ifBlank { return }
        setRoom(sync.obj("jam_room", "room_id" to id).takeIf { it.has("id") })
    }

    suspend fun create(title: String?, guestsCanControl: Boolean) =
        setRoom(sync.obj("jam_create", "title" to title?.trim()?.ifBlank { null }, "guests_can_control" to guestsCanControl))

    suspend fun join(roomId: String) {
        setRoom(sync.obj("jam_join", "room_id" to roomId))
        invites.value = invites.value.filter { it.text("room_id") != roomId }
    }

    suspend fun decline(roomId: String) {
        sync.obj("jam_decline", "room_id" to roomId)
        invites.value = invites.value.filter { it.text("room_id") != roomId }
    }

    suspend fun leave() {
        val id = roomId.ifBlank { return }
        sync.obj(if (room.value?.optString("role") == "host") "jam_end" else "jam_leave", "room_id" to id)
        setRoom(null)
    }

    suspend fun update(title: String? = null, guestsCanControl: Boolean? = null) {
        val id = roomId.ifBlank { return }
        setRoom(sync.obj("jam_update", "room_id" to id, "title" to title, "guests_can_control" to guestsCanControl))
    }

    suspend fun invite(userIds: List<String>) {
        val id = roomId.ifBlank { return }
        sync.obj("jam_invite", "room_id" to id, "user_ids" to org.json.JSONArray(userIds.take(10)))
        reload()
    }

    suspend fun kick(userId: String) {
        val id = roomId.ifBlank { return }
        sync.obj("jam_kick", "room_id" to id, "user_id" to userId)
        reload()
    }

    suspend fun add(media: MediaCard) {
        val id = roomId.ifBlank { return }
        sync.obj("jam_queue_add", "room_id" to id, "media" to media.value)
        reload()
    }

    suspend fun remove(itemId: String) {
        val id = roomId.ifBlank { return }
        sync.obj("jam_queue_remove", "room_id" to id, "item_id" to itemId)
        reload()
    }

    suspend fun vote(itemId: String, up: Boolean) {
        val id = roomId.ifBlank { return }
        sync.obj("jam_vote", "room_id" to id, "item_id" to itemId, "up" to up)
        reload()
    }

    suspend fun control(action: String, positionMs: Long? = null, itemId: String? = null) {
        val id = roomId.ifBlank { return }
        val state =
            sync.obj("jam_control", "room_id" to id, "action" to action, "position_ms" to positionMs, "item_id" to itemId)
        if (state.length() > 0) playback.value = state
        if (action == "play_item") reload()
        tick()
    }

    /** Snapshot hook: tells the room once when the local player reaches the end of its item. */
    fun onPlayer(player: PlayerState) {
        val item = playback.value?.optJSONObject("item") ?: return
        if (room.value == null) return
        val itemId = item.text("item_id")
        val media = player.media ?: return
        if (itemId.isBlank() || itemId == endedSent || MediaKeys.of(media) != item.text("media_key")) return
        val ended = player.value.optInt("playback_state") == 4
        if (!ended && !JamSteps.finished((player.position * 1000).toLong(), (player.duration * 1000).toLong())) return
        endedSent = itemId
        sync.quiet { control("ended", itemId = itemId) }
    }

    private suspend fun tick() =
        tickLock.withLock {
            if (room.value == null) return@withLock
            val player = vm.player.value
            val media = player.media?.takeUnless { player.preview }
            val key = media?.let(MediaKeys::of)
            val target =
                JamTarget.from(
                    sync.obj(
                        "jam_target",
                        "player_media_key" to key,
                        "player_position_ms" to media?.let { (player.position * 1000).toLong() },
                        "player_playing" to (media != null && player.playing),
                    )
                )
            val view =
                JamPlayerView(
                    key,
                    media != null && player.playing,
                    busy = player.preparing != null || player.value.optBoolean("navigation_pending"),
                )
            when (val step = JamSteps.decide(target, view, pendingLoad, clock())) {
                is JamStep.Load -> {
                    pendingLoad = target.mediaKey to clock()
                    RemoteControl.startAt(sync, vm, MediaCard(step.media), step.atMs, step.playing)
                }
                JamStep.Toggle -> vm.command("pause")
                is JamStep.Seek -> vm.command("seek", "seconds" to step.ms / 1000.0)
                null -> Unit
            }
        }
}
