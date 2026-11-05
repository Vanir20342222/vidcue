package dev.ytui.android

import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(manifest = Config.NONE)
class OfflineSelectionTest {
    private fun download(id: String, url: String) =
        json("id" to id, "media" to json("url" to url, "title" to id, "kind" to "song"))

    @Test
    fun selectedCopiesRetainOrderAndOfflineIdsWithoutMutatingDownloads() {
        val first = download("first-copy", "https://media.test/a")
        val second = download("second-copy", "https://media.test/b")
        val members = offlinePlaylistMembers(listOf(second, first))
        assertEquals(listOf("second-copy", "first-copy"), members.map { it.getString("offline_id") })
        assertFalse(first.getJSONObject("media").has("offline_id"))
        assertFalse(second.getJSONObject("media").has("offline_id"))
    }

    @Test
    fun addingSeveralSongsSkipsExistingUrlsAndAlternateCopies() {
        val existing = json("url" to "https://media.test/a", "offline_id" to "original")
        val downloads = listOf(
            download("alternate-a", "https://media.test/a"),
            download("b-audio", "https://media.test/b"),
            download("b-video", "https://media.test/b"),
            download("c", "https://media.test/c"),
        )
        val merged = mergeOfflinePlaylistMembers(listOf(existing), downloads)
        assertEquals(listOf("original", "b-audio", "c"), merged.map { it.getString("offline_id") })
        assertSame(existing, merged.first())
        assertEquals(merged, mergeOfflinePlaylistMembers(merged, downloads))
        assertTrue(offlinePlaylistMembers(emptyList()).isEmpty())
    }
}
