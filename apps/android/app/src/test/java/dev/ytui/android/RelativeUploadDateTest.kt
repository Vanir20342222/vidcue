package dev.ytui.android

import java.time.LocalDate
import org.junit.Assert.assertEquals
import org.junit.Test

class RelativeUploadDateTest {
    private val today = LocalDate.of(2026, 11, 3)

    @Test
    fun uploadDatesReadAsAge() {
        assertEquals("Today", relativeUploadDate("20261103", today))
        assertEquals("1 day ago", relativeUploadDate("20261102", today))
        assertEquals("6 days ago", relativeUploadDate("20261028", today))
        assertEquals("2 weeks ago", relativeUploadDate("20261020", today))
        assertEquals("7 months ago", relativeUploadDate("20250326", today.minusYears(1)))
        assertEquals("1 year ago", relativeUploadDate("20251103", today))
        assertEquals("3 years ago", relativeUploadDate("20230101", today))
    }

    @Test
    fun futureOrInvalidDatesStayIso() {
        assertEquals("2026-12-01", relativeUploadDate("20261201", today))
        assertEquals("2026-13-40", relativeUploadDate("20261340", today))
    }
}
