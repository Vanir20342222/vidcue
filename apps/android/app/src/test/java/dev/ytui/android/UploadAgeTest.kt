package dev.ytui.android

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** Cards show how long ago a video went up; the finer of the two sources wins. */
@RunWith(RobolectricTestRunner::class)
class UploadAgeTest {
    @Test
    fun providerTextIsSpelledOut() {
        assertEquals("1 hour ago", uploadAge(JSONObject().put("published_text", "1 hour ago")))
        assertEquals("5 years ago", uploadAge(JSONObject().put("published_text", "5y ago")))
    }

    @Test
    fun hoursBeatAnUploadDateOfToday() {
        val today = java.time.LocalDate.now().format(java.time.format.DateTimeFormatter.BASIC_ISO_DATE)
        val media = JSONObject().put("upload_date", today).put("published_text", "3 hours ago")
        assertEquals("3 hours ago", uploadAge(media))
        assertEquals("Today", uploadAge(JSONObject().put("upload_date", today)))
    }

    @Test
    fun olderUploadDatesAndMissingValues() {
        val old = java.time.LocalDate.now().minusYears(5).format(java.time.format.DateTimeFormatter.BASIC_ISO_DATE)
        assertEquals("5 years ago", uploadAge(JSONObject().put("upload_date", old).put("published_text", "5 years ago")))
        assertEquals("", uploadAge(JSONObject()))
        assertEquals("", uploadAge(JSONObject().put("published_text", "null")))
    }
}
