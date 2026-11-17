package dev.ytui.android

import org.junit.Assert.*
import org.junit.Test

class PageHistoryTest {
    @Test fun largePagesAreSizedOnceAndBackAndClearReleaseTheirBudget() {
        var scans = 0
        val history = PageHistory<String>(12, 2 * 1024 * 1024) { scans++; it.length.toLong() * 2 }
        repeat(30) { history.addLast("x".repeat(400_000)) }
        assertEquals(30, scans)
        assertEquals(400_000, history.removeLast().length)
        assertEquals(400_000, history.removeLast().length)
        assertTrue(history.isEmpty())
        history.addLast("y")
        history.clear()
        assertTrue(history.isEmpty())
        history.addLast("z")
        assertEquals("z", history.removeLast())
        assertEquals(32, scans)
    }
}
