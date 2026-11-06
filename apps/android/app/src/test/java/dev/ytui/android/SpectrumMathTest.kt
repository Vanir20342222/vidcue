package dev.ytui.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.abs

class SpectrumMathTest {
    private fun bands(value: Int) = IntArray(SPECTRUM_BANDS) { value }

    @Test
    fun groupsAverageTheirOwnRunOfBandsAndTreatMissingBandsAsSilence() {
        val spectrum = IntArray(SPECTRUM_BANDS)
        for (band in 0 until 6) spectrum[band] = 65535
        assertEquals(1f, liquidGroupAverage(spectrum, 0), 0.0001f)
        assertEquals(0f, liquidGroupAverage(spectrum, 1), 0f)
        assertEquals(0f, liquidGroupAverage(IntArray(2), 3), 0f)
        assertTrue(liquidGroupAverage(IntArray(SPECTRUM_BANDS) { 999_999 }, 2) <= 1f)
    }

    @Test
    fun silentGroupsLeaveTheTankFlatAndTheRingRound() {
        val silent = FloatArray(LIQUID_GROUPS)
        for (position in 0..10) {
            assertEquals(0f, surfaceOffset(position / 10f, 1.3f, silent), 0f)
        }
        for (step in 0..12) {
            assertEquals(0f, orbitOffset(step * 0.5f, 2.2f, silent), 0f)
        }
    }

    @Test
    fun surfaceAndRingStayWithinUnitAmplitudeEvenAtFullSwell() {
        val full = FloatArray(LIQUID_GROUPS) { 1f }
        var lowest = 0f
        var highest = 0f
        for (step in 0..200) {
            val position = step / 200f
            val offset = surfaceOffset(position, step * 0.05f, full)
            lowest = minOf(lowest, offset)
            highest = maxOf(highest, offset)
            assertTrue(abs(orbitOffset(position * TWO_PI, step * 0.05f, full)) <= 1f + 1e-5f)
        }
        assertTrue(lowest >= -1f - 1e-5f && highest <= 1f + 1e-5f)
        // With every group loud, the surface must actually move, not sit still.
        assertTrue(highest - lowest > 0.5f)
    }

    @Test
    fun pluckedStringsAndTheirGlowFitAboveAndBelowTheCentreAtEveryDensity() {
        for (height in listOf(128f, 360f, 1064f)) {
            for (density in listOf(0.75f, 1f, 2f, 3f, 4f)) {
                for (string in 0 until 14) {
                    val position = string / 13f
                    val glow = (2.6f - 1.2f * position) * density * 4f
                    for (level in listOf(0f, 0.25f, 0.5f, 0.75f, 1f)) {
                        for (kick in listOf(0f, 0.25f, 0.5f, 0.75f, 1f)) {
                            val amplitude = stringAmplitude(height, glow, position, level, kick)
                            val extent = amplitude + glow / 2f + 1f
                            assertTrue("height=$height density=$density string=$string level=$level kick=$kick", extent <= height / 2f)
                            assertTrue(amplitude >= 0f)
                        }
                    }
                }
            }
        }
    }

    @Test
    fun stringsStillRespondToTheirBandAndToBeatsBelowTheLimit() {
        val quiet = stringAmplitude(360f, 10.4f, 0f, 0.1f, 0f)
        val atRest = stringAmplitude(360f, 10.4f, 0f, 0.5f, 0f)
        val plucked = stringAmplitude(360f, 10.4f, 0f, 0.5f, 1f)
        assertTrue(quiet > 0f)
        assertTrue(quiet < atRest)
        assertTrue(atRest < plucked)
        assertTrue(plucked + 10.4f / 2f + 1f < 360f / 2f)
    }

    @Test
    fun orbitBarsPeakDotsGlowAndRipplesFitIncludingBeatShake() {
        for (sizeDp in listOf(64f, 128f, 240f, 360f, 800f)) {
            for (density in listOf(0.75f, 1f, 2f, 3f, 4f)) {
                val size = sizeDp * density
                for (kick in listOf(0f, 0.5f, 1f)) {
                    for (energy in listOf(0f, 0.5f, 1f)) {
                        val (inner, outer, stroke, reach) = orbitGeometry(size, density, kick, energy)
                        val shake = kick * kick * 4f * density
                        val limit = size / 2f - shake - 1f
                        val context = "size=$sizeDp density=$density kick=$kick energy=$energy"
                        assertTrue(context, inner >= 0f && reach >= 0f)
                        for (level in listOf(0f, 0.25f, 0.5f, 0.75f, 1f)) {
                            val reached = (level * (0.85f + 0.3f * kick) + 0.12f * kick).coerceAtMost(1f)
                            val barTip = inner + stroke + (reach - stroke) * reached
                            assertTrue("bar $context", barTip + stroke / 2f <= limit + 0.001f)
                            // A full-scale held peak remains outside the bar as its level falls.
                            val peakEdge = inner + stroke + (reach - stroke) + stroke * 0.6f + stroke * 0.42f
                            assertTrue("peak $context", peakEdge <= limit + 0.001f)
                        }
                        assertTrue("glow $context", outer * 1.02f <= limit + 0.001f)
                        for (step in 0..100) {
                            val t = step / 100f
                            val radius = inner + (outer * 1.05f - inner) * (1f - (1f - t) * (1f - t))
                            val halfStroke = (3f - 2f * t) * density / 2f
                            assertTrue("ripple $context t=$t", radius + halfStroke <= limit + 0.001f)
                        }
                        assertTrue("inner ring $context", inner - 4f * density + (1.5f + 2.5f * kick) * density / 2f <= limit + 0.001f)
                    }
                }
            }
        }
    }

    @Test
    fun orbitKeepsTheCoverGapAndRoomForSpectrumMotionAtNormalStageSizes() {
        for (sizeDp in listOf(128f, 240f, 360f, 800f)) {
            for (density in listOf(0.75f, 1f, 2f, 3f, 4f)) {
                val size = sizeDp * density
                val geometry = orbitGeometry(size, density, 1f, 1f)
                val coverEdge = size * 0.28f * 1.13f
                assertTrue(geometry.inner >= coverEdge + 8f * density - 0.001f)
                assertTrue(geometry.reach > geometry.stroke)
            }
        }
    }

    @Test
    fun surfaceFlowsSmoothlyBetweenNearbyPositionsAndFrames() {
        val groups = floatArrayOf(0.8f, 0.5f, 0.3f, 0.2f)
        val a = surfaceOffset(0.40f, 3.0f, groups)
        val b = surfaceOffset(0.41f, 3.0f, groups)
        val c = surfaceOffset(0.40f, 3.0167f, groups)
        assertTrue(abs(a - b) < 0.1f)
        assertTrue(abs(a - c) < 0.1f)
    }

    @Test
    fun attacksAreQuickAndReleasesAreSlowAndFrameRateIndependent() {
        val levels = LiquidLevels()
        levels.step(bands(65535), 0.016f)
        val afterAttack = levels.groups[0]
        assertTrue("attack should swell fast, got $afterAttack", afterAttack > 0.2f)

        repeat(90) { levels.step(bands(65535), 0.016f) }
        val top = levels.groups[0]
        assertTrue(top > 0.99f)
        levels.step(bands(0), 0.016f)
        val fall = top - levels.groups[0]
        assertTrue("release should settle slowly, fell $fall", fall in 0.01f..0.1f)
    }

    @Test
    fun zeroStepsLeaveTheLiquidFrozenAndLongStallsAreClamped() {
        val frozen = LiquidLevels()
        frozen.step(bands(40000), 0.016f)
        val snapshot = frozen.groups.copyOf()
        frozen.step(bands(0), 0f)
        assertTrue(snapshot.contentEquals(frozen.groups))

        val stalled = LiquidLevels()
        val normal = LiquidLevels()
        stalled.step(bands(65535), 5f)
        normal.step(bands(65535), 0.1f)
        assertEquals(normal.groups[2], stalled.groups[2], 0.0001f)
    }

    @Test
    fun overallEnergyIsTheMeanOfTheGroups() {
        val levels = LiquidLevels()
        levels.step(IntArray(SPECTRUM_BANDS) { if (it < 12) 65535 else 0 }, 0.5f)
        assertEquals((levels.groups[0] + levels.groups[1] + levels.groups[2] + levels.groups[3]) / 4f, levels.overall, 0.0001f)
        assertTrue(levels.overall > 0f && levels.overall < 1f)
    }

    @Test
    fun quietTrebleBandsAreAutoGainedToTheirOwnPeak() {
        val levels = LiquidLevels()
        // Treble at 10% of full scale, steady: its bar still reaches near the top.
        val quiet = IntArray(SPECTRUM_BANDS) { if (it >= 18) 6553 else 0 }
        repeat(60) { levels.step(quiet, 0.016f) }
        assertTrue(levels.bands[20] > 0.8f)
        assertEquals(0f, levels.bands[2], 0.0001f)
    }

    @Test
    fun aBassHitStartsAKickAndARippleThatFade() {
        val levels = LiquidLevels()
        repeat(40) { levels.step(bands(3000), 0.016f) }
        assertEquals(0f, levels.kick, 0.0001f)
        assertTrue(levels.ripples.all { it >= RIPPLE_SECONDS })
        levels.step(IntArray(SPECTRUM_BANDS) { if (it < 6) 60000 else 3000 }, 0.016f)
        assertEquals(1f, levels.kick, 0.0001f)
        assertEquals(1, levels.ripples.count { it < RIPPLE_SECONDS })
        repeat(120) { levels.step(bands(3000), 0.016f) }
        assertTrue(levels.kick < 0.01f)
        assertTrue(levels.ripples.all { it >= RIPPLE_SECONDS })
    }

    @Test
    fun peakMarkersHoldThenFallUnderGravity() {
        val levels = LiquidLevels()
        repeat(30) { levels.step(bands(65535), 0.016f) }
        val top = levels.held[3]
        assertTrue(top > 0.9f)
        // Silence: the bar drops at once, the marker holds for the hold time…
        repeat(20) { levels.step(bands(0), 0.016f) }
        assertEquals(top, levels.held[3], 0.0001f)
        assertTrue(levels.bands[3] < levels.held[3])
        // …then falls to the bar within about a second.
        repeat(80) { levels.step(bands(0), 0.016f) }
        assertEquals(levels.bands[3], levels.held[3], 0.0001f)
    }

    @Test
    fun quietPianoNotesAreBeatsButSteadyNoiseIsNot() {
        // Notes at ~3% of full scale every 0.6 s: far below the old fixed bass threshold.
        val piano = LiquidLevels()
        var notes = 0
        repeat(600) { frame ->
            val time = frame * 0.016f
            val since = time % 0.6f
            if (since < 0.016f && time > 0.5f) notes++
            val level = (2000 * kotlin.math.exp(-since / 0.25f)).toInt() + 150
            piano.step(IntArray(SPECTRUM_BANDS) { if (it < 10) level else level / 3 }, 0.016f)
        }
        assertTrue("beats=${piano.beats} notes=$notes", piano.beats >= notes * 0.8 && piano.beats <= notes + 2)
        val noise = LiquidLevels()
        val random = kotlin.random.Random(3)
        repeat(600) { noise.step(IntArray(SPECTRUM_BANDS) { 40000 + random.nextInt(3000) }, 0.016f) }
        assertTrue("noise beats=${noise.beats}", noise.beats <= 3)
    }

    private companion object {
        const val TWO_PI = 6.2831855f
    }
}
