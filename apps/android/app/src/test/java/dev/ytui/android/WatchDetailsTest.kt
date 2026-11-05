package dev.ytui.android

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class WatchDetailsTest {
    private val details =
        JSONObject()
            .put("published_text", "3 days ago")
            .put("like_count", 1234)
            .put("subscribers_text", "1.23M subscribers")

    @Test
    fun detailsFillOnlyWhatTheStreamLacks() {
        val media = JSONObject().put("url", "https://media.invalid/v").put("view_count", 10)
        val merged = withWatchDetails(media, details)!!
        assertEquals("3 days ago", merged.optString("published_text"))
        assertEquals(1234L, merged.optLong("like_count"))
        assertEquals("1.23M subscribers", subscriberLabel(merged))
        // The original is not modified.
        assertFalse(media.has("published_text"))
    }

    @Test
    fun streamValuesWin() {
        val media = JSONObject().put("upload_date", "20261101").put("like_count", 5)
        val merged = withWatchDetails(media, details)!!
        assertFalse(merged.has("published_text"))
        assertEquals(5L, merged.optLong("like_count"))
    }

    @Test
    fun noDetailsLeavesTheMediaAlone() {
        val media = JSONObject().put("title", "Clip")
        assertSame(media, withWatchDetails(media, null))
        assertSame(media, withWatchDetails(media, JSONObject()))
        assertNull(withWatchDetails(null, details))
        assertEquals("", subscriberLabel(media))
    }
}
