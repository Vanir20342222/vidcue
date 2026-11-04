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
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.unit.dp
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.sin

private const val TANK_SAMPLES = 64
private const val ORBIT_SAMPLES = 72
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
    val baseline = height * (0.74f - 0.1f * overall)
    val amplitude = height * (0.05f + 0.1f * overall)
    // The back layer sits lower, trails the front in phase and reads as depth.
    liquidLayer(
        path,
        baseline + height * 0.04f,
        amplitude * 0.8f,
        time,
        levels.groups,
        phaseShift = 1.6f,
        fill = Brush.verticalGradient(
            listOf(palette.deep.copy(alpha = 0.35f), palette.deep.copy(alpha = 0.2f)),
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
            listOf(palette.light.copy(alpha = 0.55f), palette.deep.copy(alpha = 0.5f)),
            startY = baseline,
            endY = height,
        ),
        crest = palette.light.copy(alpha = 0.7f),
    )
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
    // The inner edge keeps clear of the cover (radius 0.16 of the width) even at full swell.
    val radius = minDimension * 0.33f
    orbLayer(
        path,
        center,
        radius * 0.9f,
        amplitude = 0.3f,
        time = time,
        groups = levels.groups,
        phaseShift = 1.9f,
        color = palette.light,
        alpha = 0.22f,
        stroke = null,
    )
    orbLayer(
        path,
        center,
        radius,
        amplitude = 0.3f,
        time = time,
        groups = levels.groups,
        phaseShift = 0f,
        color = palette.deep,
        alpha = 0.28f,
        stroke = palette.light.copy(alpha = 0.6f),
    )
}

private fun DrawScope.orbLayer(
    path: Path,
    center: Offset,
    radius: Float,
    amplitude: Float,
    time: Float,
    groups: FloatArray,
    phaseShift: Float,
    color: Color,
    alpha: Float,
    stroke: Color?,
) {
    path.reset()
    for (i in 0..ORBIT_SAMPLES) {
        val angle = i.toFloat() / ORBIT_SAMPLES * TAU
        val distance = radius * (1f + amplitude * orbitOffset(angle, time + phaseShift, groups))
        val x = center.x + cos(angle) * distance
        val y = center.y + sin(angle) * distance
        if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
    }
    path.close()
    drawPath(path, color, alpha)
    if (stroke != null) drawPath(path, stroke, style = Stroke(width = 1.5f.dp.toPx()))
}
