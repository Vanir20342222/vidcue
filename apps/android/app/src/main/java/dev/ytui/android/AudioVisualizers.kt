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
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.unit.dp
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.sin

private const val TAU = 6.2831855f

/** Tonal liquid colours from the theme: the accent, and a lighter tint towards the foreground. */
internal class LiquidPalette(accent: Color, ink: Color) {
    val deep: Color = accent
    val light: Color = lerp(accent, ink, 0.3f)
}

/**
 * Visualiser colours taken from the cover, as Plexamp does: the artwork's tint pushed to a
 * vivid, light-enough hue so it reads on the dark player. Falls back to the theme accent while
 * no artwork colour is known.
 */
internal fun artPalette(art: Color?, accent: Color, ink: Color): LiquidPalette {
    if (art == null) return LiquidPalette(accent, ink)
    val hsv = FloatArray(3)
    android.graphics.Color.colorToHSV(art.toArgb(), hsv)
    // Near-grey covers keep the theme accent rather than turning the visualiser grey.
    if (hsv[1] < 0.12f) return LiquidPalette(accent, ink)
    hsv[1] = hsv[1].coerceIn(0.5f, 0.85f)
    hsv[2] = hsv[2].coerceAtLeast(0.88f)
    return LiquidPalette(Color(android.graphics.Color.HSVToColor(hsv)), ink)
}

/**
 * The "vu" style, drawn as Strings: sine strings plucked by the spectrum behind the cover.
 * [clock] is read only while drawing.
 */
@Composable
internal fun LiquidTank(
    levels: LiquidLevels,
    clock: () -> Float,
    accent: Color,
    ink: Color,
    modifier: Modifier = Modifier,
    art: Color? = null,
) {
    val path = remember { Path() }
    val palette = remember(art, accent, ink) { artPalette(art, accent, ink) }
    Canvas(modifier) { drawStrings(clock(), levels, palette, path) }
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

/** Pulse Orbit: a radial spectrum around the circular cover. */
@Composable
internal fun LiquidOrb(
    levels: LiquidLevels,
    clock: () -> Float,
    accent: Color,
    ink: Color,
    modifier: Modifier = Modifier,
    art: Color? = null,
) {
    val path = remember { Path() }
    val palette = remember(art, accent, ink) { artPalette(art, accent, ink) }
    Canvas(modifier) { drawLiquidOrb(clock(), levels, palette, path) }
}

internal fun DrawScope.drawLiquidOrb(
    time: Float,
    levels: LiquidLevels,
    palette: LiquidPalette,
    path: Path,
) {
    // A radial spectrum analyser after audioMotion-analyzer's radial mode: rounded bars whose
    // opacity follows their level, peak-hold markers that fall back under gravity, a slow spin,
    // and the cover's own colours.
    val minDimension = min(size.width, size.height)
    val center = Offset(size.width / 2f, size.height / 2f)
    val cover = minDimension * 0.28f
    val outer = minDimension * 0.495f
    val overall = levels.overall
    drawCircle(
        Brush.radialGradient(
            listOf(palette.deep.copy(alpha = 0.22f + 0.2f * overall), palette.deep.copy(alpha = 0f)),
            center = center,
            radius = outer * (0.94f + 0.06f * levels.kick),
        ),
        radius = outer,
        center = center,
    )
    // One soft ripple per beat, kept faint so the bars stay the subject.
    for (age in levels.ripples) {
        if (age >= RIPPLE_SECONDS) continue
        val t = age / RIPPLE_SECONDS
        drawCircle(
            palette.light.copy(alpha = 0.22f * (1f - t)),
            radius = cover + (outer - cover) * (1f - (1f - t) * (1f - t)),
            center = center,
            style = Stroke(width = 1.5f.dp.toPx()),
        )
    }
    val bars = 72
    val inner = cover + 8.dp.toPx()
    val reach = outer - inner - 4.dp.toPx()
    // About 10% of each slot is space, as in audioMotion's default bar spacing.
    val stroke = (TAU * inner / bars) * 0.62f
    val spin = time * TAU / 60f * 2f // two turns a minute
    for (i in 0 until bars) {
        val angle = TAU / 4f + i.toFloat() / bars * TAU + spin
        val height = if (i <= bars / 2) i / (bars / 2f) else (bars - i) / (bars / 2f)
        val level = sampleBands(levels.bands, height)
        val peak = sampleBands(levels.held, height)
        val direction = Offset(cos(angle), sin(angle))
        val length = stroke + (reach - stroke) * level
        drawLine(
            lerp(palette.deep, palette.light, level).copy(alpha = 0.35f + 0.65f * level),
            center + direction * inner,
            center + direction * (inner + length),
            stroke,
            StrokeCap.Round,
        )
        // Peak-hold cap: a small dot just past the bar's recent high.
        if (peak > level + 0.02f) {
            val at = inner + stroke + (reach - stroke) * peak + stroke * 0.6f
            drawCircle(palette.light.copy(alpha = 0.85f), stroke * 0.42f, center + direction * at)
        }
    }
    drawCircle(
        palette.light.copy(alpha = 0.55f),
        radius = cover + (2.5f + 2.5f * levels.kick).dp.toPx(),
        center = center,
        style = Stroke(width = 1.5f.dp.toPx()),
    )
}

/** Level at [position] (0 = bass … 1 = treble), interpolated between neighbouring bands. */
internal fun sampleBands(values: FloatArray, position: Float): Float {
    val at = position.coerceIn(0f, 1f) * (values.size - 1)
    val low = at.toInt().coerceIn(0, values.size - 1)
    val high = (low + 1).coerceAtMost(values.size - 1)
    val mix = at - low
    return (values[low] * (1 - mix) + values[high] * mix).coerceIn(0f, 1f)
}

/**
 * "Strings" after Plexamp's visualiser: one sine string per slice of the spectrum, bass to
 * treble, each plucked by its band's level and tapered to rest at both ends. Drawn behind the
 * cover across the whole stage, in the cover's colours.
 */
internal fun DrawScope.drawStrings(time: Float, levels: LiquidLevels, palette: LiquidPalette, path: Path) {
    val strings = 14
    val samples = 72
    val centre = size.height / 2f
    val amplitudeMax = size.height * 0.36f
    val width = size.width
    for (s in 0 until strings) {
        val position = s / (strings - 1f)
        val level = sampleBands(levels.bands, position)
        // Bass strings swing wide and slow; treble strings are finer and quicker.
        val cycles = 1f + position * 3.5f
        val speed = 0.6f + position * 2.2f
        val amplitude = amplitudeMax * (0.04f + 0.96f * level) * (1f - 0.35f * position)
        val phase = time * speed + s * 0.9f
        path.reset()
        for (i in 0..samples) {
            val u = i.toFloat() / samples
            val taper = sin(u * TAU / 2f)
            val y = centre + amplitude * taper * sin(TAU * cycles * u + phase)
            if (i == 0) path.moveTo(0f, y) else path.lineTo(u * width, y)
        }
        drawPath(
            path,
            lerp(palette.deep, palette.light, position).copy(alpha = 0.28f + 0.6f * level),
            style = Stroke(width = (2.4f - 1.2f * position).dp.toPx(), cap = StrokeCap.Round),
        )
    }
}
