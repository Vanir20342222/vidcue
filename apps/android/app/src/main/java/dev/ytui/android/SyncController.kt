package dev.ytui.android

import android.content.res.Configuration
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

/** Sheets of the sync layer, hosted once at the app root and opened from anywhere. */
internal sealed interface SyncSheet {
    data object Friends : SyncSheet

    data object Inbox : SyncSheet

    data object Jam : SyncSheet

    data object Devices : SyncSheet

    data object Playlists : SyncSheet

    data class Playlist(val id: String) : SyncSheet

    data object Stats : SyncSheet

    data object Wrapped : SyncSheet

    data object Filters : SyncSheet

    data object Following : SyncSheet

    data object Admin : SyncSheet

    data object Security : SyncSheet

    data object PairShow : SyncSheet

    data object PairHelp : SyncSheet

    data object KeyRelay : SyncSheet

    data class SendToFriend(val media: MediaCard) : SyncSheet

    data class AddToPlaylist(val media: MediaCard) : SyncSheet
}

/** How the UI treats an engine event kind (§8.2 of docs/backend/PROTOCOL.md). */
internal enum class SyncRoute {
    /** Not a sync event: the existing loop handles it. */
    Ignore,

    /** An existing kind the sync layer also refreshes on; the existing loop still handles it. */
    Observe,

    /** Consumed here; the existing loop never sees it. */
    Handle,
}

internal object SyncEvents {
    val HANDLED =
        setOf(
            "sync_status",
            "library_changed",
            "config_changed",
            "sync_key_request",
            "sync_key_state",
            "presence_changed",
            "handoff_received",
            "remote_command",
            "friends_changed",
            "friend_activity",
            "inbox_item",
            "shared_playlist_changed",
            "jam_invite",
            "jam_changed",
            "jam_state",
            "jam_ended",
            "sync_pair_link",
        )

    /** Existing kinds whose handlers stay in YtuiViewModel. */
    val OBSERVED = setOf("history_updated", "offline_playlists_changed", "account_updated")

    fun route(kind: String): SyncRoute =
        when (kind) {
            in HANDLED -> SyncRoute.Handle
            in OBSERVED -> SyncRoute.Observe
            else -> SyncRoute.Ignore
        }

    /** Event text that should be a JSON object; anything else is dropped. */
    fun payload(text: String): JSONObject? =
        if (text.isBlank() || text.length > 64 * 1024) null
        else
            try {
                JSONObject(text)
            } catch (malformed: org.json.JSONException) {
                null
            }
}

/**
 * Decides when the player's state is worth a `presence_update`: on a new item, a play/pause
 * change, a jump of more than three seconds, or every 30 s while playing. The engine applies the
 * server's limit again; this keeps five snapshots a second from each becoming a JNI call.
 */
internal class PresenceThrottle(private val minGapMs: Long = 1_000) {
    private var sent = false
    private var key: String? = null
    private var playing = false
    private var position = 0L
    private var at = 0L

    fun shouldSend(key: String?, playing: Boolean, positionMs: Long, now: Long): Boolean {
        if (sent) {
            val changed = key != this.key || playing != this.playing
            val expected = if (this.playing) position + (now - at) else position
            val jumped = key != null && kotlin.math.abs(positionMs - expected) > 3_000
            val heartbeat = playing && now - at >= 30_000
            if (!changed && !jumped && !heartbeat) return false
            if (!changed && now - at < minGapMs) return false
        }
        sent = true
        this.key = key
        this.playing = playing
        position = positionMs
        at = now
        return true
    }

    fun reset() {
        sent = false
    }
}

/** The collections a user can switch, in display order, with one-word labels. */
internal val SYNC_COLLECTIONS =
    listOf(
        "positions" to "Resume",
        "history" to "History",
        "saved" to "Saved",
        "playlists" to "Playlists",
        "offline_playlists" to "Offline",
        "subscriptions" to "Following",
        "settings" to "Settings",
        "eq_presets" to "EQ",
        "filters" to "Filters",
        "secrets" to "YouTube",
    )

/**
 * The Android side of the sync engine: state for the sync screens, thin wrappers around the
 * engine ops of docs/backend/PROTOCOL.md §8.1, and routing for the §8.2 events. Nothing here
 * talks to the network itself; every call goes through the Rust engine.
 */
internal class SyncController(
    private val vm: YtuiViewModel,
    private val scope: CoroutineScope,
    private val rpc: suspend (JSONObject) -> Any? = { RustRpc.call(it) },
    private val clock: () -> Long = System::currentTimeMillis,
) {
    val status = MutableStateFlow(JSONObject())
    val devices = MutableStateFlow<List<JSONObject>>(emptyList())
    val friends = MutableStateFlow(JSONObject())
    val activity = MutableStateFlow<List<JSONObject>>(emptyList())
    val shareActivity = MutableStateFlow("off")
    val inbox = MutableStateFlow<List<JSONObject>>(emptyList())
    val unread = MutableStateFlow(0)
    val presence = MutableStateFlow<List<JSONObject>>(emptyList())
    val recs = MutableStateFlow(JSONObject())
    val recsSettings = MutableStateFlow(JSONObject())
    val playlists = MutableStateFlow<List<JSONObject>>(emptyList())
    /** Bumped when a playlist's entries may have changed remotely. */
    val playlistRevision = MutableStateFlow(0L)
    val follows = MutableStateFlow<List<JSONObject>>(emptyList())
    val filters = MutableStateFlow<List<JSONObject>>(emptyList())
    /** `key_fingerprint`: `{state, fingerprint}`. */
    val key = MutableStateFlow(JSONObject())
    /** Another device asks for the account key (`sync_key_request`). */
    val keyRequest = MutableStateFlow<JSONObject?>(null)
    /** A `vidcue://pair` link that arrived and waits for the user's confirmation. */
    val pairLink = MutableStateFlow<String?>(null)
    val sheet = MutableStateFlow<SyncSheet?>(null)
    val jam = JamController(this, vm, scope, clock)

    val jamRoom: MutableStateFlow<JSONObject?>
        get() = jam.room

    private var started = false
    private var signedInBefore: Boolean? = null
    private val presenceThrottle = PresenceThrottle()
    private var presenceJob: Job? = null

    val signedIn: Boolean
        get() = status.value.optBoolean("signed_in")

    fun has(feature: String): Boolean = signedIn && status.value.optJSONArray("features").strings().contains(feature)

    fun open(next: SyncSheet?) {
        sheet.value = next
    }

    // --- RPC -------------------------------------------------------------------------------

    /** Request JSON without the fields that are null, so optional fields stay absent. */
    internal fun request(op: String, vararg fields: Pair<String, Any?>): JSONObject =
        JSONObject().put("op", op).apply { fields.forEach { (k, v) -> if (v != null) put(k, v) } }

    suspend fun obj(op: String, vararg fields: Pair<String, Any?>): JSONObject =
        rpc(request(op, *fields)) as? JSONObject ?: JSONObject()

    suspend fun list(op: String, vararg fields: Pair<String, Any?>): List<JSONObject> =
        when (val reply = rpc(request(op, *fields))) {
            is JSONArray -> reply.objects()
            is JSONObject -> reply.optJSONArray(LIST_FIELDS[op] ?: "items")?.objects().orEmpty()
            else -> emptyList()
        }

    /** Background refresh: failures stay quiet (the screen keeps its last state). */
    fun quiet(block: suspend () -> Unit): Job =
        scope.launch {
            try {
                block()
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (ignored: Exception) {
            }
        }

    /** A user action outside a form: failures show as the app's error notice. */
    fun act(block: suspend () -> Unit): Job =
        scope.launch {
            try {
                block()
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (failure: Exception) {
                vm.status.value = SyncText.message(failure)
                vm.actionError.value = vm.status.value
            }
        }

    // --- Lifecycle -------------------------------------------------------------------------

    /** First composition of the root host: read state once; later changes arrive as events. */
    fun start() {
        if (started) return
        started = true
        quiet { applyStatus(obj("sync_status")) }
        quiet { loadLibrary() }
    }

    suspend fun loadLibrary() {
        playlists.value = list("playlists")
        follows.value = list("follows")
        filters.value = list("filters")
    }

    private fun applyStatus(value: JSONObject) {
        if (value.length() == 0) return
        status.value = value
        val now = value.optBoolean("signed_in")
        val before = signedInBefore
        signedInBefore = now
        if (now == before) return
        val context = MediaBridge.context
        if (now) {
            if (context != null) runCatching { SyncWorker.schedule(context) }
            quiet { key.value = obj("key_fingerprint") }
            quiet { refreshInbox() }
            quiet { jam.refresh() }
            quiet { refreshRecs() }
        } else {
            if (context != null && before == true) runCatching { SyncWorker.cancel(context) }
            presenceThrottle.reset()
            devices.value = emptyList()
            friends.value = JSONObject()
            activity.value = emptyList()
            inbox.value = emptyList()
            unread.value = 0
            presence.value = emptyList()
            recs.value = JSONObject()
            jam.clear()
        }
    }

    // --- Events ----------------------------------------------------------------------------

    /** Handles one `MediaBridge` event; true when the existing loop must not see it. */
    fun onEvent(event: JSONObject): Boolean {
        val kind = event.optString("kind")
        val text = event.optString("text")
        return when (SyncEvents.route(kind)) {
            SyncRoute.Ignore -> false
            SyncRoute.Observe -> {
                if (kind == "account_updated") quiet { applyStatus(obj("sync_status")) }
                false
            }
            SyncRoute.Handle -> {
                handle(kind, text)
                true
            }
        }
    }

    private fun handle(kind: String, text: String) {
        when (kind) {
            "sync_status" -> SyncEvents.payload(text)?.let(::applyStatus)
            "library_changed" ->
                when (text) {
                    "saved" ->
                        if (vm.feeds[vm.tab.value].value.request == "Favorites")
                            vm.navigate(request = "Favorites", remember = false)
                    "playlists" -> {
                        playlistRevision.value++
                        quiet { playlists.value = list("playlists") }
                    }
                    "follows" -> quiet { follows.value = list("follows") }
                    "filters" -> quiet { filters.value = list("filters") }
                }
            "config_changed" -> quiet { reloadConfig() }
            "sync_key_request" ->
                SyncEvents.payload(text)?.takeIf {
                    it.text("id").isNotBlank() && Fingerprints.groups(it.text("fingerprint")).isNotEmpty()
                }?.let { keyRequest.value = it }
            "sync_key_state" -> {
                quiet { key.value = obj("key_fingerprint") }
                if (text == "ok" && sheet.value == SyncSheet.KeyRelay) {
                    sheet.value = null
                    Notices.show(Notice("Key received", Glyph.Key))
                }
            }
            "presence_changed" -> quiet { refreshPresence() }
            "handoff_received" -> SyncEvents.payload(text)?.let { RemoteControl.handoff(this, vm, it) }
            "remote_command" -> SyncEvents.payload(text)?.let { RemoteControl.remote(vm, it) }
            "friends_changed" -> quiet { refreshFriends() }
            "friend_activity" -> if (sheet.value == SyncSheet.Friends) quiet { refreshActivity() }
            "inbox_item" -> {
                unread.value++
                if (sheet.value == SyncSheet.Inbox) quiet { refreshInbox() }
                val from = SyncEvents.payload(text)?.optJSONObject("from")?.text("username").orEmpty()
                Notices.show(Notice(from.take(40).ifBlank { "Inbox" }, Glyph.Inbox, "Open", { open(SyncSheet.Inbox) }))
            }
            "shared_playlist_changed" -> {
                playlistRevision.value++
                quiet { playlists.value = list("playlists") }
            }
            "jam_invite", "jam_changed", "jam_state", "jam_ended" -> jam.onEvent(kind, text)
            "sync_pair_link" -> if (PairLinks.recognize(text)) pairLink.value = text
        }
    }

    private suspend fun reloadConfig() {
        val config = obj("settings")
        if (config.length() == 0) return
        vm.config.value = config
        val night =
            MediaBridge.context?.resources?.configuration?.uiMode?.and(Configuration.UI_MODE_NIGHT_MASK) ==
                Configuration.UI_MODE_NIGHT_YES
        vm.palette.value = obj("theme", "dark" to night)
        vm.effect("settings", "config" to config)
    }

    // --- Player ----------------------------------------------------------------------------

    /** Every player snapshot: presence for the user's other devices and the Jam end check. */
    fun onPlayer(player: PlayerState) {
        jam.onPlayer(player)
        if (!signedIn || !status.value.optBoolean("connected") || !has("realtime")) return
        // One update in flight at a time; the next snapshot re-evaluates against the last sent.
        if (presenceJob?.isActive == true) return
        val media = player.media?.takeIf { isOnlineMedia(it) && !player.value.optBoolean("offline") && !player.preview }
        val key = media?.let(MediaKeys::of)
        val positionMs = (player.position * 1000).toLong()
        val playing = media != null && player.playing
        if (!presenceThrottle.shouldSend(key, playing, positionMs, clock())) return
        val request =
            request(
                "presence_update",
                "media" to (media ?: JSONObject.NULL),
                "playing" to playing,
                "position_ms" to positionMs,
                "duration_ms" to (player.duration * 1000).toLong().takeIf { it > 0 },
                "volume" to (player.value.optDouble("volume", 1.0) * 100).toInt().coerceIn(0, 100),
                "queue_len" to vm.queue.value.size,
            )
        presenceJob = quiet { rpc(request) }
    }

    // --- Account ---------------------------------------------------------------------------

    suspend fun refreshStatus() = applyStatus(obj("sync_status"))

    suspend fun signIn(server: String, username: String, password: String, code: String) =
        applyStatus(
            obj(
                "sync_sign_in",
                "server" to server.trim(),
                "username" to username.trim(),
                "password" to password,
                "code" to code.trim(),
                "device_name" to deviceName(),
            )
        )

    suspend fun signOut(removeDevice: Boolean, forgetData: Boolean) {
        val own = status.value.text("device_id")
        if (removeDevice && own.isNotBlank()) runCatching { obj("sync_revoke", "device_id" to own) }
        applyStatus(obj("sync_sign_out", "forget_data" to forgetData))
    }

    suspend fun syncNow() = applyStatus(obj("sync_now"))

    suspend fun setCollection(name: String, enabled: Boolean) =
        applyStatus(obj("sync_settings", "collections" to JSONObject().put(name, enabled)))

    suspend fun setHealthReports(enabled: Boolean) = applyStatus(obj("sync_settings", "health_reports" to enabled))

    suspend fun refreshDevices() {
        devices.value = list("sync_devices")
    }

    suspend fun revoke(deviceId: String) {
        obj("sync_revoke", "device_id" to deviceId)
        if (deviceId == status.value.text("device_id")) refreshStatus() else refreshDevices()
    }

    suspend fun claimPairing(uri: String) {
        applyStatus(obj("sync_pair_claim", "uri" to uri, "device_name" to deviceName()))
        pairLink.value = null
    }

    fun deviceName(): String =
        listOf(android.os.Build.MANUFACTURER, android.os.Build.MODEL)
            .filter { !it.isNullOrBlank() }
            .joinToString(" ")
            .ifBlank { "Android" }
            .take(64)

    // --- Social ----------------------------------------------------------------------------

    suspend fun refreshFriends() {
        friends.value = obj("friends")
    }

    suspend fun refreshActivity() {
        activity.value = list("friends_activity")
    }

    suspend fun refreshShare() {
        shareActivity.value = obj("social_settings").text("share_activity").ifBlank { "off" }
    }

    suspend fun setShare(value: String) {
        shareActivity.value = obj("social_settings", "share_activity" to value).text("share_activity").ifBlank { value }
    }

    suspend fun refreshInbox() {
        val reply = obj("inbox")
        inbox.value = reply.optJSONArray("items")?.objects().orEmpty()
        unread.value = reply.optInt("unread", inbox.value.count { !it.optBoolean("read") })
    }

    suspend fun refreshPresence() {
        presence.value = list("presence_devices")
    }

    // --- Library ---------------------------------------------------------------------------

    suspend fun refreshRecs() {
        if (!has("recs")) return
        recsSettings.value = obj("recs_settings")
        recs.value = if (recsSettings.value.optBoolean("enabled")) obj("recs") else JSONObject()
    }

    suspend fun setRecs(enabled: Boolean? = null, music: Boolean? = null) {
        recsSettings.value = obj("recs_settings", "enabled" to enabled, "music" to music)
        recs.value = if (recsSettings.value.optBoolean("enabled")) obj("recs") else JSONObject()
    }

    suspend fun toggleFollow(channelId: String, title: String, thumbnail: String): Boolean {
        val following =
            obj("follow_toggle", "channel_id" to channelId, "title" to title, "thumbnail" to thumbnail.ifBlank { null })
                .optBoolean("following")
        follows.value = list("follows")
        return following
    }

    suspend fun addFilter(kind: String, value: String? = null) {
        obj("filter_add", "kind" to kind, "value" to value)
        filters.value = list("filters")
    }

    suspend fun removeFilter(id: String) {
        obj("filter_remove", "id" to id)
        filters.value = list("filters")
    }

    companion object {
        /** Object replies that wrap a list under a field name. */
        private val LIST_FIELDS =
            mapOf(
                "sync_devices" to "devices",
                "friends_activity" to "activity",
                "presence_devices" to "devices",
                "key_requests" to "requests",
                "admin_users" to "users",
                "admin_devices" to "devices",
            )
    }
}

/** `vidcue_core::PlaybackHistory::key`: `youtube:<id>` for YouTube links, else the URL. */
internal object MediaKeys {
    fun of(media: JSONObject): String = of(media.text("url"), media.text("id"))

    fun of(url: String, id: String): String {
        val parsed =
            try {
                java.net.URI(url)
            } catch (invalid: java.net.URISyntaxException) {
                null
            }
        val scheme = parsed?.scheme?.lowercase()
        val host = parsed?.host?.lowercase().orEmpty()
        if ((scheme == "https" || scheme == "http") && youtube(host)) {
            val found =
                if (host == "youtu.be") parsed!!.rawPath.orEmpty().trimStart('/')
                else
                    parsed!!.rawQuery.orEmpty()
                        .split('&')
                        .firstOrNull { it.substringBefore('=') == "v" && it.contains('=') }
                        ?.substringAfter('=')
                        ?.let { java.net.URLDecoder.decode(it, "UTF-8") }
                        ?: id
            if (found.isNotEmpty()) return "youtube:$found"
        }
        return url
    }

    private fun youtube(host: String) = host == "youtube.com" || host.endsWith(".youtube.com") || host == "youtu.be"
}
