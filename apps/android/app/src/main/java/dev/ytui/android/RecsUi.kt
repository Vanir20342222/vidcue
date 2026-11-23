package dev.ytui.android

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import org.json.JSONObject

internal object RecItems {
    /** Recommendation cards: the media of each item, online only, without duplicates. */
    fun cards(reply: JSONObject): List<MediaCard> =
        reply.optJSONArray("items")
            ?.objects()
            .orEmpty()
            .mapNotNull { item -> (item.optJSONObject("media") ?: item).takeIf(::isOnlineMedia) }
            .map(::MediaCard)
            .distinctBy { it.key }
}

/** True when Home should show the "For you" shelf: signed in, opted in, and items to show. */
@Composable
internal fun recsShown(vm: YtuiViewModel): Boolean {
    val recs by vm.sync.recs.collectAsState()
    val status by vm.sync.status.collectAsState()
    return status.optBoolean("signed_in") && recs.optBoolean("enabled") && RecItems.cards(recs).isNotEmpty()
}

/** The private recommendations shelf above Home's first shelf; nothing when off or empty. */
@Composable
internal fun RecsShelf(vm: YtuiViewModel, inset: Dp = MediaTokens.textInset) {
    val recs by vm.sync.recs.collectAsState()
    val cards = remember(recs) { RecItems.cards(recs) }
    if (cards.isEmpty()) return
    Column(Modifier.fillMaxWidth().testTag("recs-shelf"), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        ShelfHeading("For you", inset = inset)
        FadingLazyRow(
            Modifier.fillMaxWidth(),
            contentPadding = PaddingValues(horizontal = inset),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            items(cards, key = { it.key }) { card ->
                Column(
                    Modifier.width(if (card.audio) 144.dp else 224.dp)
                        .clickable { vm.play(card) }
                        .testTag("recs-${card.key}"),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Artwork(card, Modifier.fillMaxWidth().aspectRatio(if (card.audio) 1f else 16f / 9f), thumbnailsOn(vm), MediaTokens.card)
                    Text(card.title, style = MaterialTheme.typography.titleSmall, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    if (card.creator.isNotBlank())
                        Text(card.creator, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                }
            }
        }
    }
}

/** Opt-in row for the Vidcue settings: recommendations are off until switched on. */
@Composable
internal fun RecsToggle(vm: YtuiViewModel) {
    val settings by vm.sync.recsSettings.collectAsState()
    LaunchedEffect(Unit) { runCatching { vm.sync.refreshRecs() } }
    val enabled = settings.optBoolean("enabled")
    SettingGroup("For you", showTitle = false) {
        SyncToggle("Recommendations", enabled, "recs-enabled", Glyph.Browse) {
            vm.sync.act { vm.sync.setRecs(enabled = it) }
        }
        if (enabled) {
            SyncToggle("Similar artists", settings.optBoolean("music", true), "recs-music", Glyph.Music) {
                vm.sync.act { vm.sync.setRecs(music = it) }
            }
            ValueRow("Refresh", Modifier.testTag("recs-refresh"), glyph = Glyph.Refresh, chevron = false) {
                vm.sync.act {
                    vm.sync.obj("recs_refresh")
                    Notices.show(Notice("Refreshing", Glyph.Refresh))
                }
            }
        }
    }
}
