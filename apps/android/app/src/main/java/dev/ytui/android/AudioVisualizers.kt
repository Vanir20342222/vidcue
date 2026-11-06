package dev.ytui.android

import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.unit.dp
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.sin

private const val TAU = 6.2831855f

/**
 * Visualiser colours. With cover swatches (see [artworkSwatches]) the visualiser blends between
 * the cover's own dominant colours; without them it uses the theme accent and a lighter tint.
 */
internal class LiquidPalette(swatches: List<Color>) {
    constructor(accent: Color, ink: Color) : this(listOf(accent, lerp(accent, ink, 0.3f)))

    private val colours = swatches.ifEmpty { listOf(Color.White) }
    val deep: Color = colours.first()
    val light: Color = lerp(deep, Color.White, 0.4f)

    /** The palette around a loop: 0 and 1 are the dominant colour, blending through the rest. */
    fun at(position: Float): Color {
        if (colours.size == 1) return deep
        val scaled = ((position % 1f + 1f) % 1f) * colours.size
        val index = scaled.toInt().coerceIn(0, colours.size - 1)
        return lerp(colours[index], colours[(index + 1) % colours.size], scaled - index)
    }
}

/** Cover swatches when known, otherwise the theme accent. */
internal fun artPalette(swatches: List<Color>?, accent: Color, ink: Color): LiquidPalette =
    if (swatches.isNullOrEmpty()) LiquidPalette(accent, ink) else LiquidPalette(swatches)

/** A small shake on strong beats, in a fresh direction for each beat (golden-angle steps). */
internal fun beatShake(levels: LiquidLevels, amount: Float): Offset {
    val strength = levels.kick * levels.kick * amount
    val direction = levels.beats * 2.3999631f
    return Offset(cos(direction) * strength, sin(direction) * strength)
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
    swatches: List<Color>? = null,
) {
    val path = remember { Path() }
    val palette = remember(swatches, accent, ink) { artPalette(swatches, accent, ink) }
    Canvas(modifier) {
        clock()
        drawStrings(levels, palette, path)
    }
}

/**
 * Swells the cover a little with the bass. [clock] is read so the layer updates every frame of
 * the visualiser; the levels themselves are plain arrays.
 */
internal fun Modifier.beat(levels: LiquidLevels, clock: () -> Float): Modifier = graphicsLayer {
    clock()
    // The cover thumps on every beat, a little harder for strong ones, and breathes with energy.
    val swell = 1f + 0.08f * levels.kick + 0.02f * levels.energy
    scaleX = swell
    scaleY = swell
    val shake = beatShake(levels, 4.dp.toPx())
    translationX = shake.x
    translationY = shake.y
}

/** Pulse Orbit: a radial spectrum around the circular cover. */
@Composable
internal fun LiquidOrb(
    levels: LiquidLevels,
    clock: () -> Float,
    accent: Color,
    ink: Color,
    modifier: Modifier = Modifier,
    swatches: List<Color>? = null,
) {
    val palette = remember(swatches, accent, ink) { artPalette(swatches, accent, ink) }
    Canvas(modifier) {
        clock()
        drawLiquidOrb(levels, palette)
    }
}

internal fun DrawScope.drawLiquidOrb(levels: LiquidLevels, palette: LiquidPalette) {
    // A radial analyser after audioMotion-analyzer's radial mode, driven hard by the beat as
    // music-channel visualisers are: the ring jumps out and flashes on each beat, the stage shakes
    // a little on strong ones, and the ring spins faster when the music is loud.
    val minDimension = min(size.width, size.height)
    val center = Offset(size.width / 2f, size.height / 2f) + beatShake(levels, 4.dp.toPx())
    val kick = levels.kick
    val energy = levels.energy
    val (inner, outer, stroke, reach) = orbitGeometry(minDimension, density, kick, energy)
    if (outer <= 0f) return
    drawCircle(
        Brush.radialGradient(
            listOf(
                palette.at(0f).copy(alpha = (0.18f + 0.3f * energy + 0.35f * kick).coerceAtMost(0.8f)),
                palette.at(0.5f).copy(alpha = 0.08f + 0.12f * kick),
                Color.Transparent,
            ),
            center = center,
            radius = outer * (0.9f + 0.12f * kick),
        ),
        radius = outer * 1.02f,
        center = center,
    )
    for ((index, age) in levels.ripples.withIndex()) {
        if (age >= RIPPLE_SECONDS) continue
        val t = age / RIPPLE_SECONDS
        drawCircle(
            palette.at(index / RIPPLE_COUNT.toFloat()).copy(alpha = 0.45f * (1f - t) * (1f - t)),
            radius = inner + (outer * 1.05f - inner) * (1f - (1f - t) * (1f - t)),
            center = center,
            style = Stroke(width = (3f - 2f * t).dp.toPx()),
        )
    }
    val bars = ORBIT_BARS
    for (i in 0 until bars) {
        val angle = TAU / 4f + i.toFloat() / bars * TAU + levels.spin
        val height = if (i <= bars / 2) i / (bars / 2f) else (bars - i) / (bars / 2f)
        val level = sampleBands(levels.bands, height)
        val peak = sampleBands(levels.held, height)
        val direction = Offset(cos(angle), sin(angle))
        // Beats push every bar outwards on top of its own level.
        val reached = (level * (0.85f + 0.3f * kick) + 0.12f * kick).coerceAtMost(1f)
        val length = stroke + (reach - stroke) * reached
        val colour = lerp(palette.at(i.toFloat() / bars), Color.White, 0.15f * level + 0.35f * kick)
        drawLine(
            colour.copy(alpha = (0.4f + 0.6f * reached).coerceAtMost(1f)),
            center + direction * inner,
            center + direction * (inner + length),
            stroke,
            StrokeCap.Round,
        )
        if (peak > level + 0.02f) {
            val at = inner + stroke + (reach - stroke) * peak + stroke * 0.6f
            drawCircle(lerp(colour, Color.White, 0.5f).copy(alpha = 0.85f), stroke * 0.42f, center + direction * at)
        }
    }
    drawCircle(
        lerp(palette.at(0.5f), Color.White, 0.3f * kick).copy(alpha = 0.55f + 0.4f * kick),
        radius = (inner - 4.dp.toPx()).coerceAtLeast(0f),
        center = center,
        style = Stroke(width = (1.5f + 2.5f * kick).dp.toPx()),
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
internal fun DrawScope.drawStrings(levels: LiquidLevels, palette: LiquidPalette, path: Path) {
    val strings = 14
    val samples = 72
    val centre = size.height / 2f
    val width = size.width
    val kick = levels.kick
    for (s in 0 until strings) {
        val position = s / (strings - 1f)
        val level = sampleBands(levels.bands, position)
        // Bass strings swing wide and slow; treble strings are finer and quicker. A beat plucks
        // every string at once, hardest in the bass. Their motion speeds up with the music.
        val cycles = 1f + position * 3.5f
        val core = (2.6f - 1.2f * position).dp.toPx()
        val glow = core * 4f
        val amplitude = stringAmplitude(size.height, glow, position, level, kick)
        val phase = levels.spin * (1.2f + position * 3f) + s * 0.9f
        path.reset()
        for (i in 0..samples) {
            val u = i.toFloat() / samples
            val taper = sin(u * TAU / 2f)
            val y = centre + amplitude * taper * sin(TAU * cycles * u + phase)
            if (i == 0) path.moveTo(0f, y) else path.lineTo(u * width, y)
        }
        val colour = lerp(palette.at(position), Color.White, 0.3f * kick)
        // A soft glow under each string, then the string itself.
        drawPath(path, colour.copy(alpha = 0.12f + 0.2f * level), style = Stroke(width = glow, cap = StrokeCap.Round, join = StrokeJoin.Round))
        drawPath(
            path,
            colour.copy(alpha = (0.4f + 0.55f * level + 0.2f * kick).coerceAtMost(1f)),
            style = Stroke(width = core, cap = StrokeCap.Round, join = StrokeJoin.Round),
        )
    }
}
