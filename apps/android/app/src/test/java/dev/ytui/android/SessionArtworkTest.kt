package dev.ytui.android

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class SessionArtworkTest {
    private fun variant(url: String, width: Int, height: Int) =
        JSONObject().put("url", url).put("width", width).put("height", height)

    @Test
    fun notificationCoverPrefersTheLargestListedVariant() {
        val media =
            JSONObject()
                .put("url", "https://www.youtube.com/watch?v=abcdefghijk")
                .put("thumbnail", "https://i.ytimg.com/vi/abcdefghijk/default.jpg")
                .put(
                    "thumbnails",
                    JSONArray()
                        .put(variant("https://lh3.googleusercontent.com/small=w60-h60", 60, 60))
                        .put(variant("https://lh3.googleusercontent.com/large=w544-h544", 544, 544))
                        .put(variant("null", 2000, 2000)),
                )
        assertEquals(
            "https://lh3.googleusercontent.com/large=w544-h544",
            SessionArtwork.coverUri(media),
        )
    }

    @Test
    fun notificationCoverFallsBackToThumbnailThenStandardYoutubeImage() {
        assertEquals(
            "https://i.ytimg.com/vi/abcdefghijk/mqdefault.jpg",
            SessionArtwork.coverUri(
                JSONObject()
                    .put("url", "https://www.youtube.com/watch?v=abcdefghijk")
                    .put("thumbnail", "https://i.ytimg.com/vi/abcdefghijk/mqdefault.jpg"),
            ),
        )
        assertEquals(
            "https://i.ytimg.com/vi/abcdefghijk/hqdefault.jpg",
            SessionArtwork.coverUri(
                JSONObject()
                    .put("kind", "song")
                    .put("url", "https://music.youtube.com/watch?v=abcdefghijk")
                    .put("thumbnail", "null"),
            ),
        )
    }

    @Test
    fun itemsWithoutAnyCoverLeaveTheSessionArtworkEmpty() {
        assertNull(SessionArtwork.coverUri(null))
        assertNull(
            SessionArtwork.coverUri(
                JSONObject().put("kind", "station").put("url", "https://radio.invalid/stream"),
            ),
        )
    }
}
