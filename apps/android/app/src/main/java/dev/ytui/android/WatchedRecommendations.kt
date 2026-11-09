package dev.ytui.android

import java.net.URI
import org.json.JSONObject

/** Only discovery feeds hide watched entries; intentional replay remains available. */
internal object WatchedRecommendations {
    fun applies(request: Any): Boolean =
        request == "Home" ||
            (request as? JSONObject)?.let { it.has("Category") || it.has("Related") } == true

    private val videoId = Regex("[A-Za-z0-9_-]{11}")

    private fun identity(media: MediaCard): String? {
        if (media.kind !in setOf("video", "song") || media.live) return null
        media.value.optString("id").takeIf { videoId.matches(it) }?.let { return it }
        val uri = runCatching { URI(media.url) }.getOrNull() ?: return null
        val id = when (uri.host?.lowercase()) {
            "youtu.be" -> uri.path.removePrefix("/").substringBefore('/')
            "youtube.com", "www.youtube.com", "m.youtube.com", "music.youtube.com" ->
                if (uri.path == "/watch")
                    uri.rawQuery?.split('&')?.firstOrNull { it.startsWith("v=") }?.removePrefix("v=")
                else if (listOf("/shorts/", "/embed/", "/live/").any(uri.path::startsWith))
                    uri.path.split('/').getOrNull(2)
                else null
            else -> null
        }
        return id?.takeIf(videoId::matches)
    }

    fun filter(cards: List<MediaCard>, watched: List<MediaCard>): List<MediaCard> {
        val ids = watched.mapNotNull(::identity).toHashSet()
        return cards.filter { identity(it) !in ids }
    }

    fun filter(feed: FeedState, watched: List<MediaCard>): FeedState =
        if (applies(feed.request)) feed.copy(cards = filter(feed.cards, watched)) else feed
}
