package dev.ytui.android

import androidx.media3.common.C
import org.junit.Assert.*
import org.junit.Test

class LivePositionTest {
    @Test
    fun behindIsMeasuredFromTheLiveEdge() {
        assertEquals(45_000L, LivePosition.behindMs(3_600_000L, 3_555_000L))
        // Media3's position can briefly run past the edge it last reported.
        assertEquals(0L, LivePosition.behindMs(3_600_000L, 3_628_000L))
        assertEquals(0L, LivePosition.behindMs(C.TIME_UNSET, 10_000L))
        assertEquals(0L, LivePosition.behindMs(3_600_000L, -1L))
    }

    @Test
    fun theEdgeToleratesLiveSpeedControlButNotARewind() {
        assertTrue(LivePosition.atEdge(0L))
        assertTrue(LivePosition.atEdge(LivePosition.EDGE_TOLERANCE_MS))
        assertFalse(LivePosition.atEdge(5_000L))
    }

    @Test
    fun positionStaysOnTheWindowInsteadOfPassingItsDuration() {
        // The reported case: "1:00:28 / 1:00:00".
        assertEquals(3_600_000L, LivePosition.clamp(3_628_000L, 3_600_000L))
        assertEquals(1_000L, LivePosition.clamp(1_000L, 3_600_000L))
        assertEquals(0L, LivePosition.clamp(-5L, 3_600_000L))
        assertEquals(7_000L, LivePosition.clamp(7_000L, C.TIME_UNSET))
    }

    @Test
    fun behindLabelReadsAsTimeBehindLive() {
        assertEquals("", LivePosition.behindLabel(0L))
        assertEquals("", LivePosition.behindLabel(3_000L))
        assertEquals("−0:45", LivePosition.behindLabel(45_400L))
        assertEquals("−12:05", LivePosition.behindLabel(725_000L))
        assertEquals("−1:02:03", LivePosition.behindLabel(3_723_000L))
    }
}
