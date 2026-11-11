package dev.ytui.android

import java.util.Locale
import org.json.JSONArray
import org.json.JSONObject

/**
 * Movies (alpha) catalog: openly licensed films and public player-test streams, plus a Cinecat
 * section whose streams are resolved through the Cinecat sources API at play time. Direct
 * sources here are HLS or file URLs published for free viewing or player testing; the shared
 * Rust transfer code checks, plays and downloads them.
 */
internal data class MovieSubtitle(val url: String, val language: String, val display: String, val format: String) {
    fun json(): JSONObject =
        JSONObject().put("url", url).put("language", language).put("display", display).put("format", format)
}

internal data class MovieSource(
    val label: String,
    val kind: String,
    val url: String,
    /** Headers the stream requires (Cinecat providers demand Origin/Referer); empty for plain URLs. */
    val headers: Map<String, String> = emptyMap(),
    /** Sidecar captions the engine resolved; HLS renditions arrive with the playlist instead. */
    val subtitles: List<MovieSubtitle> = emptyList(),
) {
    fun json(): JSONObject =
        JSONObject()
            .put("label", label)
            .put("kind", kind)
            .put("url", url)
            .put("headers", JSONObject(headers))
            .put("subtitles", JSONArray(subtitles.map { it.json() }))
}

/** Reads a source from the engine's JSON (or the app's own intent payloads). */
internal fun movieSourceFromJson(s: JSONObject): MovieSource {
    val headers = mutableMapOf<String, String>()
    s.optJSONObject("headers")?.let { h -> h.keys().forEach { k -> headers[k] = h.optString(k) } }
    val subs = s.optJSONArray("subtitles") ?: JSONArray()
    return MovieSource(
        s.optString("label"),
        s.optString("kind"),
        s.optString("url"),
        headers,
        (0 until subs.length()).mapNotNull { j ->
            subs.optJSONObject(j)?.let {
                MovieSubtitle(it.optString("url"), it.optString("language"), it.optString("display"), it.optString("format"))
            }
        },
    )
}

internal data class MovieEpisode(
    val number: Int,
    val title: String,
    val runtimeMinutes: Int,
    val synopsis: String,
    val sources: List<MovieSource>,
)

internal data class MovieSeason(val number: Int, val episodes: List<MovieEpisode>)

internal data class MovieTitle(
    val id: String,
    val title: String,
    val year: Int,
    val runtimeMinutes: Int,
    val genres: List<String>,
    val synopsis: String,
    /** Who made it, as the licence asks to credit them. */
    val credit: String,
    val license: String,
    /** Portrait (2:3) poster image; blank draws generated art. */
    val poster: String = "",
    /** Landscape (16:9) still for wide frames such as a tablet hero; blank falls back to [poster]. */
    val backdrop: String = "",
    val sources: List<MovieSource> = emptyList(),
    val seasons: List<MovieSeason> = emptyList(),
    /** What this title exercises in the player, shown as chips. */
    val features: List<String> = emptyList(),
    /** Non-zero for Cinecat titles: the TMDB id the sources API resolves streams for. */
    val tmdb: Long = 0,
) {
    val series: Boolean
        get() = seasons.isNotEmpty()

    /** Cinecat titles resolve their streams through the sources API at play time. */
    val cinecat: Boolean
        get() = tmdb > 0
}

/** One playable thing: a film, or one episode of a series. */
internal data class MoviePick(val title: MovieTitle, val season: Int? = null, val episode: Int? = null) {
    val episodeEntry: MovieEpisode?
        get() =
            if (season == null || episode == null) null
            else title.seasons.firstOrNull { it.number == season }?.episodes?.firstOrNull { it.number == episode }

    val sources: List<MovieSource>
        get() = episodeEntry?.sources ?: title.sources

    /** Stable key for resume positions and downloads: `id` or `id:season:episode`. */
    val key: String
        get() = if (season != null && episode != null) "${title.id}:$season:$episode" else title.id

    /** `Stream Lab · S1E2 · Bip-bop (fMP4)` or the film title. */
    val displayTitle: String
        get() = episodeEntry?.let { "${title.title} · S${season}E$episode · ${it.title}" } ?: title.title
}

internal object MovieCatalog {
    private fun archive(item: String, file: String, label: String = "Internet Archive") =
        MovieSource(label, "file", "https://archive.org/download/$item/$file")

    /** TMDB's public image CDN. Paths are stable; [TmdbArt.sized] picks the size per frame. */
    private fun poster(path: String) = TmdbArt.poster(path)

    private fun backdrop(path: String) = TmdbArt.backdrop(path)

    private const val BLENDER = "Blender Foundation"

    /** The TMDB titles the Cinecat sources API resolves: same pool the standalone CLI ships. */
    private fun cinecat(
        label: String,
        tmdb: Long,
        posterPath: String,
        backdropPath: String,
        season: Int = 0,
        episode: Int = 0,
    ): MovieTitle {
        val series = season > 0 && episode > 0
        val seasons =
            if (series)
                listOf(
                    MovieSeason(
                        season,
                        listOf(MovieEpisode(episode, "Episode $episode", 0, "", emptyList())),
                    )
                )
            else emptyList()
        return MovieTitle(
            // The id carries the TMDB id only; the season/episode live in the pick key, so
            // `pick("cinecat-1396:1:1")` resolves like any other series title.
            id = "cinecat-$tmdb",
            title = label,
            year = 0,
            runtimeMinutes = 0,
            genres = listOf("Cinecat"),
            synopsis =
                "Streamed through the Cinecat sources API: the app asks each provider for the " +
                    "title and plays the first one that actually delivers video, failing over " +
                    "through the rest if a source stops mid-film.",
            credit = "Cinecat sources API",
            license = "",
            features = listOf("Cinecat", "Failover"),
            poster = poster(posterPath),
            backdrop = backdrop(backdropPath),
            tmdb = tmdb,
            seasons = seasons,
        )
    }

    /** Cinecat titles mirror the standalone CLI's curated, live-verified TMDB pool. */
    private val cinecatTitles: List<MovieTitle> =
        listOf(
            cinecat("Fight Club", 550L, "jSziioSwPVrOy9Yow3XhWIBDjq1.jpg", "c6OLXfKAk5BKeR6broC8pYiCquX.jpg"),
            cinecat("Inception", 27205L, "xlaY2zyzMfkhk0HSC5VUwzoZPU1.jpg", "8ZTVqvKDQ8emSGUEMjsS4yHAwrp.jpg"),
            cinecat("The Matrix", 603L, "dXNAPwY7VrqMAo51EKhhCJfaGb5.jpg", "tlm8UkiQsitc8rSuIAscQDCnP8d.jpg"),
            cinecat("Interstellar", 157336L, "yQvGrMoipbRoddT0ZR8tPoR7NfX.jpg", "8sNiAPPYU14PUepFNeSNGUTiHW.jpg"),
            cinecat("Parasite", 496243L, "7IiTTgloJzvGI1TAYymCfbfl3vT.jpg", "TU9NIjwzjoKPwQHoHshkFcQUCG.jpg"),
            cinecat("Avengers Endgame", 299534L, "jtDv3lCnST0fZRvdYFwR5CVwK2x.jpg", "7RyHsO4yDXtBv1zUU3mTpHeQ0d5.jpg"),
            cinecat("Dune", 438631L, "v1tRXZ4JtD2Iv6fjkPvT4GiwslV.jpg", "qVgZu5BTx6pu4owCvVOm4zjTfOi.jpg"),
            cinecat("Dune Part Two", 693134L, "6izwz7rsy95ARzTR3poZ8H6c5pp.jpg", "eZ239CUp1d6OryZEBPnO2n87gMG.jpg"),
            cinecat("Oppenheimer", 872585L, "8Gxv8gSFCU0XGDykEGv7zR1n2ua.jpg", "neeNHeXjMF5fXoCJRsOmkNGC7q.jpg"),
            cinecat("Barbie", 346698L, "iuFNMS8U5cb6xfzi51Dbkovj7vM.jpg", "1esAE8sLJRWWFsLLeh5r3g2WanI.jpg"),
            cinecat("Spider-Man No Way Home", 634649L, "1g0dhYtq4irTY1GPXvft6k4YLjm.jpg", "iQFcwSGbZXMkeyKrxbPnwnRo5fl.jpg"),
            cinecat("The Godfather", 238L, "3bhkrj58Vtu7enYsRolD1fZdja1.jpg", "tSPT36ZKlP2WVHJLM4cQPLSzv3b.jpg"),
            cinecat("Spirited Away", 129L, "39wmItIWsg5sZMyRUHLkWBcuVCM.jpg", "m4TUa2ciEWSlk37rOsjiSIvZDXE.jpg"),
            cinecat("Mad Max Fury Road", 76341L, "ulcAi4dKpAjHwYGS08vNyx9H6I9.jpg", "gqrnQA6Xppdl8vIb2eJc58VC1tW.jpg"),
            cinecat("Everything Everywhere All At Once", 545611L, "u68AjlvlutfEIcpmbYpKcdi09ut.jpg", "fIwiFha3WPu5nHkBeMQ4GzEk0Hv.jpg"),
            cinecat("Top Gun Maverick", 361743L, "n0YuM4f5lvGAP6MAW2kBIzugXnc.jpg", "AaV1YIdWKnjAIAOe8UUKBFm327v.jpg"),
            cinecat("Metropolis", 19L, "kr9wXRN23zLuWJIelahas1mtnYj.jpg", "eeMoFKxjjiCi6iep2GEZtSAMYIr.jpg"),
            cinecat("The Thing", 1091L, "tzGY49kseSE9QAKk47uuDGwnSCu.jpg", "r9leYNa8nTRCceZrZhP1DXkgKVb.jpg"),
            cinecat("The Terminator", 218L, "qvktm0BHcnmDpul4Hz01GIazWPr.jpg", "ffdqHMWkh1h9MABwIfbfRJhgFW6.jpg"),
            cinecat("Jurassic Park", 329L, "d9mtMGQDLANKieb9PbD3yK7xxzo.jpg", "f082quUW2kAtZXFZVGEfhwacZoG.jpg"),
            cinecat("Toy Story", 862L, "uXDfjJbdP4ijW5hWSBrPrlKpxab.jpg", "8AqWECiUHnz1Crj69uB9vhiK9Le.jpg"),
            cinecat("Home Alone", 771L, "onTSipZ8R3bliBdKfPtsDuHTdlL.jpg", "ih2xVgeMS8R5WUetYE8Mr9hVTlB.jpg"),
            cinecat("Project Hail Mary", 687163L, "iOb2fjXLbpJgyQXe46n1WtGCnaa.jpg", "8Tfys3mDZVp4tNoH2ktm06a0Tau.jpg"),
            cinecat("The Dark Knight", 155L, "qJ2tW6WMUDux911r6m7haRef0WH.jpg", "cfT29Im5VDvjE0RpyKOSdCKZal7.jpg"),
            cinecat("Resident Evil", 1423191L, "gAgWJhUSQWCgtKR8170nMkYlBda.jpg", "3icyRAqgakNcQn6aDVz9libFmBA.jpg"),
            cinecat("Apocalypse Z", 1507696L, "wXln4DUrwcyjcFPwgQxFPYTlG1Z.jpg", "o3XwuDUj0n8qnK91p6q8LKfr6rj.jpg"),
            cinecat("Hope", 1058424L, "6VY1ZKgpsc8CfGsQcCGiYEgHdH0.jpg", "j96rurZqmBjyY37Fncl3IB0qJNe.jpg"),
            cinecat("Spider-Man Brand New Day", 969681L, "bjiS5ipwxb9JFy3XRRN4OAilSeX.jpg", "subj7knTn4DeS6KS7qI66EQKe9i.jpg"),
            cinecat("The Odyssey", 1368337L, "5rhTDKUhPYvpdQIijFIs5VoWsON.jpg", "twiVn9oFXOVR0uoYgawyEBlnFu8.jpg"),
            cinecat("Heart of the Beast", 1263337L, "6sYemVUn79UPKz1ZsSG6kPU4RVy.jpg", "6uVRPFflyJpo8Cx872olmlGTWla.jpg"),
            cinecat("A Story of Derry", 1813355L, "4PtzGcegF5Po1NmDAW9IklyvAUh.jpg", "z26LxXDMmAIzKr06FPDudCJyvsM.jpg"),
            cinecat("The Hotel New Hampshire", 11308L, "9XGGGniMtBqhox7JAV5JujlUqFz.jpg", "6ja3HVez4A4XPNSMHhuOiu2UTXm.jpg"),
            cinecat("Coco", 354912L, "6Ryitt95xrO8KXuqRGm1fUuNwqF.jpg", "g7CHF8gTLGooTbP4GznIGwaqAGL.jpg"),
            cinecat("The Mongoose", 1294189L, "eSS5mvSG84UUuvtbHel5Yu3Wik4.jpg", "aezAUwTATFwXVvN1HUTglE8NGwu.jpg"),
            cinecat("Verity", 1283515L, "rmaNMA1seHqpjtnYKzyynFAr1g8.jpg", "cKetrF2dAnQ0AMV5i4BMOs41I5P.jpg"),
            cinecat("Fall 2 Deadpoint", 1101412L, "fgSm5ylwiXbIHn8UbUXDjk9RRu4.jpg", "nGau0KBOaRILSK5MqFcuTfv7GMl.jpg"),
            cinecat("La Bola Negra", 1422041L, "7HwFQTMghL6HOvzEnXYUvyuZXeh.jpg", "bDsmYvrzto7xrEwqDpLC3SPhn9A.jpg"),
            cinecat("Breaking Bad", 1396L, "anFx9aTOOYqgS3v7x3R84Kz67ly.jpg", "tsRy63Mu5cu8etL1X7ZLyf7UP1M.jpg", 1, 1),
            cinecat("Friends", 1668L, "2koX1xLkpTQM4IZebYvKysFW1Nh.jpg", "l0qVZIpXtIo7km9u5Yqh0nKPOr5.jpg", 1, 1),
        )

    val titles: List<MovieTitle> = listOf(
            MovieTitle(
                id = "tears-of-steel",
                title = "Tears of Steel",
                year = 2012,
                runtimeMinutes = 12,
                genres = listOf("Science fiction", "Action"),
                synopsis =
                    "In a future Amsterdam, a group of scientists and warriors try to save the world " +
                        "from destructive robots by revisiting the painful breakup that started it all.",
                credit = "$BLENDER · mango.blender.org",
                license = "CC BY 3.0",
                poster = poster("8qy3jRmaHR7f8VZh3iXCqCWfFsH.jpg"),
                backdrop = backdrop("fOy6SL5Zs2PFcNXwqEPIDPrLB1q.jpg"),
                sources =
                    listOf(
                        MovieSource("Mux (HLS, subtitles)", "hls", "https://test-streams.mux.dev/tos_ismc/main.m3u8"),
                        MovieSource(
                            "Unified Streaming (HLS, 5 qualities)",
                            "hls",
                            "https://demo.unified-streaming.com/k8s/features/stable/video/tears-of-steel/tears-of-steel.ism/.m3u8",
                        ),
                        archive("Tears-of-Steel", "tears_of_steel_720p.mp4", "Internet Archive (MP4 720p)"),
                    ),
                features = listOf("HLS", "Subtitles", "3 sources"),
            ),
            MovieTitle(
                id = "big-buck-bunny",
                title = "Big Buck Bunny",
                year = 2008,
                runtimeMinutes = 10,
                genres = listOf("Animation", "Comedy"),
                synopsis =
                    "A giant, gentle rabbit takes comical revenge on the three rodents who bully the " +
                        "smaller creatures of his forest.",
                credit = "$BLENDER · peach.blender.org",
                license = "CC BY 3.0",
                poster = poster("i9jJzvoXET4D9pOkoEwncSdNNER.jpg"),
                backdrop = backdrop("xtdybjRRZ15mCrPOvEld305myys.jpg"),
                sources =
                    listOf(
                        MovieSource("Mux (HLS, 240p–1080p)", "hls", "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8"),
                        archive("BigBuckBunny_124", "Content/big_buck_bunny_720p_surround.mp4", "Internet Archive (MP4 720p)"),
                    ),
                features = listOf("HLS", "5 qualities", "2 sources"),
            ),
            MovieTitle(
                id = "sintel",
                title = "Sintel",
                year = 2010,
                runtimeMinutes = 15,
                genres = listOf("Fantasy", "Animation"),
                synopsis =
                    "A lonely young woman searches the world for the baby dragon she once rescued " +
                        "and raised.",
                credit = "$BLENDER · durian.blender.org",
                license = "CC BY 3.0",
                poster = poster("l98Zns4oQe1BSvbF4hJ8oL6VSf0.jpg"),
                backdrop = backdrop("msqeiEyIRpPAtrCeRGFNZQ9tkJL.jpg"),
                sources = listOf(archive("Sintel", "sintel-2048-surround.mp4", "Internet Archive (MP4)")),
                features = listOf("Progressive MP4"),
            ),
            MovieTitle(
                id = "elephants-dream",
                title = "Elephants Dream",
                year = 2006,
                runtimeMinutes = 11,
                genres = listOf("Animation", "Surreal"),
                synopsis =
                    "Two men, Emo and Proog, wander an endless, shifting machine world that each of " +
                        "them sees very differently.",
                credit = "Orange Open Movie Project · $BLENDER",
                license = "CC BY 2.5",
                poster = poster("9zROtU9TkpZQrOuEaMAp68FOWLK.jpg"),
                backdrop = backdrop("9bJDwuhza19HQcYA99FeslLYmUm.jpg"),
                sources =
                    listOf(
                        archive("ElephantsDream", "ed_hd.mp4", "Internet Archive (HD)"),
                        archive("ElephantsDream", "ed_1024_512kb.mp4", "Internet Archive (512 kb/s)"),
                    ),
                features = listOf("Progressive MP4", "2 sources"),
            ),
            MovieTitle(
                id = "cosmos-laundromat",
                title = "Cosmos Laundromat",
                year = 2015,
                runtimeMinutes = 12,
                genres = listOf("Animation", "Drama"),
                synopsis =
                    "On a desolate island, a despairing sheep named Franck meets a mysterious " +
                        "salesman who offers him the gift of a lifetime.",
                credit = "$BLENDER · gooseberry.blender.org",
                license = "CC BY 4.0",
                poster = poster("5ZXi0oitpEgAdoJglFTc5SZF9nt.jpg"),
                backdrop = backdrop("f2wABsgj2lIR2dkDEfBZX8p4Iyk.jpg"),
                sources =
                    listOf(
                        archive(
                            "CosmosLaundromatFirstCycle",
                            "Cosmos%20Laundromat%20-%20First%20Cycle%20(1080p).mp4",
                            "Internet Archive (MP4 1080p)",
                        )
                    ),
                features = listOf("Progressive MP4", "1080p"),
            ),
            MovieTitle(
                id = "stream-lab",
                title = "Stream Lab",
                year = 2026,
                runtimeMinutes = 0,
                genres = listOf("Test streams"),
                synopsis =
                    "Public player-test streams from Apple and Mux, grouped as episodes. Each one " +
                        "exercises a different part of the player: transport-stream and fragmented " +
                        "MP4 segments, quality ladders, subtitles and source failover.",
                credit = "Apple Developer HLS examples · Mux test streams",
                license = "Published for player testing",
                seasons =
                    listOf(
                        MovieSeason(
                            1,
                            listOf(
                                MovieEpisode(
                                    1,
                                    "Bip-bop (TS segments)",
                                    10,
                                    "Apple's advanced example: six resolutions from 270p to 1080p, " +
                                        "MPEG-TS segments and English subtitles.",
                                    listOf(
                                        MovieSource(
                                            "Apple (HLS, TS)",
                                            "hls",
                                            "https://devstreaming-cdn.apple.com/videos/streaming/examples/img_bipbop_adv_example_ts/master.m3u8",
                                        )
                                    ),
                                ),
                                MovieEpisode(
                                    2,
                                    "Bip-bop (fMP4 segments)",
                                    10,
                                    "The same programme packaged as fragmented MP4, with " +
                                        "initialisation segments.",
                                    listOf(
                                        MovieSource(
                                            "Apple (HLS, fMP4)",
                                            "hls",
                                            "https://devstreaming-cdn.apple.com/videos/streaming/examples/img_bipbop_adv_example_fmp4/master.m3u8",
                                        )
                                    ),
                                ),
                                MovieEpisode(
                                    3,
                                    "Mux test 001",
                                    9,
                                    "A small stream with two low resolutions: quick to check and " +
                                        "download.",
                                    listOf(
                                        MovieSource("Mux (HLS)", "hls", "https://test-streams.mux.dev/test_001/stream.m3u8")
                                    ),
                                ),
                            ),
                        ),
                        MovieSeason(
                            2,
                            listOf(
                                MovieEpisode(
                                    1,
                                    "Failover drill",
                                    12,
                                    "The first source is deliberately missing, so playback has to " +
                                        "fail over to the second.",
                                    listOf(
                                        MovieSource(
                                            "Missing source (expected to fail)",
                                            "hls",
                                            "https://test-streams.mux.dev/vidcue-missing/stream.m3u8",
                                        ),
                                        MovieSource(
                                            "Unified Streaming (HLS)",
                                            "hls",
                                            "https://demo.unified-streaming.com/k8s/features/stable/video/tears-of-steel/tears-of-steel.ism/.m3u8",
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                features = listOf("Episode picker", "Subtitles", "Failover"),
            ),
        ) + cinecatTitles

    fun find(id: String): MovieTitle? = titles.firstOrNull { it.id == id }

    /** Resolves a [MoviePick.key] back to a title and episode. */
    fun pick(key: String): MoviePick? {
        val parts = key.split(':')
        val title = find(parts[0]) ?: return null
        if (parts.size == 3) {
            val season = parts[1].toIntOrNull() ?: return null
            val episode = parts[2].toIntOrNull() ?: return null
            return MoviePick(title, season, episode).takeIf { it.episodeEntry != null }
        }
        return if (parts.size == 1 && !title.series) MoviePick(title) else null
    }

    /** Case-insensitive match on title, year, genre, credit and episode titles. */
    fun search(query: String, titles: List<MovieTitle> = this.titles): List<MovieTitle> {
        val words = query.trim().lowercase(Locale.ROOT).split(Regex("\\s+")).filter { it.isNotEmpty() }
        if (words.isEmpty()) return emptyList()
        return titles.filter { title ->
            val haystack =
                buildString {
                        append(title.title).append(' ').append(title.year).append(' ')
                        append(title.genres.joinToString(" ")).append(' ').append(title.credit)
                        for (season in title.seasons) for (episode in season.episodes)
                            append(' ').append(episode.title)
                    }
                    .lowercase(Locale.ROOT)
            words.all { it in haystack }
        }
    }
}

/** `1h 48m`, `42m`, or blank when unknown. */
internal fun movieRuntime(minutes: Int): String =
    when {
        minutes <= 0 -> ""
        minutes >= 60 -> "${minutes / 60}h ${minutes % 60}m"
        else -> "${minutes}m"
    }

/** `2012 · 12m · Science fiction, Action`, or `2026 · 2 seasons · Test streams`. */
internal fun movieMeta(title: MovieTitle): String =
    listOfNotNull(
            title.year.takeIf { it > 0 }?.toString(),
            if (title.series) title.seasons.size.let { if (it == 1) "1 season" else "$it seasons" }
            else movieRuntime(title.runtimeMinutes).ifBlank { null },
            title.genres.joinToString(", ").ifBlank { null },
        )
        .joinToString(" · ")

/** `512 KB`, `48.2 MB`, `1.30 GB`. */
internal fun movieBytes(bytes: Long): String =
    when {
        bytes >= 1L shl 30 -> String.format(Locale.ROOT, "%.2f GB", bytes / (1L shl 30).toDouble())
        bytes >= 1L shl 20 -> String.format(Locale.ROOT, "%.1f MB", bytes / (1L shl 20).toDouble())
        else -> "${(bytes / 1024).coerceAtLeast(0)} KB"
    }

/** Source type for a typed URL: `.m3u8` paths (or an `/hls/` folder) are HLS, anything else a file. */
internal fun movieKindOf(url: String): String {
    val path =
        runCatching { java.net.URI(url.trim()).path.orEmpty() }.getOrDefault("").lowercase(Locale.ROOT)
    return if (path.endsWith(".m3u8") || path.endsWith(".m3u") || "/hls/" in path) "hls" else "file"
}

/** Accepts only plain `http(s)://host/…` URLs for the "Play a stream URL" panel. */
internal fun movieUrlValid(url: String): Boolean =
    runCatching {
            val uri = java.net.URI(url.trim())
            (uri.scheme == "https" || uri.scheme == "http") &&
                !uri.host.isNullOrBlank() &&
                uri.userInfo == null
        }
        .getOrDefault(false)

/** One source's result from `movie_check`. */
internal data class SourceAttempt(
    val index: Int,
    val label: String,
    val ok: Boolean,
    val elapsedMs: Long,
    val height: Int?,
    val mbps: Double?,
    val error: String,
) {
    /** `1080p · 31.1 Mb/s · 0.4 s` or the error. */
    val detail: String
        get() =
            if (!ok) error.ifBlank { "Failed" }
            else
                listOfNotNull(
                        height?.let { "${it}p" },
                        mbps?.takeIf { it > 0 }?.let { String.format(Locale.ROOT, "%.1f Mb/s", it) },
                        String.format(Locale.ROOT, "%.1f s", elapsedMs / 1000.0),
                    )
                    .joinToString(" · ")

    companion object {
        fun parse(array: JSONArray?): List<SourceAttempt> =
            (0 until (array?.length() ?: 0)).mapNotNull { i ->
                val a = array?.optJSONObject(i) ?: return@mapNotNull null
                SourceAttempt(
                    a.optInt("index"),
                    a.optString("label"),
                    a.optBoolean("ok"),
                    a.optLong("elapsed_ms"),
                    if (a.isNull("height")) null else a.optInt("height"),
                    if (a.isNull("throughput_mbps")) null else a.optDouble("throughput_mbps"),
                    a.optString("error"),
                )
            }
    }
}

/**
 * One Cinecat provider's result from `movie_cinecat_resolve` (a failover attempt) or
 * `movie_cinecat_check_all` (a per-provider qualification). Errors are sanitized engine text.
 */
internal data class CinecatReport(
    val provider: String,
    val ok: Boolean,
    val streams: Int,
    val elapsedMs: Long,
    val error: String,
) {
    /** `2 streams · 0.3 s` or the error. */
    val detail: String
        get() =
            if (!ok) error.ifBlank { "Failed" }
            else
                listOfNotNull(
                        streams.takeIf { it > 0 }?.let { "$it ${if (it == 1) "stream" else "streams"}" },
                        String.format(Locale.ROOT, "%.1f s", elapsedMs / 1000.0),
                    )
                    .joinToString(" · ")

    companion object {
        /** Reads either op's shape: attempts carry provider/ok/elapsed_ms/error. */
        fun parse(array: JSONArray?): List<CinecatReport> =
            (0 until (array?.length() ?: 0)).mapNotNull { i ->
                val r = array?.optJSONObject(i) ?: return@mapNotNull null
                CinecatReport(
                    r.optString("provider"),
                    r.optBoolean("ok"),
                    r.optInt("stream_count"),
                    r.optLong("elapsed_ms"),
                    r.optString("error"),
                )
            }
    }
}

/**
 * Sources in play order after a check: the chosen one first, then the rest in catalog order, so
 * the player can still fail over if the chosen source stops mid-film.
 */
internal fun movieFailoverOrder(sources: List<MovieSource>, chosen: Int?): List<MovieSource> {
    if (chosen == null || chosen !in sources.indices) return sources
    return listOf(sources[chosen]) + sources.filterIndexed { i, _ -> i != chosen }
}

/** Sizes for TMDB's public image CDN (`image.tmdb.org`), which needs no key. */
internal object TmdbArt {
    private const val BASE = "https://image.tmdb.org/t/p/"
    private val posterWidths = intArrayOf(154, 185, 342, 500, 780)
    private val backdropWidths = intArrayOf(300, 780, 1280)

    fun poster(path: String) = "${BASE}w500/$path"

    fun backdrop(path: String) = "${BASE}w1280/$path"

    /**
     * The same TMDB image at the smallest size at least [widthPx] wide (the largest when none
     * is), so a small card does not fetch a hero-sized file and the hero is not upscaled from a
     * card-sized one. Other URLs pass through unchanged.
     */
    fun sized(url: String, widthPx: Int, backdrop: Boolean): String {
        if (!url.startsWith(BASE)) return url
        val rest = url.removePrefix(BASE)
        val slash = rest.indexOf('/')
        if (slash < 0) return url
        val widths = if (backdrop) backdropWidths else posterWidths
        val width = widths.firstOrNull { it >= widthPx } ?: widths.last()
        return "${BASE}w$width/${rest.substring(slash + 1)}"
    }
}

/** What one poster frame fetches: the image URL and the square decode box that covers it. */
internal data class MovieArt(val url: String, val decodePixels: Int, val backdrop: Boolean)

/**
 * Picks the art for a [widthPx]×[heightPx] frame: the landscape backdrop for wide frames (tablet
 * hero, Continue watching), otherwise the portrait poster. The engine fits art into a square
 * box, so the box is sized from the image's longer side once cropped to cover the frame. Sizing
 * it from the frame width alone decoded a 2:3 poster at two thirds of the width it is shown at.
 */
internal fun movieArt(title: MovieTitle, widthPx: Int, heightPx: Int): MovieArt? {
    if (title.poster.isBlank() && title.backdrop.isBlank()) return null
    val wide = heightPx > 0 && widthPx >= heightPx * 1.2f
    val backdrop = title.backdrop.isNotBlank() && (wide || title.poster.isBlank())
    val aspect = if (backdrop) 16f / 9f else 2f / 3f
    val shownWidth = maxOf(widthPx.toFloat(), heightPx * aspect)
    val shownHeight = shownWidth / aspect
    val url = TmdbArt.sized(if (backdrop) title.backdrop else title.poster, shownWidth.toInt(), backdrop)
    return MovieArt(url, kotlin.math.ceil(maxOf(shownWidth, shownHeight)).toInt(), backdrop)
}
