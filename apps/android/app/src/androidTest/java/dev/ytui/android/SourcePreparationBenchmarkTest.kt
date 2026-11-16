package dev.ytui.android

import android.os.SystemClock
import androidx.lifecycle.ViewModelProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import org.junit.Assert.*
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** Interleaved warm-player comparison; retains the original cold-service gates separately. */
@RunWith(AndroidJUnit4::class)
class SourcePreparationBenchmarkTest {
    @get:Rule val notifications = androidx.test.rule.GrantPermissionRule.grant(
        *if (android.os.Build.VERSION.SDK_INT >= 33) arrayOf("android.permission.POST_NOTIFICATIONS") else emptyArray()
    )

    @Test fun comparePreparedSamplesWithBytesOnlyOnTheSamePlayer() {
        assumeTrue(InstrumentationRegistry.getArguments().getString("vidcue_source_benchmark") == "measure")
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val original = MobileSettings.snapshot()
        val fixture = File(instrumentation.targetContext.cacheDir, "source-comparison.m4a")
        val report = StringBuilder("Synthetic audio, same warm player, best quality, all bytes warmed. First four samples are warm-up. ABBA ordering. AudioTrack callback is not acoustic output.\n")
        instrumentation.context.assets.open("fixture-audio.m4a").use { input -> fixture.outputStream().use { input.copyTo(it) } }
        try {
            MobileSettings.save(json("preload_next" to true, "preload_mobile" to true,
                "prefer_offline" to false, "autoplay_related" to false))
            MediaRuntimeTest.FixtureServer(fixture).use { server ->
                RuntimeTest.launchOffline().use { scene ->
                    MediaRuntimeTest.await("Warm player and initialized UI required") {
                        var ready = false
                        scene.onActivity {
                            ready = ViewModelProvider(it)[YtuiViewModel::class.java].ready.value
                            PlaybackService.warmForeground()
                        }
                        ready && PlaybackService.warmReady()
                    }
                    repeat(20) { sample ->
                        val prepared = listOf(false, true, true, false)[sample % 4]
                        scene.onActivity { PlaybackService.current!!.player.pause() }
                        val media = json("url" to "${server.url()}?sample=$sample", "title" to "Source comparison", "kind" to "song")
                        val stream = PlaybackService.rpc(json("op" to "resolve", "media" to media, "audio" to true, "quality" to "best"))
                        val request = json("media" to media.copyJson(), "stream" to stream, "audio" to true, "quality" to "best")
                        PlaybackService.mergeMetadata(request.getJSONObject("media"), stream)
                        PlaybackCache.prefetchResolved(media, true, "best", stream, NativeBridge.accountIntent(), NetworkSettings.revision.get())
                        MediaRuntimeTest.await("Every sample must begin with source and byte preparation complete") {
                            var ready = false
                            scene.onActivity { ready = PlaybackService.current?.sourcePreparedFor(request) == true }
                            ready && PlaybackCache.readyFor(media, true, "best")
                        }
                        if (!prepared) scene.onActivity { PlaybackService.current!!.discardPreparedSource(Long.MAX_VALUE) }
                        assertTrue("Control must retain the same warmed bytes", PlaybackCache.readyFor(media, true, "best"))
                        var before = 0L
                        var token = 0L
                        scene.onActivity {
                            val service = PlaybackService.current!!
                            before = service.preparedSourcesUsed
                            val vm = ViewModelProvider(it)[YtuiViewModel::class.java]
                            vm.config.value = vm.config.value.copyJson().apply { getJSONObject("download").put("quality", "best") }
                            vm.play(MediaCard(media))
                            token = StartupTimings.latestToken()
                        }
                        val deadline = SystemClock.elapsedRealtime() + 6000
                        while (StartupTimings.audioUsable(token) < 0 && SystemClock.elapsedRealtime() < deadline) Thread.sleep(2)
                        val active = StartupTimings.usable(token)
                        val output = StartupTimings.audioUsable(token)
                        var reused = false
                        scene.onActivity { reused = PlaybackService.current!!.preparedSourcesUsed == before + 1 }
                        report.append("sample=$sample warmup=${sample < 4} source_prepared=$prepared reused=$reused active_ms=$active audio_output_ms=$output\n")
                        assertEquals("Comparison must actually use its selected path", prepared, reused)
                        assertTrue("Selection must reach active playback", active >= 0)
                        MediaRuntimeTest.await("Each selection must continue playing") {
                            val state = MediaRuntimeTest.status()
                            state.optBoolean("playing") && state.optDouble("position") >= 0.25
                        }
                    }
                }
            }
        } finally {
            report.append(StartupTimings.report())
            File(instrumentation.targetContext.getExternalFilesDir(null), "source-preparation-comparison.txt").writeText(report.toString())
            MediaRuntimeTest.command("stop")
            PlaybackCache.cancel()
            MobileSettings.save(original)
            fixture.delete()
        }
    }
}
