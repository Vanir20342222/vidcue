package dev.ytui.android

import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createEmptyComposeRule
import androidx.lifecycle.ViewModelProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import org.json.JSONObject
import org.junit.*
import org.junit.runner.RunWith

/** Intentionally blocked extraction checks UI ownership; these are not latency benchmarks. */
@RunWith(AndroidJUnit4::class)
@OptIn(androidx.compose.ui.test.ExperimentalTestApi::class)
@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
class PreparationUiTest {
    @get:Rule
    val compose = createEmptyComposeRule(effectContext = kotlinx.coroutines.Dispatchers.Main)
    @get:Rule
    val notifications =
        androidx.test.rule.GrantPermissionRule.grant(
            *if (android.os.Build.VERSION.SDK_INT >= 33)
                arrayOf("android.permission.POST_NOTIFICATIONS")
            else emptyArray()
        )
    private lateinit var original: JSONObject

    @Before
    fun setup() {
        original = MobileSettings.snapshot()
        MobileSettings.save(
            json("preload_next" to false, "autoplay_related" to false, "prefer_offline" to false)
        )
    }

    @After
    fun restore() {
        MediaRuntimeTest.command("stop")
        MobileSettings.save(original)
    }

    @Test
    fun surfacePreparesDuringResolutionAndRapidSelectionKeepsTheLatestIntent() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val file = File(instrumentation.targetContext.cacheDir, "early-surface-fixture.mp4")
        instrumentation.context.assets.open("fixture.mp4").use { input ->
            file.outputStream().use { input.copyTo(it) }
        }
        val gate = CountDownLatch(1)
        val occupied = CountDownLatch(2)
        val field =
            RustRpc::class.java.getDeclaredField("playbackWorkers").apply { isAccessible = true }
        val workers = field.get(RustRpc) as ThreadPoolExecutor
        val blockers = mutableListOf<java.util.concurrent.Future<*>>()
        try {
            MediaRuntimeTest.FixtureServer(file).use { server ->
                RuntimeTest.launchOffline().use { scene ->
                    val vm = AtomicReference<YtuiViewModel>()
                    scene.onActivity { vm.set(ViewModelProvider(it)[YtuiViewModel::class.java]) }
                    compose.waitUntil(10000) { vm.get().ready.value }
                    MediaRuntimeTest.await(
                        "Foreground player should be initialized before selection"
                    ) {
                        PlaybackService.warmReady()
                    }
                    compose.waitUntil(10000) { RustActivity.current()?.videoViews?.ready() == true }
                    val idleView = AtomicReference<androidx.media3.ui.PlayerView>()
                    scene.onActivity { idleView.set(it.videoViews.peek()) }
                    repeat(2) {
                        blockers +=
                            workers.submit {
                                occupied.countDown()
                                gate.await()
                            }
                    }
                    Assert.assertTrue(occupied.await(5, TimeUnit.SECONDS))
                    val first =
                        MediaCard(
                            json(
                                "url" to "${server.url()}?intent=first",
                                "title" to "First surface fixture",
                                "kind" to "video",
                            )
                        )
                    val second =
                        MediaCard(
                            json(
                                "url" to "${server.url()}?intent=second",
                                "title" to "Second surface fixture",
                                "kind" to "video",
                            )
                        )
                    scene.onActivity {
                        vm.get().feeds[1].value =
                            FeedState(cards = listOf(first, second), generation = 7781)
                    }
                    compose.onNodeWithText(first.title).performClick()
                    compose.onNodeWithTag("preparing-video").assertIsDisplayed()
                    val firstToken = vm.get().player.value.preparing!!.token
                    // A native snapshot of the old/empty source cannot erase selection intent.
                    kotlinx.coroutines.runBlocking { vm.get().snapshot() }
                    Assert.assertEquals(firstToken, vm.get().player.value.preparing!!.token)
                    Assert.assertEquals(-1L, StartupTimings.total(firstToken))
                    // Transport feedback must not erase the scoped selection
                    // while resolution is held. Reuse current volume/speed so
                    // this check does not change persisted playback preferences.
                    val beforeControls = MediaRuntimeTest.status()
                    scene.onActivity {
                        for ((op, value) in
                            listOf(
                                "volume" to beforeControls.optDouble("volume", 0.7),
                                "speed" to beforeControls.optDouble("speed", 1.0),
                            )) {
                            vm.get().command(op, "value" to value)
                            Assert.assertEquals(firstToken, vm.get().player.value.preparing?.token)
                        }
                        repeat(2) {
                            vm.get().command("pause")
                            Assert.assertEquals(firstToken, vm.get().player.value.preparing?.token)
                        }
                    }
                    kotlinx.coroutines.runBlocking { vm.get().snapshot() }
                    Assert.assertEquals(firstToken, vm.get().player.value.preparing?.token)
                    scene.onActivity {
                        val view = VisibleVideoProbe.find(it.window.decorView)
                        Assert.assertNotNull(
                            "The video surface must exist before extraction completes",
                            view,
                        )
                        Assert.assertSame(
                            "The first selection must claim its activity's idle view",
                            idleView.get(),
                            view,
                        )
                        Assert.assertSame(PlaybackService.current.controlsPlayer, view.player)
                        Assert.assertNull(
                            PlaybackService.current.currentRequest.optJSONObject("media")
                        )
                    }
                    // More selections than the queue's capacity must remain
                    // cancellable while both actual resolver workers are held.
                    scene.onActivity {
                        repeat(12) { index ->
                            vm.get()
                                .play(
                                    MediaCard(
                                        json(
                                            "url" to "${server.url()}?intent=rapid$index",
                                            "title" to "Rapid surface $index",
                                            "kind" to "video",
                                        )
                                    )
                                )
                        }
                        Assert.assertEquals(
                            "Rapid surface 11",
                            vm.get().player.value.preparing?.media?.title,
                        )
                        Assert.assertNull(vm.get().actionError.value)
                    }
                    compose.onNodeWithTag("panel-close").performClick()
                    compose.onNodeWithText(second.title).performScrollTo().performClick()
                    compose.onNodeWithTag("preparing-video").assertIsDisplayed()
                    val secondToken = vm.get().player.value.preparing!!.token
                    Assert.assertTrue(secondToken > firstToken)
                    val preparedView = AtomicReference<androidx.media3.ui.PlayerView>()
                    scene.onActivity {
                        preparedView.set(VisibleVideoProbe.find(it.window.decorView))
                    }
                    Assert.assertNotNull(preparedView.get())
                    Assert.assertEquals(-1L, StartupTimings.total(secondToken))
                    gate.countDown()
                    MediaRuntimeTest.await(
                        "Only the latest selection should reach actual playback"
                    ) {
                        val status = MediaRuntimeTest.status()
                        status.optBoolean("playing") &&
                            status.optJSONObject("media")?.optString("url") == second.url &&
                            vm.get().player.value.preparing == null
                    }
                    compose.waitUntil(10000) { StartupTimings.usable(secondToken) > 0 }
                    scene.onActivity {
                        Assert.assertSame(
                            "Handoff must retain the already prepared native view",
                            preparedView.get(),
                            VisibleVideoProbe.find(it.window.decorView),
                        )
                        Assert.assertEquals(secondToken, PlaybackService.current.startupToken)
                    }
                    Assert.assertEquals(
                        "Cancelled selection must not record a visible frame",
                        -1L,
                        StartupTimings.total(firstToken),
                    )
                    compose.onNodeWithTag("preparing-video").assertDoesNotExist()
                }
            }
        } finally {
            gate.countDown()
            blockers.forEach { it.get(10, TimeUnit.SECONDS) }
            file.delete()
        }
    }

    @Test
    fun rejectedServiceRequestClearsOnlyItsOwnPreparation() {
        RuntimeTest.launchOffline().use { scene ->
            val vm = AtomicReference<YtuiViewModel>()
            scene.onActivity { vm.set(ViewModelProvider(it)[YtuiViewModel::class.java]) }
            compose.waitUntil(10000) { vm.get().ready.value }
            for (directService in listOf(false, true)) {
                val staleToken =
                    StartupTimings.begin(android.os.SystemClock.elapsedRealtime(), true)
                val token = StartupTimings.begin(android.os.SystemClock.elapsedRealtime(), true)
                val media =
                    MediaCard(
                        json(
                            "url" to "http://127.0.0.1/invalid-fixture",
                            "title" to "Rejected surface fixture",
                            "kind" to "video",
                        )
                    )
                scene.onActivity {
                    vm.get().actionError.value = null
                    vm.get().prepareVideoSurface(media, staleToken)
                    vm.get().prepareVideoSurface(media, token)
                }
                MediaBridge.event(
                    "playback_start_failed",
                    "Stale rejection",
                    staleToken.toString(),
                    0,
                )
                kotlinx.coroutines.runBlocking { vm.get().snapshot() }
                Assert.assertEquals(token, vm.get().player.value.preparing?.token)
                Assert.assertNull(vm.get().actionError.value)
                val request =
                    json(
                        "media" to media.value,
                        "stream" to JSONObject(),
                        "audio" to false,
                        "startup_token" to token,
                    )
                // Exercise both preflight validation and an actual rejected FGS
                // intent. The latter previously crashed the process on teardown.
                if (directService) {
                    MediaBridge.context.startForegroundService(
                        android.content
                            .Intent(MediaBridge.context, PlaybackService::class.java)
                            .setAction("play")
                            .putExtra("request", request.toString())
                    )
                } else {
                    MediaBridge.execute(json("op" to "play", "request" to request).toString())
                }
                compose.waitUntil(10000) {
                    vm.get().actionError.value == "Invalid playback request" &&
                        vm.get().player.value.preparing == null &&
                        !vm.get().playerOpen.value
                }
                if (directService) {
                    MediaRuntimeTest.await("Rejected foreground service must finish safely") {
                        PlaybackService.current == null
                    }
                }
                Assert.assertEquals(-1L, StartupTimings.total(token))
                compose.onNodeWithTag("preparing-video").assertDoesNotExist()
                val settings = JSONObject(NativeBridge.request(json("op" to "settings").toString()))
                Assert.assertTrue(
                    "JNI must remain available after rejected start",
                    settings.optBoolean("ok"),
                )
            }
        }
    }

    @Test
    fun failedResolutionClearsPreparationAndRestoresTheBrowseScreen() {
        RuntimeTest.launchOffline().use { scene ->
            val vm = AtomicReference<YtuiViewModel>()
            scene.onActivity { vm.set(ViewModelProvider(it)[YtuiViewModel::class.java]) }
            compose.waitUntil(10000) { vm.get().ready.value }
            scene.onActivity {
                vm.get()
                    .play(
                        MediaCard(
                            json(
                                "url" to "not-a-url",
                                "title" to "Invalid surface fixture",
                                "kind" to "video",
                            )
                        )
                    )
            }
            compose.waitUntil(10000) {
                vm.get().actionError.value != null &&
                    vm.get().player.value.preparing == null &&
                    !vm.get().playerOpen.value
            }
            Assert.assertEquals(-1L, StartupTimings.total(StartupTimings.latestToken()))
            scene.onActivity {
                Assert.assertNull(PlaybackService.current?.currentRequest?.optJSONObject("media"))
            }
            compose.onNodeWithTag("preparing-video").assertDoesNotExist()
        }
    }
}
