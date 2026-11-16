package dev.ytui.android

import android.os.SystemClock
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONObject
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import java.io.File

/**
 * Real loading of real media over the network, timed the way a user feels it: a feed page, a
 * YouTube video, a song and an openly licensed film, each from the tap to visible/audible
 * playback that keeps going.
 *
 * Opt-in only (`-e vidcue_live measure|enforce`): never part of the offline default suite.
 * `measure` records every sample in `live-media-timings.txt`; `enforce` also fails when a sample
 * misses its target (feed 1000 ms, video 500 ms, song 100 ms, film 500 ms). Every sample is
 * collected before anything fails, so one slow video still leaves a complete report.
 *
 * Public content only: no account is imported, and the report contains media page URLs and
 * timings but never a resolved stream URL, header or cookie. Cinecat titles are excluded; only
 * the catalog's openly licensed films and public player-test streams are played.
 *
 * Optional overrides (comma separated public page URLs):
 * `vidcue_live_videos`, `vidcue_live_songs`, and `vidcue_live_query` for the search page.
 */
class LiveMediaLoadingTest {
    @get:Rule
    val notifications: androidx.test.rule.GrantPermissionRule =
        androidx.test.rule.GrantPermissionRule.grant(
            *(if (android.os.Build.VERSION.SDK_INT >= 33) arrayOf("android.permission.POST_NOTIFICATIONS")
            else emptyArray())
        )

    private val arguments = InstrumentationRegistry.getArguments()
    private val mode = arguments.getString("vidcue_live", "off")
    private val enforce get() = mode == "enforce"

    private fun requireLive() {
        assumeTrue("Live media loading is opt-in (-e vidcue_live measure|enforce)", mode == "measure" || mode == "enforce")
    }

    private fun urls(argument: String, defaults: List<String>): List<String> =
        arguments.getString(argument, "").split(',').map { it.trim() }.filter { it.isNotEmpty() }.ifEmpty { defaults }
            .onEach { url ->
                val parsed = android.net.Uri.parse(url)
                val host = parsed.host.orEmpty()
                assertTrue(
                    "Use a public YouTube HTTPS page URL: $url",
                    parsed.scheme == "https" &&
                        (host == "youtu.be" || host == "youtube.com" || host.endsWith(".youtube.com")),
                )
            }

    private class Sample(val kind: String, val label: String, val ms: Long, val target: Long, val detail: String) {
        val met get() = ms in 0 until target
        override fun toString() =
            "kind=$kind label=\"$label\" ms=$ms target_ms=$target met=$met $detail".trim()
    }

    private val samples = mutableListOf<Sample>()

    private fun report(name: String, header: String) {
        val text = buildString {
            appendLine(header)
            appendLine("mode=$mode device=${android.os.Build.MODEL} sdk=${android.os.Build.VERSION.SDK_INT}")
            samples.forEach { appendLine(it.toString()) }
        }
        android.util.Log.i("LiveMedia", text)
        File(MediaBridge.context.getExternalFilesDir(null), name).appendText(text + "\n")
        val missed = samples.filter { !it.met }
        // A sample that never played is a failure in either mode; a slow one only when enforced.
        val broken = samples.filter { it.ms < 0 }
        assertTrue("Live media did not load:\n" + broken.joinToString("\n"), broken.isEmpty())
        if (enforce) assertTrue("Live targets missed:\n" + missed.joinToString("\n"), missed.isEmpty())
    }

    private fun rpc(request: JSONObject): JSONObject {
        val response = JSONObject(NativeBridge.request(request.toString()))
        assertTrue("${request.optString("op")}: ${response.optString("error")}", response.optBoolean("ok"))
        return response.getJSONObject("data")
    }

    // ---------------------------------------------------------------- feed

    @Test
    fun liveFeedPagesLoadWithinOneSecond() {
        requireLive()
        val query = arguments.getString("vidcue_live_query", "Big Buck Bunny")
        ActivityScenario.launch<RustActivity>(
            android.content.Intent(MediaBridge.context, RustActivity::class.java).putExtra("ytui.offline-ui", true)
        ).use {
            val pages =
                listOf(
                    // The anonymous YouTube home is often empty, so it is recorded but
                    // the music home and a search carry the target.
                    Triple("youtube-home", 1, JSONObject.wrap("Home")),
                    Triple("music-home", 2, JSONObject.wrap("Home")),
                    Triple("search", 1, JSONObject().put("Search", query)),
                )
            for ((label, tab, request) in pages) {
                // Two loads: the first after process start (connection set-up included), the
                // second as a user returning to the page sees it.
                for (attempt in listOf("first", "repeat")) {
                    val start = SystemClock.elapsedRealtime()
                    val page =
                        runCatching {
                            rpc(JSONObject().put("op", "browse").put("tab", tab).put("request", request).put("offset", 0))
                        }
                    val elapsed = SystemClock.elapsedRealtime() - start
                    val cards = page.getOrNull()?.optJSONArray("items")?.length() ?: -1
                    val anonymousHome = label == "youtube-home"
                    if (anonymousHome && cards <= 0) {
                        android.util.Log.i("LiveMedia", "youtube-home empty without an account ($elapsed ms)")
                        continue
                    }
                    samples +=
                        Sample(
                            "feed",
                            "$label/$attempt",
                            if (cards > 0) elapsed else -1,
                            1000,
                            "cards=$cards" + (page.exceptionOrNull()?.let { " error=${it.message?.take(120)}" } ?: ""),
                        )
                }
            }
        }
        report("live-media-timings.txt", "Live feed: browse RPC to a parsed page with cards. Rendering is excluded; artwork is not awaited.")
    }

    // ---------------------------------------------------------------- YouTube video and songs

    /**
     * Plays [url] through the same resolve-then-play path as a tap and returns tap→usable ms
     * (video: first frame and playing; audio: playing and AudioTrack advancing), or -1.
     */
    private fun play(url: String, audio: Boolean, prefetched: Boolean): Pair<Long, String> {
        val quality = "best"
        val media =
            JSONObject().put("url", url).put("title", "Live acceptance").put("kind", if (audio) "song" else "video")
        MediaRuntimeTest.command("stop")
        // Stop is applied on the service thread; let it land before the next selection so its
        // own cancellation cannot abandon the preload below.
        MediaRuntimeTest.await("Previous selection did not stop", false) {
            !MediaRuntimeTest.status().optBoolean("playing") && !MediaRuntimeTest.status().optBoolean("buffering")
        }
        Thread.sleep(500)
        PlaybackCache.cancel()
        NativeBridge.request("{\"op\":\"resolve_cache_clear\"}")
        if (prefetched) {
            // What a visible feed card does before the tap: resolve and warm the first bytes.
            PlaybackCache.prefetch(media, audio, quality)
            val deadline = SystemClock.elapsedRealtime() + 20_000
            while (!PlaybackCache.readyFor(media, audio, quality) && SystemClock.elapsedRealtime() < deadline)
                Thread.sleep(50)
            if (!PlaybackCache.readyFor(media, audio, quality)) return -1L to "prefetch_not_ready"
        }
        val timing = StartupTimings.begin(SystemClock.elapsedRealtime(), !audio)
        StartupTimings.playerCache(timing, PlaybackService.warmReady())
        StartupTimings.byteCache(timing, prefetched)
        var passed = false
        try {
            val resolved =
                JSONObject(
                    NativeBridge.request(
                        JSONObject()
                            .put("op", "resolve")
                            .put("startup_token", timing)
                            .put("media", media)
                            .put("audio", audio)
                            .put("quality", quality)
                            .toString()
                    )
                )
            if (!resolved.optBoolean("ok")) return -1L to "resolve_error=\"${resolved.optString("error").take(160)}\""
            val stream = resolved.getJSONObject("data")
            MediaBridge.recordResolutionTimings(timing, stream)
            StartupTimings.resolved(timing, SystemClock.elapsedRealtime(), stream.optBoolean("resolution_cache_hit"))
            StartupTimings.route(timing, stream.optString("resolution_route", "direct-media"))
            MediaBridge.execute(
                JSONObject()
                    .put("op", "play")
                    .put(
                        "request",
                        JSONObject()
                            .put("media", media)
                            .put("startup_token", timing)
                            .put("stream", stream)
                            .put("quality", quality)
                            .put("audio", audio),
                    )
                    .toString()
            )
            if (!audio) MediaBridge.event("show_player", "")
            MediaRuntimeTest.await("Never reached usable output") {
                (if (audio) StartupTimings.audioUsable(timing) else StartupTimings.usable(timing)) > 0
            }
            val usable = if (audio) StartupTimings.audioUsable(timing) else StartupTimings.usable(timing)
            // Loading counts only if playback then keeps going: three seconds decoded.
            MediaRuntimeTest.await("Playback did not continue for three seconds") {
                MediaRuntimeTest.status().optDouble("position") > 3 && MediaRuntimeTest.status().optBoolean("playing")
            }
            passed = MediaRuntimeTest.status().isNull("error")
            val status = MediaRuntimeTest.status()
            return (if (passed) usable else -1L) to
                "route=${stream.optString("resolution_route", "direct-media")} " +
                "cache_hit=${stream.optBoolean("resolution_cache_hit")} " +
                (if (audio) "" else "size=${status.optInt("video_width")}x${status.optInt("video_height")} ")
        } catch (failure: AssertionError) {
            return -1L to "failure=\"${failure.message?.substringBefore(':')?.take(80)}\""
        } finally {
            if (!passed) StartupTimings.ended(timing, "failed", SystemClock.elapsedRealtime())
            MediaRuntimeTest.command("stop")
        }
    }

    private fun livePlayback(kind: String, audio: Boolean, target: Long, defaults: List<String>, argument: String) {
        val list = urls(argument, defaults)
        val original = MobileSettings.snapshot()
        // A normal network launch: the offline UI fixture mode only preloads loopback media.
        // Preloading is on, as by default, and allowed on metered networks for the run.
        MobileSettings.save(JSONObject().put("preload_next", true).put("preload_mobile", true).put("autoplay_related", false))
        try {
            ActivityScenario.launch<RustActivity>(
                android.content.Intent(MediaBridge.context, RustActivity::class.java)
            ).use {
                for (url in list) for (prefetched in listOf(false, true)) {
                    val (ms, detail) = play(url, audio, prefetched)
                    samples += Sample(kind, "$url ${if (prefetched) "prefetched" else "cold"}", ms, target, detail)
                }
            }
        } finally {
            MobileSettings.save(original)
        }
        File(MediaBridge.context.getExternalFilesDir(null), "live-media-startup-$kind.txt")
            .writeText(StartupTimings.report())
        report(
            "live-media-timings.txt",
            "Live $kind: tap to ${if (audio) "playing with AudioTrack advancing (not acoustic output)" else "first frame and playing"}; " +
                "cold = caches cleared, prefetched = resolved and first bytes warmed as a visible card does.",
        )
    }

    @Test
    fun liveYoutubeVideosStartWithinHalfASecond() {
        requireLive()
        livePlayback(
            "video",
            audio = false,
            target = 500,
            // Blender open movies on their official channel (CC BY): long-lived public uploads.
            defaults = listOf("https://www.youtube.com/watch?v=aqz-KE-bpKQ", "https://www.youtube.com/watch?v=eRsGyueVLvQ"),
            argument = "vidcue_live_videos",
        )
    }

    @Test
    fun liveSongsStartWithinAHundredMilliseconds() {
        requireLive()
        livePlayback(
            "song",
            audio = true,
            target = 100,
            defaults =
                listOf(
                    "https://music.youtube.com/watch?v=dQw4w9WgXcQ",
                    "https://www.youtube.com/watch?v=kJQP7kiw5Fk",
                ),
            argument = "vidcue_live_songs",
        )
    }

    // ---------------------------------------------------------------- films

    @Test
    fun liveFilmsStartWithinHalfASecond() {
        requireLive()
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val picks =
            MovieCatalog.titles
                .filter { !it.cinecat }
                .mapNotNull { title ->
                    if (title.seasons.isEmpty()) MoviePick(title).takeIf { it.sources.isNotEmpty() }
                    else
                        title.seasons.firstOrNull()?.episodes?.firstOrNull()?.let { MoviePick(title, title.seasons.first().number, it.number) }
                }
        assertTrue("The catalog lists openly licensed films", picks.isNotEmpty())
        for (pick in picks) {
            MovieStartup.reset()
            // A fresh key each run: a saved resume position would make it a seek, not a start.
            val key = "live-test-${pick.key}-${SystemClock.uptimeMillis()}"
            val start = SystemClock.elapsedRealtime()
            ActivityScenario.launch<MoviePlayerActivity>(
                MoviePlayerActivity.streamIntent(context, key, pick.displayTitle, pick.sources)
            ).use {
                val deadline = start + 30_000
                // ExoPlayer may only be read on its application thread.
                fun position(): Long {
                    var value = -1L
                    InstrumentationRegistry.getInstrumentation().runOnMainSync { value = MovieStartup.position() }
                    return value
                }
                while (SystemClock.elapsedRealtime() < deadline &&
                    (MovieStartup.firstFrameAt < 0 || MovieStartup.playingAt < 0 || position() < 3_000)
                ) Thread.sleep(50)
                val frame = MovieStartup.firstFrameAt
                val playing = MovieStartup.playingAt
                val continued = position() >= 3_000
                val ms = if (frame > 0 && playing > 0 && continued) maxOf(frame, playing) - start else -1L
                samples +=
                    Sample(
                        "film",
                        pick.displayTitle,
                        ms,
                        500,
                        "first_frame_ms=${if (frame > 0) frame - start else -1} " +
                            "playing_ms=${if (playing > 0) playing - start else -1} " +
                            "kind=${pick.sources.first().kind} continued=$continued",
                    )
                it.onActivity { activity -> activity.finish() }
            }
        }
        report(
            "live-media-timings.txt",
            "Live films: launch of the film player to first rendered frame and playing; then three seconds played. " +
                "Openly licensed titles and public test streams only.",
        )
    }
}
