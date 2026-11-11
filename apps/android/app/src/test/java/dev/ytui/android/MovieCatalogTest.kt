package dev.ytui.android

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(manifest = Config.NONE, sdk = [35])
class MovieCatalogTest {
    @Test
    fun cyberpunkIsDiscoverableBeyondTheBuiltInPool() {
        val result = kotlinx.coroutines.runBlocking {
            searchMovieCatalog("cyberpunk") { request ->
                assertEquals("movie_catalog_search", request.getString("op"))
                assertEquals("cyberpunk", request.getString("query"))
                JSONObject().put("titles", JSONArray().put(JSONObject()
                    .put("id", "tt12590266").put("type", "series").put("name", "Cyberpunk: Edgerunners")))
            }
        }
        assertEquals("Cyberpunk: Edgerunners", result.titles.single().title)
        assertTrue(result.titles.single().series)
        assertTrue(result.titles.single().seasons.isEmpty())
    }

    @Test
    fun completeMetadataPreservesEverySeasonSpecialAndEpisodeAcrossReload() {
        val videos = JSONArray()
        for (season in 0..2) for (episode in 1..10)
            videos.put(JSONObject().put("season", season).put("episode", episode).put("name", "Episode $episode"))
        val meta = JSONObject().put("id", "tt12590266").put("type", "series")
            .put("name", "Cyberpunk: Edgerunners").put("moviedb_id", 105248).put("videos", videos)
        val full = MovieCatalog.rememberMetadata(JSONObject(meta.toString()))
        assertEquals(listOf(1, 2, 0), full.seasons.map { it.number })
        assertEquals(30, full.seasons.sumOf { it.episodes.size })
        val pick = MoviePick(full, 2, 10)
        assertEquals(pick, MovieCatalog.pick(pick.key))
        assertEquals(105248L, pick.title.tmdb)
        assertNotEquals(full.id, movieTitleFromMetadata(JSONObject().put("id", "tt12590266")
            .put("type", "movie").put("name", "Other film")).id)
    }

    @Test
    fun everyTitleHasPlayableHttpsSourcesAndUniqueIds() {
        val ids = MovieCatalog.titles.map { it.id }
        assertEquals(ids.size, ids.toSet().size)
        for (title in MovieCatalog.titles) {
            if (title.cinecat) {
                // Cinecat titles resolve through the sources API at play time; the catalog
                // carries the TMDB id instead of a direct stream or licence.
                assertTrue(title.id, title.tmdb > 0)
                continue
            }
            val sources =
                if (title.series) title.seasons.flatMap { s -> s.episodes.flatMap { it.sources } } else title.sources
            assertTrue(title.id, sources.isNotEmpty())
            for (source in sources) {
                assertTrue(source.url, source.url.startsWith("https://"))
                assertTrue(source.url, source.kind == "hls" || source.kind == "file")
                assertTrue(source.url, movieUrlValid(source.url))
            }
            assertTrue(title.id, title.license.isNotBlank() && title.credit.isNotBlank())
        }
    }

    @Test
    fun everyFilmAndCinecatTitleHasHighResolutionPosterAndBackdrop() {
        for (title in MovieCatalog.titles) {
            // Stream Lab is a set of player-test streams with generated art, not a film.
            if (title.id == "stream-lab") continue
            assertTrue(title.id, title.poster.startsWith("https://image.tmdb.org/t/p/w500/"))
            assertTrue(title.id, title.backdrop.startsWith("https://image.tmdb.org/t/p/w1280/"))
        }
        assertNull(movieArt(MovieCatalog.find("stream-lab")!!, 400, 600))
    }

    @Test
    fun artCoversItsFrameWithoutFetchingOversizedFiles() {
        val dune = MovieCatalog.titles.first { it.tmdb == 438631L }
        // 132 dp poster card at 2.625x: the square decode box follows the 2:3 height, so the
        // poster is decoded at the width it is drawn at instead of two thirds of it.
        val card = movieArt(dune, 347, 520)!!
        assertFalse(card.backdrop)
        assertTrue(card.decodePixels.toString(), card.decodePixels in 519..522)
        assertTrue(card.url, card.url.contains("/t/p/w500/"))
        // Phone hero (about square): portrait poster at its largest size.
        val phone = movieArt(dune, 1080, 1120)!!
        assertFalse(phone.backdrop)
        assertTrue(phone.url, phone.url.contains("/t/p/w780/"))
        assertTrue(phone.decodePixels > 768)
        // Tablet hero and Continue watching (wide): the 16:9 backdrop.
        val tablet = movieArt(dune, 2560, 900)!!
        assertTrue(tablet.backdrop)
        assertTrue(tablet.url, tablet.url.contains("/t/p/w1280/"))
        val resume = movieArt(dune, 578, 325)!!
        assertTrue(resume.backdrop)
        assertTrue(resume.url, resume.url.contains("/t/p/w780/"))
        // Detail sheet thumbnail fetches a small file; unknown hosts pass through unchanged.
        assertTrue(movieArt(dune, 294, 441)!!.url.contains("/t/p/w342/"))
        assertEquals("https://example.test/a.jpg", TmdbArt.sized("https://example.test/a.jpg", 100, false))
    }

    @Test
    fun picksRoundTripThroughTheirKeys() {
        val film = MoviePick(MovieCatalog.find("sintel")!!)
        assertEquals("sintel", film.key)
        assertEquals(film, MovieCatalog.pick(film.key))
        val lab = MovieCatalog.find("stream-lab")!!
        val episode = MoviePick(lab, 1, 2)
        assertEquals("stream-lab:1:2", episode.key)
        assertEquals(episode, MovieCatalog.pick("stream-lab:1:2"))
        assertEquals("Stream Lab · S1E2 · Bip-bop (fMP4 segments)", episode.displayTitle)
        assertTrue(episode.sources.single().url.contains("fmp4"))
        // A series needs an episode; unknown episodes and titles resolve to nothing.
        assertNull(MovieCatalog.pick("stream-lab"))
        assertNull(MovieCatalog.pick("stream-lab:9:1"))
        assertNull(MovieCatalog.pick("sintel:1:1"))
        assertNull(MovieCatalog.pick("url:abc"))
    }

    @Test
    fun searchMatchesEveryWordAcrossTitleYearGenreAndEpisodes() {
        assertEquals(listOf("sintel"), MovieCatalog.search("SINTEL").map { it.id })
        assertEquals(listOf("big-buck-bunny"), MovieCatalog.search("bunny 2008").map { it.id })
        assertEquals(listOf("stream-lab"), MovieCatalog.search("fmp4").map { it.id })
        assertTrue(MovieCatalog.search("animation").size >= 3)
        assertTrue(MovieCatalog.search("sintel 1999").isEmpty())
        assertTrue(MovieCatalog.search("   ").isEmpty())
    }

    @Test
    fun metaLinesAndSizesReadNaturally() {
        assertEquals("", movieRuntime(0))
        assertEquals("42m", movieRuntime(42))
        assertEquals("1h 48m", movieRuntime(108))
        assertEquals("2012 · 12m · Science fiction, Action", movieMeta(MovieCatalog.find("tears-of-steel")!!))
        assertEquals("2026 · 2 seasons · Test streams", movieMeta(MovieCatalog.find("stream-lab")!!))
        assertEquals("512 KB", movieBytes(512 * 1024))
        assertEquals("48.0 MB", movieBytes(48L shl 20))
        assertEquals("1.50 GB", movieBytes(3L shl 29))
    }

    @Test
    fun typedUrlsAreClassifiedAndValidated() {
        assertEquals("hls", movieKindOf("https://a.example/x/master.M3U8?t=1"))
        assertEquals("hls", movieKindOf("https://a.example/hls/live"))
        assertEquals("file", movieKindOf("https://a.example/film.mp4"))
        assertEquals("file", movieKindOf("https://a.example/?next=x.m3u8"))
        assertTrue(movieUrlValid(" https://a.example/x.m3u8 "))
        assertFalse(movieUrlValid("file:///sdcard/x.mp4"))
        assertFalse(movieUrlValid("https://user:pw@a.example/x.mp4"))
        assertFalse(movieUrlValid("a.example/x.mp4"))
        assertFalse(movieUrlValid(""))
    }

    @Test
    fun checkAttemptsParseAndFailoverOrderPutsTheChosenSourceFirst() {
        val attempts =
            SourceAttempt.parse(
                JSONArray()
                    .put(JSONObject().put("index", 0).put("label", "A").put("ok", false).put("elapsed_ms", 120).put("error", "HTTP 404"))
                    .put(
                        JSONObject()
                            .put("index", 1)
                            .put("label", "B")
                            .put("ok", true)
                            .put("elapsed_ms", 950)
                            .put("height", 1080)
                            .put("throughput_mbps", 31.1)
                    )
            )
        assertEquals("HTTP 404", attempts[0].detail)
        assertEquals("1080p · 31.1 Mb/s · 1.0 s", attempts[1].detail)
        assertTrue(SourceAttempt.parse(null).isEmpty())

        val sources = MovieCatalog.find("tears-of-steel")!!.sources
        assertEquals(sources, movieFailoverOrder(sources, null))
        assertEquals(sources, movieFailoverOrder(sources, 7))
        val order = movieFailoverOrder(sources, 1)
        assertEquals(listOf(sources[1], sources[0], sources[2]), order)
    }

    @Test
    fun resumeAndDownloadRecordsSurviveAndIgnoreDamage() {
        val resume =
            MovieLibrary.parseResume(
                JSONObject().put("sintel", JSONObject().put("position", 60_000).put("duration", 888_000).put("at", 5)).toString()
            )
        assertEquals(MovieResume(60_000, 888_000, 5), resume["sintel"])
        assertEquals(60_000f / 888_000, resume["sintel"]!!.fraction, 1e-6f)
        assertTrue(MovieLibrary.parseResume("not json").isEmpty())
        assertTrue(MovieLibrary.parseResume(null).isEmpty())

        // Near the start or the end, a film starts over.
        assertFalse(MovieLibrary.shouldResume(MovieResume(5_000, 600_000, 0)))
        assertTrue(MovieLibrary.shouldResume(MovieResume(60_000, 600_000, 0)))
        assertFalse(MovieLibrary.shouldResume(MovieResume(590_000, 600_000, 0)))

        val downloads =
            MovieLibrary.parseDownloads(
                JSONObject()
                    .put("sintel", JSONObject().put("title", "Sintel").put("entrypoint", "/d/m/sintel-1/video.media").put("bytes", 9))
                    .put("broken", JSONObject().put("title", "No path"))
                    .toString()
            )
        assertEquals(setOf("sintel"), downloads.keys)
        assertEquals("/d/m/sintel-1/video.media", downloads["sintel"]!!.entrypoint)
        assertEquals("stream-lab_1_2", MovieLibrary.safeName("stream-lab:1:2"))
        assertEquals("url_7f3a", MovieLibrary.safeName("url:7f3a"))
    }

    @Test
    fun trackNamesPreferLabelsThenLanguages() {
        assertEquals("Director's commentary", trackName("Director's commentary", null, "Audio 1"))
        assertEquals("Audio 2", trackName(null, "und", "Audio 2"))
        assertEquals("Audio 3", trackName("stream_5", "", "Audio 3"))
        val german = trackName(null, "de", "x")
        assertNotEquals("x", german)
        assertEquals(trackName("ENGLISH", "en", "x"), "ENGLISH")
    }
}
