package dev.ytui.android

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class SleepTimerUiTest {
    @Test
    fun countdownIsLeftFromThePublishedDeadline() {
        assertEquals(0L, sleepLeftAt(JSONObject(), 5_000))
        assertEquals(0L, sleepLeftAt(JSONObject().put("sleep_deadline", 0), 5_000))
        val running = JSONObject().put("sleep_deadline", 65_000).put("sleep_total", 60_000)
        assertEquals(60_000L, sleepLeftAt(running, 5_000))
        assertEquals(0L, sleepLeftAt(running, 90_000))
    }

    @Test
    fun endOfTrackFollowsTheTracksRemainingTimeAndSpeed() {
        val status =
            JSONObject()
                .put("sleep_end_of_track", true)
                .put("sleep_deadline", 0)
                .put("duration", 300.0)
                .put("position", 120.0)
                .put("speed", 1.0)
        assertEquals(180_000L, sleepLeftAt(status, 5_000))
        // The wall clock does not matter: a paused track keeps its remaining time.
        assertEquals(180_000L, sleepLeftAt(status, 50_000_000))
        status.put("speed", 2.0)
        assertEquals(90_000L, sleepLeftAt(status, 5_000))
        status.put("speed", 1.0).put("position", 290.0)
        assertEquals(10_000L, sleepLeftAt(status, 5_000))
        // Still shown as running until the item really ends.
        status.put("position", 300.0)
        assertEquals(1L, sleepLeftAt(status, 5_000))
        status.put("duration", 0.0).put("position", 0.0)
        assertEquals(1L, sleepLeftAt(status, 5_000))
    }

    @Test
    fun endOfTrackShowsTheShareOfTheTrackLeft() {
        val state =
            PlayerState(
                JSONObject()
                    .put("sleep_end_of_track", true)
                    .put("duration", 200.0)
                    .put("position", 50.0)
            )
        assertEquals(0.75f, sleepFraction(state, sleepLeftAt(state.value, 0)), 0.0001f)
        val countdown = PlayerState(JSONObject().put("sleep_deadline", 1).put("sleep_total", 60_000))
        assertEquals(0.5f, sleepFraction(countdown, 30_000), 0.0001f)
        assertEquals(0f, sleepFraction(countdown, 0), 0f)
    }
}
