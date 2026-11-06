package dev.ytui.android

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import java.net.URLDecoder
import java.util.Locale

/** What a pasted, shared or opened link plays as, decided from the URL itself (AR-07). */
internal data class LinkedMedia(
    val url: String,
    val kind: String,
    /** The playlist id for a playlist link; empty otherwise. */
    val id: String,
    val live: Boolean,
)

/** Pure link parsing for pasted, shared (ACTION_SEND) and opened (ACTION_VIEW) links. */
internal object SharedLinks {
    private val urlPattern = Regex("""https?://[^\s<>"'`]+""", RegexOption.IGNORE_CASE)
    private const val TRAILING = ".,;:!?)]}>'\"`’”»"

    /**
     * Classifies a single absolute HTTP(S) URL. The current tab only matters for links that
     * are not YouTube: on Radio such a link is a live station, elsewhere a video. Throws for
     * anything that is not a plain HTTP(S) URL with a host.
     */
    fun classify(raw: String, radioTab: Boolean): LinkedMedia {
        val uri = java.net.URI(raw)
        val scheme = uri.scheme?.lowercase(Locale.ROOT)
        require(
            (scheme == "http" || scheme == "https") && uri.host != null && uri.userInfo == null
        ) {
            "Not an HTTP(S) URL"
        }
        val host = uri.host.lowercase(Locale.ROOT).trimEnd('.')
        if (!youtubeHost(host))
            return LinkedMedia(raw, if (radioTab) "station" else "video", "", radioTab)
        val path = (uri.path ?: "").trimEnd('/')
        val query = query(uri.rawQuery)
        val list = query["list"].orEmpty()
        // A watch page without a video id only names its playlist.
        val playlist =
            host != "youtu.be" &&
                list.isNotBlank() &&
                (path == "/playlist" || (path == "/watch" && query["v"].isNullOrBlank()))
        return when {
            playlist -> LinkedMedia(raw, "playlist", list, false)
            host == "music.youtube.com" -> LinkedMedia(raw, "song", "", false)
            // youtube.com, m.youtube.com, youtu.be and youtube-nocookie: watch, shorts, live
            // and embed pages are all videos; a live stream is detected when it resolves.
            else -> LinkedMedia(raw, "video", "", false)
        }
    }

    /** Every HTTP(S) URL in free text, in order, without trailing sentence punctuation. */
    fun urls(text: String?): List<String> =
        if (text.isNullOrEmpty()) emptyList()
        else urlPattern.findAll(text).map { trim(it.value) }.filter { it.length > 8 }.toList()

    /**
     * Where each HTTP(S) URL sits in free text, as (start, url) in order, without trailing
     * sentence punctuation; `start + url.length` is where the link ends. Used to make the links
     * in a video description tappable.
     */
    fun urlSpans(text: String?): List<Pair<Int, String>> =
        if (text.isNullOrEmpty()) emptyList()
        else
            urlPattern
                .findAll(text)
                .map { it.range.first to trim(it.value) }
                .filter { it.second.length > 8 }
                .toList()

    /**
     * The first YouTube media link in shared text ("Title https://youtu.be/ID"), or null when
     * the text holds none and belongs in the search box instead.
     */
    fun mediaLink(text: String?): String? = urls(text).firstOrNull(::playable)

    /** A YouTube URL that names a video, Short, live stream or playlist. */
    fun playable(raw: String): Boolean {
        val uri = runCatching { java.net.URI(raw) }.getOrNull() ?: return false
        val scheme = uri.scheme?.lowercase(Locale.ROOT)
        if ((scheme != "http" && scheme != "https") || uri.host == null || uri.userInfo != null)
            return false
        val host = uri.host.lowercase(Locale.ROOT).trimEnd('.')
        if (!youtubeHost(host)) return false
        val path = (uri.path ?: "").trimEnd('/')
        val segments = path.split('/').filter { it.isNotEmpty() }
        if (host == "youtu.be") return segments.size == 1
        val query = query(uri.rawQuery)
        return when {
            path == "/watch" -> !query["v"].isNullOrBlank() || !query["list"].isNullOrBlank()
            path == "/playlist" -> !query["list"].isNullOrBlank()
            segments.size == 2 && segments[0] in setOf("shorts", "live", "embed") -> true
            else -> false
        }
    }

    fun youtubeHost(host: String): Boolean =
        host == "youtube.com" ||
            host.endsWith(".youtube.com") ||
            host == "youtu.be" ||
            host == "youtube-nocookie.com" ||
            host.endsWith(".youtube-nocookie.com")

    /** [url] without trailing sentence punctuation; a bracket the URL opened is kept. */
    fun trimUrl(url: String): String = trim(url)

    private fun trim(url: String): String {
        var end = url.length
        while (end > 0 && url[end - 1] in TRAILING) {
            // Keep a closing bracket that belongs to the URL itself.
            val close = url[end - 1]
            val open = when (close) { ')' -> '('; ']' -> '['; '}' -> '{'; else -> null }
            if (open != null) {
                val inner = url.substring(0, end - 1)
                if (inner.count { it == open } > inner.count { it == close }) break
            }
            end--
        }
        return url.substring(0, end)
    }

    private fun query(raw: String?): Map<String, String> {
        if (raw.isNullOrEmpty()) return emptyMap()
        val values = LinkedHashMap<String, String>()
        for (part in raw.split('&')) {
            if (part.isEmpty()) continue
            val split = part.indexOf('=')
            val name = decode(if (split < 0) part else part.substring(0, split))
            val value = if (split < 0) "" else decode(part.substring(split + 1))
            values.putIfAbsent(name, value)
        }
        return values
    }

    private fun decode(value: String): String =
        runCatching { URLDecoder.decode(value, "UTF-8") }.getOrDefault(value)
}

/**
 * Plays a shared or opened YouTube link once the engine is ready and setup is not covering
 * the app; other shared text stays for the search box (BrowsePane). AR-08.
 */
@Composable
internal fun SharedLinkPlayer(vm: YtuiViewModel, blocked: Boolean) {
    val shared by vm.sharedQuery.collectAsState()
    val ready by vm.ready.collectAsState()
    LaunchedEffect(shared, ready, blocked) {
        if (blocked || !ready) return@LaunchedEffect
        val link = SharedLinks.mediaLink(shared) ?: return@LaunchedEffect
        vm.sharedQuery.value = ""
        vm.search(link)
    }
}
