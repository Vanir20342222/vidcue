package dev.ytui.android

import androidx.lifecycle.ViewModelProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** Real Media3 sample queues, cache-only speculation and foreground reuse. */
@RunWith(AndroidJUnit4::class)
class SourcePreparationTest {
    @get:Rule val notifications = androidx.test.rule.GrantPermissionRule.grant(
        *if (android.os.Build.VERSION.SDK_INT >= 33) arrayOf("android.permission.POST_NOTIFICATIONS") else emptyArray()
    )

    @Test fun cachedAudioSourceIsPreparedScopedAndReusedForContinuingPlayback() = exercise(true)
    @Test fun cachedVideoSourceIsPreparedScopedAndReusedForContinuingPlayback() = exercise(false)

    private fun exercise(audio: Boolean) {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val original = MobileSettings.snapshot()
        val fixture = File(instrumentation.targetContext.cacheDir, if (audio) "source-preparation.m4a" else "source-preparation.mp4")
        instrumentation.context.assets.open(if (audio) "fixture-audio.m4a" else "fixture.mp4").use { input -> fixture.outputStream().use { input.copyTo(it) } }
        try {
            MobileSettings.save(json("preload_next" to true, "preload_mobile" to true,
                "prefer_offline" to false, "autoplay_related" to false))
            MediaRuntimeTest.command("stop")
            MediaRuntimeTest.await("Previous service must close") { PlaybackService.current == null }
            MediaRuntimeTest.FixtureServer(fixture).use { server ->
                RuntimeTest.launchOffline().use { scene ->
                    MediaRuntimeTest.await("UI must initialize") {
                        var ready = false
                        scene.onActivity {
                            ready = ViewModelProvider(it)[YtuiViewModel::class.java].ready.value
                            PlaybackService.warmForeground()
                        }
                        ready && PlaybackService.warmReady()
                    }
                    scene.onActivity {
                        assertFalse("Idle warm-up must not construct a source preload manager", PlaybackService.current!!.sourcePreloaderCreated())
                    }
                    val media = json("url" to server.url(), "title" to "Source preparation fixture", "kind" to if (audio) "song" else "video")
                    val stream = PlaybackService.rpc(json("op" to "resolve", "media" to media, "audio" to audio, "quality" to "best"))
                    val request = json("media" to media.copyJson(), "stream" to stream, "audio" to audio, "quality" to "best")
                    PlaybackService.mergeMetadata(request.getJSONObject("media"), stream)
                    PlaybackCache.prefetchResolved(media, audio, "best", stream, NativeBridge.accountIntent(), NetworkSettings.revision.get())
                    MediaRuntimeTest.await("Cached bytes and Media3 sample queues must both prepare") {
                        var ready = false
                        scene.onActivity { ready = PlaybackService.current?.sourcePreparedFor(request) == true }
                        ready && PlaybackCache.readyFor(media, audio, "best")
                    }
                    scene.onActivity {
                        assertFalse(PlaybackService.current!!.sourcePreparedFor(request.copyJson().put("quality", "720")))
                        assertFalse(PlaybackService.current!!.sourcePreparedFor(request.copyJson().put("audio", !audio)))
                    }
                    scene.onActivity { PlaybackCache.cancel() }
                    MediaRuntimeTest.await("Cancellation must discard prepared sample queues") {
                        var discarded = false
                        scene.onActivity { discarded = PlaybackService.current?.sourcePreparedFor(request) != true }
                        discarded
                    }
                    PlaybackCache.prefetchResolved(media, audio, "best", stream, NativeBridge.accountIntent(), NetworkSettings.revision.get())
                    MediaRuntimeTest.await("Cancelled preparation must be replaceable") {
                        var ready = false
                        scene.onActivity { ready = PlaybackService.current?.sourcePreparedFor(request) == true }
                        ready
                    }
                    val framesBefore = MediaBridge.videoFrames.get()
                    var before = 0L
                    scene.onActivity {
                        before = PlaybackService.current!!.preparedSourcesUsed
                        val vm = ViewModelProvider(it)[YtuiViewModel::class.java]
                        vm.config.value = vm.config.value.copyJson().apply { getJSONObject("download").put("quality", "best") }
                        vm.play(MediaCard(media))
                    }
                    MediaRuntimeTest.await("Foreground playback must consume the prepared source") {
                        var reused = false
                        scene.onActivity { reused = PlaybackService.current?.preparedSourcesUsed == before + 1 }
                        reused
                    }
                    MediaRuntimeTest.await("Prepared source must continue through real audio output") {
                        val status = MediaRuntimeTest.status()
                        status.optBoolean("playing") && status.optDouble("position") >= 1.0 &&
                            (audio || MediaBridge.videoFrames.get() > framesBefore)
                    }
                }
            }
        } finally {
            File(instrumentation.targetContext.getExternalFilesDir(null),
                "source-preparation-${if (audio) "audio" else "video"}-timings.txt")
                .writeText(StartupTimings.report())
            MediaRuntimeTest.command("stop")
            PlaybackCache.cancel()
            MobileSettings.save(original)
            fixture.delete()
        }
    }
}
