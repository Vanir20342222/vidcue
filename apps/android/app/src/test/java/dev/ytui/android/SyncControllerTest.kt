package dev.ytui.android

import androidx.lifecycle.ViewModelStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
class SyncControllerTest {
    private lateinit var scope: CoroutineScope
    private lateinit var store: ViewModelStore
    private lateinit var model: YtuiViewModel
    private val requests = mutableListOf<JSONObject>()
    private val replies = mutableMapOf<String, Any>()

    private fun controller() =
        SyncController(model, scope, rpc = { request ->
            requests += request
            replies[request.optString("op")] ?: JSONObject()
        }, clock = { 1_763_900_000_000L })

    private fun event(kind: String, text: String = "") = JSONObject().put("kind", kind).put("text", text)

    @Before
    fun setUp() {
        Dispatchers.setMain(Dispatchers.Unconfined)
        scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        model = YtuiViewModel(browsePage = { _, _, _ -> JSONObject() }, readHistory = { emptyList() })
        store = ViewModelStore().apply { put("sync", model) }
    }

    @After
    fun tearDown() {
        scope.cancel()
        store.clear()
        Dispatchers.resetMain()
    }

    @Test
    fun everyContractEventKindIsRoutedAndExistingKindsStayWithTheViewModel() {
        for (kind in SyncEvents.HANDLED) assertEquals(kind, SyncRoute.Handle, SyncEvents.route(kind))
        for (kind in listOf("history_updated", "offline_playlists_changed", "account_updated"))
            assertEquals(kind, SyncRoute.Observe, SyncEvents.route(kind))
        for (kind in listOf("storage_updated", "shared", "error", "cookies", ""))
            assertEquals(kind, SyncRoute.Ignore, SyncEvents.route(kind))
        val sync = controller()
        assertFalse(sync.onEvent(event("history_updated")))
        assertFalse(sync.onEvent(event("shared", "https://www.youtube.com/watch?v=x")))
        assertTrue(sync.onEvent(event("friends_changed")))
        assertTrue(requests.any { it.optString("op") == "friends" })
    }

    @Test
    fun statusEventsUpdateStateAndMalformedPayloadsAreDropped() {
        val sync = controller()
        assertTrue(sync.onEvent(event("sync_status", "not json")))
        assertFalse(sync.signedIn)
        val status =
            """{"signed_in":true,"username":"alice","state":"idle","connected":true,"features":["realtime","social","jam"],"key_state":"ok"}"""
        assertTrue(sync.onEvent(event("sync_status", status)))
        assertTrue(sync.signedIn)
        assertTrue(sync.has("jam"))
        assertFalse(sync.has("recs"))
        // Signing in loads the key state, the inbox and any Jam once.
        val ops = requests.map { it.optString("op") }
        assertTrue(ops.containsAll(listOf("key_fingerprint", "inbox", "jam_current")))
        assertNull(SyncEvents.payload(""))
        assertNull(SyncEvents.payload("x".repeat(70_000)))
    }

    @Test
    fun signingOutClearsSocialState() {
        val sync = controller()
        sync.onEvent(event("sync_status", """{"signed_in":true,"features":["social"]}"""))
        sync.unread.value = 4
        sync.inbox.value = listOf(JSONObject().put("id", "a"))
        sync.onEvent(event("sync_status", """{"signed_in":false,"state":"signed_out","reason":"revoked"}"""))
        assertEquals(0, sync.unread.value)
        assertTrue(sync.inbox.value.isEmpty())
        assertFalse(sync.has("social"))
    }

    @Test
    fun pairingLinksNeedTheExactShapeBeforeTheConfirmSheet() {
        val sync = controller()
        val link = "vidcue://pair?server=https%3A%2F%2Fvidcue.example.org&code=ABCDEFGH23#k=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
        assertTrue(PairLinks.recognize(link))
        assertTrue(sync.onEvent(event("sync_pair_link", link)))
        assertEquals(link, sync.pairLink.value)
        sync.pairLink.value = null
        for (bad in
            listOf(
                "",
                "https://vidcue.example.org/pair?code=ABCDEFGH23",
                "vidcue://join?server=x&code=y",
                "vidcue://pair",
                "vidcue://pair/extra?code=y",
                "vidcue://pair?server=x &code=y",
                "vidcue://pair?" + "a".repeat(PairLinks.MAX_LENGTH),
            )) {
            assertFalse(bad, PairLinks.recognize(bad))
            sync.onEvent(event("sync_pair_link", bad))
            assertNull(bad, sync.pairLink.value)
        }
    }

    @Test
    fun queuedHandoffsAndRemoteCommandsExpire() {
        val now = 100_000L
        assertFalse(SyncEvents.stale("handoff_received", now - 2_000, now))
        assertTrue(SyncEvents.stale("handoff_received", now - SyncEvents.STALE_COMMAND_MS - 1, now))
        assertTrue(SyncEvents.stale("remote_command", now - 60_000, now))
        // Other kinds keep their meaning however late they are handled.
        assertFalse(SyncEvents.stale("inbox_item", now - 60_000, now))
        assertFalse(SyncEvents.stale("jam_state", now - 60_000, now))
        // Unstamped events (tests, older builds) are never stale.
        assertFalse(SyncEvents.stale("remote_command", 0, now))
    }

    @Test
    fun onlyASignedOutReplyEndsBackgroundSync() {
        assertTrue(SyncWorker.signedOut(JSONObject().put("signed_in", false)))
        assertFalse(SyncWorker.signedOut(JSONObject().put("signed_in", true)))
    }

    @Test
    fun keyRequestsNeedAnIdAndAWellFormedFingerprint() {
        val sync = controller()
        sync.onEvent(event("sync_key_request", """{"id":"r1","device_id":"d","device_name":"Tab","fingerprint":"12345"}"""))
        assertNull(sync.keyRequest.value)
        sync.onEvent(event("sync_key_request", """{"id":"r1","device_id":"d","device_name":"Tab","fingerprint":"042 917 305 118"}"""))
        assertEquals("r1", sync.keyRequest.value?.optString("id"))
    }

    @Test
    fun fingerprintsAreFourGroupsOfThreeDigits() {
        assertEquals(listOf("042", "917", "305", "118"), Fingerprints.groups("042 917 305 118"))
        assertEquals(listOf("042", "917", "305", "118"), Fingerprints.groups("042917305118"))
        assertTrue(Fingerprints.groups("042 917 305").isEmpty())
        assertTrue(Fingerprints.groups("042 917 305 11a").isEmpty())
        assertTrue(Fingerprints.groups("042 917 305 1180").isEmpty())
        assertEquals("0 4 2, 9 1 7", Fingerprints.spoken(listOf("042", "917")))
    }

    @Test
    fun inboxItemsCountAndJamEventsReachTheJamController() {
        val sync = controller()
        sync.onEvent(event("inbox_item", """{"id":"i1","from":{"username":"bob"}}"""))
        assertEquals(1, sync.unread.value)
        replies["jam_current"] = JSONObject().put("current", JSONObject.NULL).put("invites", JSONArray().put(JSONObject().put("room_id", "r1").put("title", "Friday")))
        sync.onEvent(event("jam_invite", """{"room_id":"r1","title":"Friday","from":{"username":"bob"}}"""))
        assertEquals("r1", sync.jam.invites.value.single().optString("room_id"))
        sync.onEvent(event("jam_ended", """{"room_id":"r1","reason":"ended"}"""))
        assertTrue(sync.jam.invites.value.isEmpty())
    }

    @Test
    fun requestsLeaveOutAbsentOptionalFields() {
        val sync = controller()
        val request = sync.request("social_settings", "share_activity" to null)
        assertFalse(request.has("share_activity"))
        assertEquals("social_settings", request.optString("op"))
        val move = sync.request("playlist_move", "after_entry_id" to JSONObject.NULL)
        assertTrue(move.has("after_entry_id"))
        assertTrue(move.isNull("after_entry_id"))
    }

    @Test
    fun presenceIsSentOnChangesJumpsAndHeartbeatsOnly() {
        val throttle = PresenceThrottle()
        assertTrue(throttle.shouldSend("youtube:a", true, 0, 0))
        assertFalse("steady playback", throttle.shouldSend("youtube:a", true, 5_000, 5_000))
        assertTrue("paused", throttle.shouldSend("youtube:a", false, 5_000, 5_200))
        assertFalse(throttle.shouldSend("youtube:a", false, 5_000, 9_000))
        assertTrue("seek while paused", throttle.shouldSend("youtube:a", false, 60_000, 9_500))
        assertTrue("next item", throttle.shouldSend("youtube:b", true, 0, 9_600))
        assertFalse(throttle.shouldSend("youtube:b", true, 20_000, 29_600))
        assertTrue("heartbeat", throttle.shouldSend("youtube:b", true, 30_000, 39_600))
        assertTrue("idle", throttle.shouldSend(null, false, 0, 40_000))
        throttle.reset()
        assertTrue(throttle.shouldSend(null, false, 0, 40_100))
    }

    @Test
    fun mediaKeysMatchTheCoreHistoryKey() {
        assertEquals("youtube:abc123", MediaKeys.of("https://www.youtube.com/watch?v=abc123&t=4", ""))
        assertEquals("youtube:abc123", MediaKeys.of("https://music.youtube.com/watch?list=x&v=abc123", ""))
        assertEquals("youtube:abc123", MediaKeys.of("https://youtu.be/abc123", ""))
        assertEquals("youtube:fallback", MediaKeys.of("https://www.youtube.com/shorts/xyz", "fallback"))
        assertEquals("https://radio.example/stream", MediaKeys.of("https://radio.example/stream", "id"))
        assertEquals("file:///x.mp3", MediaKeys.of("file:///x.mp3", "id"))
    }

    @Test
    fun remoteCommandsMapToTheExistingPlayerCalls() {
        assertEquals(listOf(PlayerOp("pause")), RemoteControl.ops("play", null, null, playing = false, hasMedia = true))
        assertTrue(RemoteControl.ops("play", null, null, playing = true, hasMedia = true).isEmpty())
        assertEquals(listOf(PlayerOp("pause")), RemoteControl.ops("pause", null, null, playing = true, hasMedia = true))
        assertTrue(RemoteControl.ops("pause", null, null, playing = false, hasMedia = true).isEmpty())
        assertEquals(listOf(PlayerOp("pause")), RemoteControl.ops("toggle", null, null, playing = false, hasMedia = true))
        assertEquals(listOf(PlayerOp("seek", "seconds", 51.5)), RemoteControl.ops("seek", 51_500, null, true, true))
        assertTrue("seek needs a position", RemoteControl.ops("seek", null, null, true, true).isEmpty())
        assertEquals(listOf(PlayerOp("next")), RemoteControl.ops("next", null, null, true, true))
        assertEquals(listOf(PlayerOp("previous")), RemoteControl.ops("prev", null, null, true, true))
        assertEquals(listOf(PlayerOp("volume", "value", 0.8)), RemoteControl.ops("volume", null, 80, true, true))
        assertTrue(RemoteControl.ops("volume", null, 101, true, true).isEmpty())
        assertTrue(RemoteControl.ops("shutdown", null, null, true, true).isEmpty())
        assertTrue("nothing loaded", RemoteControl.ops("toggle", null, null, false, false).isEmpty())
    }

    @Test
    fun remotePositionAdvancesOnlyWhilePlaying() {
        val state = JSONObject().put("position_ms", 10_000).put("playing", true).put("duration_ms", 12_000)
        assertEquals(11_000L, remotePositionMs(state, 1_000, 2_000))
        assertEquals(12_000L, remotePositionMs(state, 1_000, 9_000))
        assertEquals(10_000L, remotePositionMs(state.put("playing", false), 1_000, 9_000))
    }

    @Test
    fun playlistMovesNameTheEntryTheyFollow() {
        val ids = listOf("a", "b", "c", "d")
        assertNull(PlaylistMoves.after(ids, 2, 0))
        assertEquals("a", PlaylistMoves.after(ids, 3, 1))
        assertEquals("d", PlaylistMoves.after(ids, 0, 3))
        assertEquals("c", PlaylistMoves.after(ids, 1, 2))
        assertNull(PlaylistMoves.after(ids, 5, 0))
    }

    @Test
    fun recommendationsKeepOnlineMediaOnce() {
        val reply =
            JSONObject(
                """{"enabled":true,"items":[
                {"media":{"url":"https://www.youtube.com/watch?v=a","title":"A"},"reason":"related","source":"x"},
                {"media":{"url":"https://www.youtube.com/watch?v=a","title":"A"},"reason":"similar_artist","source":"y"},
                {"media":{"url":"file:///b.mp3","title":"B"},"reason":"related","source":"x"},
                {"url":"https://www.youtube.com/watch?v=c","title":"C","reason":"subscription"}]}"""
            )
        assertEquals(listOf("A", "C"), RecItems.cards(reply).map { it.title })
    }

    @Test
    fun friendListsAndFiltersReadTheServerShapes() {
        val lists =
            FriendLists.of(
                JSONObject("""{"friends":[{"user_id":"1","username":"bob","since":1}],"incoming":[{"user_id":"2","username":"carol","at":1}],"outgoing":[]}""")
            )
        assertEquals("bob", lists.friends.single().optString("username"))
        assertEquals("carol", lists.incoming.single().optString("username"))
        assertTrue(lists.outgoing.isEmpty())
        val filters = listOf(JSONObject().put("id", "f1").put("kind", "shorts"), JSONObject().put("id", "f2").put("kind", "keyword").put("value", "spoiler"))
        assertEquals("f1", Filters.switched(filters, "shorts")?.optString("id"))
        assertNull(Filters.switched(filters, "live"))
        assertEquals(listOf("f2"), Filters.valued(filters).map { it.optString("id") })
        val card = MediaCard(JSONObject().put("creator", "Nova Lane").put("channel_id", "UCsyntheticchannel0000ab"))
        assertEquals("UCsyntheticchannel0000ab", Filters.channelValue(card))
        assertEquals("Nova Lane", Filters.channelValue(MediaCard(JSONObject().put("creator", " Nova Lane "))))
        assertNull(Filters.channelValue(MediaCard(JSONObject())))
    }

    @Test
    fun smallTextHelpers() {
        assertEquals("", SyncText.age(0, 10_000))
        assertEquals("now", SyncText.age(9_000, 10_000))
        assertEquals("5m", SyncText.age(0 + 1, 1 + 5 * 60_000))
        assertEquals("3h", SyncText.age(1, 1 + 3 * 3_600_000))
        assertEquals("2d", SyncText.age(1, 1 + 2 * 86_400_000L))
        assertEquals("1.5", SyncText.hours(5_400.0))
        assertEquals("12", SyncText.hours(43_300.0))
        assertEquals("95%", SyncText.percent(0.955))
        assertEquals("–", SyncText.percent(Double.NaN))
        assertEquals(60, StatsPeriods.tzOffsetMinutes(0, java.util.TimeZone.getTimeZone("GMT+01:00")))
        assertEquals(-840, StatsPeriods.tzOffsetMinutes(0, java.util.TimeZone.getTimeZone("GMT-15:00")))
        assertEquals(1_000L - 7 * 86_400_000L to 1_000L, StatsPeriods.range(7, 1_000))
    }

    @Test
    fun theWorkerWaitsForTheEngineWithinItsLimit() {
        var now = 0L
        val sleep: (Long) -> Unit = { now += it }
        assertTrue(SyncWorker.awaitEngine(1_000, { now >= 300 }, { false }, { now }, sleep))
        now = 0
        assertFalse("never ready", SyncWorker.awaitEngine(1_000, { false }, { false }, { now }, sleep))
        assertTrue(now >= 1_000)
        now = 0
        assertFalse("start failed", SyncWorker.awaitEngine(1_000, { false }, { true }, { now }, sleep))
        assertEquals(0L, now)
    }

    @Test
    fun wrappedPagesSkipEmptyHighlights() {
        val empty = WrappedPages.of(JSONObject("""{"year":2026,"seconds":0.0,"plays":0}"""))
        assertEquals(1, empty.size)
        val full =
            WrappedPages.of(
                JSONObject(
                    """{"year":2026,"seconds":7200.0,"plays":40,"top_creators":[{"creator":"Nova","seconds":3600.0,"thumbnail":"https://i.test/a.jpg"}],
                    "top_songs":[{"title":"Song","creator":"Nova","url":"https://music.youtube.com/watch?v=s"}],"top_videos":[],
                    "busiest_day":{"day":"2026-08-14","seconds":3600.0},"longest_streak_days":3,"music_share":0.5,"new_creators":2}"""
                )
            )
        assertEquals(listOf("hours", "top creator", "top song", "busiest day", "day streak", "music"), full.map { it.caption })
    }
}
