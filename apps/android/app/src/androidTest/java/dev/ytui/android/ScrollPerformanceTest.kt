package dev.ytui.android

import android.os.Bundle
import android.os.Handler
import android.os.HandlerThread
import android.view.FrameMetrics
import android.view.Window
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createEmptyComposeRule
import androidx.lifecycle.ViewModelProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import java.net.ServerSocket
import java.util.Collections
import org.junit.*
import org.junit.runner.RunWith

/**
 * Scroll smoothness and the floating feed chrome on the emulator. Each feed is filled with 150
 * synthetic items whose covers come from a loopback server (no network, account or real media),
 * then flung with real touch input while FrameMetrics records every frame.
 *
 * Frame times on the hosted software emulator are far slower than a phone's GPU, so the numbers
 * are recorded as evidence (instrumentation output and logcat tag `ytui.scroll`), and only a
 * hang ceiling fails the test. Compare runs against each other, not against a phone.
 */
@RunWith(AndroidJUnit4::class)
class ScrollPerformanceTest {
    @get:Rule
    val compose = createEmptyComposeRule(effectContext = kotlinx.coroutines.Dispatchers.Main)

    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val device = UiDevice.getInstance(instrumentation)

    private fun report(text: String) {
        android.util.Log.i("ytui.scroll", text)
        instrumentation.sendStatus(0, Bundle().apply { putString("stream", "ytui.scroll $text\n") })
    }

    /** Serves a distinct solid-and-gradient JPEG for every `/cover-N` path. */
    private class Covers : AutoCloseable {
        private val socket = ServerSocket(0, 32, java.net.InetAddress.getByName("127.0.0.1"))
        private val cache = java.util.concurrent.ConcurrentHashMap<Int, ByteArray>()
        private val worker =
            Thread {
                    while (!socket.isClosed) try {
                        val client = socket.accept()
                        Thread {
                                client.use {
                                    it.soTimeout = 5000
                                    val reader = it.getInputStream().bufferedReader()
                                    val request = reader.readLine() ?: return@use
                                    while (!reader.readLine().isNullOrEmpty()) {}
                                    val index =
                                        Regex("/cover-(\\d+)").find(request)?.groupValues?.get(1)?.toIntOrNull() ?: 0
                                    val body = cache.getOrPut(index % 24) { image(index % 24) }
                                    it.getOutputStream().apply {
                                        write(
                                            ("HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Length: ${body.size}\r\n" +
                                                    "Connection: close\r\n\r\n")
                                                .toByteArray()
                                        )
                                        write(body)
                                        flush()
                                    }
                                }
                            }
                            .start()
                    } catch (_: java.io.IOException) {}
                }
                .apply {
                    isDaemon = true
                    start()
                }

        fun url(index: Int) = "http://127.0.0.1:${socket.localPort}/cover-$index.jpg"

        private fun image(index: Int): ByteArray {
            val bitmap = android.graphics.Bitmap.createBitmap(480, 480, android.graphics.Bitmap.Config.ARGB_8888)
            val canvas = android.graphics.Canvas(bitmap)
            val hue = index * 15f
            val paint =
                android.graphics.Paint().apply {
                    shader =
                        android.graphics.LinearGradient(
                            0f,
                            0f,
                            480f,
                            480f,
                            android.graphics.Color.HSVToColor(floatArrayOf(hue, 0.7f, 0.9f)),
                            android.graphics.Color.HSVToColor(floatArrayOf(hue + 40f, 0.8f, 0.3f)),
                            android.graphics.Shader.TileMode.CLAMP,
                        )
                }
            canvas.drawRect(0f, 0f, 480f, 480f, paint)
            val out = java.io.ByteArrayOutputStream()
            bitmap.compress(android.graphics.Bitmap.CompressFormat.JPEG, 85, out)
            bitmap.recycle()
            return out.toByteArray()
        }

        override fun close() {
            socket.close()
            worker.join(2000)
        }
    }

    private fun cards(tab: Int, covers: Covers): List<MediaCard> =
        (0 until 150).map { index ->
            MediaCard(
                json(
                    "title" to "Scroll fixture ${if (tab == 1) "video" else if (tab == 2) "song" else "station"} $index",
                    "creator" to "Fixture creator ${index % 7}",
                    "url" to "https://media.invalid/scroll-$tab-$index",
                    "kind" to
                        when (tab) {
                            2 -> "song"
                            3 -> "station"
                            else -> "video"
                        },
                    "live" to (tab == 3),
                    "thumbnail" to covers.url(index),
                    "duration" to 180 + index,
                    "view_count" to 1_234_567L * (index + 1),
                    "upload_date" to "20260901",
                )
            )
        }

    /** Frame durations in milliseconds while [action] runs. */
    private fun recordFrames(window: Window, action: () -> Unit): List<Double> {
        val frames = Collections.synchronizedList(mutableListOf<Double>())
        val thread = HandlerThread("frame-metrics").apply { start() }
        val listener =
            Window.OnFrameMetricsAvailableListener { _, metrics, _ ->
                frames += metrics.getMetric(FrameMetrics.TOTAL_DURATION) / 1_000_000.0
            }
        instrumentation.runOnMainSync { window.addOnFrameMetricsAvailableListener(listener, Handler(thread.looper)) }
        try {
            action()
            Thread.sleep(400)
        } finally {
            instrumentation.runOnMainSync { window.removeOnFrameMetricsAvailableListener(listener) }
            thread.quitSafely()
        }
        return synchronized(frames) { frames.toList() }
    }

    private fun percentile(sorted: List<Double>, p: Double) =
        if (sorted.isEmpty()) 0.0 else sorted[((sorted.size - 1) * p).toInt()]

    @Test
    fun feedsScrollWithFloatingChromeAndRecordFrameTimes() {
        Covers().use { covers ->
            RuntimeTest.launchOffline().use { scene ->
                var vm: YtuiViewModel? = null
                scene.onActivity { vm = ViewModelProvider(it)[YtuiViewModel::class.java] }
                compose.waitUntil(15_000) { vm!!.ready.value }
                var window: Window? = null
                scene.onActivity { window = it.window }
                val width = device.displayWidth
                val height = device.displayHeight
                for ((tab, name) in listOf(1 to "home", 2 to "music", 3 to "radio")) {
                    val items = cards(tab, covers)
                    scene.onActivity {
                        vm!!.chooseTab(tab)
                        vm!!.feeds[tab].value = FeedState(title = name, cards = items, generation = 9000L + tab)
                    }
                    compose.waitUntil(15_000) {
                        compose.onAllNodesWithText(items.first().title).fetchSemanticsNodes().isNotEmpty()
                    }
                    // Let the first covers load so the scroll exercises real images.
                    Thread.sleep(1500)
                    compose.waitForIdle()
                    val headerTop =
                        compose.onNodeWithTag("browse-header").fetchSemanticsNode().boundsInRoot.top
                    val frames =
                        recordFrames(window!!) {
                            repeat(6) {
                                device.swipe(width / 2, height * 3 / 4, width / 2, height / 4, 12)
                                Thread.sleep(250)
                            }
                        }
                    compose.waitForIdle()
                    // Scrolling down slid the search header away; the dock stays over the feed.
                    val movedTop =
                        compose.onNodeWithTag("browse-header").fetchSemanticsNode().boundsInRoot.top
                    Assert.assertTrue("Header should slide up while scrolling down", movedTop < headerTop - 10f)
                    compose.onNodeWithTag("bottom-navigation").assertIsDisplayed()
                    MediaRuntimeTest.capture("restyle-scroll-$name.png")
                    // A small upward scroll brings it back.
                    device.swipe(width / 2, height / 2, width / 2, height * 3 / 4, 20)
                    compose.waitForIdle()
                    Thread.sleep(300)
                    val back = compose.onNodeWithTag("browse-header").fetchSemanticsNode().boundsInRoot.top
                    Assert.assertTrue("Header should return on an upward scroll", back > movedTop + 10f)

                    val sorted = frames.sorted()
                    val slow = frames.count { it > 16.7 }
                    val verySlow = frames.count { it > 33.4 }
                    report(
                        "$name frames=${frames.size} " +
                            "p50=%.1fms p90=%.1fms p99=%.1fms max=%.1fms over16.7ms=%.1f%% over33ms=%.1f%%".format(
                                percentile(sorted, 0.5),
                                percentile(sorted, 0.9),
                                percentile(sorted, 0.99),
                                sorted.lastOrNull() ?: 0.0,
                                if (frames.isEmpty()) 0.0 else slow * 100.0 / frames.size,
                                if (frames.isEmpty()) 0.0 else verySlow * 100.0 / frames.size,
                            )
                    )
                    Assert.assertTrue("No frames were recorded for $name", frames.size > 20)
                    // Hang ceiling only: the software emulator is no measure of a phone's GPU.
                    Assert.assertTrue("A $name frame took over 2 s", (sorted.lastOrNull() ?: 0.0) < 2000.0)
                }
            }
        }
    }

    @Test
    fun musicPlayerSettingOffersNoFullScreenStyles() {
        RuntimeTest.launchOffline().use { scene ->
            var vm: YtuiViewModel? = null
            scene.onActivity { vm = ViewModelProvider(it)[YtuiViewModel::class.java] }
            compose.waitUntil(15_000) { vm!!.ready.value }
            compose.onNodeWithTag("settings-button").performClick()
            compose.onNodeWithTag("settings-Appearance").performClick()
            compose.onNodeWithText("Music player").performScrollTo().performClick()
            // The current choice also shows on the settings row, so a name may appear twice.
            for (style in listOf("Cover", "Vinyl", "Strings", "Pulse orbit"))
                Assert.assertTrue(
                    "Missing style $style",
                    compose.onAllNodesWithText(style).fetchSemanticsNodes().isNotEmpty(),
                )
            Assert.assertTrue(
                compose.onAllNodes(hasText("Full screen", substring = true)).fetchSemanticsNodes().isEmpty()
            )
            MediaRuntimeTest.capture("restyle-player-styles.png")
        }
    }
}
