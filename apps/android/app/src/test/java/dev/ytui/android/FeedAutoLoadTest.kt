package dev.ytui.android

import androidx.lifecycle.ViewModelStore
import kotlinx.coroutines.*
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Exercises tab selection against the real ViewModel/cache, with offline provider responses. */
@RunWith(RobolectricTestRunner::class)
@Config(manifest = Config.NONE, sdk = [35])
@OptIn(ExperimentalCoroutinesApi::class)
class FeedAutoLoadTest {
    private fun page(title: String) =
        json(
            "title" to title,
            "items" to
                JSONArray().put(json("url" to "https://example.invalid/$title", "title" to title)),
        )

    private fun withModel(
        browse: suspend (Int, Any, Int) -> JSONObject,
        test: suspend (YtuiViewModel) -> Unit,
    ) = runBlocking {
        Dispatchers.setMain(Dispatchers.Unconfined)
        val offline = MediaBridge.offlineUi
        MediaBridge.offlineUi = false
        val model = YtuiViewModel(browsePage = browse)
        // Disable speculative requests so assertions cover visible-tab loads only.
        model.mobile.value = json("preload_next" to false)
        model.ready.value = true
        val store = ViewModelStore().apply { put("feeds", model) }
        try {
            test(model)
        } finally {
            store.clear()
            MediaBridge.offlineUi = offline
            Dispatchers.resetMain()
        }
    }

    private suspend fun settled(model: YtuiViewModel, tab: Int) {
        withTimeout(5_000) { while (model.feeds[tab].value.loading) delay(1) }
    }

    @Test
    fun openingEachFeedLoadsItsDefaultSourceOnce() {
        val calls = mutableListOf<Pair<Int, Any>>()
        withModel({ tab, request, _ ->
            calls.add(tab to request)
            page("feed $tab")
        }) { model ->
            for (tab in 1..3) {
                model.chooseTab(tab)
                settled(model, tab)
                assertEquals("feed $tab", model.feeds[tab].value.cards.single().title)
            }
            for (tab in 3 downTo 1) model.chooseTab(tab)
            assertEquals(listOf(1 to "Home", 2 to "Home", 3 to "Cliamp"), calls)
        }
    }

    @Test
    fun accountInvalidationMakesBothVideoAndMusicLoadAgainOnSelection() {
        var calls = 0
        withModel({ tab, _, _ -> page("$tab load ${++calls}") }) { model ->
            model.chooseTab(1)
            settled(model, 1)
            model.chooseTab(2)
            settled(model, 2)
            // This is the shared invalidation used by sign-in, disconnect and network edits.
            YtuiViewModel::class.java.getDeclaredMethod("invalidateAccount").apply {
                isAccessible = true
                invoke(model)
            }
            for (tab in 1..2) {
                assertTrue(model.feeds[tab].value.generation > 0)
                assertTrue(model.feeds[tab].value.cards.isEmpty())
                model.chooseTab(tab)
                settled(model, tab)
                assertEquals("$tab load ${tab + 2}", model.feeds[tab].value.cards.single().title)
            }
            assertEquals(4, calls)
        }
    }

    @Test
    fun switchingAwayAndBackDuringLoadingDoesNotRestartTheRequest() {
        val response = CompletableDeferred<JSONObject>()
        var calls = 0
        withModel({ _, _, _ ->
            calls++
            response.await()
        }) { model ->
            model.chooseTab(2)
            model.chooseTab(0)
            model.chooseTab(2)
            assertTrue(model.feeds[2].value.loading)
            assertEquals(1, calls)
            response.complete(page("music"))
            settled(model, 2)
            assertEquals("music", model.feeds[2].value.cards.single().title)
        }
    }

    @Test
    fun validEmptyFeedDoesNotReloadEveryTimeItIsSelected() {
        var calls = 0
        withModel({ _, _, _ ->
            calls++
            json("items" to JSONArray())
        }) { model ->
            model.chooseTab(2)
            settled(model, 2)
            model.chooseTab(0)
            model.chooseTab(2)
            assertTrue(model.feeds[2].value.cards.isEmpty())
            assertFalse(model.feeds[2].value.loading)
            assertEquals(1, calls)
        }
    }

    @Test
    fun selectionWaitsForSettingsAndOfflineUiNeverFetches() {
        var calls = 0
        withModel({ _, _, _ ->
            calls++
            page("music")
        }) { model ->
            model.ready.value = false
            model.chooseTab(2)
            assertEquals(2, model.tab.value)
            assertEquals(0, calls)
            model.ready.value = true
            MediaBridge.offlineUi = true
            model.chooseTab(2)
            assertEquals(0, calls)
            MediaBridge.offlineUi = false
            model.chooseTab(2)
            settled(model, 2)
            assertEquals(1, calls)
        }
    }
}
