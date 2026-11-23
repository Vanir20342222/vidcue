package dev.ytui.android

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
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

/** The Jam: start or join one, then the room with its queue, members and controls. */
@Composable
internal fun JamSheet(vm: YtuiViewModel, close: () -> Unit) {
    val room by vm.sync.jam.room.collectAsState()
    val invites by vm.sync.jam.invites.collectAsState()
    LaunchedEffect(Unit) { runCatching { vm.sync.jam.refresh() } }
    val current = room
    if (current == null) JamStart(vm, invites, close) else JamRoom(vm, current, close)
}

@Composable
private fun JamStart(vm: YtuiViewModel, invites: List<JSONObject>, close: () -> Unit) {
    val runner = rememberSyncRunner()
    var guests by remember { mutableStateOf(false) }
    Panel("Jam", close) {
        for (invite in invites) InviteRow(vm, invite)
        Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
            GlyphBadge(Glyph.Jam, 88.dp)
        }
        SyncToggle("Guests can control", guests, "jam-guests-new", Glyph.Play) { guests = it }
        ErrorLine(runner.error)
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            Action("Start", "jam-create", enabled = !runner.busy) { runner.run { vm.sync.jam.create(null, guests) } }
        }
    }
}

@Composable
private fun InviteRow(vm: YtuiViewModel, invite: JSONObject) {
    val roomId = invite.text("room_id")
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).testTag("jam-invite-$roomId"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Monogram(invite.optJSONObject("from")?.text("username").orEmpty(), 40.dp)
        Text(invite.text("title"), Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
        MediaIconButton(Glyph.Close, "Decline", "jam-decline-$roomId") { vm.sync.act { vm.sync.jam.decline(roomId) } }
        MediaIconButton(Glyph.Check, "Join", "jam-join-$roomId") { vm.sync.act { vm.sync.jam.join(roomId) } }
    }
}

@Composable
private fun JamRoom(vm: YtuiViewModel, room: JSONObject, close: () -> Unit) {
    val playback by vm.sync.jam.playback.collectAsState()
    val player by vm.player.collectAsState()
    val host = room.optString("role") == "host"
    val control = JamSteps.canControl(room)
    var inviting by remember { mutableStateOf(false) }
    var settings by remember { mutableStateOf(false) }
    val item = playback?.optJSONObject("item")
    val media = item?.optJSONObject("media")?.let(::MediaCard)
    val state = playback?.optString("state") ?: "stopped"
    Panel(
        room.text("title"),
        close,
        actions = {
            MediaIconButton(Glyph.Plus, "Invite", "jam-invite") { inviting = true }
            if (host) MediaIconButton(Glyph.Settings, "Room settings", "jam-settings") { settings = true }
        },
    ) {
        // Members: monograms with online dots.
        LazyRow(Modifier.fillMaxWidth().testTag("jam-members"), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            items(room.optJSONArray("members")?.objects().orEmpty(), key = { it.text("user_id") }) { member ->
                Box(Modifier.semantics { contentDescription = member.text("username") }) {
                    Monogram(member.text("username"), 40.dp, member.optBoolean("online"))
                    if (member.optString("role") == "host")
                        MediaIcon(Glyph.Jam, Modifier.align(Alignment.TopEnd).size(14.dp), MaterialTheme.colorScheme.primary)
                }
            }
        }
        // Now playing, large.
        if (media != null) {
            Artwork(media, Modifier.fillMaxWidth().aspectRatio(if (media.audio) 1f else 16f / 9f).testTag("jam-now"), thumbnailsOn(vm), MediaTokens.card)
            Text(media.title, style = MaterialTheme.typography.titleLarge, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text(media.creator, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
        } else EmptyMediaState(Glyph.Jam, "Add something", "")
        // Transport: the room's seek, play/pause and next, only for those who may steer. The
        // local player follows the room, so its position is the room's.
        val playerKey = player.media?.let(MediaKeys::of)
        if (control && media != null && playerKey == item?.text("media_key") && player.duration > 0f) {
            var dragging by remember { mutableStateOf<Float?>(null) }
            MediaSeekSlider(
                dragging ?: player.position.coerceIn(0f, player.duration),
                { dragging = it },
                0f..player.duration,
                {
                    val target = dragging
                    dragging = null
                    if (target != null) vm.sync.act { vm.sync.jam.control("seek", (target * 1000).toLong()) }
                },
                Modifier.fillMaxWidth().testTag("jam-seek"),
            )
        }
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterHorizontally),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            RoundIconButton(
                if (state == "playing") Glyph.Pause else Glyph.Play,
                if (state == "playing") "Pause" else "Play",
                Modifier.testTag("jam-play"),
                size = 64.dp,
                iconSize = 30.dp,
                container = MaterialTheme.colorScheme.onBackground,
                tint = MaterialTheme.colorScheme.background,
                enabled = control && media != null,
            ) {
                vm.sync.act { vm.sync.jam.control(if (state == "playing") "pause" else "play") }
            }
            RoundIconButton(Glyph.Next, "Next", Modifier.testTag("jam-next"), enabled = control) {
                vm.sync.act { vm.sync.jam.control("next") }
            }
        }
        // Queue: ordered by votes; a filled arrow marks your vote.
        val queue = room.optJSONArray("queue")?.objects().orEmpty()
        val syncStatus by vm.sync.status.collectAsState()
        val me = syncStatus.text("user_id")
        for (entry in queue) {
            val card = MediaCard(entry.optJSONObject("media") ?: JSONObject())
            val itemId = entry.text("item_id")
            val voted = entry.optBoolean("voted")
            MediaListRow(
                card.title,
                entry.optJSONObject("added_by")?.text("username").orEmpty(),
                Modifier.testTag("jam-queue-$itemId"),
                artwork = { Artwork(card, it, thumbnailsOn(vm)) },
                onClick = if (control) ({ vm.sync.act { vm.sync.jam.control("play_item", itemId = itemId) } }) else null,
                trailing = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(entry.optInt("votes").toString(), style = MaterialTheme.typography.labelLarge)
                        MediaModeButton(Glyph.Up, if (voted) "Remove vote" else "Vote", "jam-vote-$itemId", voted) {
                            vm.sync.act { vm.sync.jam.vote(itemId, !voted) }
                        }
                        if (host || entry.optJSONObject("added_by")?.text("user_id") == me)
                            MediaIconButton(Glyph.Close, "Remove", "jam-remove-$itemId") { vm.sync.act { vm.sync.jam.remove(itemId) } }
                    }
                },
            )
        }
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            QuietAction(if (host) "End" else "Leave", Modifier.testTag("jam-leave"), destructive = true) {
                vm.sync.act { vm.sync.jam.leave() }
            }
        }
    }
    if (inviting) JamInviteDialog(vm, room) { inviting = false }
    if (settings) JamSettingsDialog(vm, room) { settings = false }
}

@Composable
private fun JamSettingsDialog(vm: YtuiViewModel, room: JSONObject, close: () -> Unit) {
    var title by remember { mutableStateOf(room.text("title")) }
    MediaDialog(
        close,
        "Jam",
        actions = {
            QuietAction("Cancel") { close() }
            Action("Save", "jam-settings-save", enabled = title.isNotBlank()) {
                close()
                if (title.trim() != room.text("title")) vm.sync.act { vm.sync.jam.update(title = title.trim().take(80)) }
            }
        },
    ) {
        MediaTextField(title, { title = it.take(80) }, "Title", Modifier.testTag("jam-title"))
        SyncToggle("Guests can control", room.optBoolean("guests_can_control"), "jam-guests", Glyph.Play) {
            vm.sync.act { vm.sync.jam.update(guestsCanControl = it) }
        }
        val members = room.optJSONArray("members")?.objects().orEmpty().filter { it.optString("role") != "host" }
        for (member in members)
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Monogram(member.text("username"), 32.dp, member.optBoolean("online"))
                Text(member.text("username"), Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                MediaIconButton(Glyph.Close, "Remove ${member.text("username")}", "jam-kick-${member.text("user_id")}") {
                    vm.sync.act { vm.sync.jam.kick(member.text("user_id")) }
                }
            }
    }
}

/** Invite friends who are not in the room yet. */
@Composable
private fun JamInviteDialog(vm: YtuiViewModel, room: JSONObject, close: () -> Unit) {
    val reply by vm.sync.friends.collectAsState()
    val inside =
        (room.optJSONArray("members")?.objects().orEmpty() + room.optJSONArray("invited")?.objects().orEmpty())
            .map { it.text("user_id") }
            .toSet()
    val friends = remember(reply, inside) { FriendLists.of(reply).friends.filter { it.text("user_id") !in inside } }
    var chosen by remember { mutableStateOf<Set<String>>(emptySet()) }
    LaunchedEffect(Unit) { runCatching { vm.sync.refreshFriends() } }
    MediaDialog(
        close,
        "Invite",
        actions = {
            QuietAction("Cancel") { close() }
            Action("Invite", "jam-invite-send", enabled = chosen.isNotEmpty()) {
                close()
                vm.sync.act { vm.sync.jam.invite(chosen.toList()) }
            }
        },
    ) {
        if (friends.isEmpty())
            Text("Add friends first", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        LazyRow(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            items(friends.take(50), key = { it.text("user_id") }) { friend ->
                val id = friend.text("user_id")
                val selected = id in chosen
                Column(
                    Modifier.width(64.dp)
                        .clip(MediaTokens.medium)
                        .clickable(role = Role.Checkbox) { chosen = if (selected) chosen - id else if (chosen.size < 10) chosen + id else chosen }
                        .testTag("jam-invite-$id"),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Box {
                        Monogram(friend.text("username"), 48.dp)
                        if (selected) ChoiceMark(true, Modifier.align(Alignment.BottomEnd))
                    }
                    Text(friend.text("username"), style = MaterialTheme.typography.labelMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
    }
}

/** A pending Jam invite while not in a room: a small card at the top with join and dismiss. */
@Composable
internal fun JamInviteBanner(vm: YtuiViewModel) {
    val invites by vm.sync.jam.invites.collectAsState()
    val room by vm.sync.jam.room.collectAsState()
    var dismissed by remember { mutableStateOf<Set<String>>(emptySet()) }
    val invite = invites.firstOrNull { it.text("room_id") !in dismissed }.takeIf { room == null }
    AnimatedVisibility(invite != null, enter = fadeIn(), exit = fadeOut()) {
        val shown = invite ?: return@AnimatedVisibility
        val roomId = shown.text("room_id")
        Box(Modifier.fillMaxWidth().statusBarsPadding().padding(12.dp), contentAlignment = Alignment.TopCenter) {
            Row(
                Modifier.widthIn(max = 480.dp)
                    .fillMaxWidth()
                    .clip(MediaTokens.dock)
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    .clickable { vm.sync.open(SyncSheet.Jam) }
                    .padding(horizontal = 12.dp, vertical = 6.dp)
                    .testTag("jam-invite-banner"),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                MediaIcon(Glyph.Jam, Modifier.size(22.dp))
                Monogram(shown.optJSONObject("from")?.text("username").orEmpty(), 32.dp)
                Text(shown.text("title"), Modifier.weight(1f), style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis, textAlign = TextAlign.Start)
                MediaIconButton(Glyph.Close, "Dismiss", "jam-banner-dismiss") { dismissed = dismissed + roomId }
                MediaIconButton(Glyph.Check, "Join", "jam-banner-join") { vm.sync.act { vm.sync.jam.join(roomId) } }
            }
        }
    }
}

/** "Add to Jam" from a card: queues the item in the current room. */
internal fun addToJam(vm: YtuiViewModel, media: MediaCard) {
    vm.sync.act {
        vm.sync.jam.add(media)
        Notices.show(Notice("Added", Glyph.Jam))
    }
}
