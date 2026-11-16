package dev.ytui.android

import org.junit.Assert.assertEquals
import org.junit.Test

class CardTextTest {
    @Test
    fun agesAreSpelledOut() {
        assertEquals("7 days ago", spelledAge("7d ago"))
        assertEquals("1 month ago", spelledAge("1mo ago"))
        assertEquals("3 weeks ago", spelledAge("3 weeks ago"))
        assertEquals("Streamed 2 hours ago", spelledAge("Streamed 2 hours ago"))
    }

    @Test
    fun unknownViewTextIsKept() {
        assertEquals("No views", compactViewsText("No views"))
        assertEquals("1.2K watching", compactViewsText("1.2K watching"))
    }
}
