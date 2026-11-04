package dev.ytui.android

import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(manifest = Config.NONE)
class PlaybackContextsTest {
    private fun card(id: String, kind: String = "song", live: Boolean = false) =
        MediaCard(json("id" to id, "url" to "https://media.test/$id", "kind" to kind, "live" to live))

    @Test
    fun randomMusicSongBuildsRecommendationsWithoutNeedingAnOpenVideoPage() {
        val song = card("aaaaaaaaaaa")
        val context = PlaybackContexts.create(song, FeedState(), 2, true)!!
        assertEquals("prepare_context", context.getString("op"))
        assertEquals("aaaaaaaaaaa", context.getJSONObject("request").getString("Related"))
        assertTrue(context.getBoolean("music"))
        assertNull(PlaybackContexts.create(song, FeedState(), 2, false))
        // Replaying a saved song from a different tab must still use WEB_REMIX.
        assertTrue(PlaybackContexts.create(song, FeedState(), 4, true)!!.getBoolean("music"))
    }

    @Test
    fun randomStationKeepsItsListAndWrapsFromTheLastRowWithoutAutoplayRelated() {
        val first = card("first", "station", true)
        val selected = card("last", "station", true)
        val feed = FeedState(cards = listOf(first, card("album", "album"), selected, first))
        val context = PlaybackContexts.create(selected, feed, 3, false)!!
        assertEquals("radio_context", context.getString("op"))
        assertEquals(
            listOf(selected.url, first.url),
            context.getJSONArray("items").objects().map { it.getString("url") },
        )
        assertFalse(context.has("request"))
        assertNull(PlaybackContexts.create(card("unknown", "station", true), feed, 3, true))
    }

    @Test
    fun finiteRadioTrackLibraryStillLoadsItsFullCollection() {
        val song = card("track")
        val request = json("CliampTracks" to "https://radio.cliamp.stream/library")
        val context =
            PlaybackContexts.create(
                song, FeedState(request = request, cards = listOf(song)), 3, false,
            )!!
        assertEquals("prepare_context", context.getString("op"))
        assertEquals(request.toString(), context.getJSONObject("request").toString())
    }

    @Test
    fun directRadioSongsUseTheVisibleTracksAndVideoRecommendationsStayVideo() {
        val a = card("a")
        val b = card("b")
        val context = PlaybackContexts.create(a, FeedState(cards = listOf(a, b)), 3, true)!!
        assertEquals("radio_context", context.getString("op"))
        assertEquals(b.url, context.getJSONArray("items").getJSONObject(1).getString("url"))
        val video = PlaybackContexts.create(card("bbbbbbbbbbb", "video"), FeedState(), 1, true)!!
        assertFalse(video.getBoolean("music"))
        assertEquals("bbbbbbbbbbb", video.getJSONObject("request").getString("Related"))
        assertNull(PlaybackContexts.create(card("live", "video", true), FeedState(), 1, true))
    }
}
