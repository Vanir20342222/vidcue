package dev.ytui.android

import java.net.URI

/** Related-video responses can omit artwork even though their watch URL is complete. */
internal fun youtubeArtworkFallback(
    mediaUrl: String,
    kind: String = "video",
    thumbnail: String = "",
    hasVariants: Boolean = false,
): String? {
    if ((thumbnail.isNotBlank() && thumbnail != "null") || hasVariants) return null
    if (kind !in setOf("video", "song")) return null
    val url = runCatching { URI(mediaUrl) }.getOrNull() ?: return null
    if (url.scheme?.lowercase() !in setOf("http", "https")) return null
    val id =
        when (url.host?.lowercase()) {
            "youtu.be" -> url.path.removePrefix("/").substringBefore('/')
            "youtube.com", "www.youtube.com", "m.youtube.com", "music.youtube.com" ->
                if (url.path == "/watch")
                    url.rawQuery?.split('&')?.firstOrNull { it.startsWith("v=") }?.removePrefix("v=")
                else if (url.path.startsWith("/shorts/"))
                    url.path.removePrefix("/shorts/").substringBefore('/')
                else null
            else -> null
        }
    if (id == null || !id.matches(Regex("[A-Za-z0-9_-]{11}"))) return null
    return "https://i.ytimg.com/vi/$id/hqdefault.jpg"
}
