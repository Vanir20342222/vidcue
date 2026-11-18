package dev.ytui.android

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.grid.items as gridItems
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
        section == "All" -> LibraryOverview(vm, connected, onOpen = { section = it })
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
    onOpen: (String) -> Unit,
) {
    val downloads by vm.downloads.collectAsState()
    val history by vm.recent.collectAsState()
    val playlists by vm.offlinePlaylists.collectAsState()
    val config by vm.config.collectAsState()
    val mobile by vm.mobile.collectAsState()
    val thumbnails = config.optBoolean("thumbnails", true)
    val complete = downloads.count { it.optString("state") == "Complete" && it.optJSONObject("media") != null }
    LaunchedEffect(Unit) { vm.loadHistory() }
    // The rows are the way into each section, so the overview has no chip row repeating them;
    // inside a section the chips (with "All") lead back.
    val list = androidx.compose.foundation.lazy.rememberLazyListState()
    // YouTube Music's colour wash, from what you played last (else the accent).
    val base = androidx.compose.material3.MaterialTheme.colorScheme.primary
    val left = rememberArtworkTint(history.firstOrNull(), base, thumbnails)
    // Nothing played yet: the same quiet wash as every other empty page.
    val second = rememberArtworkTint(history.getOrNull(1), left, thumbnails)
    androidx.compose.runtime.SideEffect {
        PageChrome.washColors.value = if (history.isEmpty()) fallbackWash(base) else washTone(left) to washTone(second)
    }
    androidx.compose.runtime.DisposableEffect(Unit) { onDispose { PageChrome.washColors.value = null } }
    val density = androidx.compose.ui.platform.LocalDensity.current
    LaunchedEffect(list) {
        val span = with(density) { 280.dp.toPx() }
        androidx.compose.runtime.snapshotFlow {
                maxOf(
                    if (list.firstVisibleItemIndex > 0) 1f else list.firstVisibleItemScrollOffset / span,
                    if (history.isEmpty()) 0.4f else 0f,
                )
            }
            .collect { PageChrome.washFade.floatValue = it.coerceIn(0f, 1f) }
    }
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        LazyColumn(
            Modifier.fillMaxSize().testTag("library-overview"),
            state = list,
            contentPadding = PaddingValues(top = 4.dp, bottom = 24.dp + LocalBottomOverlay.current),
        ) {
            // Each collection as a coloured tile, two to a row, with what it holds in its corner.
            val tiles =
                buildList {
                    add(LibraryTile(Glyph.Download, "Downloads", complete, Color(0xFF23A383)))
                    add(LibraryTile(Glyph.Playlist, "Playlists", playlists.size, Color(0xFF7B5CD6)))
                    if (connected) {
                        add(LibraryTile(Glyph.Library, "YouTube playlists", 0, Color(0xFFD9483B)))
                        add(LibraryTile(Glyph.Save, "Liked", 0, Color(0xFFE0568F)))
                        add(LibraryTile(Glyph.UpNext, "Watch later", 0, Color(0xFFD99A2B)))
                    }
                    add(LibraryTile(Glyph.Save, "Saved", 0, Color(0xFFD6336C)))
                    // History is "Recently played" below (its › opens the full list), so the same
                    // three tiles show whether or not anything was played.
                }
            // Three fit one row (icon over name); otherwise two to a row.
            val perRow = if (tiles.size == 3) 3 else 2
            items(tiles.chunked(perRow), key = { row -> "tiles:" + row.first().title }) { row ->
                androidx.compose.foundation.layout.Row(
                    Modifier.fillMaxWidth().padding(start = MediaTokens.textInset, end = MediaTokens.textInset, bottom = 12.dp),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    for (tile in row) LibraryTileCard(tile, Modifier.weight(1f), stacked = perRow == 3) { onOpen(tile.title) }
                    repeat(perRow - row.size) { androidx.compose.foundation.layout.Spacer(Modifier.weight(1f)) }
                }
            }
            // Nothing played or saved yet: one way to start, no explanation needed.
            if (history.isEmpty() && complete == 0 && playlists.isEmpty())
                item(key = "start") {
                    // The same empty state as every other page.
                    EmptyMediaState(Glyph.Library, "Nothing yet", "", Modifier.padding(top = 8.dp)) {
                        Action("Find music", "library-browse") { vm.chooseTab(2) }
                    }
                }
            if (history.isNotEmpty()) {
                item(key = "recent-head") {
                    ShelfHeading(
                        "Recently played",
                        Modifier.padding(top = 8.dp, bottom = 12.dp),
                        action = "Show all",
                        actionTag = "library-recent-all",
                        actionGlyph = null,
                    ) {
                        onOpen("History")
                    }
                }
                item(key = "recent") {
                    // Two rows that page sideways, so what you played fills the screen.
                    androidx.compose.foundation.lazy.grid.LazyHorizontalGrid(
                        androidx.compose.foundation.lazy.grid.GridCells.Fixed(if (history.size > 3) 2 else 1),
                        Modifier.fillMaxWidth()
                            .height(if (history.size > 3) 336.dp else 162.dp)
                            .testTag("library-recent"),
                        contentPadding = PaddingValues(horizontal = MediaTokens.textInset),
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        gridItems(history.take(16), key = { it.key }) { media ->
                            Column(
                                Modifier.width(132.dp)
                                    .clip(MediaTokens.medium)
                                    .clickable(onClickLabel = "Play") { vm.play(media, resume = true) },
                                verticalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                Artwork(
                                    media,
                                    Modifier.fillMaxWidth().aspectRatio(if (media.audio || media.kind == "song") 1f else 16f / 9f)
                                        .clip(MediaTokens.medium),
                                    thumbnails,
                                    shape = MediaTokens.medium,
                                )
                                androidx.compose.material3.Text(
                                    media.displayTitle,
                                    style = androidx.compose.material3.MaterialTheme.typography.titleSmall,
                                    maxLines = 1,
                                    overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

/** A Library collection: its mark, its name, how many things are in it, and its colour. */
private data class LibraryTile(val glyph: Glyph, val title: String, val count: Int, val hue: Color)

@Composable
private fun LibraryTileCard(tile: LibraryTile, modifier: Modifier, stacked: Boolean = false, onClick: () -> Unit) {
    val colors = androidx.compose.material3.MaterialTheme.colorScheme
    val dark = colors.background.luminance() < 0.4f
    // Translucent like the chips, so the wash shows through.
    val icon = colors.onSurface.copy(alpha = 0.9f)
    if (stacked) {
        Column(
            modifier.height(72.dp)
                .clip(MediaTokens.card)
                .background(colors.onBackground.copy(alpha = if (dark) 0.08f else 0.05f))
                .clickable(role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
                // Narrow side padding, so "Downloads" fits a third of a 320 dp phone.
                .padding(horizontal = 2.dp, vertical = 10.dp)
                .testTag("library-entry-${tile.title}"),
            horizontalAlignment = androidx.compose.ui.Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(6.dp, androidx.compose.ui.Alignment.CenterVertically),
        ) {
            MediaIcon(tile.glyph, Modifier.size(24.dp), icon)
            androidx.compose.material3.Text(
                tile.title,
                style = androidx.compose.material3.MaterialTheme.typography.labelLarge,
                color = colors.onSurface,
                maxLines = 1,
                overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
            )
        }
        return
    }
    androidx.compose.foundation.layout.Row(
        modifier.height(56.dp)
            .clip(MediaTokens.card)
            .background(colors.onBackground.copy(alpha = if (dark) 0.08f else 0.05f))
            .clickable(role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
            // Tight enough that "Downloads" fits a tile on a 320 dp phone.
            .padding(horizontal = 12.dp)
            .testTag("library-entry-${tile.title}"),
        verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        MediaIcon(tile.glyph, Modifier.size(24.dp), icon)
        androidx.compose.material3.Text(
            tile.title,
            style = androidx.compose.material3.MaterialTheme.typography.titleSmall,
            color = colors.onSurface,
            maxLines = 1,
            overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
        )
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
