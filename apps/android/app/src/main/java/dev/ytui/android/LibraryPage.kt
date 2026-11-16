package dev.ytui.android

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
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
        "Your playlists" to "Playlists",
        "Liked" to "Liked",
        "Watch later" to "WatchLater",
        "Saved" to "Favorites",
    )

/** Offline sections and the name the offline page uses for each. */
private val OFFLINE_SECTIONS =
    linkedMapOf("Downloads" to "Offline", "Offline playlists" to "Playlists", "Transfers" to "Transfers")

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
    var section by rememberSaveable { mutableStateOf("Downloads") }
    val account by vm.account.collectAsState()
    val connected = account.optBoolean("connected")
    val downloads by vm.downloads.collectAsState()
    val transfers = downloads.count { it.optString("state") != "Complete" || it.optJSONObject("media") == null }
    val sections =
        buildList {
            add("Downloads")
            add("Offline playlists")
            if (transfers > 0 || section == "Transfers") add("Transfers")
            if (connected) addAll(listOf("Your playlists", "Liked", "Watch later"))
            add("Saved")
            add("History")
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
            Modifier.fillMaxWidth().testTag("library-sections"),
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
            if (!connected)
                item {
                    MediaChip("Sign in for playlists and likes", modifier = Modifier.testTag("library-sign-in")) {
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
        else -> feedPage(chips)
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
