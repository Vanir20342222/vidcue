package dev.ytui.android

import android.app.Application
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.os.Looper
import androidx.lifecycle.ViewModelProvider
import java.io.File
import java.time.Duration
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.annotation.Implementation
import org.robolectric.annotation.Implements

/**
 * Renders the phone screens with synthetic content, for design review without an emulator.
 * Runs only when asked: `VIDCUE_SCREENS=<dir> gradlew :app:testDebugUnitTest -Pscreens
 * --tests '*ScreenTourTest*'`. Never loads JNI, a network or an account.
 */
@RunWith(RobolectricTestRunner::class)
// The app's own singletons outlive one test in a shared sandbox: the main tour runs first.
@org.junit.FixMethodOrder(org.junit.runners.MethodSorters.NAME_ASCENDING)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(
    sdk = [35],
    qualifiers = "w360dp-h800dp-xhdpi",
    application = TourApplication::class,
    shadows = [TourNative::class],
)
class ScreenTourTest {
    private val out = System.getenv("VIDCUE_SCREENS")?.let(::File)

    @Test
    fun phoneScreens() {
        assumeTrue("Set VIDCUE_SCREENS to render the screen tour", out != null)
        tour(out!!.apply { mkdirs() })
    }

    /** The same tour on a 320 dp phone: nothing may clip or scroll sideways. */
    @Test
    @Config(qualifiers = "w320dp-h640dp-xhdpi")
    fun phoneScreensNarrow() {
        assumeTrue("Set VIDCUE_SCREENS to render the screen tour", out != null)
        tour(File(out!!, "w320").apply { mkdirs() })
    }

    /** The same tour on a German device: the English UI keeps English numbers. */
    @Test
    @Config(qualifiers = "de-rDE-w360dp-h800dp-xhdpi")
    fun phoneScreensGerman() {
        assumeTrue("Set VIDCUE_SCREENS to render the screen tour", out != null)
        tour(File(out!!, "de").apply { mkdirs() })
    }

    /** The same tour on an 11-inch tablet in landscape (Galaxy Tab S9 size): the sidebar layout. */
    @Test
    @Config(qualifiers = "w1280dp-h800dp-land-xhdpi")
    fun tabletScreens() {
        assumeTrue("Set VIDCUE_SCREENS to render the screen tour", out != null)
        tour(File(out!!, "tablet").apply { mkdirs() }, tablet = true)
    }

    /** The same tablet held upright: narrower than the sidebar layout, so it gets the dock. */
    @Test
    @Config(qualifiers = "w800dp-h1280dp-port-xhdpi")
    fun tabletScreensPortrait() {
        assumeTrue("Set VIDCUE_SCREENS to render the screen tour", out != null)
        tour(File(out!!, "tablet-port").apply { mkdirs() })
    }

    private fun tour(dir: File, tablet: Boolean = false) {
        // Setup is done, so the app opens on Home and loads feeds as it does for a user.
        OnboardingStore.complete(org.robolectric.RuntimeEnvironment.getApplication())
        val intent =
            Intent(Intent.ACTION_MAIN).setClassName("dev.ytui.android", RustActivity::class.java.name)
        preloadArtwork()
        val controller = Robolectric.buildActivity(RustActivity::class.java, intent).setup()
        val activity = controller.get()
        // The window behind the app is the app's background on a device; Robolectric leaves it black.
        activity.window.setBackgroundDrawable(android.graphics.drawable.ColorDrawable(0xff0f0f0f.toInt()))
        val vm = ViewModelProvider(activity)[YtuiViewModel::class.java]
        val log = StringBuilder()
        var frames = 0
        fun settle(ms: Long = 1500) {
            val end = System.currentTimeMillis() + ms
            while (System.currentTimeMillis() < end) {
                shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(16))
                frames++
                Thread.sleep(4)
            }
            log.append("  settled ${ms}ms, frames so far $frames\n")
        }
        fun shot(name: String) {
            settle(2000)
            val started = System.currentTimeMillis()
            log.append("$name tab=${vm.tab.value} ").append(
                (0..3).joinToString { "t$it=${vm.feeds[it].value.cards.size}${if (vm.feeds[it].value.loading) "L" else ""} scroll=${vm.feeds[it].value.scrollIndex}" }
            ).append('\n')
            val view = activity.window.decorView
            // Robolectric lays the window out only when it is drawn: draw once, let the frame's
            // state changes (a header that grew) recompose, then capture.
            view.draw(Canvas(Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)))
            settle(600)
            val bitmap = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
            view.draw(Canvas(bitmap))
            File(dir, "$name.png").outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            // The visible text runs, for the per-screen text budget (text whose semantics are
            // cleared, such as names drawn on covers, is not counted).
            val texts = visibleTexts(view)
            File(dir, "$name.txt").writeText("${texts.size}\n" + texts.joinToString("\n"))
            File(dir, "text-counts.txt").appendText("$name ${texts.size}\n")
            log.append("  captured in ${System.currentTimeMillis() - started}ms at ${System.currentTimeMillis()}\n")
            File(dir, "log.txt").writeText(log.toString())
        }
        settle(4000)
        shot("04-home")
        vm.chooseTab(2)
        settle(2500)
        shot("05-music")
        // Tapping the selected tab again returns a page to its top: shows whether the first
        // capture started scrolled. (Tablets have a sidebar instead of the bottom dock.)
        if (!tablet) run {
            val view = activity.window.decorView
            val density = view.resources.displayMetrics.density
            val x = 16f * density + (view.width - 32f * density) * 3f / 8f
            val y = view.height - 40f * density
            val time = android.os.SystemClock.uptimeMillis()
            view.dispatchTouchEvent(android.view.MotionEvent.obtain(time, time, android.view.MotionEvent.ACTION_DOWN, x, y, 0))
            view.dispatchTouchEvent(android.view.MotionEvent.obtain(time, time + 50, android.view.MotionEvent.ACTION_UP, x, y, 0))
        }
        if (!tablet) {
            settle(2000)
            shot("05b-music-top")
        }
        // Opened from its card, as a user does, so the page knows the playlist's name and owner.
        vm.play(MediaCard(TourNative.collection(0, "playlist", "")))
        settle(9000)
        shot("06-collection")
        // A song loaded: the mini player over the same page.
        PlaybackSource.set("Playing from playlist", "Late Night Drive")
        val playing = TourNative.song(1)
        vm.player.value =
            PlayerState(
                JSONObject()
                    .put("title", playing.getString("title"))
                    .put("media", playing)
                    .put("playing", false) // playing runs tickers that keep the looper busy
                    .put("shuffle", true)
                    .put("position", 47.0)
                    .put("duration", playing.getDouble("duration")),
            )
        shot("07-collection-playing")
        vm.chooseTab(3)
        settle(4000)
        shot("11-radio")
        vm.chooseTab(0)
        settle(2000)
        shot("12-library")
        vm.chooseTab(2)
        vm.navigate(2, SearchRequests.request(2, "nova lane"), remember = true)
        settle(4000)
        shot("10-search-results")
        vm.playerOpen.value = true
        settle(2000)
        shot("08-player")
        File(dir, "ops.txt").writeText(TourNative.seen.sorted().joinToString("\n"))
        controller.pause().stop().destroy()
    }

    @Test
    fun setupScreens() {
        assumeTrue("Set VIDCUE_SCREENS to render the screen tour", out != null)
        val dir = out!!.apply { mkdirs() }
        val intent = Intent(Intent.ACTION_MAIN).setClassName("dev.ytui.android", RustActivity::class.java.name)
        val controller = Robolectric.buildActivity(RustActivity::class.java, intent).setup()
        val activity = controller.get()
        activity.window.setBackgroundDrawable(android.graphics.drawable.ColorDrawable(0xff0f0f0f.toInt()))
        fun settle(ms: Long) {
            val end = System.currentTimeMillis() + ms
            while (System.currentTimeMillis() < end) {
                shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(16))
                Thread.sleep(4)
            }
        }
        fun shot(name: String) {
            settle(2000)
            val view = activity.window.decorView
            // Robolectric lays the window out only when it is drawn: draw once, let the frame's
            // state changes (a header that grew) recompose, then capture.
            view.draw(Canvas(Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)))
            settle(600)
            val bitmap = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
            view.draw(Canvas(bitmap))
            File(dir, "$name.png").outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        }
        /** Taps the main button: centred, 48dp above the bottom edge (under the step dots). */
        fun tapMain() {
            val view = activity.window.decorView
            val x = view.width / 2f
            val y = view.height - 48f * view.resources.displayMetrics.density
            val time = android.os.SystemClock.uptimeMillis()
            view.dispatchTouchEvent(android.view.MotionEvent.obtain(time, time, android.view.MotionEvent.ACTION_DOWN, x, y, 0))
            view.dispatchTouchEvent(android.view.MotionEvent.obtain(time, time + 50, android.view.MotionEvent.ACTION_UP, x, y, 0))
        }
        settle(3000)
        shot("01-welcome")
        tapMain()
        settle(1500)
        shot("02-look")
        tapMain()
        settle(1500)
        shot("03-account")
        controller.pause().stop().destroy()
    }

    /**
     * States other than "signed out with content": loading, a feed that failed (as when offline),
     * empty saved stations and search results, and a Library with nothing played yet.
     */
    @Test
    fun stateScreens() {
        assumeTrue("Set VIDCUE_SCREENS to render the screen tour", out != null)
        val dir = File(out!!, "states").apply { mkdirs() }
        OnboardingStore.complete(org.robolectric.RuntimeEnvironment.getApplication())
        val intent = Intent(Intent.ACTION_MAIN).setClassName("dev.ytui.android", RustActivity::class.java.name)
        TourNative.mode = "loading"
        val controller = Robolectric.buildActivity(RustActivity::class.java, intent).setup()
        val activity = controller.get()
        activity.window.setBackgroundDrawable(android.graphics.drawable.ColorDrawable(0xff0f0f0f.toInt()))
        val vm = ViewModelProvider(activity)[YtuiViewModel::class.java]
        fun settle(ms: Long) {
            val end = System.currentTimeMillis() + ms
            while (System.currentTimeMillis() < end) {
                shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(16))
                Thread.sleep(4)
            }
        }
        fun shot(name: String) {
            val view = activity.window.decorView
            // Robolectric lays the window out only when it is drawn: draw once, let the frame's
            // state changes (a header that grew) recompose, then capture.
            view.draw(Canvas(Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)))
            settle(600)
            val bitmap = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
            view.draw(Canvas(bitmap))
            File(dir, "$name.png").outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            val texts = visibleTexts(view)
            File(dir, "$name.txt").writeText("${texts.size}\n" + texts.joinToString("\n"))
        }
        try {
            settle(2500)
            shot("s1-home-loading")
            TourNative.mode = "error"
            vm.chooseTab(2)
            settle(4000)
            shot("s2-music-error")
            TourNative.mode = "empty"
            vm.chooseTab(3)
            vm.navigate(3, "Favorites", remember = false)
            settle(3000)
            shot("s3-radio-saved-empty")
            vm.chooseTab(2)
            vm.navigate(2, SearchRequests.request(2, "zzzz"), remember = true)
            settle(3000)
            shot("s4-search-empty")
            vm.chooseTab(0)
            settle(3000)
            shot("s5-library-empty")
        } finally {
            TourNative.mode = ""
            controller.pause().stop().destroy()
        }
    }
}

/**
 * Puts every sample cover into the artwork memory caches before the screens compose. Under
 * Robolectric the loader's hand-offs do not finish while a screen is shown, so covers would stay
 * placeholders; real devices load them normally.
 */
private fun preloadArtwork() {
    fun lru(name: String) =
        ArtworkCache::class.java.getDeclaredField(name).apply { isAccessible = true }.get(ArtworkCache)
            as android.util.LruCache<String, Bitmap>
    val memory = lru("memory")
    // The tour preloads every cover at once; a device's cache holds what is on screen.
    memory.resize(96 * 1024 * 1024)
    val recent = lru("recentByItem")
    fun bitmap(card: JSONObject, pixels: Int): Bitmap {
        val data = TourNative.thumbnail(card.toString(), pixels)
        return Bitmap.createBitmap(data, 2, data[0], data[0], data[1], Bitmap.Config.ARGB_8888)
    }
    for (card in TourNative.allCards()) {
        val media = MediaCard(card)
        val wide = media.kind == "video"
        // Albums and playlists show in large shelf cards (176dp); songs and stations smaller.
        val size = if (wide) 768 else if (media.collection) 480 else 320
        memory.put("${media.key}:${media.artworkSource}:$size", bitmap(card, size))
        // The playing song also fills the full-screen player's cover.
        if (card.optString("url") == TourNative.song(1).optString("url"))
            memory.put("${media.key}:${media.artworkSource}:1024", bitmap(card, 1024))
        recent.put(media.key, bitmap(card, if (wide) 320 else 240))
    }
}

/** The app without its engine start-up: the shadow bridge answers instead. */
class TourApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        MediaBridge.context = this
    }
}

/** Synthetic engine replies: fictional artists, channels and stations, generated cover art. */
@Implements(NativeBridge::class)
class TourNative {
    companion object {
        val seen = java.util.concurrent.ConcurrentHashMap.newKeySet<String>()

        /** "" for content; "loading" holds pages back, "error" fails them, "empty" returns none. */
        @Volatile @JvmStatic var mode = ""

        @JvmStatic @Implementation fun __staticInitializer__() {}

        @JvmStatic @Implementation fun init(files: String, cache: String): String = ""

        @JvmStatic @Implementation fun beginAccountChange(): Long = 0

        @JvmStatic @Implementation fun accountIntent(): Long = 0

        @JvmStatic @Implementation fun cancel(id: Long) {}

        @JvmStatic @Implementation fun activateContext(id: String) {}

        @JvmStatic
        @Implementation
        fun thumbnail(media: String, pixels: Int): IntArray {
            val card = JSONObject(media)
            val seed = (card.optString("thumbnail") + card.optString("title")).hashCode()
            val wide = card.optString("kind", "video") == "video" && !card.optString("url").contains("music.")
            val w = pixels.coerceIn(16, 1280)
            val h = if (wide) w * 9 / 16 else w
            val started = System.nanoTime()
            return art(seed, w, h).also { seen += "thumbnail $w px ${(System.nanoTime() - started) / 1_000_000} ms" }
        }

        @JvmStatic
        @Implementation
        fun request(payload: String): String {
            val request = JSONObject(payload)
            val op = request.optString("op")
            seen += op
            if (op == "browse" && mode == "loading") Thread.sleep(8000)
            if (op == "browse" && mode == "error")
                return JSONObject()
                    .put("ok", false)
                    .put("code", "network")
                    .put("error", "Could not reach YouTube. Check the connection and try again.")
                    .toString()
            if (mode == "empty" && (op == "browse" || op == "history_list"))
                return JSONObject()
                    .put("ok", true)
                    .put("data", if (op == "browse") JSONObject().put("items", JSONArray()) else JSONArray())
                    .toString()
            val data: Any =
                when (op) {
                    "theme" -> palette()
                    "themes" -> JSONArray().put("system").put("vidcue-dark").put("vidcue-light")
                    "palettes" ->
                        JSONObject()
                            .put("vidcue-dark", palette())
                            .put("system", palette())
                            .put(
                                "vidcue-light",
                                palette()
                                    .put("name", "vidcue-light")
                                    .put("background", JSONArray(listOf(0xf5, 0xf6, 0xf8)))
                                    .put("surface", JSONArray(listOf(0xff, 0xff, 0xff)))
                                    .put("text", JSONArray(listOf(0x15, 0x18, 0x1c)))
                                    .put("light", true),
                            )
                    // Reduced motion: still frames only, and entry animations that wait for a frame clock
                    // the test does not run in real time do not leave pages blank.
                    "settings" -> JSONObject().put("theme", "vidcue-dark").put("thumbnails", true).put("reduced_motion", true)
                    "account" -> JSONObject().put("connected", false)
                    "browse" ->
                        page(request.optInt("tab"), request.opt("request")).also {
                            seen += "browse ${request.optInt("tab")} ${request.opt("request")} -> ${it.getJSONArray("items").length()}"
                        }
                    // Someone who has listened a little: Library and Music show what they played.
                    "history_list" -> JSONArray(history().map { it.toString() }.map(::JSONObject))
                    "queue_list", "downloads", "offline_list", "search_history" -> JSONArray()
                    else -> JSONObject()
                }
            return JSONObject().put("ok", true).put("data", data).toString()
        }

        private fun palette() =
            JSONObject()
                .put("name", "vidcue-dark")
                .put("background", JSONArray(listOf(0x10, 0x14, 0x18)))
                .put("surface", JSONArray(listOf(0x1b, 0x23, 0x2c)))
                .put("text", JSONArray(listOf(0xe5, 0xed, 0xf4)))
                .put("muted", JSONArray(listOf(0xa8, 0xb9, 0xc9)))
                .put("accent", JSONArray(listOf(0x6d, 0xdb, 0xc1))) // the app maps Vidcue Dark to the logo pink
                .put("light", false)

        private val artists = listOf("Nova Lane", "The Quiet Harbors", "Mira Sol", "Atlas Room", "Juno Park", "Eli Marsh")
        private val songs =
            listOf(
                "Paper Moons", "Northbound", "Glass Garden", "Slow Weather", "Halflight", "Small Hours",
                "Satellite Heart", "Driftwood", "Velvet Static", "Summer in Lisbon", "Neon Rain", "Open Water",
            )

        fun song(i: Int) =
            JSONObject()
                .put("title", songs[i % songs.size])
                .put("creator", "${artists[i % artists.size]} • ${(i + 3) * 7}M plays")
                .put("url", "https://music.youtube.com/watch?v=song${i}xxxxx")
                .put("kind", "song")
                .put("duration", 150 + i * 13)
                .put("thumbnail", "art:song$i")

        fun collection(i: Int, kind: String, section: String) =
            JSONObject()
                .put("title", listOf("Late Night Drive", "Morning Coffee", "Indie Essentials", "Focus Flow", "Weekend Mix", "Acoustic Calm")[i % 6])
                .put("creator", "${artists[i % artists.size]}, ${artists[(i + 2) % artists.size]} and more")
                .put("url", "https://music.youtube.com/playlist?list=PL$kind$i")
                .put("id", "PL$kind$i")
                .put("kind", kind)
                .put("thumbnail", "art:$kind$i")
                .put("section", section)

        private fun video(i: Int) =
            JSONObject()
                .put("title", listOf(
                    "Building a tiny cabin in the mountains, start to finish",
                    "The science of sleep, explained in 12 minutes",
                    "Street food tour of Osaka at night",
                    "Why this bridge has stood for 900 years",
                    "Learning the cello in 30 days",
                    "Ten minute vegetable curry anyone can make",
                )[i % 6])
                .put("creator", listOf("Field Notes", "Curious Minds", "Night Market", "Old Stones", "Practice Room", "Home Kitchen")[i % 6])
                .put("url", "https://www.youtube.com/watch?v=vid${i}xxxxxx")
                .put("kind", "video")
                .put("duration", 420 + i * 97)
                .put("view_count", 120_000L * (i + 2) * (i + 1))
                .put("upload_date", "2026111${i % 6}")
                .put("thumbnail", "art:video$i")
                .put("channel_thumbnail", "art:channel$i")

        private fun station(i: Int) =
            JSONObject()
                .put("title", listOf("Lofi Study", "Jazz Lounge", "Synthwave FM", "Classical Morning", "Ambient Drift", "Indie Pop", "City Talk", "Tokyo Hits", "Night Drive", "Coastal FM", "Blue Hour", "Deep Focus")[i % 12])
                .put("creator", listOf("Germany", "France", "The Netherlands", "Austria", "Iceland", "Sweden", "Canada", "Japan")[i % 8])
                .put("description", listOf("lofi,chill", "jazz,lounge", "electronic,synthwave", "classical", "ambient,drone", "indie,pop", "news,talk", "j-pop")[i % 8])
                .put("url", "https://radio.example.invalid/$i")
                .put("kind", "station")
                .put("live", true)
                .put("thumbnail", "art:station$i")

        /** Played earlier; not on the sample pages, so no page drops them as watched. */
        fun history(): List<JSONObject> = (30 until 36).map { song(it).put("audio", true) }

        /** Every card the tour shows, for preloading covers. */
        fun allCards(): List<JSONObject> =
            history() + listOf(
                    page(1, SearchRequests.topic("All")),
                    page(2, "Home"),
                    page(2, JSONObject().put("Collection", "x")),
                    page(2, JSONObject().put("Search", "x")),
                    page(3, "Cliamp"),
                )
                .flatMap { p -> (0 until p.getJSONArray("items").length()).map { p.getJSONArray("items").getJSONObject(it) } }
                .distinctBy { it.optString("url") }

        private fun page(tab: Int, request: Any?): JSONObject {
            val items = JSONArray()
            val r = request as? JSONObject
            when {
                tab == 1 && request == "Home" ->
                    return JSONObject().put("items", items).put("notice", "No anonymous Home recommendations were returned.")
                tab == 1 -> (0 until 8).forEach { items.put(video(it)) }
                tab == 2 && r?.has("Collection") == true -> (0 until 14).forEach { items.put(song(it + 2)) }
                tab == 2 && (r?.has("Search") == true || r?.has("MusicSearch") == true) -> {
                    items.put(JSONObject().put("title", "Nova Lane").put("creator", "Artist • 1.2M subscribers").put("url", "https://music.youtube.com/channel/UCnova").put("kind", "artist").put("thumbnail", "art:artist"))
                    (0 until 6).forEach { items.put(song(it).let { s -> s.put("creator", "Song • ${s.optString("creator")} • ${2 + it}:1$it") }) }
                    items.put(collection(1, "album", "").put("creator", "Album • Nova Lane"))
                }
                tab == 2 -> {
                    (0 until 12).forEach { items.put(song(it).put("section", "Quick picks")) }
                    (0 until 6).forEach { items.put(collection(it, "playlist", "Mixed for you")) }
                    (0 until 6).forEach { items.put(collection(it + 3, "album", "New releases")) }
                }
                tab == 3 -> (0 until 12).forEach { items.put(station(it)) }
            }
            val page = JSONObject().put("items", items).put("title", "Sample")
            // Music Home carries mood chips, as YouTube Music's does.
            if (tab == 2 && request == "Home")
                page.put(
                    "categories",
                    JSONArray(
                        listOf("Relax", "Energize", "Workout", "Focus", "Commute").map { mood ->
                            JSONObject()
                                .put("title", mood)
                                .put("request", JSONObject().put("Category", JSONObject().put("endpoint", "browse").put("payload", JSONObject().put("browseId", "FEmusic_$mood"))))
                        }
                    ),
                )
            return page
        }

        /** A soft two-colour gradient with a circle, distinct per item. */
        private fun art(seed: Int, w: Int, h: Int): IntArray {
            val hue = ((seed ushr 3) % 360 + 360) % 360
            val a = hsv(hue.toFloat(), 0.55f, 0.55f)
            val b = hsv(((hue + 40) % 360).toFloat(), 0.65f, 0.25f)
            val out = IntArray(w * h + 2)
            out[0] = w
            out[1] = h
            val cx = w * (0.3f + ((seed ushr 9) and 3) * 0.12f)
            val cy = h * 0.45f
            val radius = minOf(w, h) * 0.28f
            for (y in 0 until h) for (x in 0 until w) {
                val t = (x + y).toFloat() / (w + h)
                var c = mix(a, b, t)
                val dx = x - cx
                val dy = y - cy
                if (dx * dx + dy * dy < radius * radius) c = mix(c, 0xffffffff.toInt(), 0.18f)
                out[2 + y * w + x] = c
            }
            return out
        }

        private fun mix(a: Int, b: Int, t: Float): Int {
            fun ch(c: Int, s: Int) = (c shr s) and 0xff
            fun m(s: Int) = (ch(a, s) * (1 - t) + ch(b, s) * t).toInt().coerceIn(0, 255)
            return (0xff shl 24) or (m(16) shl 16) or (m(8) shl 8) or m(0)
        }

        private fun hsv(h: Float, s: Float, v: Float): Int = android.graphics.Color.HSVToColor(floatArrayOf(h, s, v))
    }
}

/** Text on screen in the Compose tree under [root]: one entry per text run inside the window. */
private fun visibleTexts(root: android.view.View): List<String> {
    fun find(v: android.view.View): androidx.compose.ui.platform.ViewRootForTest? =
        v as? androidx.compose.ui.platform.ViewRootForTest
            ?: (v as? android.view.ViewGroup)?.let { g -> (0 until g.childCount).firstNotNullOfOrNull { find(g.getChildAt(it)) } }
    val owner = find(root)?.semanticsOwner ?: return emptyList()
    val window = androidx.compose.ui.geometry.Rect(0f, 0f, root.width.toFloat(), root.height.toFloat())
    val out = ArrayList<String>()
    fun walk(node: androidx.compose.ui.semantics.SemanticsNode) {
        val bounds = node.boundsInRoot
        val key = androidx.compose.ui.semantics.SemanticsProperties.Text
        val text: List<androidx.compose.ui.text.AnnotatedString>? = if (key in node.config) node.config[key] else null
        if (text != null && bounds.height > 1f && bounds.overlaps(window))
            text.map { it.text }.filter { it.isNotBlank() }.forEach(out::add)
        node.children.forEach(::walk)
    }
    // The merged tree, as TalkBack reads it: content hidden behind the open player or drawn on
    // covers with cleared semantics is left out; each line of a merged row still counts once.
    walk(owner.rootSemanticsNode)
    return out
}
