package dev.ytui.android

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.animation.togetherWith
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONObject

@Composable
internal fun OfflineLibrary(vm: YtuiViewModel) {
    val downloads by vm.downloads.collectAsState()
    val playlists by vm.offlinePlaylists.collectAsState()
    val config by vm.config.collectAsState()
    val playerOpen by vm.playerOpen.collectAsState()
    var section by rememberSaveable { mutableStateOf("Offline") }
    var add by remember { mutableStateOf(false) }
    var newPlaylist by remember { mutableStateOf(false) }
    var addTo by remember { mutableStateOf<List<JSONObject>?>(null) }
    var selecting by rememberSaveable { mutableStateOf(false) }
    var selectedIds by rememberSaveable { mutableStateOf(emptyList<String>()) }
    var removingSelected by remember { mutableStateOf<List<JSONObject>?>(null) }
    var playlistItems by remember { mutableStateOf(emptyList<JSONObject>()) }
    var options by remember { mutableStateOf<JSONObject?>(null) }
    var opened by rememberSaveable { mutableStateOf("") }
    var removing by remember { mutableStateOf<JSONObject?>(null) }
    var deleting by remember { mutableStateOf<JSONObject?>(null) }
    val thumbnails = config.optBoolean("thumbnails", true)
    val available =
        downloads.filter {
            it.optString("state") == "Complete" && it.optJSONObject("media") != null
        }
    val transfers =
        downloads.filter {
            it.optString("state") != "Complete" || it.optJSONObject("media") == null
        }
    var query by rememberSaveable { mutableStateOf("") }
    val shown = if (query.isBlank()) available else available.filter { offlineMatches(it, query) }
    val shownPlaylists =
        if (query.isBlank()) playlists else playlists.filter { offlinePlaylistMatches(it, query) }
    val selectedItems = available.filter { it.optString("id") in selectedIds }
    val haptic = rememberGestureHaptic()
    fun clearSelection() {
        selectedIds = emptyList()
        selecting = false
    }
    fun toggleSelection(id: String) {
        selectedIds = if (id in selectedIds) selectedIds - id else selectedIds + id
    }
    BackHandler(
        selecting && !playerOpen && !add && addTo == null && !newPlaylist && removingSelected == null
    ) {
        clearSelection()
    }
    LaunchedEffect(available.map { it.optString("id") }) {
        selectedIds = selectedItems.map { it.optString("id") }
    }
    LaunchedEffect(Unit) { vm.loadOfflinePlaylists() }
    DisposableEffect(playerOpen) {
        vm.effect("library_visible", "value" to !playerOpen)
        onDispose { vm.effect("library_visible", "value" to false) }
    }
    val motion = LocalMotion.current
    val selectionBar = 88.dp
    Box(Modifier.fillMaxSize()) {
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        // While selecting, the filter row and actions give way to a selection header.
        androidx.compose.animation.AnimatedContent(
            selecting && section == "Offline",
            transitionSpec = {
                if (!motion.animate)
                    androidx.compose.animation.EnterTransition.None togetherWith
                        androidx.compose.animation.ExitTransition.None
                else
                    (androidx.compose.animation.fadeIn(motion.fade(180)) +
                        androidx.compose.animation.slideInVertically(motion.enter(260)) { -it / 4 }) togetherWith
                        androidx.compose.animation.fadeOut(motion.fade(120)) using
                        androidx.compose.animation.SizeTransform(clip = false) { _, _ -> motion.spring() }
            },
            label = "offline-header",
        ) { selectingNow ->
        if (selectingNow) {
            val allSelected = shown.isNotEmpty() && shown.all { it.optString("id") in selectedIds }
            OfflineSelectionHeader(
                selectedItems.size,
                allSelected,
                onClose = ::clearSelection,
            ) {
                val visibleIds = shown.map { it.optString("id") }
                selectedIds =
                    if (allSelected) selectedIds - visibleIds.toSet()
                    else (selectedIds + visibleIds).distinct()
            }
        } else Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        // No page heading: the tab label already names this screen. One filter
        // row carries the sections, with the two creation actions at its end.
        Row(
            Modifier.fillMaxWidth().padding(end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            LazyRow(
                Modifier.weight(1f),
                contentPadding = PaddingValues(horizontal = MediaTokens.textInset),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(listOf("Offline", "Playlists", "Transfers")) { title ->
                    val count =
                        when (title) {
                            "Transfers" -> transfers.size
                            else -> 0
                        }
                    MediaChip(
                        if (count > 0) "$title · $count" else title,
                        selected = section == title,
                        modifier = Modifier.testTag("library-$title"),
                    ) {
                        section = title
                        clearSelection()
                    }
                }
            }
        }
        if (section != "Transfers" && (available.isNotEmpty() || playlists.isNotEmpty()))
            OfflineSearchField(
                query,
                { query = it },
                if (section == "Playlists") "Search offline playlists" else "Search offline media",
                Modifier.fillMaxWidth().padding(horizontal = MediaTokens.textInset),
            )
        // Labelled actions instead of two look-alike icons, plus what the library holds.
        FlowRow(
            Modifier.fillMaxWidth().padding(horizontal = MediaTokens.textInset),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Pill(
                "Save a link",
                Modifier.testTag("add-download").semantics {
                    contentDescription = "Save a video or playlist offline"
                },
                glyph = Glyph.Link,
            ) {
                add = true
            }
            Pill(
                "New playlist",
                Modifier.testTag("create-offline-playlist").semantics {
                    contentDescription = "Create offline playlist"
                },
                glyph = Glyph.Plus,
            ) {
                playlistItems = emptyList()
                newPlaylist = true
            }
            if (section == "Offline" && available.isNotEmpty()) {
                Pill(
                    "Select",
                    Modifier.testTag("offline-select"),
                    style = PillStyle.Quiet,
                    glyph = Glyph.Check,
                ) {
                    selecting = true
                }
            }
        }
        }
        }
        LazyColumn(
            Modifier.weight(1f).fillMaxWidth(),
            contentPadding =
                PaddingValues(
                    start = MediaTokens.textInset,
                    end = MediaTokens.textInset,
                    top = 4.dp,
                    bottom =
                        4.dp + LocalBottomOverlay.current +
                            if (selecting && section == "Offline") selectionBar else 0.dp,
                ),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            when (section) {
                "Offline" -> {
                    if (available.isNotEmpty() && shown.isEmpty())
                        item(key = "no-matches") { OfflineNoMatches(query.trim()) }
                    if (shown.isNotEmpty())
                        item(key = "summary") {
                            val stored = shown.sumOf { it.optLong("bytes") }
                            Text(
                                (if (query.isNotBlank()) "${shown.size} of ${available.size} · " else "") +
                                (if (shown.size == 1) "1 item" else "${shown.size} items") +
                                    if (stored > 0) " · ${byteLabel(stored)} on this device" else "",
                                Modifier.padding(horizontal = 6.dp, vertical = 4.dp).testTag("library-summary"),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    if (available.isEmpty())
                        item {
                            EmptyMediaState(
                                Glyph.Download,
                                "No offline media",
                                "Save songs or videos to play without a connection.",
                            ) {
                                Action("Save from a link") { add = true }
                            }
                        }
                    items(shown, key = { it.optString("id") }) { item ->
                        val media = MediaCard(item.getJSONObject("media"))
                        val chosen = item.optString("id") in selectedIds
                        MediaListRow(
                            item.optString("title"),
                            media.creator,
                            Modifier.testTag("offline-item-${item.optString("id")}").semantics {
                                contentDescription =
                                    if (selecting) "Select offline ${item.optString("title")}"
                                    else "Play offline ${item.optString("title")}"
                                if (selecting) selected = chosen
                            }.then(if (motion.animate) Modifier.animateItem() else Modifier),
                            artwork = { art ->
                                // Chosen covers ease back a little, like a pressed card.
                                val inset by
                                    androidx.compose.animation.core.animateFloatAsState(
                                        if (selecting && chosen) 0.9f else 1f,
                                        motion.spring(),
                                        label = "offline-chosen",
                                    )
                                Artwork(
                                    media,
                                    art.graphicsLayer {
                                        scaleX = inset
                                        scaleY = inset
                                    },
                                    thumbnails,
                                )
                            },
                            square = item.optBoolean("audio"),
                            highlighted = selecting && chosen,
                            onClick = {
                                if (selecting) toggleSelection(item.optString("id"))
                                else vm.playOffline(item)
                            },
                            onLongClick = {
                                haptic()
                                if (!selecting) {
                                    selecting = true
                                    selectedIds = listOf(item.optString("id"))
                                } else toggleSelection(item.optString("id"))
                            },
                            extra = {
                                // What the copy is and how much space it takes, on its own line
                                // so a long channel name cannot push it out of view.
                                Text(
                                    offlineDetails(
                                        item.optBoolean("audio"),
                                        item.optString("quality"),
                                        item.optLong("bytes"),
                                    ),
                                    Modifier.testTag("offline-details-${item.optString("id")}"),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                item
                                    .optString("storage_error")
                                    .takeIf { it.isNotBlank() }
                                    ?.let {
                                        Text(
                                            it,
                                            style = MaterialTheme.typography.bodySmall,
                                            color = MaterialTheme.colorScheme.error,
                                            maxLines = 2,
                                            overflow = TextOverflow.Ellipsis,
                                        )
                                    }
                            },
                        ) {
                            if (selecting) {
                                SelectionMark(
                                    chosen,
                                    Modifier.testTag("offline-checkbox-${item.optString("id")}"),
                                )
                            } else MediaIconButton(
                                Glyph.More,
                                "Offline media options",
                                "offline-menu-${item.optString("id")}",
                            ) {
                                options = item
                            }
                        }
                    }
                }
                "Playlists" -> {
                    if (playlists.isEmpty())
                        item {
                            EmptyMediaState(
                                Glyph.Queue,
                                "No offline playlists",
                                "Download a playlist to save it here, or create your own.",
                            )
                        }
                    else if (shownPlaylists.isEmpty())
                        item(key = "no-matches") { OfflineNoMatches(query.trim()) }
                    items(shownPlaylists, key = { it.optString("id") }) { playlist ->
                        val members = playlist.optJSONArray("items")?.objects() ?: emptyList()
                        val first = members.firstOrNull()
                        MediaListRow(
                            playlist.optString("title"),
                            if (members.size == 1) "1 item" else "${members.size} items",
                            artwork = { modifier ->
                                if (first != null) Artwork(MediaCard(first), modifier, thumbnails)
                                else GlyphTile(Glyph.Queue, modifier)
                            },
                            onClick = { opened = playlist.optString("id") },
                        ) {
                            MediaIconButton(
                                Glyph.Play,
                                "Play offline playlist ${playlist.optString("title")}",
                            ) {
                                vm.playOfflinePlaylist(playlist)
                            }
                        }
                    }
                }
                else -> {
                    val active =
                        transfers.filter {
                            !it.optString("id").startsWith("export:") &&
                                !it.optString("id").startsWith("job:") &&
                                transferAction(it).pauses
                        }
                    if (transfers.isNotEmpty())
                        item(key = "transfers-header") {
                            TransfersHeader(
                                transferSummary(transfers.map { transferRowTone(it) }),
                                canPauseAll = active.size > 1,
                            ) {
                                for (item in active)
                                    vm.effect("download_toggle", "id" to item.optString("id"))
                            }
                        }
                    if (transfers.isEmpty())
                        item {
                            EmptyMediaState(
                                Glyph.Check,
                                "No active transfers",
                                "Saves in progress appear here.",
                            )
                        }
                    items(transfers, key = { it.optString("id") }) { item ->
                        TransferCard(
                            item,
                            thumbnails,
                            modifier = if (motion.animate) Modifier.animateItem() else Modifier,
                            onToggle = {
                                vm.effect("download_toggle", "id" to item.optString("id"))
                            },
                            onRemove = { removing = item },
                        )
                    }
                }
            }
        }
    }
    OfflineSelectionActions(
        visible = selecting && section == "Offline",
        enabled = selectedItems.isNotEmpty(),
        modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = LocalBottomOverlay.current + 8.dp),
        onAdd = { addTo = selectedItems },
        onNewPlaylist = {
            playlistItems = offlinePlaylistMembers(selectedItems)
            newPlaylist = true
        },
        onDelete = { removingSelected = selectedItems },
    )
    }
    options?.let { item ->
        val media = MediaCard(item.getJSONObject("media"))
        fun execute(action: () -> Unit) {
            options = null
            action()
        }
        Panel("", { options = null }) {
            ActionSheetHeader(media, thumbnails, square = item.optBoolean("audio"))
            SheetAction(
                text = { Text("Play") },
                leadingIcon = { MediaIcon(Glyph.Play) },
                onClick = { execute { vm.playOffline(item) } },
            )
            SheetAction(
                text = { Text("Add to offline playlist") },
                leadingIcon = { MediaIcon(Glyph.Queue) },
                onClick = { execute { addTo = listOf(item) } },
            )
            SheetAction(
                text = { Text("Export file") },
                leadingIcon = { MediaIcon(Glyph.ArrowRight) },
                enabled = item.optBoolean("cached"),
                onClick = { execute { vm.effect("download_export", "id" to item.optString("id")) } },
            )
            SheetAction(
                text = { Text("Retry folder save") },
                leadingIcon = { MediaIcon(Glyph.Repeat) },
                onClick = { execute { vm.effect("folder_retry") } },
            )
            SheetAction(
                text = { Text("Remove offline copy", color = MaterialTheme.colorScheme.error) },
                leadingIcon = { MediaIcon(Glyph.Close, tint = MaterialTheme.colorScheme.error) },
                onClick = { execute { removing = item } },
            )
        }
    }
    if (add)
        AddOfflinePanel(vm) {
            add = false
            section = "Transfers"
        }
    if (addTo != null)
        Panel("Add to offline playlist", { addTo = null }) {
            SheetAction(
                text = { Text("New playlist") },
                leadingIcon = { MediaIcon(Glyph.Plus) },
                onClick = {
                    playlistItems = offlinePlaylistMembers(addTo.orEmpty())
                    addTo = null
                    newPlaylist = true
                },
            )
            for (playlist in playlists) {
                val members = playlist.optJSONArray("items")?.objects() ?: emptyList()
                val merged = mergeOfflinePlaylistMembers(members, addTo.orEmpty())
                val contained = merged.size == members.size
                SheetAction(
                    text = {
                        Column {
                            Text(playlist.optString("title"))
                            Text(
                                if (contained) "Already added" else "${members.size} items",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    },
                    leadingIcon = { MediaIcon(if (contained) Glyph.Check else Glyph.Queue) },
                    onClick = {
                        if (!contained)
                            vm.updateOfflinePlaylist(playlist.optString("id"), items = merged)
                        clearSelection()
                        addTo = null
                    },
                )
            }
        }
    if (newPlaylist) {
        var name by remember { mutableStateOf("") }
        fun create() {
            if (name.isBlank()) return
            vm.createOfflinePlaylist(name.trim(), playlistItems)
            newPlaylist = false
            playlistItems = emptyList()
            clearSelection()
            section = "Playlists"
        }
        Panel("New offline playlist", { newPlaylist = false }) {
            if (playlistItems.isNotEmpty()) Text("${playlistItems.size} selected items")
            MediaTextField(
                name,
                { if (it.length <= 120) name = it },
                "Playlist name",
                Modifier.testTag("offline-playlist-name"),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions =
                    KeyboardActions(
                        onDone = {
                            create()
                        }
                    ),
            )
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                Action("Create", enabled = name.isNotBlank()) {
                    create()
                }
            }
        }
    }
    playlists
        .firstOrNull { it.optString("id") == opened }
        ?.let { playlist ->
            var name by remember(opened) { mutableStateOf(playlist.optString("title")) }
            val items = playlist.optJSONArray("items")?.objects() ?: emptyList()
            Panel("Offline playlist", { opened = "" }, scroll = false) {
                MediaTextField(
                    name,
                    { if (it.length <= 120) name = it },
                    "Name",
                    Modifier.testTag("offline-playlist-edit-name"),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                    keyboardActions =
                        KeyboardActions(
                            onDone = {
                                if (name.isNotBlank() && name != playlist.optString("title"))
                                    vm.updateOfflinePlaylist(opened, title = name)
                            }
                        ),
                )
                // Wraps at large font scales instead of clipping the destructive action.
                FlowRow(
                    Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Action("Play all", enabled = items.isNotEmpty()) {
                        vm.playOfflinePlaylist(playlist)
                    }
                    SecondaryAction(
                        "Rename",
                        enabled = name.isNotBlank() && name != playlist.optString("title"),
                    ) {
                        vm.updateOfflinePlaylist(opened, title = name)
                    }
                    QuietAction("Delete playlist", destructive = true) { deleting = playlist }
                }
                if (items.isEmpty())
                    Text(
                        "Add media from Offline with its ⋮ menu.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                ReorderableList(
                    items,
                    key = { it.optString("offline_id") },
                    onMove = { from, to ->
                        vm.updateOfflinePlaylist(
                            opened,
                            items = items.toMutableList().apply { add(to, removeAt(from)) },
                        )
                    },
                    modifier = Modifier.weight(1f, fill = false),
                    onSwipeRemove = { index, _ ->
                        vm.updateOfflinePlaylist(
                            opened,
                            items = items.filterIndexed { at, _ -> at != index },
                        )
                    },
                ) { index, item, handle ->
                        val saved =
                            available.any { it.optString("id") == item.optString("offline_id") }
                        var menu by remember(item.optString("offline_id")) { mutableStateOf(false) }
                        Row(verticalAlignment = Alignment.CenterVertically) {
                        Box(handle.size(36.dp, 56.dp), contentAlignment = Alignment.Center) {
                            MediaIcon(Glyph.Drag, Modifier.size(20.dp), MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        Box(Modifier.weight(1f)) {
                        MediaListRow(
                            item.optString("title"),
                            if (saved) item.optString("creator") else "Offline copy unavailable",
                            artwork = { Artwork(MediaCard(item), it, thumbnails) },
                            enabled = saved,
                            onClick = { vm.playOfflinePlaylist(playlist, index) },
                        ) {
                            Box {
                                MediaIconButton(Glyph.More, "Playlist item actions") { menu = true }
                                MediaMenu(menu, { menu = false }) {
                                    fun move(to: Int) {
                                        menu = false
                                        vm.updateOfflinePlaylist(
                                            opened,
                                            items =
                                                items.toMutableList().apply {
                                                    add(to, removeAt(index))
                                                },
                                        )
                                    }
                                    MediaMenuItem("Move up", Glyph.Up, enabled = index > 0) {
                                        move(index - 1)
                                    }
                                    MediaMenuItem(
                                        "Move down",
                                        Glyph.Down,
                                        enabled = index + 1 < items.size,
                                    ) {
                                        move(index + 1)
                                    }
                                    MediaMenuItem(
                                        "Remove from playlist",
                                        Glyph.Close,
                                        destructive = true,
                                    ) {
                                        menu = false
                                        vm.updateOfflinePlaylist(
                                            opened,
                                            items = items.filterIndexed { at, _ -> at != index },
                                        )
                                    }
                                }
                            }
                        }
                        }
                        }
                }
            }
        }
    removingSelected?.let { targets ->
        MediaDialog(
            { removingSelected = null },
            "Delete ${targets.size} offline items?",
            actions = {
                QuietAction("Cancel") { removingSelected = null }
                QuietAction("Delete", destructive = true) {
                    targets.forEach { vm.effect("download_remove", "id" to it.optString("id")) }
                    removingSelected = null
                    clearSelection()
                }
            },
        ) {
            Text("The selected items will be removed from app storage and their saved folder copies. Your playlists keep their references.")
        }
    }
    if (removing != null)
        MediaDialog(
            { removing = null },
            "Remove offline copy?",
            actions = {
                QuietAction("Cancel") { removing = null }
                QuietAction("Remove", destructive = true) {
                    vm.effect("download_remove", "id" to removing!!.optString("id"))
                    removing = null
                }
            },
        ) {
            Text(
                "${removing!!.optString("title")} will be removed from app storage and its saved folder copy. Your playlists keep their references.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    if (deleting != null)
        MediaDialog(
            { deleting = null },
            "Delete playlist?",
            actions = {
                QuietAction("Cancel") { deleting = null }
                QuietAction("Delete", destructive = true) {
                    vm.deleteOfflinePlaylist(deleting!!.optString("id"))
                    deleting = null
                    opened = ""
                }
            },
        ) {
            Text(
                "Saved media files will stay in your library.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
}

@Composable
private fun AddOfflinePanel(vm: YtuiViewModel, close: () -> Unit) {
    var url by rememberSaveable { mutableStateOf("") }
    fun save() {
        if (url.isBlank()) return
        runCatching { vm.linked(url.trim()) }
            .onSuccess {
                vm.play(it, download = true)
                close()
            }
            .onFailure { vm.actionError.value = "Enter a valid HTTP(S) media URL" }
    }
    Panel("Save offline", close) {
        MediaTextField(
            url,
            { url = it },
            "Video or playlist link",
            Modifier.testTag("download-url-field"),
            keyboardOptions =
                KeyboardOptions(
                    keyboardType = androidx.compose.ui.text.input.KeyboardType.Uri,
                    imeAction = ImeAction.Go,
                ),
            keyboardActions = KeyboardActions(onGo = { save() }),
        )
        Text(
            "Quality, audio-only saves and Music/Video folders are in Settings › Offline & storage.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            Action("Save offline", enabled = url.isNotBlank()) { save() }
        }
    }
}
