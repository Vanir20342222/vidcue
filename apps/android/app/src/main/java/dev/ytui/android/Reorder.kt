package dev.ytui.android

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import kotlinx.coroutines.launch

/**
 * A list you can rearrange by dragging a row's handle, and trim by swiping a row away to the
 * left. Rows still carry their own menus, so every action stays reachable without gestures.
 * [onMove] is called once per drag, with the original and final positions.
 */
@Composable
internal fun <T> ReorderableList(
    items: List<T>,
    key: (T) -> Any,
    onMove: (from: Int, to: Int) -> Unit,
    modifier: Modifier = Modifier,
    swipeEnabled: Boolean = true,
    onSwipeRemove: ((Int, T) -> Unit)? = null,
    header: LazyListScope.() -> Unit = {},
    row: @Composable (index: Int, item: T, handle: Modifier) -> Unit,
) {
    val state = rememberLazyListState()
    val motion = LocalMotion.current
    val haptic = rememberGestureHaptic()
    // Positions hold original indexes, so keys stay stable while rows trade places.
    var order by remember(items) { mutableStateOf(items.indices.toList()) }
    var dragging by remember(items) { mutableStateOf<Int?>(null) }
    var start by remember(items) { mutableIntStateOf(-1) }
    val offset = remember(items) { Animatable(0f) }
    val scope = rememberCoroutineScope()
    fun rowKey(original: Int) = "${key(items[original])}#$original"
    fun height(original: Int) =
        state.layoutInfo.visibleItemsInfo.firstOrNull { it.key == rowKey(original) }?.size?.toFloat()
    LazyColumn(modifier, state = state, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        header()
        items(order, key = { rowKey(it) }) { original ->
            val item = items[original]
            val position = order.indexOf(original)
            val lifted = dragging == original
            val handle =
                Modifier.semantics { contentDescription = "Drag to reorder" }
                    .testTag("drag-handle")
                    .pointerInput(original, items) {
                        detectDragGestures(
                            onDragStart = {
                                dragging = original
                                start = order.indexOf(original)
                                haptic()
                            },
                            onDragEnd = {
                                val end = order.indexOf(original)
                                dragging = null
                                scope.launch { offset.animateTo(0f, motion.spring()) }
                                if (start >= 0 && end != start) onMove(start, end)
                            },
                            onDragCancel = {
                                dragging = null
                                order = items.indices.toList()
                                scope.launch { offset.snapTo(0f) }
                            },
                        ) { change, amount ->
                            change.consume()
                            scope.launch {
                                var shifted = offset.value + amount.y
                                val at = order.indexOf(original)
                                // Trade places once the row passes half of its neighbour.
                                if (shifted > 0 && at < order.lastIndex) {
                                    val next = height(order[at + 1]) ?: Float.MAX_VALUE
                                    if (shifted > next / 2) {
                                        order = order.toMutableList().apply { add(at + 1, removeAt(at)) }
                                        shifted -= next
                                        haptic()
                                    }
                                } else if (shifted < 0 && at > 0) {
                                    val previous = height(order[at - 1]) ?: Float.MAX_VALUE
                                    if (-shifted > previous / 2) {
                                        order = order.toMutableList().apply { add(at - 1, removeAt(at)) }
                                        shifted += previous
                                        haptic()
                                    }
                                }
                                offset.snapTo(shifted)
                            }
                        }
                    }
            val swipe = remember(original, items) { Animatable(0f) }
            val width = with(LocalDensity.current) { 360.dp.toPx() }
            Box(
                Modifier.then(
                        if (lifted)
                            Modifier.zIndex(1f).graphicsLayer {
                                translationY = offset.value
                                scaleX = 1.02f
                                scaleY = 1.02f
                            }
                        else Modifier.animateItem()
                    )
                    .then(
                        if (lifted)
                            Modifier.shadow(12.dp, MediaTokens.medium, ambientColor = Color.Black, spotColor = Color.Black)
                                .clip(MediaTokens.medium)
                                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                        else Modifier
                    )
            ) {
                // What a swipe reveals: the remove glyph on the error colour.
                if (onSwipeRemove != null && swipe.value < 0f)
                    Box(
                        Modifier.matchParentSize()
                            .clip(MediaTokens.medium)
                            .background(MaterialTheme.colorScheme.error.copy(alpha = 0.18f))
                            .padding(end = 20.dp),
                        contentAlignment = Alignment.CenterEnd,
                    ) {
                        MediaIcon(Glyph.Close, tint = MaterialTheme.colorScheme.error)
                    }
                Box(
                    Modifier.graphicsLayer { translationX = swipe.value }
                        .background(
                            if (swipe.value != 0f)
                                androidx.compose.ui.graphics.lerp(
                                    MaterialTheme.colorScheme.background,
                                    MaterialTheme.colorScheme.surface,
                                    0.6f,
                                )
                            else Color.Transparent
                        )
                        .swipeAxis(
                            enabled = swipeEnabled && onSwipeRemove != null && dragging == null,
                            horizontal = true,
                            offset = swipe,
                            threshold = with(LocalDensity.current) { 120.dp.toPx() },
                            allow = { it < 0 },
                        ) {
                            swipe.animateTo(-width, motion.exit(200))
                            onSwipeRemove?.invoke(position, item)
                        }
                ) {
                    row(position, item, handle)
                }
            }
        }
    }
}
