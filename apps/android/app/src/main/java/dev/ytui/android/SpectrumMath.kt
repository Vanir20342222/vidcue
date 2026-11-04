package dev.ytui.android

import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.sin

/** Log-spaced band count. Keep in step with `BANDS` in `crates/ytui-mobile/src/meter.rs`. */
internal const val SPECTRUM_BANDS = 24

/** The liquid responds to four groups of bands: bass, low-mid, high-mid and treble. */
internal const val LIQUID_GROUPS = 4
private const val BANDS_PER_GROUP = SPECTRUM_BANDS / LIQUID_GROUPS

private const val TAU = 6.2831855f
private const val LEVEL_ATTACK = 0.06f
private const val LEVEL_RELEASE = 0.32f
private const val BAR_ATTACK = 0.03f
private const val BAR_RELEASE = 0.18f

/** A stalled frame must not jump the liquid, so the smoothing never sees a longer step. */
private const val MAX_STEP = 0.1f

/** Spatial cycles across the tank's width, one per group: long swells for bass, ripples for treble. */
private val SURFACE_CYCLES = floatArrayOf(1f, 2f, 3f, 5f)
private val SURFACE_SPEEDS = floatArrayOf(0.9f, 1.3f, 1.8f, 2.6f)

/** Lobes around the orbit ring, one per group. */
private val ORBIT_LOBES = floatArrayOf(2f, 3f, 4f, 6f)
private val ORBIT_SPEEDS = floatArrayOf(0.5f, 0.8f, 1.2f, 1.7f)

/** Exponential approach with a quicker attack than release, so the liquid swells fast and settles slowly. */
internal fun followLevel(current: Float, target: Float, step: Float, attack: Float, release: Float): Float {
    val tau = if (target > current) attack else release
    return current + (target - current) * (1f - exp(-step / tau))
}

/** Mean energy of one group's bands, 0..1. Missing or out-of-range values count as silence. */
internal fun liquidGroupAverage(spectrum: IntArray, group: Int): Float {
    var sum = 0f
    val first = group * BANDS_PER_GROUP
    for (band in first until first + BANDS_PER_GROUP) {
        sum += (spectrum.getOrElse(band) { 0 } / 65535f).coerceIn(0f, 1f)
    }
    return sum / BANDS_PER_GROUP
}

/**
 * Surface height at [position] (0 = left edge, 1 = right edge), in -1..1 of the amplitude.
 * Each group moves its own wave, so silence leaves the surface flat and loud bass swells it.
 */
internal fun surfaceOffset(position: Float, time: Float, groups: FloatArray): Float {
    var sum = 0f
    for (group in 0 until LIQUID_GROUPS) {
        val phase = TAU * SURFACE_CYCLES[group] * position + time * SURFACE_SPEEDS[group] + group * 1.7f
        sum += groups[group] * sin(phase)
    }
    return sum / LIQUID_GROUPS
}

/** Outline offset at [angle] radians, in -1..1, for the liquid ring around the cover. */
internal fun orbitOffset(angle: Float, time: Float, groups: FloatArray): Float {
    var sum = 0f
    for (group in 0 until LIQUID_GROUPS) {
        sum += groups[group] * cos(ORBIT_LOBES[group] * angle + time * ORBIT_SPEEDS[group] + group * 2.1f)
    }
    return sum / LIQUID_GROUPS
}

/** Frame-to-frame group energies for the liquid views. Plain state, so it never recomposes. */
internal class LiquidLevels {
    /** Smoothed energy per group, 0..1: bass, low-mid, high-mid, treble. */
    val groups = FloatArray(LIQUID_GROUPS)

    /** Smoothed energy per band, 0..1, for spectrum bars. Quicker than the groups. */
    val bands = FloatArray(SPECTRUM_BANDS)

    /** Mean group energy, 0..1. Drives the tank's height and the ring's size. */
    val overall: Float
        get() = groups.sum() / LIQUID_GROUPS

    /**
     * Advances one frame from the decoder's 0..65535 band levels. A zero [elapsedSeconds],
     * such as the first frame or a repeated timestamp, changes nothing, so a paused animation
     * keeps its last shape.
     */
    fun step(spectrum: IntArray, elapsedSeconds: Float) {
        val step = elapsedSeconds.coerceIn(0f, MAX_STEP)
        if (step == 0f) return
        for (band in 0 until SPECTRUM_BANDS) {
            bands[band] =
                followLevel(
                    bands[band],
                    (spectrum.getOrElse(band) { 0 } / 65535f).coerceIn(0f, 1f),
                    step,
                    BAR_ATTACK,
                    BAR_RELEASE,
                )
        }
        for (group in 0 until LIQUID_GROUPS) {
            groups[group] =
                followLevel(
                    groups[group],
                    liquidGroupAverage(spectrum, group),
                    step,
                    LEVEL_ATTACK,
                    LEVEL_RELEASE,
                )
        }
    }
}
