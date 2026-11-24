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
    /** The room's playback rate (0.5 to 2.0). */
    val rate: Double = 1.0,
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
                rate = reply.optDouble("rate", 1.0).takeIf { it.isFinite() }?.coerceIn(0.5, 2.0) ?: 1.0,
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

    /**
     * Whether [next] may replace the stored room playback: the server's versions only grow, so an
     * HTTP reply overtaken by a newer `jam_state` (or a refetch started earlier) is ignored.
     */
    fun newer(current: JSONObject?, next: JSONObject): Boolean =
        current == null || next.optLong("version") >= current.optLong("version")

    /**
     * What the user did to the local player since the last tick that this controller did not
     * cause: "play" or "pause" on the room's item, "media" for another item, or null. Nothing
     * counts while the room itself changed, a step of ours is settling or a start is running.
     */
    fun userChange(
        before: JamPlayerView?,
        now: JamPlayerView,
        roomKey: String,
        roomUnchanged: Boolean,
        settling: Boolean,
    ): String? {
        if (before == null || !roomUnchanged || settling || now.busy || before.busy) return null
        if (before.mediaKey != roomKey) return null
        return when {
            now.mediaKey == before.mediaKey && now.playing != before.playing -> if (now.playing) "play" else "pause"
            now.mediaKey != null && now.mediaKey != before.mediaKey -> "media"
            else -> null
        }
    }
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
    /** The local player as the last tick left it, the room version then, and our last step. */
    private var observed: JamPlayerView? = null
    private var observedVersion = -1L
    private var stepped = false
    private var rateApplied = false

    val roomId: String
        get() = room.value?.text("id").orEmpty()

    fun clear() {
        setRoom(null)
        invites.value = emptyList()
    }

    private fun setRoom(value: JSONObject?) {
        val sameRoom = value != null && value.text("id") == roomId
        room.value = value
        val next = value?.optJSONObject("playback")
        if (!sameRoom || next == null || JamSteps.newer(playback.value, next)) playback.value = next
        if (value == null) {
            loop?.cancel()
            loop = null
            pendingLoad = null
            observed = null
            if (rateApplied) {
                rateApplied = false
                vm.command("speed_restore")
            }
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
                if (room.value == null || !JamSteps.newer(playback.value, state)) return
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
        if (state.length() > 0 && JamSteps.newer(playback.value, state)) playback.value = state
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
            // Local transport changes the user made (lock screen, notification, another video)
            // are forwarded when this member may steer the room, otherwise explained.
            val version = playback.value?.optLong("version", -1L) ?: -1L
            val loading = pendingLoad?.let { clock() - it.second < JamSteps.LOAD_GRACE_MS } == true
            val change = JamSteps.userChange(observed, view, target.mediaKey, version == observedVersion, stepped || loading)
            observed = view
            observedVersion = version
            stepped = false
            if (change == "play" || change == "pause") {
                if (JamSteps.canControl(room.value)) {
                    val state =
                        sync.obj(
                            "jam_control",
                            "room_id" to roomId,
                            "action" to change,
                            "position_ms" to (player.position * 1000).toLong().coerceAtLeast(0),
                        )
                    if (state.length() > 0 && JamSteps.newer(playback.value, state)) {
                        playback.value = state
                        observedVersion = state.optLong("version", -1L)
                    }
                    return@withLock
                }
                Notices.show(Notice("Playback follows the Jam", Glyph.Jam))
            } else if (change == "media")
                Notices.show(Notice("Playback follows the Jam", Glyph.Jam, "Leave", { sync.quiet { leave() } }))
            // The room's rate, applied without changing the user's saved speed.
            val speed = player.value.optDouble("speed", 1.0)
            if (target.inRoom && media != null && kotlin.math.abs(speed - target.rate) > 0.01) {
                rateApplied = true
                vm.command("speed", "value" to target.rate, "transient" to true)
            }
            val step = JamSteps.decide(target, view, pendingLoad, clock())
            stepped = step != null
            when (step) {
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
