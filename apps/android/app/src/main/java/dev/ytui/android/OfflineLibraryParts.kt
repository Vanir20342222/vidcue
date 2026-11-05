package dev.ytui.android

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.text
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONObject

/**
 * Whether an offline copy matches a library search: every word of [query] must appear in its
 * title, channel/artist or quality, ignoring case and accents.
 */
internal fun offlineMatches(item: JSONObject, query: String): Boolean {
    val words = foldForSearch(query).split(' ').filter { it.isNotBlank() }
    if (words.isEmpty()) return true
    val media = item.optJSONObject("media")
    val haystack =
        foldForSearch(
            listOf(
                    item.optString("title"),
                    media?.optString("title").orEmpty(),
                    media?.optString("creator").orEmpty(),
                    item.optString("quality"),
                )
                .joinToString(" ")
        )
    return words.all { it in haystack }
}

/** Offline playlists match on their name or on any member's title or artist. */
internal fun offlinePlaylistMatches(playlist: JSONObject, query: String): Boolean {
    val words = foldForSearch(query).split(' ').filter { it.isNotBlank() }
    if (words.isEmpty()) return true
    val members = playlist.optJSONArray("items")?.objects().orEmpty()
    val haystack =
        foldForSearch(
            (listOf(playlist.optString("title")) +
                    members.flatMap { listOf(it.optString("title"), it.optString("creator")) })
                .joinToString(" ")
        )
    return words.all { it in haystack }
}

private val MARKS = Regex("\\p{Mn}+")

internal fun foldForSearch(text: String): String =
    java.text.Normalizer.normalize(text.lowercase(), java.text.Normalizer.Form.NFD).replace(MARKS, "")

/** Library filter: a calm rounded field with a clear button, filtering as you type. */
@Composable
internal fun OfflineSearchField(query: String, onQuery: (String) -> Unit, placeholder: String, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    val motion = LocalMotion.current
    val focus = LocalFocusManager.current
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val edge by
        animateColorAsState(
            if (focused) colors.onSurface.copy(alpha = 0.45f) else colors.onSurface.copy(alpha = 0f),
            motion.fade(180),
            label = "offline-search-edge",
        )
    BasicTextField(
        query,
        onQuery,
        modifier.testTag("offline-search"),
        singleLine = true,
        textStyle = MaterialTheme.typography.bodyLarge.copy(color = colors.onSurface),
        cursorBrush = SolidColor(colors.primary),
        interactionSource = source,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { focus.clearFocus() }),
        decorationBox = { inner ->
            Row(
                Modifier.fillMaxWidth()
                    .heightIn(min = 48.dp)
                    .clip(MediaTokens.pill)
                    .background(colors.surfaceContainer)
                    .border(1.5.dp, edge, MediaTokens.pill)
                    .padding(start = 16.dp, end = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                MediaIcon(Glyph.Search, Modifier.size(20.dp), colors.onSurfaceVariant)
                Spacer(Modifier.width(12.dp))
                Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
                    if (query.isEmpty())
                        Text(
                            placeholder,
                            style = MaterialTheme.typography.bodyLarge,
                            color = colors.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    inner()
                }
                AnimatedVisibility(
                    query.isNotEmpty(),
                    enter = if (motion.animate) fadeIn(motion.fade(120)) + scaleIn(motion.spring(), 0.6f) else EnterTransition.None,
                    exit = if (motion.animate) fadeOut(motion.fade(100)) + scaleOut(motion.exit(120), 0.6f) else ExitTransition.None,
                ) {
                    RoundIconButton(
                        Glyph.Close,
                        "Clear search",
                        Modifier.testTag("offline-search-clear"),
                        size = 40.dp,
                        iconSize = 18.dp,
                        tint = colors.onSurfaceVariant,
                    ) {
                        onQuery("")
                    }
                }
            }
        },
    )
}

/**
 * Replaces the library's filter row while selecting: close, how many are chosen, and a
 * select-all toggle. The count rolls when it changes.
 */
@Composable
internal fun OfflineSelectionHeader(
    count: Int,
    allSelected: Boolean,
    onClose: () -> Unit,
    onToggleAll: () -> Unit,
) {
    val motion = LocalMotion.current
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth()
            .padding(horizontal = 8.dp)
            .heightIn(min = 56.dp)
            .testTag("offline-selection-header"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RoundIconButton(Glyph.Close, "Cancel selection", Modifier.testTag("offline-selection-close"), action = onClose)
        Spacer(Modifier.width(4.dp))
        AnimatedContent(
            count,
            // One stable node carries the count for TalkBack and tests while the digits roll.
            Modifier.weight(1f).testTag("offline-selection-count").clearAndSetSemantics {
                text = androidx.compose.ui.text.AnnotatedString("$count selected")
            },
            transitionSpec = {
                if (!motion.animate) EnterTransition.None togetherWith ExitTransition.None
                else {
                    val up = targetState > initialState
                    (fadeIn(motion.fade(160)) + slideInVertically(motion.enter(220)) { if (up) it / 2 else -it / 2 }) togetherWith
                        (fadeOut(motion.fade(120)) + slideOutVertically(motion.exit(160)) { if (up) -it / 2 else it / 2 })
                }
            },
            contentAlignment = Alignment.CenterStart,
            label = "selection-count",
        ) { shown ->
            Text(
                "$shown selected",
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.SemiBold,
                color = colors.onSurface,
            )
        }
        Pill(
            if (allSelected) "Deselect all" else "Select all",
            Modifier.testTag("offline-select-all"),
            style = PillStyle.Quiet,
            glyph = if (allSelected) Glyph.Close else Glyph.Check,
            onClick = onToggleAll,
        )
    }
}

/**
 * Floating action dock for a selection: three large labelled actions that rise in above the
 * navigation, so they are reachable with a thumb and never shift the list.
 */
@Composable
internal fun OfflineSelectionActions(
    visible: Boolean,
    enabled: Boolean,
    modifier: Modifier = Modifier,
    onAdd: () -> Unit,
    onNewPlaylist: () -> Unit,
    onDelete: () -> Unit,
) {
    val motion = LocalMotion.current
    val colors = MaterialTheme.colorScheme
    AnimatedVisibility(
        visible,
        modifier,
        enter =
            if (motion.animate)
                fadeIn(motion.fade(160)) + slideInVertically(motion.spring()) { it } + scaleIn(motion.spring(), 0.92f)
            else EnterTransition.None,
        exit =
            if (motion.animate)
                fadeOut(motion.fade(140)) + slideOutVertically(motion.exit(200)) { it }
            else ExitTransition.None,
    ) {
        Row(
            Modifier.padding(horizontal = 16.dp)
                .fillMaxWidth()
                .shadow(16.dp, MediaTokens.dock, ambientColor = Color.Black, spotColor = Color.Black)
                .clip(MediaTokens.dock)
                .background(lerpSurface(colors.background, colors.surfaceContainerHigh))
                .padding(6.dp)
                .testTag("offline-selection-actions"),
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            SelectionAction(Glyph.Queue, "Add to playlist", "offline-add-selected", enabled, Modifier.weight(1f), onClick = onAdd)
            SelectionAction(Glyph.Plus, "New playlist", "offline-playlist-selected", enabled, Modifier.weight(1f), onClick = onNewPlaylist)
            SelectionAction(Glyph.Close, "Delete", "offline-delete-selected", enabled, Modifier.weight(1f), destructive = true, onClick = onDelete)
        }
    }
}

private fun lerpSurface(background: Color, surface: Color): Color =
    androidx.compose.ui.graphics.lerp(background, surface, 0.92f)

@Composable
private fun SelectionAction(
    glyph: Glyph,
    label: String,
    tag: String,
    enabled: Boolean,
    modifier: Modifier,
    destructive: Boolean = false,
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val motion = LocalMotion.current
    val source = remember { MutableInteractionSource() }
    val ink by
        animateColorAsState(
            (if (destructive) colors.error else colors.onSurface).copy(alpha = if (enabled) 1f else 0.38f),
            motion.fade(200),
            label = "selection-action",
        )
    Column(
        modifier
            .pressScale(source, 0.92f)
            .clip(MediaTokens.card)
            .clickable(source, SoftIndication, enabled = enabled, role = Role.Button, onClick = onClick)
            .heightIn(min = 64.dp)
            .padding(vertical = 8.dp)
            .testTag(tag)
            .semantics { contentDescription = label },
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(4.dp, Alignment.CenterVertically),
    ) {
        MediaIcon(glyph, Modifier.size(22.dp), ink)
        Text(
            label,
            Modifier.clearAndSetSemantics {},
            style = MaterialTheme.typography.labelMedium,
            color = ink,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** Round check that fills and pops when an item is chosen. */
@Composable
internal fun SelectionMark(selected: Boolean, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    val motion = LocalMotion.current
    val fill by
        animateColorAsState(
            if (selected) colors.primary else Color.Transparent,
            motion.fade(160),
            label = "selection-fill",
        )
    val edge by
        animateColorAsState(
            if (selected) colors.primary else colors.onSurfaceVariant.copy(alpha = 0.7f),
            motion.fade(160),
            label = "selection-edge",
        )
    val tick by animateFloatAsState(if (selected) 1f else 0f, motion.spring(), label = "selection-tick")
    Box(
        modifier
            .size(48.dp)
            .clearAndSetSemantics {},
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier.size(24.dp)
                .graphicsLayer {
                    val pop = 1f + 0.12f * tick * (1f - tick) * 4f
                    scaleX = pop
                    scaleY = pop
                }
                .clip(CircleShape)
                .background(fill)
                .border(2.dp, edge, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            MediaIcon(
                Glyph.Check,
                Modifier.size(16.dp).graphicsLayer {
                    scaleX = tick
                    scaleY = tick
                    alpha = tick.coerceIn(0f, 1f)
                },
                mediaOnAccent(colors.primary),
            )
        }
    }
}

/** Shown when a library search finds nothing. */
@Composable
internal fun OfflineNoMatches(query: String) {
    Column(
        Modifier.fillMaxWidth().padding(vertical = 32.dp, horizontal = 24.dp).testTag("offline-no-matches"),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        MediaIcon(Glyph.Search, Modifier.size(28.dp), MaterialTheme.colorScheme.onSurfaceVariant)
        Text("Nothing matches “$query”", style = MaterialTheme.typography.titleMedium, color = LocalContentColor.current)
        Text(
            "Search looks at titles, artists, channels and quality.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
