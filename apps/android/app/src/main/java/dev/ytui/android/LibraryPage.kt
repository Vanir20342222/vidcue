package dev.ytui.android

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.size
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp

/** Online Library sections and the feed each one loads (on tab 0). */
internal val LIBRARY_FEEDS =
    linkedMapOf(
        "YouTube playlists" to "Playlists",
        "Liked" to "Liked",
        "Watch later" to "WatchLater",
        "Saved" to "Favorites",
    )

/** Offline sections and the name the offline page uses for each. */
private val OFFLINE_SECTIONS =
    linkedMapOf("Downloads" to "Offline", "Playlists" to "Playlists", "Transfers" to "Transfers")

/**
 * Everything that is yours, in one place: downloads, offline playlists, transfers, and (signed
 * in) your playlists, liked videos and watch later, plus what you saved and your history. One
 * chip row switches between them.
 */
@Composable
internal fun LibraryPage(
    vm: YtuiViewModel,
    onAccount: () -> Unit,
    /** The shared feed page for the online sections, with this page's chips in its header. */
    feedPage: @Composable (chips: @Composable () -> Unit) -> Unit,
) {
    // Opens on an overview of everything that is yours, as Spotify's and YouTube Music's
    // libraries do, instead of one section that may be empty.
    var section by rememberSaveable { mutableStateOf("All") }
    val account by vm.account.collectAsState()
    val connected = account.optBoolean("connected")
    val downloads by vm.downloads.collectAsState()
    val transfers = downloads.count { it.optString("state") != "Complete" || it.optJSONObject("media") == null }
    val sections =
        buildList {
            add("All")
            add("Downloads")
            add("Playlists")
            if (transfers > 0 || section == "Transfers") add("Transfers")
            if (connected) addAll(listOf("YouTube playlists", "Liked", "Watch later"))
            add("Saved")
            add("History")
        }
    // "+" in the title row opens the download dialog, which lives in Downloads.
    val addRequests = PageChrome.libraryAdd.intValue
    LaunchedEffect(addRequests) {
        if (addRequests > PageChrome.libraryAddHandled && OFFLINE_SECTIONS[section] == null) section = "Downloads"
    }
    // Signing out leaves an account-only section; return to Downloads.
    LaunchedEffect(connected) { if (section !in sections) section = "Downloads" }
    LaunchedEffect(section) {
        LIBRARY_FEEDS[section]?.let { request ->
            if (vm.feeds[0].value.request.toString() != request || vm.feeds[0].value.generation == 0L)
                vm.navigate(0, request, remember = false)
        }
        if (section == "History") vm.loadHistory()
    }
    val chips: @Composable () -> Unit = {
        FadingLazyRow(
            // Same place as the chips on Home, Music and Radio.
            Modifier.fillMaxWidth().padding(top = 4.dp).testTag("library-sections"),
            contentPadding = PaddingValues(horizontal = MediaTokens.textInset),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(sections) { label ->
                val tag = OFFLINE_SECTIONS[label] ?: LIBRARY_FEEDS[label] ?: label
                MediaChip(
                    if (label == "Transfers" && transfers > 0) "$label · $transfers" else label,
                    selected = section == label,
                    modifier = Modifier.testTag("library-$tag"),
                ) {
                    section = label
                }
            }
            // The overview has its own sign-in row.
            if (!connected && section != "All")
                item {
                    MediaChip("Sign in for playlists and likes", modifier = Modifier.testTag("library-sign-in-chip")) {
                        onAccount()
                    }
                }
        }
    }
    val offline = OFFLINE_SECTIONS[section]
    when {
        offline != null ->
            OfflineLibrary(
                vm,
                offline,
                onSection = { name -> section = OFFLINE_SECTIONS.entries.first { it.value == name }.key },
                chips = chips,
            )
        section == "History" -> HistoryPage(vm, chips)
        section == "All" -> LibraryOverview(vm, connected, chips, onOpen = { section = it }, onAccount = onAccount)
        else -> feedPage(chips)
    }
}

/**
 * The Library's first view: one row per collection (downloads, offline playlists, saved, history,
 * and signed in your YouTube playlists, likes and Watch later) with what it holds, then what you
 * played last.
 */
@Composable
private fun LibraryOverview(
    vm: YtuiViewModel,
    connected: Boolean,
    chips: @Composable () -> Unit,
    onOpen: (String) -> Unit,
    onAccount: () -> Unit,
) {
    val downloads by vm.downloads.collectAsState()
    val history by vm.recent.collectAsState()
    val playlists by vm.offlinePlaylists.collectAsState()
    val config by vm.config.collectAsState()
    val mobile by vm.mobile.collectAsState()
    val thumbnails = config.optBoolean("thumbnails", true)
    val complete = downloads.count { it.optString("state") == "Complete" && it.optJSONObject("media") != null }
    LaunchedEffect(Unit) { vm.loadHistory() }
    fun items(n: Int) = if (n == 1) "1 item" else "$n items"
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        chips()
        LazyColumn(
            Modifier.fillMaxSize().testTag("library-overview"),
            contentPadding = PaddingValues(top = 4.dp, bottom = 24.dp + LocalBottomOverlay.current),
        ) {
            // Signed out: one card at the top, set apart from your own collections.
            if (!connected)
                item(key = "sign-in") {
                    androidx.compose.foundation.layout.Row(
                        Modifier.fillMaxWidth()
                            .padding(start = MediaTokens.textInset, end = MediaTokens.textInset, bottom = 12.dp)
                            .clip(MediaTokens.card)
                            .background(androidx.compose.material3.MaterialTheme.colorScheme.surfaceContainer)
                            .padding(start = 16.dp, end = 12.dp, top = 14.dp, bottom = 14.dp)
                            .testTag("library-sign-in-card"),
                        verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            androidx.compose.material3.Text(
                                "Your YouTube library",
                                style = androidx.compose.material3.MaterialTheme.typography.titleSmall,
                            )
                            androidx.compose.material3.Text(
                                "Sign in for your playlists, likes and Watch later.",
                                style = androidx.compose.material3.MaterialTheme.typography.bodySmall,
                                color = androidx.compose.material3.MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Pill("Sign in", Modifier.testTag("library-sign-in"), PillStyle.Primary, onClick = onAccount)
                    }
                }
            item(key = "downloads") {
                LibraryEntry(Glyph.Download, "Downloads", if (complete > 0) items(complete) else "Nothing downloaded yet") {
                    onOpen("Downloads")
                }
            }
            if (playlists.isEmpty())
                item(key = "new-playlist") {
                    LibraryEntry(Glyph.Playlist, "Playlists", "No playlists yet") { onOpen("Playlists") }
                }
            else
                items(playlists, key = { "playlist:" + it.optString("id") }) { playlist ->
                    LibraryEntry(
                        Glyph.Playlist,
                        playlist.optString("title"),
                        "Playlist · " + items(playlist.optJSONArray("items")?.length() ?: 0),
                    ) {
                        onOpen("Playlists")
                    }
                }
            if (connected) {
                item(key = "yt-playlists") { LibraryEntry(Glyph.Library, "YouTube playlists", "From your account") { onOpen("YouTube playlists") } }
                item(key = "liked") { LibraryEntry(Glyph.Save, "Liked", "Videos and songs you liked") { onOpen("Liked") } }
                item(key = "watch-later") { LibraryEntry(Glyph.UpNext, "Watch later", "Saved on YouTube to watch later") { onOpen("Watch later") } }
            }
            item(key = "saved") { LibraryEntry(Glyph.Save, "Saved", "Nothing saved yet") { onOpen("Saved") } }
            item(key = "history") {
                LibraryEntry(Glyph.History, "History", if (history.isEmpty()) "Nothing played yet" else items(history.size)) { onOpen("History") }
            }
            if (history.isNotEmpty()) {
                item(key = "recent-head") {
                    ShelfHeading(
                        "Recently played",
                        Modifier.padding(top = 16.dp, bottom = 4.dp),
                        action = "Show all",
                        actionTag = "library-recent-all",
                        actionGlyph = null,
                    ) {
                        onOpen("History")
                    }
                }
                items(history.take(6), key = { "recent:" + it.key }) { media ->
                    androidx.compose.foundation.layout.Box(Modifier.padding(horizontal = MediaTokens.textInset)) {
                        MediaListRow(
                            media.title,
                            if (mobile.optBoolean("show_channel", true)) media.creator else "",
                            artwork = { Artwork(media, it, thumbnails) },
                            square = media.audio,
                            onClick = { vm.play(media, resume = true) },
                        )
                    }
                }
            }
        }
    }
}

/** One collection on the Library overview: a 52dp tile with its mark, its name and what it holds. */
@Composable
private fun LibraryEntry(glyph: Glyph, title: String, detail: String, accent: Boolean = false, onClick: () -> Unit) {
    val colors = androidx.compose.material3.MaterialTheme.colorScheme
    androidx.compose.foundation.layout.Row(
        Modifier.fillMaxWidth()
            .clickable(role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
            .padding(horizontal = MediaTokens.textInset, vertical = 6.dp)
            .testTag("library-entry-$title"),
        verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        androidx.compose.foundation.layout.Box(
            Modifier.size(52.dp)
                .clip(MediaTokens.small)
                .background(if (accent) colors.primary.copy(alpha = 0.18f) else colors.surfaceContainerHigh),
            contentAlignment = androidx.compose.ui.Alignment.Center,
        ) {
            MediaIcon(glyph, Modifier.size(24.dp), if (accent) colors.primary else colors.onSurface)
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            androidx.compose.material3.Text(
                title,
                style = androidx.compose.material3.MaterialTheme.typography.titleSmall,
                maxLines = 1,
                overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
            )
            androidx.compose.material3.Text(
                detail,
                style = androidx.compose.material3.MaterialTheme.typography.bodySmall,
                color = colors.onSurfaceVariant,
                maxLines = 1,
                overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
            )
        }
    }
}

/** Your history as a Library section: the same rows as the History sheet, full height. */
@Composable
private fun HistoryPage(vm: YtuiViewModel, chips: @Composable () -> Unit) {
    val history by vm.recent.collectAsState()
    val config by vm.config.collectAsState()
    val mobile by vm.mobile.collectAsState()
    val context = LocalContext.current
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        chips()
        if (history.isEmpty())
            EmptyMediaState(
                Glyph.History,
                "No history yet",
                "Videos and songs you play appear here.",
            )
        else
            LazyColumn(
                Modifier.fillMaxSize().testTag("library-history"),
                contentPadding =
                    PaddingValues(
                        start = MediaTokens.textInset,
                        end = MediaTokens.textInset,
                        bottom = 24.dp + LocalBottomOverlay.current,
                    ),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                item {
                    QuietAction("Clear history and resume positions", destructive = true) {
                        vm.command("history_clear")
                        RecentSearches.clear(context)
                    }
                }
                items(history, key = { it.key }) { media ->
                    MediaListRow(
                        media.title,
                        if (mobile.optBoolean("show_channel", true)) media.creator else "",
                        artwork = { Artwork(media, it, config.optBoolean("thumbnails", true)) },
                        square = media.audio,
                        onClick = { vm.play(media, resume = true) },
                    )
                }
            }
    }
}
