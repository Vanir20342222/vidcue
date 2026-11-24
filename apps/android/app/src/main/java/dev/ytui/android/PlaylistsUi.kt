package dev.ytui.android

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONObject

internal object PlaylistMoves {
    /**
     * After dragging the entry at [from] to [to] in [ids] (both indexes in the old order), the
     * entry it now follows: null when it moved to the top (`playlist_move` `after_entry_id`).
     */
    fun after(ids: List<String>, from: Int, to: Int): String? {
        if (from !in ids.indices || to !in ids.indices) return null
        val order = ids.toMutableList()
        val moved = order.removeAt(from)
        order.add(to, moved)
        return if (to == 0) null else order[to - 1]
    }
}

/** Your playlists: covers, a share mark on shared ones, and a field to create one. */
@Composable
internal fun PlaylistsSheet(vm: YtuiViewModel, close: () -> Unit) {
    val playlists by vm.sync.playlists.collectAsState()
    var title by remember { mutableStateOf("") }
    LaunchedEffect(Unit) { runCatching { vm.sync.playlists.value = vm.sync.list("playlists") } }
    Panel("Playlists", close) {
        NewPlaylistRow(title, { title = it }) {
            val name = title.trim()
            title = ""
            vm.sync.act {
                vm.sync.obj("playlist_create", "title" to name)
                vm.sync.playlists.value = vm.sync.list("playlists")
            }
        }
        if (playlists.isEmpty()) EmptyMediaState(Glyph.Playlist, "No playlists", "")
        for (playlist in playlists) PlaylistRow(vm, playlist) { vm.sync.open(SyncSheet.Playlist(playlist.text("id"))) }
    }
}

@Composable
private fun NewPlaylistRow(title: String, change: (String) -> Unit, create: () -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Box(Modifier.weight(1f)) { MediaTextField(title, { change(it.take(200)) }, "New playlist", Modifier.testTag("playlist-new-title")) }
        RoundIconButton(Glyph.Plus, "Create playlist", Modifier.testTag("playlist-create"), enabled = title.isNotBlank(), action = create)
    }
}

@Composable
private fun PlaylistRow(vm: YtuiViewModel, playlist: JSONObject, open: () -> Unit) {
    val cover = playlist.optJSONObject("cover")
    MediaListRow(
        playlist.text("title"),
        playlist.optInt("count").toString(),
        Modifier.testTag("playlist-${playlist.text("id")}"),
        artwork = { modifier ->
            if (cover != null) Artwork(MediaCard(cover), modifier, thumbnailsOn(vm)) else GlyphTile(Glyph.Playlist, modifier)
        },
        square = true,
        onClick = open,
        trailing = {
            if (playlist.optBoolean("shared"))
                MediaIcon(Glyph.People, Modifier.size(20.dp).semantics { contentDescription = "Shared" }, MaterialTheme.colorScheme.onSurfaceVariant)
        },
    )
}

/** One playlist: play, drag to reorder, swipe to remove; rename, share or delete from the header. */
@Composable
internal fun PlaylistSheet(vm: YtuiViewModel, id: String, close: () -> Unit) {
    val playlists by vm.sync.playlists.collectAsState()
    val revision by vm.sync.playlistRevision.collectAsState()
    val playlist = playlists.firstOrNull { it.text("id") == id }
    var entries by remember(id) { mutableStateOf<List<JSONObject>>(emptyList()) }
    var editing by remember { mutableStateOf(false) }
    var sharing by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf(false) }
    suspend fun reload() {
        entries = vm.sync.list("playlist_entries", "id" to id)
    }
    LaunchedEffect(id, revision) { runCatching { reload() } }
    val cards = remember(entries) { entries.map { MediaCard(it.optJSONObject("media") ?: JSONObject()) } }
    val shared = playlist?.optBoolean("shared") == true
    val owner = !shared || playlist?.optString("role") == "owner"
    Panel(
        playlist?.text("title").orEmpty(),
        close,
        scroll = false,
        actions = {
            if (owner) MediaIconButton(Glyph.Edit, "Rename", "playlist-rename") { editing = true }
            if (owner && vm.sync.has("social")) MediaIconButton(Glyph.People, "Share", "playlist-share") { sharing = true }
            MediaIconButton(Glyph.Close, if (owner) "Delete" else "Leave", "playlist-delete") { deleting = true }
        },
    ) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
            if (cards.size > 1)
                RoundIconButton(Glyph.Shuffle, "Shuffle play", Modifier.testTag("playlist-shuffle")) { vm.playAll(cards.shuffled()) }
            if (cards.isNotEmpty())
                RoundIconButton(
                    Glyph.Play,
                    "Play",
                    Modifier.testTag("playlist-play"),
                    size = 56.dp,
                    iconSize = 28.dp,
                    container = MaterialTheme.colorScheme.onBackground,
                    tint = MaterialTheme.colorScheme.background,
                ) {
                    vm.playAll(cards)
                }
        }
        if (entries.isEmpty()) EmptyMediaState(Glyph.Playlist, "Empty", "")
        ReorderableList(
            entries,
            key = { it.text("entry_id") },
            onMove = { from, to ->
                val ids = entries.map { it.text("entry_id") }
                val moved = ids.getOrNull(from) ?: return@ReorderableList
                vm.sync.act {
                    vm.sync.obj("playlist_move", "id" to id, "entry_id" to moved, "after_entry_id" to (PlaylistMoves.after(ids, from, to) ?: org.json.JSONObject.NULL))
                    reload()
                }
            },
            modifier = Modifier.weight(1f, fill = false).fillMaxWidth().testTag("playlist-entries"),
            onSwipeRemove = { _, entry ->
                entries = entries.filter { it.text("entry_id") != entry.text("entry_id") }
                vm.sync.act {
                    vm.sync.obj("playlist_remove", "id" to id, "entry_id" to entry.text("entry_id"))
                    reload()
                    vm.sync.playlists.value = vm.sync.list("playlists")
                }
            },
        ) { index, entry, handle ->
            val card = cards.getOrNull(index) ?: MediaCard(entry.optJSONObject("media") ?: JSONObject())
            Row(Modifier.fillMaxWidth().testTag("playlist-entry-${entry.text("entry_id")}"), verticalAlignment = Alignment.CenterVertically) {
                Box(handle.size(40.dp, 56.dp), contentAlignment = Alignment.Center) {
                    MediaIcon(Glyph.Drag, Modifier.size(20.dp), MaterialTheme.colorScheme.onSurfaceVariant)
                }
                Box(Modifier.weight(1f)) {
                    MediaListRow(
                        card.title,
                        card.creator,
                        artwork = { Artwork(card, it, thumbnailsOn(vm)) },
                        onClick = { vm.playAll(cards.drop(index)) },
                    )
                }
            }
        }
    }
    if (editing && playlist != null) RenameDialog(vm, id, playlist.text("title")) { editing = false }
    if (sharing && playlist != null) ShareDialog(vm, playlist) { sharing = false }
    if (deleting)
        MediaDialog(
            { deleting = false },
            playlist?.text("title").orEmpty(),
            actions = {
                QuietAction("Cancel") { deleting = false }
                QuietAction(if (owner) "Delete" else "Leave", Modifier.testTag("playlist-delete-confirm"), destructive = true) {
                    deleting = false
                    vm.sync.act {
                        // An owner's "Delete" removes the server copy for everyone and then
                        // the local playlist (leaving alone would keep it as a normal one).
                        if (shared) vm.sync.obj("shared_playlist_leave", "playlist_id" to id)
                        if (owner) vm.sync.obj("playlist_delete", "id" to id)
                        vm.sync.playlists.value = vm.sync.list("playlists")
                    }
                    close()
                }
            },
        ) {}
}

@Composable
private fun RenameDialog(vm: YtuiViewModel, id: String, current: String, close: () -> Unit) {
    var title by remember { mutableStateOf(current) }
    MediaDialog(
        close,
        "Rename",
        actions = {
            QuietAction("Cancel") { close() }
            Action("Save", "playlist-rename-save", enabled = title.isNotBlank()) {
                close()
                vm.sync.act {
                    vm.sync.obj("playlist_rename", "id" to id, "title" to title.trim())
                    vm.sync.playlists.value = vm.sync.list("playlists")
                }
            }
        },
    ) {
        MediaTextField(title, { title = it.take(200) }, "Title", Modifier.testTag("playlist-rename-title"))
    }
}

/**
 * Members as monogram chips: tap a friend to add, tap a member to remove. The first share
 * creates the server copy (`shared_playlist_share`); later changes use `shared_playlist_members`.
 */
@Composable
private fun ShareDialog(vm: YtuiViewModel, playlist: JSONObject, close: () -> Unit) {
    val reply by vm.sync.friends.collectAsState()
    val friends = remember(reply) { FriendLists.of(reply).friends }
    val runner = rememberSyncRunner()
    val id = playlist.text("id")
    val shared = playlist.optBoolean("shared")
    var members by remember(id) { mutableStateOf<Set<String>>(emptySet()) }
    // Shared already: start from the server's member list (taps add or remove from it).
    var loaded by remember(id) { mutableStateOf(!shared) }
    LaunchedEffect(id) {
        runCatching { vm.sync.refreshFriends() }
        if (shared) {
            runner.run {
                val info = vm.sync.obj("shared_playlist_info", "playlist_id" to id)
                val owner = info.optJSONObject("owner")?.text("user_id").orEmpty()
                members =
                    info.optJSONArray("members")?.objects().orEmpty().map { it.text("user_id") }.filter { it.isNotBlank() && it != owner }.toSet()
                loaded = true
            }
        }
    }
    MediaDialog(close, playlist.text("title"), actions = { QuietAction("Done", Modifier.testTag("playlist-share-done")) { close() } }) {
        if (friends.isEmpty())
            Text("Add friends first", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        LazyRow(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            items(friends, key = { it.text("user_id") }) { friend ->
                val user = friend.text("user_id")
                val member = user in members
                Column(
                    Modifier.width(64.dp)
                        .clip(MediaTokens.medium)
                        .clickable(enabled = loaded && !runner.busy, role = Role.Checkbox) {
                            runner.run {
                                if (!shared && members.isEmpty())
                                    vm.sync.obj("shared_playlist_share", "playlist_id" to id, "user_ids" to org.json.JSONArray(listOf(user)))
                                else if (member)
                                    vm.sync.obj("shared_playlist_members", "playlist_id" to id, "remove" to org.json.JSONArray(listOf(user)))
                                else vm.sync.obj("shared_playlist_members", "playlist_id" to id, "add" to org.json.JSONArray(listOf(user)))
                                members = if (member) members - user else members + user
                                vm.sync.playlists.value = vm.sync.list("playlists")
                            }
                        }
                        .testTag("playlist-member-$user"),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Box {
                        Monogram(friend.text("username"), 48.dp)
                        if (member) ChoiceMark(true, Modifier.align(Alignment.BottomEnd))
                    }
                    Text(friend.text("username"), style = MaterialTheme.typography.labelMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
        ErrorLine(runner.error)
    }
}

/** Add one item to a playlist, or start a new playlist with it. */
@Composable
internal fun AddToPlaylistSheet(vm: YtuiViewModel, media: MediaCard, close: () -> Unit) {
    val playlists by vm.sync.playlists.collectAsState()
    var title by remember { mutableStateOf("") }
    val runner = rememberSyncRunner()
    LaunchedEffect(Unit) { runCatching { vm.sync.playlists.value = vm.sync.list("playlists") } }
    // Confirms only after the engine accepted the change; a failure stays in the sheet.
    fun add(block: suspend () -> Unit) =
        runner.run {
            block()
            vm.sync.playlists.value = vm.sync.list("playlists")
            Notices.show(Notice("Added", Glyph.Playlist))
            close()
        }
    Panel("", close) {
        ActionSheetHeader(media, thumbnailsOn(vm))
        ErrorLine(runner.error)
        NewPlaylistRow(title, { title = it }) {
            val name = title.trim()
            add { vm.sync.obj("playlist_create", "title" to name, "media" to media.value) }
        }
        for (playlist in playlists)
            PlaylistRow(vm, playlist) {
                add { vm.sync.obj("playlist_add", "id" to playlist.text("id"), "media" to media.value) }
            }
    }
}
