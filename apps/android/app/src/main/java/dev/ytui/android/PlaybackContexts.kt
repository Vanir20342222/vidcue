package dev.ytui.android

import android.net.Uri
import java.util.UUID
import org.json.JSONArray
import org.json.JSONObject

/** Playback continuation is captured when selecting a row, before the browse tab can change. */
internal object PlaybackContexts {
    fun videoId(media: MediaCard): String? =
        media.value.optString("id").takeIf { it.matches(Regex("[A-Za-z0-9_-]{11}")) }
            ?: Uri.parse(media.url)
                .let { uri ->
                    uri.getQueryParameter("v")
                        ?: if (uri.host == "youtu.be") uri.lastPathSegment else null
                }
                ?.takeIf { it.matches(Regex("[A-Za-z0-9_-]{11}")) }

    fun create(
        media: MediaCard,
        feed: FeedState,
        tab: Int,
        autoplayRelated: Boolean,
        forceRelated: Boolean = false,
    ): JSONObject? {
        if (media.collection) return null
        val request = feed.request as? JSONObject
        val finite =
            !forceRelated &&
                !media.live &&
                request != null &&
                listOf("Collection", "Details", "CliampTracks").any(request::has)
        // Stations have no YouTube related endpoint. Keep the visible radio list
        // as a complete context, including live stations. Do not skip songs inside
        // a broadcast: Next switches to the next station. Track libraries retain
        // their existing full-collection preparation below.
        if (!finite && !forceRelated && (media.kind == "station" || tab == 3)) {
            val items =
                feed.cards.filter {
                    !it.collection &&
                        (if (media.kind == "station") it.kind == "station" else !it.live)
                }.distinctBy { it.url }
            val selected = items.indexOfFirst { it.url == media.url }
            if (selected < 0) return null
            // A selection at the bottom still has a Next: visit the remaining
            // visible entries once, wrapping to the beginning of the list.
            val ordered = (items.drop(selected) + items.take(selected)).take(500)
            return json(
                "op" to "radio_context",
                "context_id" to UUID.randomUUID().toString(),
                "source_url" to media.url,
                "items" to JSONArray(ordered.map { it.value }),
            )
        }
        if (media.live) return null
        val id = videoId(media)
        if (!finite && (!autoplayRelated || id == null)) return null
        return json(
                "op" to "prepare_context",
                "context_id" to UUID.randomUUID().toString(),
                "request" to if (finite) request else json("Related" to id),
                "source_url" to media.url,
                "source_id" to id,
                "music" to (tab == 2 || media.kind == "song" || media.musicSource),
            )
            .apply { if (finite) put("items", JSONArray(feed.cards.take(500).map { it.value })) }
    }
}
