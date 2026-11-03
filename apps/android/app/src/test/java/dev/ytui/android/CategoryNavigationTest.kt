package dev.ytui.android

import androidx.lifecycle.ViewModelStore
import kotlinx.coroutines.*
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Offline ViewModel integration. No native requests, cookies or public services. */
@RunWith(RobolectricTestRunner::class)
@Config(manifest = Config.NONE, sdk = [35])
@OptIn(ExperimentalCoroutinesApi::class)
class CategoryNavigationTest {
    private fun request(name: String) = json("Category" to json("endpoint" to name))

    private fun page(title: String) =
        json(
            "title" to title,
            "items" to
                JSONArray().put(json("url" to "https://example.invalid/$title", "title" to title)),
        )

    private fun checkNavigation(test: suspend () -> Unit) = runBlocking {
        Dispatchers.setMain(Dispatchers.Unconfined)
        try {
            val offline = MediaBridge.offlineUi
            MediaBridge.offlineUi = true
            try {
                test()
            } finally {
                MediaBridge.offlineUi = offline
            }
        } finally {
            Dispatchers.resetMain()
        }
    }

    private suspend fun settled(model: YtuiViewModel) {
        withTimeout(5_000) { while (model.feeds[1].value.loading) delay(1) }
    }

    @Test
    fun cachedCategoryAndRepeatedChipTapPublishCardsSynchronously() = checkNavigation {
        var calls = 0
        val model =
            YtuiViewModel(
                browsePage = { _, request, _ ->
                    calls++
                    page(request.toString())
                }
            )
        val store = ViewModelStore().apply { put("categories", model) }
        try {
            model.navigate(request = request("A"))
            settled(model)
            model.navigate(request = request("B"))
            settled(model)
            val before = calls
            // Queue all new coroutines: a cache hit must still publish immediately.
            Dispatchers.setMain(StandardTestDispatcher())
            model.navigate(request = request("A"))
            // No yield or Compose idle: these must already hold in the tap's call stack.
            assertFalse(model.feeds[1].value.loading)
            assertEquals(request("A").toString(), model.feeds[1].value.cards.single().title)
            assertTrue(model.feeds[1].value.instantContent)
            model.navigate(request = request("A"))
            assertFalse(model.feeds[1].value.loading)
            assertEquals(before, calls)
        } finally {
            store.clear()
        }
    }

    @Test
    fun lateReplyCannotReplaceTheMostRecentlySelectedCategory() = checkNavigation {
        val response = CompletableDeferred<JSONObject>()
        val model =
            YtuiViewModel(
                browsePage = { _, target, _ ->
                    if (target.toString() == request("A").toString()) response.await()
                    else page("B")
                }
            )
        val store = ViewModelStore().apply { put("categories", model) }
        try {
            model.navigate(request = request("A"))
            model.navigate(request = request("B"))
            settled(model)
            response.complete(page("A"))
            yield()
            assertEquals("B", model.feeds[1].value.cards.single().title)
            model.navigate(request = request("A"))
            assertEquals("A", model.feeds[1].value.cards.single().title)
            assertFalse(model.feeds[1].value.loading)
        } finally {
            store.clear()
        }
    }

    @Test
    fun staleCategoryRemainsVisibleUntilRefreshCompletes() = checkNavigation {
        var now = 0L
        var refresh = false
        val response = CompletableDeferred<JSONObject>()
        val model =
            YtuiViewModel(
                browsePage = { _, _, _ ->
                    if (refresh) response.await() else page("previous cards")
                },
                clock = { now },
            )
        val store = ViewModelStore().apply { put("categories", model) }
        try {
            model.navigate(request = request("A"))
            settled(model)
            model.navigate(request = request("B"))
            settled(model)
            now = 60_000
            refresh = true
            model.navigate(request = request("A"))
            assertEquals("previous cards", model.feeds[1].value.cards.single().title)
            assertFalse(model.feeds[1].value.loading)
            response.complete(page("fresh cards"))
            withTimeout(5_000) {
                while (model.feeds[1].value.cards.single().title != "fresh cards") delay(1)
            }
        } finally {
            store.clear()
        }
    }

    @Test
    fun explicitRefreshFetchesAgainAndPaginationBypassesFirstPageCache() = checkNavigation {
        val offsets = mutableListOf<Int>()
        val model =
            YtuiViewModel(
                browsePage = { _, _, offset ->
                    offsets.add(offset)
                    page("page ${offsets.size}").put("continuation", "next")
                }
            )
        val store = ViewModelStore().apply { put("categories", model) }
        try {
            model.navigate(request = request("A"))
            settled(model)
            model.navigate(request = request("A"), remember = false)
            settled(model)
            assertEquals("page 2", model.feeds[1].value.cards.single().title)
            model.navigate(request = json("Continue" to "next"), append = true)
            settled(model)
            assertEquals(listOf(0, 0, 1), offsets)
            assertEquals(listOf("page 2", "page 3"), model.feeds[1].value.cards.map { it.title })
        } finally {
            store.clear()
        }
    }
}
