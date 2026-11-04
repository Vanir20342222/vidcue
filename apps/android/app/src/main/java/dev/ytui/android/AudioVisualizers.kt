package dev.ytui.android

import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.unit.dp
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.sin

private const val TANK_SAMPLES = 64
private const val TAU = 6.2831855f

/** Tonal liquid colours from the theme: the accent, and a lighter tint towards the foreground. */
internal class LiquidPalette(accent: Color, ink: Color) {
    val deep: Color = accent
    val light: Color = lerp(accent, ink, 0.3f)
}

/**
 * Neon VU as a liquid tank under the cover. The surface is read from the band groups, so the
 * wave shape follows the music while the flow stays smooth. [clock] is read only while drawing.
 */
@Composable
internal fun LiquidTank(
    levels: LiquidLevels,
    clock: () -> Float,
    accent: Color,
    ink: Color,
    modifier: Modifier = Modifier,
) {
    val path = remember { Path() }
    val palette = remember(accent, ink) { LiquidPalette(accent, ink) }
    Canvas(modifier) { drawLiquidTank(clock(), levels, palette, path) }
}

/**
 * Swells the cover a little with the bass. [clock] is read so the layer updates every frame of
 * the visualiser; the levels themselves are plain arrays.
 */
internal fun Modifier.beat(levels: LiquidLevels, clock: () -> Float): Modifier = graphicsLayer {
    clock()
    val swell = 1f + 0.035f * levels.kick
    scaleX = swell
    scaleY = swell
}

/** Pulse Orbit as a liquid ring around the circular cover. */
@Composable
internal fun LiquidOrb(
    levels: LiquidLevels,
    clock: () -> Float,
    accent: Color,
    ink: Color,
    modifier: Modifier = Modifier,
) {
    val path = remember { Path() }
    val palette = remember(accent, ink) { LiquidPalette(accent, ink) }
    Canvas(modifier) { drawLiquidOrb(clock(), levels, palette, path) }
}

internal fun DrawScope.drawLiquidTank(
    time: Float,
    levels: LiquidLevels,
    palette: LiquidPalette,
    path: Path,
) {
    val height = size.height
    val overall = levels.overall
    // Calm liquid: a low swell at the foot of the stage, behind everything else.
    val baseline = height * (0.8f - 0.06f * overall)
    val amplitude = height * (0.025f + 0.05f * overall)
    liquidLayer(
        path,
        baseline + height * 0.03f,
        amplitude * 0.8f,
        time,
        levels.groups,
        phaseShift = 1.6f,
        fill = Brush.verticalGradient(
            listOf(palette.deep.copy(alpha = 0.22f), palette.deep.copy(alpha = 0.08f)),
            startY = baseline,
            endY = height,
        ),
        crest = null,
    )
    liquidLayer(
        path,
        baseline,
        amplitude,
        time,
        levels.groups,
        phaseShift = 0f,
        fill = Brush.verticalGradient(
            listOf(palette.light.copy(alpha = 0.32f), palette.deep.copy(alpha = 0.12f)),
            startY = baseline,
            endY = height,
        ),
        crest = palette.light.copy(alpha = 0.5f),
    )
    // A glow behind the cover that flashes on each beat.
    drawCircle(
        Brush.radialGradient(
            listOf(palette.deep.copy(alpha = 0.18f + 0.3f * levels.kick), palette.deep.copy(alpha = 0f)),
            center = Offset(size.width / 2f, height / 2f),
            radius = size.width * 0.5f,
        ),
        radius = size.width * 0.5f,
        center = Offset(size.width / 2f, height / 2f),
    )
    // Equaliser: bass in the middle, mirrored outwards, rounded bars fading to the floor.
    val count = 32
    val gap = size.width * 0.01f
    val barWidth = (size.width - gap * (count + 1)) / count
    for (i in 0 until count) {
        val fromMiddle = kotlin.math.abs(i - (count - 1) / 2f) / (count / 2f)
        val position = fromMiddle * (SPECTRUM_BANDS - 1)
        val low = position.toInt().coerceIn(0, SPECTRUM_BANDS - 1)
        val high = (low + 1).coerceAtMost(SPECTRUM_BANDS - 1)
        val mix = position - low
        val level = (levels.bands[low] * (1 - mix) + levels.bands[high] * mix).coerceIn(0f, 1f)
        val barHeight = height * (0.02f + 0.24f * level)
        val left = gap + i * (barWidth + gap)
        drawRoundRect(
            Brush.verticalGradient(
                listOf(palette.light.copy(alpha = 0.55f + 0.4f * level), palette.deep.copy(alpha = 0.05f)),
                startY = height - barHeight,
                endY = height,
            ),
            topLeft = Offset(left, height - barHeight),
            size = androidx.compose.ui.geometry.Size(barWidth, barHeight),
            cornerRadius = androidx.compose.ui.geometry.CornerRadius(barWidth / 2),
        )
    }
}

/** One wave layer: an optional crest line, then the liquid body down to the bottom edge. */
private fun DrawScope.liquidLayer(
    path: Path,
    baseline: Float,
    amplitude: Float,
    time: Float,
    groups: FloatArray,
    phaseShift: Float,
    fill: Brush,
    crest: Color?,
) {
    val width = size.width
    path.reset()
    for (i in 0..TANK_SAMPLES) {
        val u = i.toFloat() / TANK_SAMPLES
        val y = baseline + amplitude * surfaceOffset(u, time + phaseShift, groups)
        if (i == 0) path.moveTo(0f, y) else path.lineTo(u * width, y)
    }
    if (crest != null) drawPath(path, crest, style = Stroke(width = 1.5f.dp.toPx()))
    path.lineTo(width, size.height)
    path.lineTo(0f, size.height)
    path.close()
    drawPath(path, brush = fill)
}

internal fun DrawScope.drawLiquidOrb(
    time: Float,
    levels: LiquidLevels,
    palette: LiquidPalette,
    path: Path,
) {
    val minDimension = min(size.width, size.height)
    val center = Offset(size.width / 2f, size.height / 2f)
    // The cover is 56% of the stage; everything else is drawn outside its edge.
    val cover = minDimension * 0.28f
    val outer = minDimension * 0.49f
    val overall = levels.overall
    // A soft halo that breathes with the music.
    drawCircle(
        Brush.radialGradient(
            listOf(palette.deep.copy(alpha = 0.28f + 0.22f * overall), palette.deep.copy(alpha = 0f)),
            center = center,
            radius = outer * (0.92f + 0.08f * levels.kick),
        ),
        radius = outer,
        center = center,
    )
    // Beat ripples: thin rings travelling out from the cover and fading.
    for (age in levels.ripples) {
        if (age >= RIPPLE_SECONDS) continue
        val t = age / RIPPLE_SECONDS
        val eased = 1f - (1f - t) * (1f - t)
        drawCircle(
            palette.light.copy(alpha = 0.4f * (1f - t)),
            radius = cover + (outer - cover) * eased,
            center = center,
            style = Stroke(width = (2.5f - 1.5f * t).dp.toPx()),
        )
    }
    // Spectrum ring: fine bars, mirrored left/right with the bass at the bottom, so the
    // figure stays symmetric and every band (auto-gained) is visible.
    val bars = 96
    val inner = cover + 10.dp.toPx()
    val reach = outer - inner
    val stroke = maxOf(1.5f.dp.toPx(), (TAU * inner / bars) * 0.38f)
    for (i in 0 until bars) {
        val angle = TAU / 4f + i.toFloat() / bars * TAU
        // 0 at the bottom, 1 at the top, the same on both sides.
        val height = if (i <= bars / 2) i / (bars / 2f) else (bars - i) / (bars / 2f)
        val position = height * (SPECTRUM_BANDS - 1)
        val low = position.toInt().coerceIn(0, SPECTRUM_BANDS - 1)
        val high = (low + 1).coerceAtMost(SPECTRUM_BANDS - 1)
        val mix = position - low
        val level = (levels.bands[low] * (1 - mix) + levels.bands[high] * mix).coerceIn(0f, 1f)
        val length = 3.dp.toPx() + (reach - 3.dp.toPx()) * level
        val direction = Offset(cos(angle), sin(angle))
        drawLine(
            lerp(palette.deep, palette.light, 0.35f + 0.65f * level).copy(alpha = 0.55f + 0.45f * level),
            center + direction * inner,
            center + direction * (inner + length),
            stroke,
            StrokeCap.Round,
        )
    }
    // A clean ring hugging the cover, swelling a little on each beat.
    drawCircle(
        palette.light.copy(alpha = 0.7f),
        radius = cover + (3f + 3f * levels.kick).dp.toPx(),
        center = center,
        style = Stroke(width = 2.dp.toPx()),
    )
}
