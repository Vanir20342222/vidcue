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
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.Implementation
import org.robolectric.annotation.Implements

@RunWith(RobolectricTestRunner::class)
@Config(manifest = Config.NONE, sdk = [35], shadows = [FeedStartupNative::class])
@OptIn(ExperimentalCoroutinesApi::class)
class FeedStartupTest {
    @Test
    fun tabSelectedWhileSettingsLoadIsFetchedWhenInitializationFinishes() = runBlocking {
        // The application normally publishes this after native/proxy initialization.
        EngineStartup.current.record("")
        val dispatcher = StandardTestDispatcher()
        Dispatchers.setMain(dispatcher)
        val previousContext = MediaBridge.context
        val offline = MediaBridge.offlineUi
        MediaBridge.context = RuntimeEnvironment.getApplication()
        MediaBridge.offlineUi = false
        MobileSettings.prefs().edit().putBoolean("preload_next", false).commit()
        val calls = mutableListOf<Pair<Int, Any>>()
        val themeNames = CompletableDeferred<List<String>>()
        val model =
            YtuiViewModel(browsePage = { tab, request, _ ->
                calls.add(tab to request)
                json(
                    "items" to JSONArray().put(
                        json("title" to "music", "url" to "https://example.invalid/music")
                    )
                )
            }, readThemes = { themeNames.await() })
        val store = ViewModelStore().apply { put("startup", model) }
        try {
            model.initialize(false)
            model.chooseTab(2)
            assertFalse(model.ready.value)
            assertTrue(calls.isEmpty())
            withTimeout(5_000) {
                while (!model.ready.value || model.feeds[2].value.cards.isEmpty()) {
                    dispatcher.scheduler.runCurrent()
                    delay(1)
                }
            }
            model.initialize(false)
            dispatcher.scheduler.runCurrent()
            assertEquals(listOf(2 to "Home"), calls)
            assertEquals(2, model.tab.value)
            assertEquals("music", model.feeds[2].value.cards.single().title)
            assertEquals("Ready", model.status.value)
            assertTrue("First content must not wait for the theme picker", model.themes.value.isEmpty())
            themeNames.complete(listOf("system", "dark"))
            dispatcher.scheduler.runCurrent()
            assertEquals(listOf("system", "dark"), model.themes.value)
        } finally {
            store.clear()
            dispatcher.scheduler.runCurrent()
            MediaBridge.context = previousContext
            MediaBridge.offlineUi = offline
            Dispatchers.resetMain()
        }
    }
}

/** Synthetic initialization replies; never loads JNI or a real account. */
@Implements(NativeBridge::class)
class FeedStartupNative {
    companion object {
        @JvmStatic
        @Implementation
        fun __staticInitializer__() {}

        @JvmStatic
        @Implementation
        fun request(payload: String): String {
            val data =
                if (JSONObject(payload).getString("op") == "themes") JSONArray() else JSONObject()
            return json("ok" to true, "data" to data).toString()
        }
    }
}
