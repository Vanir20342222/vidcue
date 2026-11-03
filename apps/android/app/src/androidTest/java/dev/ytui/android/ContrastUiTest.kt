package dev.ytui.android

import android.graphics.BitmapFactory
import android.graphics.Rect
import android.view.View
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createEmptyComposeRule
import androidx.media3.ui.PlayerView
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** Actual decoded white pixels exercise the worst background for white transport glyphs. */
@RunWith(AndroidJUnit4::class)
@OptIn(androidx.compose.ui.test.ExperimentalTestApi::class)
@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
class ContrastUiTest {
    @get:Rule
    val compose = createEmptyComposeRule(effectContext = kotlinx.coroutines.Dispatchers.Main)
    @get:Rule
    val notifications =
        androidx.test.rule.GrantPermissionRule.grant(
            *if (android.os.Build.VERSION.SDK_INT >= 33)
                arrayOf("android.permission.POST_NOTIFICATIONS")
            else emptyArray()
        )

    @Test
    fun enabledNativeTransportsKeepContrastOnBrightVideo() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val context = instrumentation.targetContext
        val fixture = File(context.cacheDir, "contrast-fixture.mp4")
        instrumentation.context.assets.open("fixture-white.mp4").use { input ->
            fixture.outputStream().use { input.copyTo(it) }
        }
        val original = PlaybackService.rpc(json("op" to "settings"))
        try {
            for (theme in listOf("ytui-dark", "ytui-light")) {
                PlaybackService.rpc(
                    json(
                        "op" to "save_settings",
                        "config" to original.copyJson().put("theme", theme),
                    )
                )
                val palette =
                    PlaybackService.rpc(json("op" to "theme", "dark" to (theme == "ytui-dark")))
                val expectedAccent = palette.color("accent", Color.White)
                val initialFrames = MediaBridge.videoFrames.get()
                RuntimeTest.launchOffline().use { scenario ->
                    MediaBridge.execute(
                        json(
                                "op" to "play",
                                "request" to
                                    json(
                                        "media" to
                                            json(
                                                "url" to "https://media.invalid/bright-contrast",
                                                "title" to "Bright video contrast fixture",
                                                "kind" to "video",
                                            ),
                                        "audio" to false,
                                        "stream" to
                                            json(
                                                "url" to
                                                    android.net.Uri.fromFile(fixture).toString()
                                            ),
                                    ),
                            )
                            .toString()
                    )
                    MediaBridge.event("show_player", "")
                    compose.waitUntil(10_000) {
                        compose
                            .onAllNodesWithTag("native-video-surface")
                            .fetchSemanticsNodes()
                            .isNotEmpty()
                    }
                    compose.waitForIdle()
                    MediaRuntimeTest.await("White fixture must submit a decoder frame") {
                        MediaBridge.videoFrames.get() > initialFrames &&
                            MediaRuntimeTest.status().optBoolean("playing") &&
                            MediaRuntimeTest.status().optDouble("position") > 1.5
                    }
                    val videoBounds = Rect()
                    MediaRuntimeTest.await(
                        "Decoded white video must be actually visible, not a black placeholder"
                    ) {
                        scenario.onActivity { activity ->
                            activity
                                .findViewById<View>(androidx.media3.ui.R.id.exo_content_frame)
                                ?.getGlobalVisibleRect(videoBounds)
                        }
                        val shot = instrumentation.uiAutomation.takeScreenshot()
                        if (shot == null) false
                        else
                            try {
                                if (videoBounds.isEmpty) false
                                else {
                                    val pixel =
                                        Color(
                                            shot.getPixel(
                                                (videoBounds.left + videoBounds.width() * 0.025f)
                                                    .toInt(),
                                                videoBounds.centerY(),
                                            )
                                        )
                                    minOf(pixel.red, pixel.green, pixel.blue) > 0.9f
                                }
                            } finally {
                                shot.recycle()
                            }
                    }
                    MediaRuntimeTest.command("pause")
                    MediaRuntimeTest.await("White fixture must pause on its decoded frame") {
                        MediaRuntimeTest.status().optBoolean("paused")
                    }
                    val buttons = mutableListOf<Pair<String, Rect>>()
                    val timeline = Rect()
                    scenario.onActivity { activity ->
                        val play =
                            activity.findViewById<View>(androidx.media3.ui.R.id.exo_play_pause)
                        assertNotNull("Native play control must exist", play)
                        val playerView =
                            generateSequence(play) { it.parent as? View }
                                .filterIsInstance<PlayerView>()
                                .first()
                        playerView.controllerShowTimeoutMs = 0
                        playerView.showController()
                    }
                    compose.waitForIdle()
                    instrumentation.uiAutomation.waitForIdle(300, 3000)
                    // A compact inline frame hides rewind/forward (PlayerChrome.fitTransport;
                    // fullscreen keeps them). Check that, then show them so their contrast and
                    // touch targets are still inspected below.
                    scenario.onActivity { activity ->
                        val playerView =
                            generateSequence(
                                    activity.findViewById<View>(
                                        androidx.media3.ui.R.id.exo_play_pause
                                    )
                                ) {
                                    it.parent as? View
                                }
                                .filterIsInstance<PlayerView>()
                                .first()
                        val widthDp = playerView.width / activity.resources.displayMetrics.density
                        if (widthDp < 560f)
                            for (id in
                                listOf(
                                    androidx.media3.ui.R.id.exo_rew_with_amount,
                                    androidx.media3.ui.R.id.exo_ffwd_with_amount,
                                ))
                                assertFalse(
                                    "A compact inline frame hides rewind and forward",
                                    activity.findViewById<View>(id)?.isShown == true,
                                )
                        playerView.setShowRewindButton(true)
                        playerView.setShowFastForwardButton(true)
                    }
                    compose.waitForIdle()
                    instrumentation.uiAutomation.waitForIdle(300, 3000)
                    scenario.onActivity { activity ->
                        assertTrue(
                            activity
                                .findViewById<View>(androidx.media3.ui.R.id.exo_progress)
                                .getGlobalVisibleRect(timeline)
                        )
                        for ((name, id) in
                            listOf(
                                "play" to androidx.media3.ui.R.id.exo_play_pause,
                                "previous" to androidx.media3.ui.R.id.exo_prev,
                                "next" to androidx.media3.ui.R.id.exo_next,
                                "rewind" to androidx.media3.ui.R.id.exo_rew_with_amount,
                                "forward" to androidx.media3.ui.R.id.exo_ffwd_with_amount,
                            )) {
                            val button = activity.findViewById<View>(id)
                            if (button != null && button.isShown && button.isEnabled) {
                                val bounds = Rect()
                                assertTrue(button.getGlobalVisibleRect(bounds))
                                val minimum = 48f * activity.resources.displayMetrics.density - 1
                                assertTrue(
                                    "$name needs a 48dp touch target",
                                    bounds.width() >= minimum && bounds.height() >= minimum,
                                )
                                assertNotNull(
                                    "$name needs local contrast backing",
                                    button.background,
                                )
                                if (name == "rewind" || name == "forward") {
                                    val backgroundGlyph =
                                        (button.background
                                                as? android.graphics.drawable.LayerDrawable)
                                            ?.findDrawableByLayerId(android.R.id.icon)
                                    assertTrue(
                                        "$name must retain its foreground or background arrow",
                                        button.foreground != null || backgroundGlyph != null,
                                    )
                                    val parentBackground = (button.parent as View).background
                                    assertFalse(
                                        "A local seek disc must not paint the shared transport strip",
                                        parentBackground is
                                            android.graphics.drawable.GradientDrawable &&
                                            parentBackground.shape ==
                                                android.graphics.drawable.GradientDrawable.OVAL,
                                    )
                                }
                                assertFalse(
                                    "$name needs a spoken label",
                                    button.contentDescription.isNullOrBlank(),
                                )
                                buttons += name to bounds
                            }
                        }
                    }
                    assertTrue(
                        "Test must inspect the actual active play, rewind and forward controls",
                        buttons.map { it.first }.containsAll(listOf("play", "rewind", "forward")),
                    )
                    val filename = "restyle-fidelity-$theme-white-video-controls.png"
                    MediaRuntimeTest.capture(filename)
                    val capture = File(context.getExternalFilesDir(null), filename)
                    val image = BitmapFactory.decodeFile(capture.absolutePath)
                    try {
                        val clear =
                            Color(
                                image.getPixel(
                                    (videoBounds.left + videoBounds.width() * 0.025f).toInt(),
                                    videoBounds.centerY(),
                                )
                            )
                        assertTrue(
                            "Final capture must still show actual white video",
                            minOf(clear.red, clear.green, clear.blue) > 0.9f,
                        )
                        val x = (timeline.left + timeline.width() * 0.12f).toInt()
                        val active = Color(image.getPixel(x, timeline.centerY()))
                        val density = context.resources.displayMetrics.density
                        val backing =
                            Color(image.getPixel(x, timeline.centerY() + (2f * density).toInt()))
                        assertTrue(
                            "$theme timeline accent must be readable against its real keyline",
                            mediaContrast(active, backing) >= 3f,
                        )
                        assertEquals(
                            "$theme played accent must not be replaced",
                            expectedAccent.red,
                            active.red,
                            0.04f,
                        )
                        assertEquals(expectedAccent.green, active.green, 0.04f)
                        assertEquals(expectedAccent.blue, active.blue, 0.04f)
                        for ((name, bounds) in buttons) {
                            // Inside the local disc, diagonally outside the central glyph.
                            val radius = minOf(bounds.width(), bounds.height()) * 0.5f
                            val x = (bounds.exactCenterX() + radius * 0.6f).toInt()
                            val y = (bounds.exactCenterY() - radius * 0.6f).toInt()
                            assertTrue(
                                "$name must have 3:1 white-icon contrast on actual white video",
                                mediaContrast(Color.White, Color(image.getPixel(x, y))) >= 3f,
                            )
                        }
                    } finally {
                        image.recycle()
                    }
                }
                MediaRuntimeTest.command("stop")
            }
        } finally {
            MediaBridge.execute(json("op" to "stop").toString())
            PlaybackService.rpc(json("op" to "save_settings", "config" to original))
            fixture.delete()
        }
    }
}
