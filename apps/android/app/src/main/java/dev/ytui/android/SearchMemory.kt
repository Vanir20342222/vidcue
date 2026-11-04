package dev.ytui.android

import android.content.Context
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONArray

/**
 * Recent searches, kept only on this device and only while History & resume is on. Video/music
 * searches and radio searches are remembered separately.
 */
internal object RecentSearches {
    private const val LIMIT = 12
    val version = mutableIntStateOf(0)

    private fun prefs(context: Context) =
        context.getSharedPreferences("ytui-search", Context.MODE_PRIVATE)

    private fun key(radio: Boolean) = if (radio) "radio" else "media"

    fun load(context: Context, radio: Boolean): List<String> {
        val raw = prefs(context).getString(key(radio), null) ?: return emptyList()
        return runCatching {
                JSONArray(raw).let { array -> (0 until array.length()).map { array.getString(it) } }
            }
            .getOrDefault(emptyList())
    }

    private fun save(context: Context, radio: Boolean, values: List<String>) {
        prefs(context).edit().putString(key(radio), JSONArray(values.take(LIMIT)).toString()).apply()
        version.intValue++
    }

    fun add(context: Context, radio: Boolean, query: String) {
        val clean = query.trim()
        if (clean.isBlank() || clean.startsWith("http://") || clean.startsWith("https://")) return
        save(context, radio, listOf(clean) + load(context, radio).filterNot { it.equals(clean, true) })
    }

    fun remove(context: Context, radio: Boolean, query: String) =
        save(context, radio, load(context, radio).filterNot { it == query })

    fun clear(context: Context) {
        prefs(context).edit().clear().apply()
        version.intValue++
    }
}

/** True when the clipboard holds text, read from its description only (no paste prompt). */
internal fun clipboardHasText(context: Context): Boolean {
    val clipboard =
        context.getSystemService(Context.CLIPBOARD_SERVICE) as? android.content.ClipboardManager
            ?: return false
    val description = runCatching { clipboard.primaryClipDescription }.getOrNull() ?: return false
    return description.hasMimeType(android.content.ClipDescription.MIMETYPE_TEXT_PLAIN) ||
        description.hasMimeType(android.content.ClipDescription.MIMETYPE_TEXT_URILIST)
}

internal fun clipboardText(context: Context): String {
    val clipboard =
        context.getSystemService(Context.CLIPBOARD_SERVICE) as? android.content.ClipboardManager
            ?: return ""
    return runCatching { clipboard.primaryClip?.getItemAt(0)?.coerceToText(context)?.toString() }
        .getOrNull()
        ?.trim()
        .orEmpty()
}

/**
 * Shown while the search field is focused and empty: paste what you copied, or tap a recent search
 * to run it again. Each recent search can be forgotten on its own.
 */
@Composable
internal fun SearchSuggestions(
    radio: Boolean,
    remember: Boolean,
    onSearch: (String) -> Unit,
    onPaste: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val version by RecentSearches.version
    val recent = remember(radio, version, remember) {
        if (remember) RecentSearches.load(context, radio) else emptyList()
    }
    val canPaste = remember(version) { clipboardHasText(context) }
    val colors = MaterialTheme.colorScheme
    LazyColumn(
        modifier.background(colors.background).testTag("search-suggestions"),
        contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp),
    ) {
        if (canPaste)
            item {
                SheetAction(
                    modifier = Modifier.testTag("search-paste"),
                    text = {
                        Column {
                            Text("Paste from clipboard", style = MaterialTheme.typography.bodyLarge)
                            Text(
                                "Play a copied link or search copied text",
                                style = MaterialTheme.typography.bodySmall,
                                color = colors.onSurfaceVariant,
                            )
                        }
                    },
                    leadingIcon = { GlyphTile(Glyph.Clipboard, Modifier.size(36.dp).clip(MediaTokens.medium)) },
                    onClick = { clipboardText(context).takeIf { it.isNotBlank() }?.let(onPaste) },
                )
            }
        if (recent.isNotEmpty())
            item {
                Row(
                    Modifier.fillMaxWidth().padding(start = 10.dp, top = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        "Recent searches",
                        Modifier.weight(1f),
                        style = MaterialTheme.typography.titleSmall,
                        color = colors.onSurfaceVariant,
                    )
                    QuietAction("Clear") { RecentSearches.clear(context) }
                }
            }
        items(recent, key = { it }) { query ->
            Row(
                Modifier.fillMaxWidth()
                    .heightIn(min = 52.dp)
                    .clip(MediaTokens.medium)
                    .clickable { onSearch(query) }
                    .padding(start = 10.dp)
                    .animateItem()
                    .testTag("recent-search"),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                MediaIcon(Glyph.History, Modifier.size(20.dp), colors.onSurfaceVariant)
                Text(
                    query,
                    Modifier.weight(1f),
                    style = MaterialTheme.typography.bodyLarge,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                RoundIconButton(
                    Glyph.Close,
                    "Forget $query",
                    iconSize = 16.dp,
                    tint = colors.onSurfaceVariant,
                ) {
                    RecentSearches.remove(context, radio, query)
                }
            }
        }
        if (!canPaste && recent.isEmpty())
            item {
                Text(
                    if (remember) "Your recent searches will appear here."
                    else "Recent searches are off while History & resume is off.",
                    Modifier.padding(16.dp),
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.onSurfaceVariant,
                )
            }
    }
}
