package dev.ytui.android

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.togetherWith
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.AnimationVector1D
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.spring as springSpec
import androidx.compose.animation.core.tween as tweenSpec
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.material3.LocalContentColor
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.unit.dp
import kotlin.math.abs
import kotlin.math.sign
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import org.json.JSONObject

/**
 * User-tunable motion. Every Compose animation reads its specs from here so one setting changes
 * the feel of the whole app. `reduced` (the shared ytui "Reduced motion" setting) always wins and
 * turns every spec into an instant snap.
 *
 * Styles:
 * - smooth: emphasized-decelerate easing, critically damped springs (default)
 * - snappy: shorter, crisper curves
 * - bouncy: springs with a little overshoot
 * - gentle: longer, softer curves
 *
 * `speed` is a percentage: 200 halves every duration, 50 doubles it.
 */
@Immutable
internal data class Motion(
    val style: String = "smooth",
    val speed: Int = 100,
    val reduced: Boolean = false,
    val cards: Boolean = true,
    val haptics: Boolean = true,
) {
    val animate: Boolean
        get() = !reduced

    private val styleScale: Float
        get() =
            when (style) {
                "snappy" -> 0.7f
                "gentle" -> 1.4f
                else -> 1f
            }

    /** Scaled duration in milliseconds for a base duration designed at 100% speed. */
    fun millis(base: Int): Int =
        if (reduced) 0 else (base * styleScale * 100f / speed.coerceIn(50, 200)).toInt()

    private val enterEasing: Easing
        get() =
            when (style) {
                "snappy" -> CubicBezierEasing(0.2f, 0f, 0f, 1f)
                "bouncy" -> CubicBezierEasing(0.34f, 1.45f, 0.64f, 1f) // gentle overshoot
                "gentle" -> CubicBezierEasing(0.4f, 0f, 0.2f, 1f)
                else -> CubicBezierEasing(0.05f, 0.7f, 0.1f, 1f) // M3 emphasized decelerate
            }

    private val exitEasing: Easing
        get() = CubicBezierEasing(0.3f, 0f, 0.8f, 0.15f) // M3 emphasized accelerate

    /** Entering/state-change tween. */
    fun <T> enter(base: Int, delay: Int = 0): FiniteAnimationSpec<T> =
        if (reduced) snap() else tweenSpec(millis(base), millis(delay), enterEasing)

    /** Leaving tween: quicker and accelerating, so the next thing is not kept waiting. */
    fun <T> exit(base: Int): FiniteAnimationSpec<T> =
        if (reduced) snap() else tweenSpec(millis((base * 0.8f).toInt()), easing = exitEasing)

    /** Neutral tween without overshoot (fades, colors, progress). */
    fun <T> fade(base: Int): FiniteAnimationSpec<T> =
        if (reduced) snap()
        else tweenSpec(millis(base), easing = CubicBezierEasing(0.2f, 0f, 0f, 1f))

    /** Physics spring for direct manipulation (drags settling, press feedback). */
    fun <T> spring(visibilityThreshold: T? = null): FiniteAnimationSpec<T> {
        if (reduced) return snap()
        val damping =
            when (style) {
                "bouncy" -> 0.55f
                "smooth" -> 0.9f
                else -> Spring.DampingRatioNoBouncy
            }
        val base =
            when (style) {
                "snappy" -> Spring.StiffnessMedium
                "gentle" -> Spring.StiffnessLow
                else -> Spring.StiffnessMediumLow
            }
        // A spring's settle time scales with 1/sqrt(stiffness).
        val factor = speed.coerceIn(50, 200) / 100f
        return springSpec(damping, base * factor * factor, visibilityThreshold)
    }

    companion object {
        fun from(mobile: JSONObject, reduced: Boolean) =
            Motion(
                style = mobile.optString("animation_style", "smooth").ifBlank { "smooth" },
                speed = mobile.optInt("animation_speed", 100),
                reduced = reduced,
                cards = mobile.optBoolean("animate_cards", true),
                haptics = mobile.optBoolean("haptics", true),
            )
    }
}

internal val LocalMotion = staticCompositionLocalOf { Motion() }

/** Master gesture switch plus an individual gesture. */
internal fun JSONObject.gesture(key: String): Boolean =
    optBoolean("gestures", true) && optBoolean(key, true)

/** Haptic tick for committed gestures, honouring the haptics setting. */
@Composable
internal fun rememberGestureHaptic(): () -> Unit {
    val haptics = LocalHapticFeedback.current
    val motion by rememberUpdatedState(LocalMotion.current)
    return remember(haptics) {
        { if (motion.haptics) haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove) }
    }
}

/** Subtle press-in scale for tappable media. Observes the touch without consuming it. */
@Composable
internal fun Modifier.pressFeedback(): Modifier {
    val motion = LocalMotion.current
    val scale = remember { Animatable(1f) }
    val scope = rememberCoroutineScope()
    if (!motion.cards || !motion.animate) return this
    return this.graphicsLayer {
            scaleX = scale.value
            scaleY = scale.value
        }
        .pointerInput(motion) {
            awaitEachGesture {
                awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                scope.launch { scale.animateTo(0.965f, motion.spring()) }
                waitForUpOrCancellation(PointerEventPass.Initial)
                scope.launch { scale.animateTo(1f, motion.spring()) }
            }
        }
}

/**
 * Staggered fade/rise for the first items of a freshly loaded list. `seen` is owned by the list
 * (reset when its content changes) so items scrolled back into view do not animate again.
 */
@Composable
internal fun Modifier.appearance(key: Any, index: Int, seen: MutableSet<Any>): Modifier {
    val motion = LocalMotion.current
    val first = remember(key) { motion.animate && motion.cards && seen.add(key) && index < 12 }
    val progress = remember(key) { Animatable(if (first) 0f else 1f) }
    androidx.compose.runtime.LaunchedEffect(key) {
        if (progress.value < 1f) progress.animateTo(1f, motion.enter(360, delay = index * 35))
    }
    return graphicsLayer {
        alpha = progress.value.coerceIn(0f, 1f)
        translationY = (1f - progress.value) * 24.dp.toPx()
    }
}

/**
 * Finger-following swipe along one axis. While dragging, [offset] tracks the finger (with rubber
 * banding where [allow] refuses a direction); on release, a swipe past [threshold] or a fling
 * calls [onSwipe] with the direction (+1 right/down, -1 left/up), which owns the follow-up
 * animation. Otherwise [offset] springs back to rest.
 *
 * [follow] = false keeps [offset] at rest (e.g. over a native video surface, which must never
 * be moved by a Compose layer) while still recognising the gesture.
 */
@Composable
internal fun Modifier.swipeAxis(
    enabled: Boolean,
    horizontal: Boolean,
    offset: Animatable<Float, AnimationVector1D>,
    threshold: Float,
    follow: Boolean = true,
    allow: (Int) -> Boolean = { true },
    onSwipe: suspend CoroutineScope.(Int) -> Unit,
): Modifier {
    val motion by rememberUpdatedState(LocalMotion.current)
    val allowed by rememberUpdatedState(allow)
    val swiped by rememberUpdatedState(onSwipe)
    val scope = rememberCoroutineScope()
    val haptic = rememberGestureHaptic()
    if (!enabled) return this
    return this.pointerInput(horizontal, follow) {
        val tracker = VelocityTracker()
        val fling = 900.dp.toPx()
        var distance = 0f
        var crossed = false
        val start: (androidx.compose.ui.geometry.Offset) -> Unit = {
            tracker.resetTracking()
            distance = offset.value
            crossed = false
            scope.launch { offset.stop() }
        }
        val drag: (androidx.compose.ui.input.pointer.PointerInputChange, Float) -> Unit =
            { change, amount ->
                change.consume()
                tracker.addPosition(change.uptimeMillis, change.position)
                val direction = sign(distance + amount).toInt()
                // Resist directions that would do nothing (first/last tab, etc.).
                distance += if (direction != 0 && !allowed(direction)) amount * 0.2f else amount
                val past = abs(distance) > threshold && allowed(sign(distance).toInt())
                if (past && !crossed) haptic()
                crossed = past
                if (follow && motion.animate) scope.launch { offset.snapTo(distance) }
            }
        val end: () -> Unit = {
            val velocity = tracker.calculateVelocity().let { if (horizontal) it.x else it.y }
            val direction = sign(distance).toInt()
            val flung = abs(velocity) > fling && sign(velocity).toInt() == direction
            val commit =
                direction != 0 &&
                    allowed(direction) &&
                    (abs(distance) > threshold || (flung && abs(distance) > threshold / 3))
            distance = 0f
            scope.launch {
                if (commit) swiped(direction)
                else offset.animateTo(0f, motion.spring())
            }
        }
        val cancel: () -> Unit = {
            distance = 0f
            scope.launch { offset.animateTo(0f, motion.spring()) }
        }
        if (horizontal)
            detectHorizontalDragGestures(
                onDragStart = start,
                onDragEnd = end,
                onDragCancel = cancel,
                onHorizontalDrag = drag,
            )
        else
            detectVerticalDragGestures(
                onDragStart = start,
                onDragEnd = end,
                onDragCancel = cancel,
                onVerticalDrag = drag,
            )
    }
}

/** A full-screen panel's drag-to-dismiss state, shared with gesture areas inside it. */
internal class PanelDrag(
    val offset: Animatable<Float, AnimationVector1D>,
    val dismiss: suspend CoroutineScope.(Int) -> Unit,
)

internal val LocalPanelDrag = staticCompositionLocalOf<PanelDrag?> { null }

/** Play/pause glyph that morphs (scale + fade) between states. */
@Composable
internal fun PlayPauseIcon(
    playing: Boolean,
    modifier: Modifier = Modifier,
    tint: Color = LocalContentColor.current,
) {
    val motion = LocalMotion.current
    AnimatedContent(
        targetState = playing,
        transitionSpec = {
            if (!motion.animate) EnterTransition.None togetherWith ExitTransition.None
            else
                (fadeIn(motion.fade(140)) + scaleIn(motion.spring(), initialScale = 0.6f)) togetherWith
                    (fadeOut(motion.fade(90)) + scaleOut(motion.exit(120), targetScale = 0.6f))
        },
        contentAlignment = Alignment.Center,
        label = "play-pause",
    ) { now ->
        MediaIcon(if (now) Glyph.Pause else Glyph.Play, modifier, tint)
    }
}
