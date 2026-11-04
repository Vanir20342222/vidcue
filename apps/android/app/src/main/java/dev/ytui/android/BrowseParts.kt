package dev.ytui.android

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

/**
 * Search input: a calm field that brightens its edge while focused. The submit arrow turns into a
 * filled button once there is something to search for.
 */
@Composable
internal fun SearchField(
    query: String,
    onQuery: (String) -> Unit,
    placeholder: String,
    modifier: Modifier,
    searching: Boolean,
    onClear: () -> Unit,
    onSubmit: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val motion = LocalMotion.current
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val edge by
        animateColorAsState(
            if (focused) colors.onSurface.copy(alpha = 0.55f) else colors.onSurface.copy(alpha = 0f),
            motion.fade(180),
            label = "search-edge",
        )
    val fill by
        animateColorAsState(
            if (focused) colors.surfaceContainerHigh else colors.surfaceContainer,
            motion.fade(180),
            label = "search-fill",
        )
    BasicTextField(
        query,
        onQuery,
        modifier,
        singleLine = true,
        textStyle = MaterialTheme.typography.bodyLarge.copy(color = colors.onSurface),
        cursorBrush = SolidColor(colors.primary),
        interactionSource = source,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { onSubmit() }),
        decorationBox = { inner ->
            Row(
                Modifier.fillMaxWidth()
                    .heightIn(min = 52.dp)
                    .clip(MediaTokens.fieldShape)
                    .background(fill)
                    .border(1.5.dp, edge, MediaTokens.fieldShape)
                    .padding(start = 14.dp, end = 4.dp),
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
                if (searching && query.isNotEmpty())
                    RoundIconButton(
                        Glyph.Close,
                        "Clear search",
                        iconSize = 18.dp,
                        tint = colors.onSurfaceVariant,
                        action = onClear,
                    )
                val ready = query.isNotBlank()
                RoundIconButton(
                    Glyph.ArrowRight,
                    "Search",
                    Modifier.testTag("search-button"),
                    size = 44.dp,
                    iconSize = 18.dp,
                    container = if (ready) colors.onBackground else MediaTokens.transparent,
                    tint = if (ready) colors.background else colors.onSurfaceVariant,
                    action = onSubmit,
                )
            }
        },
    )
}

/** Placeholder cards with a drifting glint, shaped like the cards that will replace them. */
@Composable
internal fun FeedSkeleton(square: Boolean, modifier: Modifier = Modifier) {
    Column(
        modifier
            .fillMaxWidth()
            .padding(horizontal = if (square) 0.dp else 0.dp)
            .semantics { contentDescription = "Loading…" },
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(
            Modifier.fillMaxWidth()
                .aspectRatio(if (square) 1f else 16f / 9f)
                .shimmer(if (square) MediaTokens.medium else RoundedCornerShape(0.dp))
        )
        Row(
            Modifier.padding(horizontal = if (square) 0.dp else 16.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            if (!square) Box(Modifier.size(36.dp).shimmer(CircleShape))
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Box(Modifier.fillMaxWidth(0.82f).height(14.dp).shimmer(MediaTokens.small))
                Box(Modifier.fillMaxWidth(0.5f).height(12.dp).shimmer(MediaTokens.small))
            }
        }
    }
}

/** Duration or live marker on artwork. Live gets a pulsing dot instead of capitals. */
@Composable
internal fun ArtworkBadge(live: Boolean, text: String, modifier: Modifier = Modifier, pulse: Boolean = true) {
    val motion = LocalMotion.current
    Row(
        modifier
            .padding(8.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(if (live) Color(0xffd7263d) else MediaTokens.videoScrim)
            .padding(horizontal = 6.dp, vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(5.dp),
    ) {
        if (live) {
            val pulse =
                // A feed of stations would otherwise run one animation per card every frame,
                // including while scrolling; there the dot stays still.
                if (!motion.animate || !pulse) remember { mutableStateOf(1f) }
                else
                    rememberInfiniteTransition(label = "live")
                        .animateFloat(
                            0.35f,
                            1f,
                            infiniteRepeatable(tween(900), RepeatMode.Reverse),
                            label = "live-dot",
                        )
            Box(
                Modifier.size(6.dp)
                    .graphicsLayer { alpha = pulse.value }
                    .background(MediaTokens.videoInk, CircleShape)
            )
        }
        Text(
            text,
            color = MediaTokens.videoInk,
            style =
                MaterialTheme.typography.labelSmall.merge(
                    TextStyle(fontFeatureSettings = "tnum")
                ),
        )
    }
}

/** Three bars that dance while this card is the one playing; still when paused. */
@Composable
internal fun PlayingBars(playing: Boolean, modifier: Modifier = Modifier, color: Color = MediaTokens.videoInk) {
    val motion = LocalMotion.current
    val live = playing && motion.animate
    val phase =
        if (!live) remember { mutableStateOf(0f) }
        else
            rememberInfiniteTransition(label = "bars")
                .animateFloat(
                    0f,
                    (2 * Math.PI).toFloat(),
                    infiniteRepeatable(tween(1100, easing = LinearEasing)),
                    label = "bars-phase",
                )
    Box(
        modifier
            .padding(8.dp)
            .size(30.dp)
            .shadow(6.dp, CircleShape)
            .background(MediaTokens.videoScrim, CircleShape)
            .semantics { contentDescription = if (playing) "Playing" else "Paused" },
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.size(14.dp)) {
            val bar = size.width / 5
            for (i in 0..2) {
                val level =
                    if (!live) listOf(0.45f, 0.8f, 0.6f)[i]
                    else 0.3f + 0.7f * ((kotlin.math.sin(phase.value * (1 + i * 0.37f) + i * 1.7f) + 1) / 2)
                val h = size.height * level
                drawLine(
                    color,
                    Offset(bar * (0.5f + i * 2f), size.height),
                    Offset(bar * (0.5f + i * 2f), size.height - h),
                    bar,
                    StrokeCap.Round,
                )
            }
        }
    }
}

/** Pull-to-refresh disc: an arc that fills as you pull, then spins while the feed reloads. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun androidx.compose.foundation.layout.BoxScope.RefreshDisc(
    state: PullToRefreshState,
    refreshing: Boolean,
) {
    val colors = MaterialTheme.colorScheme
    AnimatedVisibility(
        refreshing || state.distanceFraction > 0f,
        Modifier.align(Alignment.TopCenter),
        enter = fadeIn() + scaleIn(initialScale = 0.6f),
        exit = fadeOut() + scaleOut(targetScale = 0.6f),
    ) {
        Box(
            Modifier.graphicsLayer {
                    val pulled = if (refreshing) 1f else state.distanceFraction.coerceIn(0f, 1.4f)
                    translationY = pulled * 56.dp.toPx() - 20.dp.toPx()
                }
                .size(40.dp)
                .shadow(8.dp, CircleShape)
                .background(colors.surfaceContainerHigh, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            if (refreshing) MediaSpinner(Modifier.size(20.dp))
            else
                Canvas(Modifier.size(20.dp)) {
                    val stroke = 2.5.dp.toPx()
                    drawArc(
                        colors.onSurface,
                        -90f,
                        300f * state.distanceFraction.coerceIn(0f, 1f),
                        false,
                        Offset(stroke, stroke),
                        Size(size.width - stroke * 2, size.height - stroke * 2),
                        style = Stroke(stroke, cap = StrokeCap.Round),
                    )
                }
        }
    }
}
