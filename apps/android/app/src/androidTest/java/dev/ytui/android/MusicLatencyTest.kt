package dev.ytui.android

import android.os.SystemClock
import androidx.lifecycle.ViewModelProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import org.junit.Assert.*
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.Rule
import org.junit.runner.RunWith

/** Explicit performance qualification; synthetic audio, no account or public-service access. */
@RunWith(AndroidJUnit4::class)
class MusicLatencyTest {
    @get:Rule
    val notifications = androidx.test.rule.GrantPermissionRule.grant(
        *if (android.os.Build.VERSION.SDK_INT >= 33)
            arrayOf("android.permission.POST_NOTIFICATIONS") else emptyArray()
    )

    @Test
    fun selectionToPlayingStaysBelowHundredMilliseconds() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        assumeTrue(
            "Opt in with -e vidcue_music_latency enforce",
            InstrumentationRegistry.getArguments().getString("vidcue_music_latency") == "enforce",
        )
        val original = MobileSettings.snapshot()
        val fixture = File(instrumentation.targetContext.cacheDir, "music-latency.m4a")
        instrumentation.context.assets.open("fixture-audio.m4a").use { input ->
            fixture.outputStream().use { input.copyTo(it) }
        }
        val evidence = StringBuilder(
            "Synthetic music: selection callback to active playback and AudioTrack advancement, not acoustic output.\n" +
                "Player infrastructure may already be warm. Byte-cold and byte-warm reported separately.\n"
        )
        val results = mutableListOf<Long>()
        try {
            MobileSettings.save(json("autoplay_related" to false, "prefer_offline" to false))
            MediaRuntimeTest.FixtureServer(fixture).use { server ->
                RuntimeTest.launchOffline().use { scene ->
                    MediaRuntimeTest.await("Engine UI must initialize before selecting music") {
                        var ready = false
                        scene.onActivity { ready = ViewModelProvider(it)[YtuiViewModel::class.java].ready.value }
                        ready
                    }
                    // Songs are chosen on the Music page, as a listener does.
                    scene.onActivity { ViewModelProvider(it)[YtuiViewModel::class.java].tab.value = 2 }
                    for (prepared in listOf(false, true)) {
                        repeat(3) { sample ->
                            scene.onActivity { PlaybackService.current?.stopAndClear() }
                            MediaRuntimeTest.await("Prior service must finish closing") {
                                PlaybackService.current == null
                            }
                            PlaybackCache.cancel()
                            NativeBridge.request(json("op" to "resolve_cache_clear").toString())
                            MobileSettings.save(json("preload_next" to prepared, "preload_mobile" to true))
                            val media = json(
                                "url" to "${server.url()}?prepared=$prepared&sample=$sample",
                                "title" to "Synthetic music latency",
                                "kind" to "song",
                            )
                            if (prepared) {
                                val stream = PlaybackService.rpc(
                                    json("op" to "resolve", "media" to media, "audio" to true, "quality" to "best")
                                )
                                PlaybackCache.prefetchResolved(
                                    media, true, "best", stream,
                                    NativeBridge.accountIntent(), NetworkSettings.revision.get(),
                                )
                                MediaRuntimeTest.await("Music startup bytes must finish warming") {
                                    PlaybackCache.readyFor(media, true, "best")
                                }
                            }
                            var token = 0L
                            var selectedAt = 0L
                            scene.onActivity {
                                val vm = ViewModelProvider(it)[YtuiViewModel::class.java]
                                // Keep the exact same quality for speculative and foreground work.
                                vm.config.value = vm.config.value.copyJson().apply {
                                    getJSONObject("download").put("quality", "best")
                                }
                                selectedAt = SystemClock.elapsedRealtime()
                                vm.play(MediaCard(media))
                                token = StartupTimings.latestToken()
                            }
                            val deadline = SystemClock.elapsedRealtime() + 6000
                            while (StartupTimings.audioUsable(token) < 0 && SystemClock.elapsedRealtime() < deadline)
                                Thread.sleep(2)
                            val signal = StartupTimings.audioUsable(token)
                            // The callback clock includes work before the internal timing token.
                            // Observation/scheduling overhead stays in this conservative result.
                            val elapsed = if (signal >= 0) SystemClock.elapsedRealtime() - selectedAt else -1L
                            results.add(elapsed)
                            evidence.append("byte_warm=$prepared sample=$sample selection_to_playing_ms=$elapsed internal_signal_ms=$signal target_ms=100\n")
                            // A fast signal must be followed by actual continuing playback.
                            if (elapsed >= 0) MediaRuntimeTest.await("Music must continue playing") {
                                val status = MediaRuntimeTest.status()
                                status.optBoolean("playing") && status.optDouble("position") >= 1.0
                            }
                        }
                    }
                }
            }
        } finally {
            evidence.append(StartupTimings.report())
            File(instrumentation.targetContext.getExternalFilesDir(null), "music-latency-timings.txt")
                .writeText(evidence.toString())
            MediaRuntimeTest.command("stop")
            PlaybackCache.cancel()
            MobileSettings.save(original)
            fixture.delete()
        }
        assertEquals("All byte-cold and byte-warm samples must run", 6, results.size)
        assertTrue(evidence.toString(), results.all { it in 0 until 100L })
    }
}
