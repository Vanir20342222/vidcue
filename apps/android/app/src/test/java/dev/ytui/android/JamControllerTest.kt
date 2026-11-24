package dev.ytui.android

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class JamControllerTest {
    private val media = JSONObject().put("url", "https://music.youtube.com/watch?v=synthetic01").put("kind", "song")

    private fun target(action: String, key: String = "youtube:synthetic01", playing: Boolean = true) =
        JamTarget(true, action, key, media, playing, targetMs = 51_234, seekMs = 51_384)

    private fun player(key: String? = "youtube:synthetic01", playing: Boolean = true, busy: Boolean = false) =
        JamPlayerView(key, playing, busy)

    @Test
    fun olderPlaybackVersionsNeverReplaceNewerOnes() {
        val v10 = JSONObject().put("version", 10)
        assertTrue(JamSteps.newer(null, v10))
        assertTrue(JamSteps.newer(v10, JSONObject().put("version", 11)))
        assertTrue("same version: idempotent", JamSteps.newer(v10, JSONObject().put("version", 10)))
        assertFalse(JamSteps.newer(JSONObject().put("version", 11), v10))
    }

    @Test
    fun userTransportChangesAreToldApartFromOurOwnSteps() {
        val key = "youtube:synthetic01"
        val playing = player(playing = true)
        val paused = player(playing = false)
        // The user paused the room's item from the lock screen.
        assertEquals("pause", JamSteps.userChange(playing, paused, key, roomUnchanged = true, settling = false))
        assertEquals("play", JamSteps.userChange(paused, playing, key, roomUnchanged = true, settling = false))
        // Another video was started.
        assertEquals("media", JamSteps.userChange(playing, player(key = "youtube:other"), key, true, false))
        // Not the user: the room changed, our own step is settling, or a start is running.
        assertNull(JamSteps.userChange(playing, paused, key, roomUnchanged = false, settling = false))
        assertNull(JamSteps.userChange(playing, paused, key, roomUnchanged = true, settling = true))
        assertNull(JamSteps.userChange(playing, player(playing = false, busy = true), key, true, false))
        // Before the first tick or while the local player was on another item, nothing counts.
        assertNull(JamSteps.userChange(null, paused, key, true, false))
        assertNull(JamSteps.userChange(player(key = "youtube:other"), paused, key, true, false))
    }

    @Test
    fun theRoomRateIsRead() {
        assertEquals(1.5, JamTarget.from(JSONObject().put("rate", 1.5)).rate, 0.0)
        assertEquals(1.0, JamTarget.from(JSONObject()).rate, 0.0)
        assertEquals(2.0, JamTarget.from(JSONObject().put("rate", 9.0)).rate, 0.0)
    }

    @Test
    fun readsTheEngineTargetReply() {
        val reply =
            JSONObject("""{"in_room":true,"media":{"url":"https://x.test/v"},"media_key":"youtube:abc","playing":true,"target_ms":51234,"action":"seek","seek_ms":51384}""")
        val parsed = JamTarget.from(reply)
        assertTrue(parsed.inRoom)
        assertEquals("seek", parsed.action)
        assertEquals("youtube:abc", parsed.mediaKey)
        assertEquals(51_234L, parsed.targetMs)
        assertEquals(51_384L, parsed.seekMs)
        assertEquals("none", JamTarget.from(JSONObject()).action)
        assertFalse(JamTarget.from(JSONObject()).inRoom)
    }

    @Test
    fun loadStartsTheRoomItemAtTheTargetOnce() {
        val load = JamSteps.decide(target("load"), player(key = "youtube:other"), null, 10_000)
        assertEquals(JamStep.Load(media, 51_234, true), load)
        // While that start may still be running, the same load is not issued again...
        assertNull(JamSteps.decide(target("load"), player(key = "youtube:other"), "youtube:synthetic01" to 10_000L, 15_000))
        // ...but it is retried once the grace period has passed, and a different item loads at once.
        assertEquals(load, JamSteps.decide(target("load"), player(key = "youtube:other"), "youtube:synthetic01" to 10_000L, 10_000 + JamSteps.LOAD_GRACE_MS))
        assertTrue(JamSteps.decide(target("load"), player(key = null), "youtube:previous" to 10_000L, 11_000) is JamStep.Load)
    }

    @Test
    fun loadNeverStartsLocalOrMissingMedia() {
        val local = target("load").copy(media = JSONObject().put("url", "file:///sdcard/song.mp3"))
        assertNull(JamSteps.decide(local, player(key = null), null, 0))
        assertNull(JamSteps.decide(target("load").copy(media = null), player(key = null), null, 0))
    }

    @Test
    fun playAndPauseOnlyToggleTowardsTheRoomState() {
        assertEquals(JamStep.Toggle, JamSteps.decide(target("play"), player(playing = false), null, 0))
        assertNull("already playing", JamSteps.decide(target("play"), player(playing = true), null, 0))
        assertNull("other item: a load comes first", JamSteps.decide(target("play"), player(key = "youtube:other", playing = false), null, 0))
        assertEquals(JamStep.Toggle, JamSteps.decide(target("pause", playing = false), player(playing = true), null, 0))
        assertNull("already paused", JamSteps.decide(target("pause", playing = false), player(playing = false), null, 0))
    }

    @Test
    fun seekUsesTheEngineLeadAndOnlyOnTheSameItem() {
        assertEquals(JamStep.Seek(51_384), JamSteps.decide(target("seek"), player(), null, 0))
        assertNull(JamSteps.decide(target("seek"), player(key = "youtube:other"), null, 0))
        assertNull(JamSteps.decide(target("none"), player(), null, 0))
    }

    @Test
    fun nothingHappensWhileAStartIsInFlightOrOutsideARoom() {
        assertNull(JamSteps.decide(target("seek"), player(busy = true), null, 0))
        assertNull(JamSteps.decide(target("load"), player(busy = true), null, 0))
        assertNull(JamSteps.decide(target("seek").copy(inRoom = false), player(), null, 0))
    }

    @Test
    fun theEndIsReachedWithinTheEndWindow() {
        assertFalse(JamSteps.finished(100_000, 0))
        assertFalse(JamSteps.finished(198_000, 200_000))
        assertTrue(JamSteps.finished(198_500, 200_000))
        assertTrue(JamSteps.finished(200_000, 200_000))
    }

    @Test
    fun guestsSteerOnlyWhenTheHostAllowsIt() {
        assertFalse(JamSteps.canControl(null))
        assertTrue(JamSteps.canControl(JSONObject().put("role", "host")))
        assertFalse(JamSteps.canControl(JSONObject().put("role", "guest")))
        assertTrue(JamSteps.canControl(JSONObject().put("role", "guest").put("guests_can_control", true)))
    }
}
