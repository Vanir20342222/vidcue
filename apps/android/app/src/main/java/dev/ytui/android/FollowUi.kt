package dev.ytui.android

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

/**
 * Follow toggle for a channel page header. Follows are Vidcue's own subscriptions (synced as
 * `subscriptions`), separate from the YouTube account's.
 */
@Composable
internal fun FollowButton(vm: YtuiViewModel, channelId: String, title: String, thumbnail: String) {
    if (!isChannelId(channelId)) return
    val follows by vm.sync.follows.collectAsState()
    LaunchedEffect(Unit) { vm.sync.start() }
    val following = follows.any { it.text("channel_id") == channelId }
    MediaModeButton(Glyph.People, if (following) "Unfollow $title" else "Follow $title", "follow-$channelId", following) {
        vm.sync.act { vm.sync.toggleFollow(channelId, title, thumbnail) }
    }
}

/** Followed channels as a row of round pictures; tap opens the channel. */
@Composable
internal fun FollowingRow(vm: YtuiViewModel, modifier: Modifier = Modifier) {
    val follows by vm.sync.follows.collectAsState()
    if (follows.isEmpty()) return
    FadingLazyRow(modifier.fillMaxWidth().testTag("following-row"), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
        items(follows, key = { it.text("channel_id") }) { follow ->
            Column(
                Modifier.width(72.dp)
                    .clip(MediaTokens.medium)
                    .clickable { vm.openChannel(follow.text("channel_id")) }
                    .testTag("following-${follow.text("channel_id")}"),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                ChannelAvatar(follow.text("title"), 60.dp, follow.text("thumbnail"), thumbnailsOn(vm))
                Text(follow.text("title"), style = MaterialTheme.typography.labelMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** All followed channels, with unfollow. */
@Composable
internal fun FollowingSheet(vm: YtuiViewModel, close: () -> Unit) {
    val follows by vm.sync.follows.collectAsState()
    LaunchedEffect(Unit) { runCatching { vm.sync.follows.value = vm.sync.list("follows") } }
    Panel("Following", close) {
        if (follows.isEmpty()) EmptyMediaState(Glyph.People, "Nobody yet", "")
        FollowingRow(vm)
        for (follow in follows) {
            val id = follow.text("channel_id")
            Row(
                Modifier.fillMaxWidth()
                    .heightIn(min = 56.dp)
                    .clip(MediaTokens.medium)
                    .clickable {
                        close()
                        vm.openChannel(id)
                    }
                    .testTag("follow-row-$id"),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                ChannelAvatar(follow.text("title"), 40.dp, follow.text("thumbnail"), thumbnailsOn(vm))
                Text(follow.text("title"), Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                MediaIconButton(Glyph.Close, "Unfollow", "unfollow-$id") {
                    vm.sync.act { vm.sync.toggleFollow(id, follow.text("title"), follow.text("thumbnail")) }
                }
            }
        }
    }
}
