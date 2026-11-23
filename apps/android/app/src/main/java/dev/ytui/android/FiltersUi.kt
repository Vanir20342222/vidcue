package dev.ytui.android

import androidx.compose.foundation.layout.*
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONObject

/** Filter kinds that are on or off as a whole (no value). */
internal val SWITCH_FILTERS = listOf(Triple("shorts", Glyph.Film, "Shorts"), Triple("watched", Glyph.Check, "Watched"), Triple("live", Glyph.Radio, "Live"))

internal object Filters {
    fun switched(filters: List<JSONObject>, kind: String): JSONObject? = filters.firstOrNull { it.text("kind") == kind }

    fun valued(filters: List<JSONObject>): List<JSONObject> = filters.filter { it.text("kind") == "channel" || it.text("kind") == "keyword" }

    /** The value a "Hide channel" filter stores: the channel id when known, else its name. */
    fun channelValue(media: MediaCard): String? = media.channelId.ifBlank { media.creator.trim() }.takeIf { it.isNotBlank() }
}

/** Hidden in Home, search, related and up next: switches for whole kinds, then channels and words. */
@Composable
internal fun FiltersSheet(vm: YtuiViewModel, close: () -> Unit) {
    val filters by vm.sync.filters.collectAsState()
    var keyword by remember { mutableStateOf("") }
    LaunchedEffect(Unit) { runCatching { vm.sync.filters.value = vm.sync.list("filters") } }
    Panel("Filters", close) {
        SettingGroup("Hide", showTitle = false) {
            for ((kind, glyph, label) in SWITCH_FILTERS) {
                val existing = Filters.switched(filters, kind)
                SyncToggle(label, existing != null, "filter-$kind", glyph) { on ->
                    vm.sync.act {
                        if (on) vm.sync.addFilter(kind)
                        else Filters.switched(vm.sync.filters.value, kind)?.let { vm.sync.removeFilter(it.text("id")) }
                    }
                }
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Box(Modifier.weight(1f)) { MediaTextField(keyword, { keyword = it.take(100) }, "Word", Modifier.testTag("filter-keyword")) }
            RoundIconButton(Glyph.Plus, "Hide titles with this word", Modifier.testTag("filter-keyword-add"), enabled = keyword.isNotBlank()) {
                val value = keyword.trim()
                keyword = ""
                vm.sync.act { vm.sync.addFilter("keyword", value) }
            }
        }
        for (filter in Filters.valued(filters))
            Row(
                Modifier.fillMaxWidth().heightIn(min = 52.dp).testTag("filter-${filter.text("id")}"),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                GlyphTile(if (filter.text("kind") == "channel") Glyph.Account else Glyph.Search, Modifier.size(36.dp))
                Text(filter.text("value"), Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                MediaIconButton(Glyph.Close, "Show again", "filter-remove-${filter.text("id")}") {
                    vm.sync.act { vm.sync.removeFilter(filter.text("id")) }
                }
            }
    }
}

/** Card-menu helper: hide everything from this item's channel. */
internal fun hideChannel(vm: YtuiViewModel, media: MediaCard) {
    val value = Filters.channelValue(media) ?: return
    vm.sync.act {
        vm.sync.addFilter("channel", value)
        Notices.show(Notice(media.creator.take(40).ifBlank { "Hidden" }, Glyph.Preview, "Undo", {
            vm.sync.filters.value
                .lastOrNull { it.text("kind") == "channel" && it.text("value") == value }
                ?.let { found -> vm.sync.act { vm.sync.removeFilter(found.text("id")) } }
        }))
    }
}
