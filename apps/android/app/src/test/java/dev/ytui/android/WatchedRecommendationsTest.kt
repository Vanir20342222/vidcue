package dev.ytui.android

import androidx.lifecycle.ViewModelStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.json.JSONArray
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(manifest = Config.NONE, sdk = [35])
@OptIn(ExperimentalCoroutinesApi::class)
class WatchedRecommendationsTest {
    private fun card(id: String, url: String = "https://www.youtube.com/watch?v=$id") =
        MediaCard(json("id" to id, "url" to url, "kind" to "video"))

    @Test
    fun urlVariantsMatchEvenWithoutIdsAndOnlyDiscoveryFeedsHideThem() {
        val watched = card("", "https://music.youtube.com/watch?v=abcdefghijk&list=example")
        for (url in listOf(
            "https://youtu.be/abcdefghijk?t=15",
            "https://m.youtube.com/watch?list=example&v=abcdefghijk",
            "https://www.youtube.com/shorts/abcdefghijk",
        )) {
            val repeated = card("", url)
            for (request in listOf("Home", json("Category" to json()), json("Related" to "source"))) {
                val result = WatchedRecommendations.filter(FeedState(request = request, cards = listOf(repeated)), listOf(watched))
                assertTrue(result.cards.isEmpty())
            }
            for (request in listOf("History", "Liked", "WatchLater", "Favorites", json("Search" to "video"), json("Collection" to "playlist"))) {
                assertEquals(listOf(repeated), WatchedRecommendations.filter(FeedState(request = request, cards = listOf(repeated)), listOf(watched)).cards)
            }
        }
        val live = MediaCard(watched.value.copyJson().put("live", true))
        val collection = MediaCard(watched.value.copyJson().put("kind", "playlist"))
        assertEquals(listOf(live, collection), WatchedRecommendations.filter(listOf(live, collection), listOf(watched)))
    }

    @Test
    fun watchedVideosStayOutOfFreshCachedPaginatedAndRestoredRecommendations() {
        val dispatcher = StandardTestDispatcher()
        Dispatchers.setMain(dispatcher)
        val previousContext = MediaBridge.context
        MediaBridge.context = RuntimeEnvironment.getApplication()
        MobileSettings.prefs().edit().putBoolean("preload_next", false).commit()
        val watched = card("abcdefghijk", "https://youtu.be/abcdefghijk?t=30")
        val repeated = card("abcdefghijk")
        val unseen = card("lmnopqrstuv")
        var recorded = emptyList<MediaCard>()
        val model = YtuiViewModel(readHistory = { recorded }, browsePage = { _, _, _ ->
            json("items" to JSONArray(listOf(repeated.value, unseen.value)), "continuation" to "next")
        })
        val store = ViewModelStore().apply { put("recommendations", model) }
        try {
            model.navigate(1, "Home")
            dispatcher.scheduler.runCurrent()
            assertEquals(2, model.feeds[1].value.cards.size)
            recorded = listOf(watched)
            model.loadHistory()
            dispatcher.scheduler.runCurrent()
            assertEquals(listOf(unseen.url), model.feeds[1].value.cards.map { it.url })
            model.more()
            dispatcher.scheduler.runCurrent()
            assertEquals(listOf(unseen.url), model.feeds[1].value.cards.map { it.url })
            model.navigate(1, json("Search" to "example"))
            dispatcher.scheduler.runCurrent()
            assertEquals(2, model.feeds[1].value.cards.size)
            assertTrue(model.back())
            assertEquals(listOf(unseen.url), model.feeds[1].value.cards.map { it.url })
            model.navigate(1, "Home")
            dispatcher.scheduler.runCurrent()
            assertEquals(listOf(unseen.url), model.feeds[1].value.cards.map { it.url })
            model.navigate(1, json("Collection" to "playlist"))
            dispatcher.scheduler.runCurrent()
            assertEquals(2, model.feeds[1].value.cards.size)
            recorded = emptyList()
            model.loadHistory()
            dispatcher.scheduler.runCurrent()
            model.navigate(1, "Home")
            dispatcher.scheduler.runCurrent()
            assertEquals(2, model.feeds[1].value.cards.size)
        } finally {
            store.clear()
            dispatcher.scheduler.runCurrent()
            MediaBridge.context = previousContext
            Dispatchers.resetMain()
        }
    }
}
