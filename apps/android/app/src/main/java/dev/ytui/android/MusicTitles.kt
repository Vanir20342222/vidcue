package dev.ytui.android

/**
 * Song titles as music apps show them. YouTube uploads often read "Artist - Song (Official
 * Video)"; the artist is already on the next line and the tag says nothing about the song, so
 * both go. Edition notes such as "(2004 Remaster)" or "(Live)" stay: they tell versions apart.
 */
internal object MusicTitles {
    private val tagWords =
        listOf(
            "official", "lyric", "lyrics", "audio", "video", "visualizer", "visualiser", "music video",
            "mv", "hd", "4k", "clip officiel", "videoclip", "official music video",
        )
    private val bracketed = Regex("""\s*[(\[]([^)\]]*)[)\]]""")
    private val dash = Regex("""\s[-–—|]\s""")

    fun clean(title: String, creator: String): String {
        val original = title.trim()
        var text =
            bracketed.replace(original) { match ->
                val inside = match.groupValues[1].lowercase()
                if (tagWords.any { Regex("""\b${Regex.escape(it)}\b""").containsMatchIn(inside) }) "" else match.value
            }.trim()
        dash.find(text)?.let { split ->
            val left = text.substring(0, split.range.first).trim()
            val right = text.substring(split.range.last + 1).trim()
            if (right.isNotEmpty() && namesCreator(left, creator)) text = right
        }
        return text.ifBlank { original }
    }

    /** Whether [left] ("AYLIVA x APACHE 207") names the same people as [creator] ("AYLIVA & Apache 207"). */
    private fun namesCreator(left: String, creator: String): Boolean {
        val people = words(creator.removeSuffix(" - Topic"))
        if (people.isEmpty()) return false
        return words(left).any { it in people }
    }

    private fun words(text: String): Set<String> =
        text.lowercase().split(Regex("""[^\p{L}\p{N}]+""")).filter { it.length >= 2 && it !in setOf("x", "ft", "feat", "and", "the") }.toSet()
}

/** The title to show for [this]: cleaned for songs, unchanged for videos and collections. */
internal val MediaCard.displayTitle: String
    get() = if (!collection && (audio || kind == "song" || musicSource)) MusicTitles.clean(title, creator) else title

/**
 * A row's byline without what the row already shows elsewhere: "Song • AYLIVA • 3:05" and
 * "Dua Lipa • 361M plays" become "AYLIVA" and "Dua Lipa". Kind words, play and view counts and
 * durations go; names and album titles stay, joined with one separator.
 */
internal fun cleanByline(byline: String, keepKind: Boolean = false): String {
    val kinds = setOf("song", "video", "album", "single", "ep", "playlist", "artist", "episode", "podcast", "station")
    return byline.split(" • ", " · ")
        .map { it.trim() }
        .filter { part ->
            part.isNotEmpty() &&
                (keepKind || part.lowercase() !in kinds) &&
                !part.matches(Regex("""\d+(:\d{2}){1,2}""")) &&
                !part.matches(Regex("""[\d.,]+\s*[KMB]?\s+(plays|views|subscribers|songs|videos|episodes)""", RegexOption.IGNORE_CASE))
        }
        .joinToString(" · ")
}
