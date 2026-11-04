package dev.ytui.android

import kotlin.math.abs
import kotlin.math.exp
import kotlin.math.max
import kotlin.math.min

/** Log-spaced band count. Keep in step with `BANDS` in `crates/ytui-mobile/src/meter.rs`. */
internal const val SPECTRUM_BANDS = 24

/** Hue offsets, in degrees from the theme accent, for the lowest and highest band. */
internal const val SPECTRUM_HUE_LOW = -48f
internal const val SPECTRUM_HUE_HIGH = 52f

private const val BAND_ATTACK = 0.03f
private const val BAND_RELEASE = 0.22f
private const val METER_ATTACK = 0.025f
private const val METER_RELEASE = 0.2f
private const val CAP_HOLD = 0.45f
private const val CAP_FALL_PER_SECOND = 0.6f

/** A stalled frame must not jump the bars, so the smoothing never sees a longer step. */
private const val MAX_STEP = 0.1f

/** Reads one 16-bit channel of the packed stereo word from `meter::pcm16`, as 0..1. */
internal fun meterChannel(packed: Long, shift: Int): Float =
    ((packed ushr shift) and 0xFFFFL).toFloat() / 65535f

/**
 * Exponential approach with a quicker attack than release, so transients read as hits and
 * decays glide. Time constants are seconds, so the result does not depend on the frame rate.
 */
internal fun followLevel(
    current: Float,
    target: Float,
    step: Float,
    attack: Float,
    release: Float,
): Float {
    val tau = if (target > current) attack else release
    return current + (target - current) * (1f - exp(-step / tau))
}

/**
 * Frame-to-frame state for the audio-reactive views. Band levels rise quickly and fall slowly.
 * Each peak cap holds briefly, then drops at a steady rate. The stereo values follow the
 * meter word that the decoder tap produces.
 */
internal class SpectrumSmoother {
    /** Smoothed band levels, 0..1, low to high frequency. */
    val bands = FloatArray(SPECTRUM_BANDS)

    /** Falling peak caps per band, 0..1. */
    val caps = FloatArray(SPECTRUM_BANDS)
    private val holds = FloatArray(SPECTRUM_BANDS)

    var left = 0f
        private set
    var right = 0f
        private set
    var peak = 0f
        private set

    /**
     * Advances one frame. [spectrum] holds 0..65535 per band and [packed] is the stereo meter
     * word. A zero [elapsedSeconds], such as the first frame or a repeated timestamp, changes
     * nothing, so a paused or restarted animation keeps its last shape.
     */
    fun step(spectrum: IntArray, packed: Long, elapsedSeconds: Float) {
        val step = elapsedSeconds.coerceIn(0f, MAX_STEP)
        if (step == 0f) return
        for (band in 0 until SPECTRUM_BANDS) {
            val target = (spectrum.getOrElse(band) { 0 } / 65535f).coerceIn(0f, 1f)
            bands[band] = followLevel(bands[band], target, step, BAND_ATTACK, BAND_RELEASE)
            if (bands[band] >= caps[band]) {
                caps[band] = bands[band]
                holds[band] = CAP_HOLD
            } else if (holds[band] > 0f) {
                holds[band] = max(0f, holds[band] - step)
            } else {
                caps[band] = max(bands[band], caps[band] - CAP_FALL_PER_SECOND * step)
            }
        }
        left = followLevel(left, meterChannel(packed, 0), step, METER_ATTACK, METER_RELEASE)
        right = followLevel(right, meterChannel(packed, 16), step, METER_ATTACK, METER_RELEASE)
        peak = followLevel(peak, meterChannel(packed, 32), step, METER_ATTACK, METER_RELEASE)
    }
}

/** RGB in 0..1 to `[hue degrees, saturation, value]`. */
internal fun rgbToHsv(red: Float, green: Float, blue: Float): FloatArray {
    val high = max(red, max(green, blue))
    val low = min(red, min(green, blue))
    val delta = high - low
    val hue =
        when {
            delta == 0f -> 0f
            high == red -> 60f * (((green - blue) / delta) % 6f)
            high == green -> 60f * ((blue - red) / delta + 2f)
            else -> 60f * ((red - green) / delta + 4f)
        }
    return floatArrayOf((hue + 360f) % 360f, if (high == 0f) 0f else delta / high, high)
}

/** `[hue degrees, saturation, value]` to RGB in 0..1. */
internal fun hsvToRgb(hue: Float, saturation: Float, value: Float): FloatArray {
    val h = ((hue % 360f) + 360f) % 360f
    val chroma = value * saturation
    val x = chroma * (1f - abs((h / 60f) % 2f - 1f))
    val m = value - chroma
    val (r, g, b) =
        when ((h / 60f).toInt()) {
            0 -> Triple(chroma, x, 0f)
            1 -> Triple(x, chroma, 0f)
            2 -> Triple(0f, chroma, x)
            3 -> Triple(0f, x, chroma)
            4 -> Triple(x, 0f, chroma)
            else -> Triple(chroma, 0f, x)
        }
    return floatArrayOf(r + m, g + m, b + m)
}

/**
 * Colour for one band, derived from the accent by rotating its hue across the spectrum. Low
 * bands stay near the accent's cool side and high bands move towards its warm side, so each
 * theme keeps its identity. Returns null for neutral accents, which have no hue to rotate.
 */
internal fun spectrumBandRgb(red: Float, green: Float, blue: Float, band: Int): FloatArray? {
    val hsv = rgbToHsv(red, green, blue)
    if (hsv[1] < 0.1f) return null
    val t = band / (SPECTRUM_BANDS - 1f)
    val hue = hsv[0] + SPECTRUM_HUE_LOW + (SPECTRUM_HUE_HIGH - SPECTRUM_HUE_LOW) * t
    return hsvToRgb(hue, max(hsv[1], 0.7f), max(hsv[2], 0.85f))
}
