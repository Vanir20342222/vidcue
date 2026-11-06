package dev.ytui.android

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONObject

/**
 * "1.2M subscribers" from the stream's follower count, else YouTube's own label from the watch
 * details, or blank when neither is known.
 */
internal fun subscriberLabel(media: JSONObject?): String {
    val count = media?.optLong("channel_follower_count", -1) ?: -1
    if (count < 0)
        return media?.optString("subscribers_text")?.takeUnless { it == "null" }.orEmpty()
    return "${compactCount(count)} ${if (count == 1L) "subscriber" else "subscribers"}"
}

/**
 * The playing media with the watch details filled in where the stream left gaps. Values the
 * stream already had win; nothing is invented.
 */
internal fun withWatchDetails(media: JSONObject?, details: JSONObject?): JSONObject? {
    if (media == null || details == null || details.length() == 0) return media
    val merged = media.copyJson()
    val date = media.optString("upload_date").takeUnless { it.isBlank() || it == "null" }
    val published = details.optString("published_text")
    if (date == null && published.isNotBlank()) merged.put("published_text", published)
    if (media.optLong("like_count", -1) < 0 && details.has("like_count"))
        merged.put("like_count", details.optLong("like_count"))
    val subscribers = details.optString("subscribers_text")
    if (subscribers.isNotBlank()) merged.put("subscribers_text", subscribers)
    return merged
}

/** The description without blank runs, so a collapsed preview is not mostly empty lines. */
internal fun descriptionPreview(text: String): String =
    text.trim().replace(Regex("\n{3,}"), "\n\n")

/**
 * Watch-page details under the inline video. Three calm groups with room between them: what it
 * is (title, then views and age), who made it (avatar, name, subscribers), and what you can do.
 * The description sits in a soft card below the actions; it opens in place when the card or the
 * title is tapped, so the page stays short until more is wanted.
 */
@Composable
internal fun WatchDetails(
    title: String,
    media: JSONObject?,
    creator: String,
    showCreator: Boolean,
    mobile: JSONObject,
    thumbnails: Boolean,
    headerModifier: Modifier,
    error: String?,
    openChannel: (String) -> Unit = {},
    actions: @Composable () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val motion = LocalMotion.current
    val description = descriptionPreview(media?.optString("description").orEmpty().takeUnless { it == "null" }.orEmpty())
    val information = media?.let { mediaInformation(it, mobile) }.orEmpty()
    // A new video starts collapsed.
    var open by rememberSaveable(media?.optString("url")) { mutableStateOf(false) }
    val canOpen = description.isNotBlank()
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Column(
            Modifier.fillMaxWidth()
                .then(headerModifier)
                .then(
                    if (canOpen)
                        Modifier.clickable(
                            onClickLabel = if (open) "Hide description" else "Show description",
                            role = Role.Button,
                        ) {
                            open = !open
                        }
                    else Modifier
                ),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(
                title.ifBlank { " " },
                Modifier.animateContentSize(motion.spring()),
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.SemiBold,
                maxLines = if (open) 6 else 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (information.isNotBlank() || canOpen)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (information.isNotBlank())
                        Text(
                            information,
                            Modifier.weight(1f, fill = false).testTag("video-information"),
                            style = MaterialTheme.typography.bodyMedium,
                            color = colors.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    if (canOpen)
                        Text(
                            if (open) "  less" else "  …more",
                            style = MaterialTheme.typography.bodyMedium,
                            fontWeight = FontWeight.SemiBold,
                            color = colors.onSurface,
                            maxLines = 1,
                        )
                }
        }
        if (showCreator) {
            val channelId = media?.optString("channel_id")?.takeIf(::isChannelId).orEmpty()
            Row(
                Modifier.opensChannel(creator, channelId, openChannel),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                ChannelAvatar(
                    creator,
                    40.dp,
                    imageUrl = media?.optString("channel_thumbnail").orEmpty(),
                    loadImage = thumbnails,
                )
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
                    Text(
                        creator,
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    val subscribers = subscriberLabel(media)
                    if (subscribers.isNotBlank())
                        Text(
                            subscribers,
                            Modifier.testTag("video-subscribers"),
                            style = MaterialTheme.typography.bodySmall,
                            color = colors.onSurfaceVariant,
                            maxLines = 1,
                        )
                }
            }
        }
        error?.let { Text(it, color = colors.error, maxLines = 2) }
        actions()
        if (canOpen) {
            val turn by animateFloatAsState(if (open) 180f else 0f, motion.spring(), label = "description-chevron")
            Column(
                Modifier.fillMaxWidth()
                    .clip(MediaTokens.card)
                    .background(colors.onSurface.copy(alpha = 0.06f))
                    .clickable(role = Role.Button) { open = !open }
                    .padding(horizontal = 14.dp, vertical = 12.dp)
                    .animateContentSize(motion.spring())
                    .testTag("video-description")
                    .semantics {
                        contentDescription = "Description"
                        stateDescription = if (open) "Expanded" else "Collapsed"
                    },
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        "Description",
                        Modifier.weight(1f),
                        style = MaterialTheme.typography.labelLarge,
                        fontWeight = FontWeight.SemiBold,
                    )
                    MediaIcon(
                        Glyph.Down,
                        Modifier.size(18.dp).graphicsLayer { rotationZ = turn },
                        colors.onSurfaceVariant,
                    )
                }
                Text(
                    description,
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.onSurfaceVariant,
                    maxLines = if (open) Int.MAX_VALUE else 2,
                    overflow = TextOverflow.Ellipsis,
                )
                AnimatedVisibility(
                    open && media?.optLong("like_count", -1)?.let { it >= 0 } == true,
                    enter = if (motion.animate) fadeIn(motion.fade(200)) + expandVertically(motion.spring()) else EnterTransition.None,
                    exit = if (motion.animate) fadeOut(motion.fade(120)) + shrinkVertically(motion.exit(200)) else ExitTransition.None,
                ) {
                    Text(
                        "${compactCount(media!!.optLong("like_count"))} likes",
                        style = MaterialTheme.typography.labelMedium,
                        color = colors.onSurfaceVariant,
                    )
                }
            }
        }
    }
}
