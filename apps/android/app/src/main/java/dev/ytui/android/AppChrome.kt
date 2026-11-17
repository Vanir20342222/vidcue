package dev.ytui.android

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.foundation.background
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONObject

/**
 * The one-row page header. Home carries the app's mark, other destinations their name, and a page
 * opened from another (a playlist, album, channel or station list) a back arrow and its own name.
 * Search is an icon that opens the search field; the round "You" button opens account, history,
 * settings and About in one place.
 */
@Composable
internal fun AppToolbar(
    vm: YtuiViewModel,
    tab: Int,
    connected: Boolean,
    onSearch: () -> Unit,
    onYou: () -> Unit,
    /** False on tablets, whose sidebar holds account and settings. */
    showYou: Boolean = true,
) {
    val feed by vm.feeds[tab].collectAsState()
    val subpage = tab in 0..3 && SubPages.isSubpage(feed.request) && vm.canGoBack(tab)
    // On a collection page the title row carries the top of the header's wash, so the colour
    // runs from the top edge instead of starting under a dark bar.
    val washed =
        subpage && SubPages.collectionId(feed.request) != null && PageChrome.headerTitleShown.value &&
            PageChrome.headerTint.value != Color.Unspecified
    val barColor by
        androidx.compose.animation.animateColorAsState(
            if (washed) PageChrome.headerTint.value else Color.Transparent,
            LocalMotion.current.fade(200),
            label = "toolbar-wash",
        )
    Row(
        Modifier.fillMaxWidth()
            .background(barColor)
            .testTag("app-toolbar")
            .heightIn(min = 56.dp)
            .padding(start = 8.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        when {
            subpage -> {
                MediaIconButton(Glyph.Back, "Back", "feed-back") { vm.back() }
                // A collection's header carries its name; the toolbar repeats it only after the
                // header has scrolled away.
                val hidden = SubPages.collectionId(feed.request) != null && PageChrome.headerTitleShown.value
                val alpha by
                    androidx.compose.animation.core.animateFloatAsState(
                        if (hidden) 0f else 1f,
                        LocalMotion.current.fade(160),
                        label = "toolbar-title",
                    )
                Text(
                    SubPages.title(vm, feed),
                    Modifier.weight(1f)
                        .padding(start = 4.dp)
                        .graphicsLayer { this.alpha = alpha }
                        .semantics { heading() }
                        .testTag("feed-heading"),
                    style = MaterialTheme.typography.titleLarge,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            tab == 1 ->
                Row(
                    Modifier.weight(1f).padding(start = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Image(
                        painterResource(R.drawable.ic_vidcue_launcher_foreground),
                        contentDescription = null,
                        modifier = Modifier.size(44.dp),
                    )
                    Spacer(Modifier.width(2.dp))
                    // The wordmark is a brand mark, not a page title: smaller, so content leads.
                    Text(
                        "Vidcue",
                        Modifier.semantics { heading() },
                        style = MaterialTheme.typography.titleLarge.copy(fontSize = androidx.compose.ui.unit.TextUnit(21f, androidx.compose.ui.unit.TextUnitType.Sp)),
                    )
                }
            else -> PageTitle(destinationTitle(tab), Modifier.padding(start = 8.dp).weight(1f))
        }
        // Library: "+" downloads from a link (the page's own search is in its list).
        if (tab == 0 && !subpage)
            MediaIconButton(Glyph.Plus, "Download a video or playlist from a link", "add-download") {
                PageChrome.libraryAdd.intValue++
            }
        // A page opened from another (playlist, album, channel) keeps its bar to back and title.
        if (tab in 1..3 && !subpage)
            MediaIconButton(
                Glyph.Search,
                if (tab == 3) "Search stations" else "Search",
                "search-button",
                onSearch,
            )
        if (showYou && !subpage) YouButton(connected, onYou)
    }
}

/**
 * Account, history, settings and About live behind one round button, as in the major media apps.
 * The tag stays `settings-button`: it opens the Settings sheet, which now starts with the account.
 */
@Composable
internal fun YouButton(connected: Boolean, open: () -> Unit) {
    RoundIconButton(
        Glyph.Account,
        if (connected) "You: account, history and settings" else "Sign in, history and settings",
        Modifier.testTag("settings-button"),
        action = open,
    )
}

/** Pages opened from another page: they get a back arrow and their own title. */
internal object SubPages {
    private val keys = listOf("Collection", "CliampTracks", "Related", "RadioCountry", "RadioTag")

    fun isSubpage(request: Any?): Boolean = (request as? JSONObject)?.let { r -> keys.any(r::has) } == true

    fun collectionId(request: Any?): String? =
        (request as? JSONObject)?.optString("Collection")?.takeIf { it.isNotBlank() }

    fun title(vm: YtuiViewModel, feed: FeedState): String {
        val request = feed.request as? JSONObject
        collectionId(request)?.let { id -> vm.collectionCard(id)?.title?.let { return it } }
        request?.optString("RadioCountry")?.takeIf { it.isNotBlank() }?.let { return it }
        request?.optString("RadioTag")?.takeIf { it.isNotBlank() }?.let { return it.replaceFirstChar(Char::titlecase) }
        return feed.title.takeUnless { it.isBlank() || it == "Recommendations" } ?: ""
    }
}

/**
 * Search requests and their result filters. Video search narrows with YouTube's own search
 * parameters (sent as a `search` category request); Music uses the engine's `MusicSearch`.
 */
internal object SearchRequests {
    /** Filter label to YouTube search parameters ("" = no filter). */
    private val videoFilters =
        listOf("All" to "", "Videos" to "EgIQAQ%3D%3D", "Channels" to "EgIQAg%3D%3D", "Playlists" to "EgIQAw%3D%3D")
    private val musicFilters =
        listOf(
            "All" to "",
            "Songs" to "songs",
            "Albums" to "albums",
            "Artists" to "artists",
            "Playlists" to "playlists",
            "Videos" to "videos",
        )

    /**
     * Signed out, YouTube gives no Home recommendations. Home then shows what is popular this
     * week in a topic: a video search sorted by views, uploaded this week. It is a feed, not a
     * search, so it keeps the page's header and chips.
     */
    private const val TOPIC_PARAMS = "CAMSBAgDEAE%3D"
    /** "All" is the week's most viewed videos on YouTube (an empty query). */
    val topics = listOf("All", "Music videos", "Gaming", "News", "Sports", "Podcasts", "Cooking", "Science", "Comedy")

    fun topic(name: String): JSONObject =
        JSONObject()
            .put(
                "Category",
                JSONObject()
                    .put("endpoint", "search")
                    .put(
                        "payload",
                        JSONObject()
                            .put("query", if (name == "All") "" else name.lowercase())
                            .put("params", TOPIC_PARAMS),
                    ),
            )

    /** The topic a Home topic feed shows, or null for other requests. */
    fun topicName(request: Any?): String? {
        val payload = (request as? JSONObject)?.optJSONObject("Category")?.optJSONObject("payload") ?: return null
        if (payload.optString("params") != TOPIC_PARAMS) return null
        val query = payload.optString("query")
        return if (query.isEmpty()) "All" else topics.firstOrNull { it.equals(query, ignoreCase = true) }
    }

    fun filters(tab: Int): List<String> =
        when (tab) {
            1 -> videoFilters.map { it.first }
            2 -> musicFilters.map { it.first }
            else -> emptyList()
        }

    /** The query of a search request of any kind, or null for other requests. */
    fun query(request: Any?): String? {
        val r = request as? JSONObject ?: return null
        if (topicName(r) != null) return null
        r.optString("Search").takeIf { r.has("Search") && it.isNotBlank() }?.let { return it }
        r.optString("RadioSearch").takeIf { r.has("RadioSearch") && it.isNotBlank() }?.let { return it }
        r.optJSONObject("MusicSearch")?.optString("query")?.takeIf { it.isNotBlank() }?.let { return it }
        r.optJSONObject("Category")?.let { category ->
            if (category.optString("endpoint") == "search")
                category.optJSONObject("payload")?.optString("query")?.takeIf { it.isNotBlank() }?.let { return it }
        }
        return null
    }

    fun isSearch(request: Any?): Boolean = query(request) != null

    /** Which filter a search request uses ("All" when it is a plain search). */
    fun filter(request: Any?): String {
        val r = request as? JSONObject ?: return "All"
        r.optJSONObject("MusicSearch")?.optString("category")?.let { category ->
            return musicFilters.firstOrNull { it.second == category }?.first ?: "All"
        }
        r.optJSONObject("Category")?.optJSONObject("payload")?.optString("params")?.let { params ->
            return videoFilters.firstOrNull { it.second == params && params.isNotEmpty() }?.first ?: "All"
        }
        return "All"
    }

    /** The request for [query] on [tab], narrowed by [filter]. */
    fun request(tab: Int, query: String, filter: String = "All"): JSONObject {
        if (tab == 3) return JSONObject().put("RadioSearch", query)
        if (tab == 2) {
            val category = musicFilters.firstOrNull { it.first == filter }?.second.orEmpty()
            if (category.isNotEmpty())
                return JSONObject().put("MusicSearch", JSONObject().put("query", query).put("category", category))
        }
        if (tab == 1) {
            val params = videoFilters.firstOrNull { it.first == filter }?.second.orEmpty()
            if (params.isNotEmpty())
                return JSONObject()
                    .put(
                        "Category",
                        JSONObject()
                            .put("endpoint", "search")
                            .put("payload", JSONObject().put("query", query).put("params", params)),
                    )
        }
        return JSONObject().put("Search", query)
    }
}

/** Page state the toolbar reacts to. */
internal object PageChrome {
    /** Whether a collection header's own title is on screen. */
    val headerTitleShown = androidx.compose.runtime.mutableStateOf(false)

    /** The colour at the top of a collection header's wash; the title row continues it. */
    val headerTint = androidx.compose.runtime.mutableStateOf(Color.Unspecified)

    /** Taps on Library's "+" (download from a link); the Library page handles each once. */
    val libraryAdd = androidx.compose.runtime.mutableIntStateOf(0)
    var libraryAddHandled = 0
}

/**
 * Where the playing item was started, for the player's header ("Playing from playlist" over
 * the playlist's name). Set when something plays from a page; null when unknown.
 */
internal object PlaybackSource {
    val current = androidx.compose.runtime.mutableStateOf<Pair<String, String>?>(null)

    fun set(caption: String, title: String) {
        current.value = if (title.isBlank()) null else caption to title
    }
}
