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
private const val PEAK_DECAY = 2.5f
/** Peak markers hold this long (audioMotion-analyzer's default is 500 ms) before falling. */
internal const val PEAK_HOLD = 0.5f
/** Full-scale units per second squared: a marker at the top falls in about 0.75 s. */
private const val PEAK_GRAVITY = 3.5f
private const val PEAK_FLOOR = 0.06f
private const val KICK_DECAY = 0.2f
private const val BASS_BANDS = 6
private const val WARM_UP = 0.3f
private const val MIN_BEAT_GAP = 0.18f
private const val FLUX_FLOOR = 0.06f
private const val FLUX_SPREAD = 1.6f
private const val FLUX_WINDOW = 0.6f
internal const val RIPPLE_COUNT = 4
internal const val RIPPLE_SECONDS = 1.2f
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

    /**
     * Smoothed energy per band, 0..1, for spectrum bars. Each band is scaled by its own recent
     * peak (auto-gain), so quiet treble bands move as visibly as the bass instead of staying flat.
     */
    val bands = FloatArray(SPECTRUM_BANDS)

    /**
     * Peak-hold marker per band, as on a hardware analyser: it jumps to a new high, holds for
     * [PEAK_HOLD] seconds, then falls under constant gravity.
     */
    val held = FloatArray(SPECTRUM_BANDS)
    private val heldAge = FloatArray(SPECTRUM_BANDS)
    private val heldSpeed = FloatArray(SPECTRUM_BANDS)

    /** Recent peak per band, decaying over a few seconds; the auto-gain reference. */
    private val peaks = FloatArray(SPECTRUM_BANDS)

    /** Previous auto-gained level per band, for spectral flux. */
    private val previous = FloatArray(SPECTRUM_BANDS)
    private var warmUp = 0f
    private var fluxAverage = 0f
    private var fluxDeviation = 0f
    private var sinceBeat = 1f

    /**
     * Strength of the latest beat (0.35..1) right after it, easing back to 0. Beats are onsets in
     * spectral flux (the sum of rising band levels, weighted to the bass) above an adaptive
     * threshold, so quiet piano and loud EDM both find their own beat.
     */
    var kick = 0f
        private set

    /** Counts detected beats; a view emits particles when it changes. */
    var beats = 0
        private set

    /** Strength of the most recent beat, 0.35..1. */
    var beatStrength = 0f
        private set

    /** Overall loudness after auto-gain, 0..1, quick to rise and slow to fall. */
    var energy = 0f
        private set

    /** Accumulated ring rotation in radians: idles slowly and speeds up with energy and beats. */
    var spin = 0f
        private set

    /** Seconds since each recent beat ripple started; values past [RIPPLE_SECONDS] are idle. */
    val ripples = FloatArray(RIPPLE_COUNT) { RIPPLE_SECONDS }
    private var nextRipple = 0

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
        val peakFade = exp(-step / PEAK_DECAY)
        var flux = 0f
        var weights = 0f
        var loudness = 0f
        for (band in 0 until SPECTRUM_BANDS) {
            val raw = (spectrum.getOrElse(band) { 0 } / 65535f).coerceIn(0f, 1f)
            peaks[band] = maxOf(raw, peaks[band] * peakFade)
            val scaled = (raw / maxOf(peaks[band], PEAK_FLOOR)).coerceIn(0f, 1f)
            val weight = if (band < BASS_BANDS) 1f else 0.35f
            flux += maxOf(0f, scaled - previous[band]) * weight
            weights += weight
            previous[band] = scaled
            // A gentle curve keeps quiet passages low and lets peaks reach out.
            bands[band] = followLevel(bands[band], scaled * scaled, step, BAR_ATTACK, BAR_RELEASE)
            loudness += bands[band]
            if (bands[band] >= held[band]) {
                held[band] = bands[band]
                heldAge[band] = 0f
                heldSpeed[band] = 0f
            } else {
                heldAge[band] += step
                if (heldAge[band] > PEAK_HOLD) {
                    heldSpeed[band] += PEAK_GRAVITY * step
                    held[band] = maxOf(bands[band], held[band] - heldSpeed[band] * step)
                }
            }
        }
        flux /= weights
        energy = followLevel(energy, loudness / SPECTRUM_BANDS, step, 0.05f, 0.45f)
        // Beats: a flux onset clearly above the song's own recent flux, at most ~5 a second.
        warmUp += step
        sinceBeat += step
        kick *= exp(-step / KICK_DECAY)
        for (i in ripples.indices) ripples[i] = (ripples[i] + step).coerceAtMost(RIPPLE_SECONDS)
        val threshold = fluxAverage + FLUX_SPREAD * fluxDeviation + FLUX_FLOOR
        if (warmUp > WARM_UP && flux > threshold && sinceBeat > MIN_BEAT_GAP) {
            sinceBeat = 0f
            beatStrength = ((flux - fluxAverage) / (fluxDeviation * 2f + 0.05f)).coerceIn(0.35f, 1f)
            kick = maxOf(kick, beatStrength)
            beats++
            ripples[nextRipple] = 0f
            nextRipple = (nextRipple + 1) % RIPPLE_COUNT
        }
        val adapt = 1f - exp(-step / FLUX_WINDOW)
        fluxAverage += (flux - fluxAverage) * adapt
        fluxDeviation += (kotlin.math.abs(flux - fluxAverage) - fluxDeviation) * adapt
        spin += step * (0.25f + 1.6f * energy + 2.5f * kick)
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

/**
 * Sparks thrown outwards from the orbit ring: a burst on every beat (bigger for stronger beats)
 * and a light stream that follows the music's energy. Plain arrays, stepped with the levels.
 */
internal class OrbParticles(private val capacity: Int = 120, seed: Int = 7) {
    private val random = kotlin.random.Random(seed)
    /** Distance travelled from the ring outwards, 0..1; values >= 1 are free slots. */
    val progress = FloatArray(capacity) { 1f }
    val angle = FloatArray(capacity)
    val speed = FloatArray(capacity)
    val size = FloatArray(capacity)
    /** Position on the cover's palette, 0..1. */
    val tone = FloatArray(capacity)
    private var seenBeats = 0
    private var stream = 0f

    fun step(levels: LiquidLevels, elapsedSeconds: Float) {
        val step = elapsedSeconds.coerceIn(0f, 0.1f)
        if (step == 0f) return
        for (i in 0 until capacity) if (progress[i] < 1f) progress[i] += speed[i] * step
        if (levels.beats != seenBeats) {
            seenBeats = levels.beats
            repeat((10 + 26 * levels.beatStrength).toInt()) { emit(0.9f + 1.3f * levels.beatStrength) }
        }
        stream += step * 16f * levels.energy
        while (stream >= 1f) {
            stream -= 1f
            emit(0.35f + 0.5f * levels.energy)
        }
    }

    private fun emit(velocity: Float) {
        val slot = (0 until capacity).firstOrNull { progress[it] >= 1f } ?: return
        progress[slot] = 0f
        angle[slot] = random.nextFloat() * 6.2831855f
        speed[slot] = velocity * (0.6f + 0.8f * random.nextFloat())
        size[slot] = 0.6f + random.nextFloat()
        tone[slot] = random.nextFloat()
    }
}
