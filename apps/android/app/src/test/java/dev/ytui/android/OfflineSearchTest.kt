package dev.ytui.android

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class OfflineSearchTest {
    private fun copy(title: String, creator: String, quality: String = "1080p") =
        JSONObject()
            .put("id", title.hashCode().toString())
            .put("title", title)
            .put("quality", quality)
            .put("state", "Complete")
            .put("media", JSONObject().put("title", title).put("creator", creator).put("url", "https://media.invalid/$title"))

    @Test
    fun everyWordMustMatchTitleCreatorOrQuality() {
        val song = copy("Café del Mar", "Synthetic Artist", "audio")
        assertTrue(offlineMatches(song, ""))
        assertTrue(offlineMatches(song, "  "))
        assertTrue(offlineMatches(song, "cafe"))
        assertTrue(offlineMatches(song, "MAR synthetic"))
        assertTrue(offlineMatches(song, "audio"))
        assertFalse(offlineMatches(song, "cafe jazz"))
    }

    @Test
    fun playlistsMatchOnNameOrMembers() {
        val playlist =
            JSONObject()
                .put("title", "Road trip")
                .put("items", JSONArray().put(JSONObject().put("title", "Night drive").put("creator", "Synthetic Band")))
        assertTrue(offlinePlaylistMatches(playlist, "road"))
        assertTrue(offlinePlaylistMatches(playlist, "synthetic band"))
        assertFalse(offlinePlaylistMatches(playlist, "morning"))
    }

    @Test
    fun downloadLookupFindsTheNewestTransferForTheSavedItem() {
        val card = MediaCard(JSONObject().put("title", "Clip").put("url", "https://media.invalid/clip"))
        val older = JSONObject().put("id", "a").put("state", "Failed").put("media", JSONObject().put("url", "https://media.invalid/clip"))
        val newer = JSONObject().put("id", "b").put("state", "Downloading").put("media", JSONObject().put("url", "https://media.invalid/clip"))
        val other = JSONObject().put("id", "c").put("media", JSONObject().put("url", "https://media.invalid/other"))
        assertEquals("b", downloadFor(listOf(older, other, newer), card)?.optString("id"))
        assertNull(downloadFor(listOf(other), card))
    }

    @Test
    fun subscriberCountIsShortAndOptional() {
        assertEquals("", subscriberLabel(JSONObject()))
        assertEquals("", subscriberLabel(null))
        assertEquals("1 subscriber", subscriberLabel(JSONObject().put("channel_follower_count", 1)))
        assertTrue(subscriberLabel(JSONObject().put("channel_follower_count", 1_250_000)).endsWith("subscribers"))
    }

    @Test
    fun descriptionPreviewDropsLongBlankRuns() {
        assertEquals("One\n\nTwo", descriptionPreview("\n One\n\n\n\n\nTwo \n"))
    }
}
