package dev.ytui.android

import java.util.Locale

/** What a tappable piece of a video description does. */
internal sealed interface DescriptionTarget {
    /** An absolute HTTP(S) link; bare `www.` text gets `https://` in front. */
    data class Link(val url: String) : DescriptionTarget

    /** A YouTube channel: a `UC…` ID or an `@handle`. */
    data class Channel(val id: String) : DescriptionTarget

    /** A `#hashtag`, searched for including the `#`. */
    data class Hashtag(val tag: String) : DescriptionTarget

    /** A timestamp such as `1:23` or `1:02:03`, in seconds. */
    data class Time(val seconds: Int) : DescriptionTarget
}

/** One tappable range of a description: `text.substring(start, end)` is what is shown. */
internal data class DescriptionSpan(val start: Int, val end: Int, val target: DescriptionTarget)

/** Pure parsing of the links, handles, hashtags and timestamps in a video description. */
internal object DescriptionLinks {
    private val www = Regex("""(?<![\w/@.])www\.[^\s<>"'`]+\.[^\s<>"'`]+""", RegexOption.IGNORE_CASE)
    // Not part of an email address, URL path or longer word.
    private val handle = Regex("""(?<![\w@./+-])@[A-Za-z0-9_.-]{3,30}""")
    // At least one letter, so "#1" stays plain; not a URL fragment or HTML entity.
    private val hashtag = Regex("""(?<![\w#&/])#[\p{L}\p{N}_]*\p{L}[\p{L}\p{N}_]*""")
    private val time = Regex("""(?<![\w:.,/])(?:(\d{1,2}):)?(\d{1,2}):(\d{2})(?![\w:]|[.,]\d)""")

    /** Every tappable range in [text], in order and never overlapping. */
    fun spans(text: String?): List<DescriptionSpan> {
        if (text.isNullOrEmpty()) return emptyList()
        val found = mutableListOf<DescriptionSpan>()
        fun add(start: Int, end: Int, target: DescriptionTarget?) {
            if (target == null || end <= start) return
            if (found.any { start < it.end && it.start < end }) return
            found += DescriptionSpan(start, end, target)
        }
        // Full URLs first, so a handle or hashtag inside one never splits it.
        for ((start, url) in SharedLinks.urlSpans(text))
            add(start, start + url.length, linkTarget(url))
        for (match in www.findAll(text)) {
            val shown = SharedLinks.trimUrl(match.value)
            add(match.range.first, match.range.first + shown.length, linkTarget("https://$shown"))
        }
        for (match in handle.findAll(text)) {
            val shown = match.value.trimEnd('.', '-')
            if (shown.length >= 4)
                add(match.range.first, match.range.first + shown.length, DescriptionTarget.Channel(shown))
        }
        for (match in hashtag.findAll(text))
            add(match.range.first, match.range.last + 1, DescriptionTarget.Hashtag(match.value))
        for (match in time.findAll(text))
            add(match.range.first, match.range.last + 1, timeTarget(match))
        return found.sortedBy { it.start }
    }

    /** A YouTube channel page opens as a channel; any other link stays a link. */
    fun linkTarget(url: String): DescriptionTarget {
        val uri = runCatching { java.net.URI(url) }.getOrNull() ?: return DescriptionTarget.Link(url)
        val host = uri.host?.lowercase(Locale.ROOT)?.trimEnd('.')
        if (host != null && host != "music.youtube.com" && host != "youtu.be" && SharedLinks.youtubeHost(host)) {
            val segments = (uri.path ?: "").split('/').filter { it.isNotEmpty() }
            fun tab(index: Int) = segments.size == index || (segments.size == index + 1 && segments[index] in CHANNEL_TABS)
            val channel =
                when {
                    segments.firstOrNull()?.startsWith("@") == true && tab(1) -> segments[0]
                    segments.firstOrNull() == "channel" && segments.size >= 2 && tab(2) -> segments[1]
                    else -> null
                }
            if (channel != null && (isChannelId(channel) || isHandle(channel)))
                return DescriptionTarget.Channel(channel)
        }
        return DescriptionTarget.Link(url)
    }

    fun isHandle(id: String): Boolean =
        id.length in 4..31 &&
            id.startsWith("@") &&
            id.drop(1).all { (it.code < 128 && it.isLetterOrDigit()) || it in "_.-" }

    private val CHANNEL_TABS = setOf("videos", "featured", "shorts", "streams", "playlists", "community", "about")

    private fun timeTarget(match: MatchResult): DescriptionTarget? {
        val hours = match.groupValues[1].takeIf { it.isNotEmpty() }?.toInt()
        val minutes = match.groupValues[2].toInt()
        val seconds = match.groupValues[3].toInt()
        if (seconds > 59 || (hours != null && minutes > 59)) return null
        return DescriptionTarget.Time((hours ?: 0) * 3600 + minutes * 60 + seconds)
    }
}
