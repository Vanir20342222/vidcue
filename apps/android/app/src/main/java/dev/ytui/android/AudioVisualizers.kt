package dev.ytui.android

import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.unit.dp
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.sin

/**
 * One colour per band, rotated from the theme accent so every palette keeps its identity.
 * Neutral accents, which have no hue to rotate, fade towards [ink] across the bands instead.
 */
internal fun spectrumTints(accent: Color, ink: Color): Array<Color> =
    Array(SPECTRUM_BANDS) { band ->
        val rgb = spectrumBandRgb(accent.red, accent.green, accent.blue, band)
        if (rgb != null) Color(rgb[0], rgb[1], rgb[2])
        else lerp(accent, ink, 0.6f * band / (SPECTRUM_BANDS - 1f))
    }

/**
 * Neon bars rising from a shared floor, each with a glow, a faint reflection and a falling
 * peak cap. [generation] is read only so that Compose redraws on every smoothing tick.
 */
internal fun DrawScope.drawNeonSpectrum(
    @Suppress("UNUSED_PARAMETER") generation: Long,
    smoother: SpectrumSmoother,
    tints: Array<Color>,
    ink: Color,
) {
    val slot = size.width / SPECTRUM_BANDS
    val barWidth = slot * 0.62f
    val inset = (slot - barWidth) / 2f
    val floor = size.height * 0.8f
    val radius = min(barWidth / 2f, 4.dp.toPx())
    val glow = 3.dp.toPx()
    val capHeight = 3.dp.toPx()
    drawLine(
        ink.copy(alpha = 0.14f),
        Offset(0f, floor),
        Offset(size.width, floor),
        strokeWidth = 1.dp.toPx(),
    )
    for (band in 0 until SPECTRUM_BANDS) {
        val level = smoother.bands[band]
        val tint = tints[band]
        val x = band * slot + inset
        val height = level * floor
        if (height > 0.5f) {
            drawRoundRect(
                tint.copy(alpha = 0.12f + 0.16f * level),
                Offset(x - glow, floor - height - glow),
                Size(barWidth + glow * 2, height + glow * 2),
                CornerRadius(radius + glow),
            )
            drawRoundRect(
                lerp(tint, ink, 0.25f * level),
                Offset(x, floor - height),
                Size(barWidth, height),
                CornerRadius(radius),
            )
            val reflection = min(height * 0.4f, size.height - floor - 2.dp.toPx())
            if (reflection > 0.5f) {
                drawRoundRect(
                    tint.copy(alpha = 0.14f),
                    Offset(x, floor + 2.dp.toPx()),
                    Size(barWidth, reflection),
                    CornerRadius(radius),
                )
            }
        }
        val cap = smoother.caps[band]
        if (cap > 0.01f) {
            drawRoundRect(
                ink.copy(alpha = 0.85f),
                Offset(x, floor - cap * floor - capHeight / 2f),
                Size(barWidth, capHeight),
                CornerRadius(capHeight / 2f),
            )
        }
    }
}

/**
 * Stereo L/R meters under the spectrum. Each row fills from the left in the band tints, with a
 * peak tick, so balance and loudness read without a second numeric display.
 */
internal fun DrawScope.drawStereoMeter(
    @Suppress("UNUSED_PARAMETER") generation: Long,
    smoother: SpectrumSmoother,
    tints: Array<Color>,
    ink: Color,
) {
    val gap = 3.dp.toPx()
    val rowHeight = (size.height - gap) / 2f
    val corner = CornerRadius(rowHeight / 2f)
    val low = tints.first()
    val high = tints.last()
    for ((row, level) in listOf(smoother.left, smoother.right).withIndex()) {
        val top = row * (rowHeight + gap)
        drawRoundRect(
            ink.copy(alpha = 0.08f),
            Offset(0f, top),
            Size(size.width, rowHeight),
            corner,
        )
        val fill = size.width * level.coerceIn(0f, 1f)
        if (fill > 0.5f) {
            drawRoundRect(
                lerp(low, high, level),
                Offset(0f, top),
                Size(fill, rowHeight),
                corner,
            )
        }
    }
    val tick = size.width * smoother.peak.coerceIn(0f, 1f)
    drawRoundRect(
        ink.copy(alpha = 0.85f),
        Offset(tick - 1.dp.toPx(), 0f),
        Size(2.dp.toPx(), size.height),
        CornerRadius(1.dp.toPx()),
    )
}

/**
 * Dots fixed around the artwork, one per band, so each stays on its frequency. A breathing
 * ring follows the bass bands, and a peak highlight rotates on its own clock.
 */
internal fun DrawScope.drawPulseOrbit(
    @Suppress("UNUSED_PARAMETER") generation: Long,
    smoother: SpectrumSmoother,
    tints: Array<Color>,
    ink: Color,
    phaseDegrees: Float,
) {
    val minDimension = min(size.width, size.height)
    val center = Offset(size.width / 2f, size.height / 2f)
    // Full-level dots and their glow stay inside the square, so the container never clips them.
    val base = minDimension * 0.31f
    val reach = minDimension * 0.12f
    val bass = (smoother.bands[0] + smoother.bands[1] + smoother.bands[2] + smoother.bands[3]) / 4f
    drawCircle(
        ink.copy(alpha = 0.08f + 0.2f * bass),
        radius = base * (1f + 0.06f * bass),
        center = center,
        style = Stroke(width = 1.5f.dp.toPx()),
    )
    for (band in 0 until SPECTRUM_BANDS) {
        val level = smoother.bands[band]
        val angle = (-PI / 2 + band * 2 * PI / SPECTRUM_BANDS).toFloat()
        val distance = base + level * reach
        val position = center + Offset(cos(angle) * distance, sin(angle) * distance)
        val dot = minDimension * 0.014f + level * minDimension * 0.018f
        drawCircle(tints[band].copy(alpha = 0.12f + 0.25f * level), dot * 2f, position)
        drawCircle(lerp(tints[band], ink, 0.25f * level), dot, position)
    }
    val peakAngle = (phaseDegrees * PI / 180.0).toFloat()
    val peakDistance = base + smoother.peak * reach * 1.15f
    drawCircle(
        ink,
        minDimension * 0.016f,
        center + Offset(cos(peakAngle) * peakDistance, sin(peakAngle) * peakDistance),
    )
}
