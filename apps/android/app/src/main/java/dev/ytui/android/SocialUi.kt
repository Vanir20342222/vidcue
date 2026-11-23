package dev.ytui.android

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
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
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONObject

/** Who sees what you play: three levels, as three icon chips. */
internal val SHARE_LEVELS = listOf(Triple("off", Glyph.Lock, "Private"), Triple("now_playing", Glyph.Play, "Now"), Triple("history", Glyph.History, "History"))

/** Friend lists from `GET /api/friends`, by role. */
internal data class FriendLists(
    val friends: List<JSONObject>,
    val incoming: List<JSONObject>,
    val outgoing: List<JSONObject>,
) {
    companion object {
        fun of(reply: JSONObject) =
            FriendLists(
                reply.optJSONArray("friends")?.objects().orEmpty(),
                reply.optJSONArray("incoming")?.objects().orEmpty(),
                reply.optJSONArray("outgoing")?.objects().orEmpty(),
            )
    }
}

@Composable
internal fun FriendsSheet(vm: YtuiViewModel, close: () -> Unit) {
    val reply by vm.sync.friends.collectAsState()
    val activity by vm.sync.activity.collectAsState()
    val share by vm.sync.shareActivity.collectAsState()
    val lists = remember(reply) { FriendLists.of(reply) }
    val runner = rememberSyncRunner()
    var username by remember { mutableStateOf("") }
    var menu by remember { mutableStateOf<JSONObject?>(null) }
    LaunchedEffect(Unit) {
        runCatching { vm.sync.refreshFriends() }
        runCatching { vm.sync.refreshShare() }
        runCatching { vm.sync.refreshActivity() }
    }
    Panel("Friends", close) {
        // Your own sharing level first: nothing is shared until you choose to.
        Row(
            Modifier.fillMaxWidth().testTag("share-activity"),
            horizontalArrangement = Arrangement.spacedBy(24.dp, Alignment.CenterHorizontally),
        ) {
            for ((value, glyph, label) in SHARE_LEVELS)
                MediaModeButton(glyph, label, "share-$value", share == value) {
                    if (share != value) vm.sync.act { vm.sync.setShare(value) }
                }
        }
        SelectedLevel(share)
        if (lists.friends.isNotEmpty())
            LazyRow(Modifier.fillMaxWidth().testTag("friends-row"), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                items(lists.friends, key = { it.text("user_id") }) { friend ->
                    Column(
                        Modifier.width(64.dp)
                            .clip(MediaTokens.medium)
                            .clickable(role = Role.Button) { menu = friend }
                            .testTag("friend-${friend.text("user_id")}"),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(4.dp),
                    ) {
                        Monogram(friend.text("username"), 52.dp)
                        Text(friend.text("username"), style = MaterialTheme.typography.labelMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        // Requests waiting for you, then the ones you sent.
        for (request in lists.incoming)
            PersonRow(request.text("username"), "friend-incoming-${request.text("user_id")}") {
                MediaIconButton(Glyph.Check, "Accept ${request.text("username")}", "friend-accept-${request.text("user_id")}") {
                    vm.sync.act {
                        vm.sync.obj("friend_accept", "user_id" to request.text("user_id"))
                        vm.sync.refreshFriends()
                    }
                }
                MediaIconButton(Glyph.Close, "Decline ${request.text("username")}", "friend-decline-${request.text("user_id")}") {
                    vm.sync.act {
                        vm.sync.obj("friend_remove", "user_id" to request.text("user_id"))
                        vm.sync.refreshFriends()
                    }
                }
            }
        for (request in lists.outgoing)
            PersonRow(request.text("username"), "friend-outgoing-${request.text("user_id")}", pending = true) {
                MediaIconButton(Glyph.Close, "Cancel request to ${request.text("username")}", "friend-cancel-${request.text("user_id")}") {
                    vm.sync.act {
                        vm.sync.obj("friend_remove", "user_id" to request.text("user_id"))
                        vm.sync.refreshFriends()
                    }
                }
            }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Box(Modifier.weight(1f)) {
                MediaTextField(username, { username = it.take(64); runner.error = null }, "Username", Modifier.testTag("friend-username"))
            }
            RoundIconButton(Glyph.Plus, "Add friend", Modifier.testTag("friend-add"), enabled = username.isNotBlank() && !runner.busy) {
                runner.run {
                    val result = vm.sync.obj("friend_request", "username" to username.trim())
                    username = ""
                    vm.sync.refreshFriends()
                    Notices.show(Notice(if (result.optString("status") == "accepted") "Friends" else "Request sent", Glyph.People))
                }
            }
        }
        ErrorLine(runner.error)
        for (item in activity) ActivityCard(vm, item)
    }
    val person = menu
    if (person != null)
        MediaDialog(
            { menu = null },
            person.text("username"),
            actions = {
                QuietAction("Cancel") { menu = null }
                QuietAction("Remove", Modifier.testTag("friend-remove"), destructive = true) {
                    menu = null
                    vm.sync.act {
                        vm.sync.obj("friend_remove", "user_id" to person.text("user_id"))
                        vm.sync.refreshFriends()
                    }
                }
            },
        ) {
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) { Monogram(person.text("username"), 72.dp) }
        }
}

@Composable
private fun SelectedLevel(share: String) {
    val label = SHARE_LEVELS.firstOrNull { it.first == share }?.third ?: return
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
        StatusDot(if (share == "off") MaterialTheme.colorScheme.outline else SyncColors.online)
        Spacer(Modifier.width(6.dp))
        Text(label, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun PersonRow(name: String, tag: String, pending: Boolean = false, actions: @Composable RowScope.() -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Monogram(name, 40.dp)
        Text(
            name,
            Modifier.weight(1f),
            style = MaterialTheme.typography.bodyLarge,
            color = if (pending) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        actions()
    }
}

/** A friend's now playing (artwork with their monogram) and their recent plays. */
@Composable
private fun ActivityCard(vm: YtuiViewModel, item: JSONObject) {
    val name = item.text("username")
    val now = item.optJSONObject("now_playing")?.optJSONObject("media")
    val recent = item.optJSONArray("recent")?.objects().orEmpty()
    if (now == null && recent.isEmpty()) return
    Column(Modifier.fillMaxWidth().testTag("activity-${item.text("user_id")}"), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (now != null) {
            val card = MediaCard(now)
            Box(Modifier.fillMaxWidth().clip(MediaTokens.card).clickable { vm.play(card) }) {
                Artwork(card, Modifier.fillMaxWidth().aspectRatio(16f / 9f), thumbnailsOn(vm), MediaTokens.card)
                Row(
                    Modifier.align(Alignment.BottomStart).padding(10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Monogram(name, 36.dp)
                    if (item.optJSONObject("now_playing")?.optBoolean("playing") == true)
                        MediaIcon(Glyph.Play, Modifier.size(18.dp), MediaTokens.videoInk)
                }
            }
            Text(card.title, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
        } else
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Monogram(name, 32.dp)
                Text(name, style = MaterialTheme.typography.titleSmall)
            }
        if (recent.isNotEmpty())
            LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(recent.take(20), key = { "${it.text("url")}#${it.optLong("at")}" }) { play ->
                    val card = MediaCard(play)
                    Artwork(
                        card,
                        Modifier.width(120.dp).aspectRatio(16f / 9f).clickable { vm.play(card) }.semantics { contentDescription = card.title },
                        thumbnailsOn(vm),
                    )
                }
            }
    }
}

/** Things friends sent: tap plays and marks read, swipe or the cross deletes. */
@Composable
internal fun InboxSheet(vm: YtuiViewModel, close: () -> Unit) {
    val items by vm.sync.inbox.collectAsState()
    LaunchedEffect(Unit) { runCatching { vm.sync.refreshInbox() } }
    Panel("Inbox", close, scroll = false) {
        if (items.isEmpty()) EmptyMediaState(Glyph.Inbox, "Nothing yet", "")
        ReorderableList(
            items,
            key = { it.text("id") },
            onMove = { _, _ -> },
            modifier = Modifier.weight(1f, fill = false).fillMaxWidth().testTag("inbox-list"),
            onSwipeRemove = { _, item -> deleteInbox(vm, item) },
        ) { _, item, _ ->
            val media = item.optJSONObject("media") ?: JSONObject()
            val card = MediaCard(media)
            val from = item.optJSONObject("from")?.text("username").orEmpty()
            MediaListRow(
                card.title,
                listOf(from, item.text("note")).filter { it.isNotBlank() }.joinToString(" · "),
                Modifier.testTag("inbox-${item.text("id")}"),
                artwork = { Artwork(card, it, thumbnailsOn(vm)) },
                highlighted = !item.optBoolean("read"),
                onClick = {
                    vm.play(card)
                    if (!item.optBoolean("read"))
                        vm.sync.quiet {
                            vm.sync.obj("inbox_read", "id" to item.text("id"))
                            vm.sync.refreshInbox()
                        }
                },
                trailing = {
                    MediaIconButton(Glyph.Close, "Delete", "inbox-delete-${item.text("id")}") { deleteInbox(vm, item) }
                },
            )
        }
    }
}

private fun deleteInbox(vm: YtuiViewModel, item: JSONObject) {
    vm.sync.inbox.value = vm.sync.inbox.value.filter { it.text("id") != item.text("id") }
    vm.sync.act {
        vm.sync.obj("inbox_delete", "id" to item.text("id"))
        vm.sync.refreshInbox()
    }
}

/** Send one item to a friend: pick a monogram, optionally add a short note. */
@Composable
internal fun SendToFriendSheet(vm: YtuiViewModel, media: MediaCard, close: () -> Unit) {
    val reply by vm.sync.friends.collectAsState()
    val friends = remember(reply) { FriendLists.of(reply).friends }
    val runner = rememberSyncRunner()
    var chosen by remember { mutableStateOf<String?>(null) }
    var note by remember { mutableStateOf("") }
    LaunchedEffect(Unit) { runCatching { vm.sync.refreshFriends() } }
    Panel("", close) {
        ActionSheetHeader(media, thumbnailsOn(vm))
        if (friends.isEmpty())
            Text(
                "Add friends first",
                Modifier.fillMaxWidth(),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
        LazyRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            items(friends, key = { it.text("user_id") }) { friend ->
                val id = friend.text("user_id")
                val selected = chosen == id
                Column(
                    Modifier.width(64.dp)
                        .clip(MediaTokens.medium)
                        .clickable(role = Role.RadioButton) { chosen = id }
                        .testTag("send-to-$id")
                        .semantics { if (selected) contentDescription = "${friend.text("username")}, selected" },
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Box(contentAlignment = Alignment.Center) {
                        Monogram(friend.text("username"), 52.dp)
                        if (selected) ChoiceMark(true, Modifier.align(Alignment.BottomEnd))
                    }
                    Text(friend.text("username"), style = MaterialTheme.typography.labelMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
        MediaTextField(note, { note = it.take(200) }, "Note", Modifier.testTag("send-note"))
        ErrorLine(runner.error)
        Action("Send", "send-to-friend", enabled = chosen != null && !runner.busy && isOnlineMedia(media.value)) {
            runner.run {
                vm.sync.obj("inbox_send", "to" to chosen, "media" to media.value, "note" to note.trim().ifBlank { null })
                Notices.show(Notice("Sent", Glyph.Inbox))
                close()
            }
        }
    }
}
