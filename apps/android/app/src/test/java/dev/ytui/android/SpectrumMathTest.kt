package dev.ytui.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SpectrumMathTest {
    private fun bands(value: Int) = IntArray(SPECTRUM_BANDS) { value }

    @Test
    fun meterChannelDecodesEachPackedWordAsUnitRange() {
        val packed = 0xFFFFL or (0x8000L shl 16) or (0x0000L shl 32)
        assertEquals(1f, meterChannel(packed, 0), 0f)
        assertEquals(0.5f, meterChannel(packed, 16), 0.001f)
        assertEquals(0f, meterChannel(packed, 32), 0f)
    }

    @Test
    fun attacksAreQuickAndReleasesAreSlowAndFrameRateIndependent() {
        val smoother = SpectrumSmoother()
        smoother.step(bands(65535), 0L, 0.016f)
        val afterAttack = smoother.bands[0]
        assertTrue("attack should move at least a third in one frame: $afterAttack", afterAttack > 0.33f)

        // Reaching the target then dropping: a single decay frame must only lose a little.
        repeat(60) { smoother.step(bands(65535), 0L, 0.016f) }
        val top = smoother.bands[0]
        assertTrue(top > 0.99f)
        smoother.step(bands(0), 0L, 0.016f)
        val fall = top - smoother.bands[0]
        assertTrue("release should be gradual, fell $fall", fall in 0.01f..0.2f)
    }

    @Test
    fun zeroStepsLeaveTheShapeFrozenAndLongStallsAreClamped() {
        val frozen = SpectrumSmoother()
        frozen.step(bands(40000), 0L, 0.016f)
        val snapshot = frozen.bands.copyOf()
        frozen.step(bands(0), 0L, 0f)
        assertTrue(snapshot.contentEquals(frozen.bands))

        val stalled = SpectrumSmoother()
        val normal = SpectrumSmoother()
        stalled.step(bands(65535), 0L, 5f)
        normal.step(bands(65535), 0L, 0.1f)
        assertEquals(normal.bands[3], stalled.bands[3], 0.0001f)
    }

    @Test
    fun peakCapsHoldThenFallWithoutEverExceedingTheLevel() {
        val smoother = SpectrumSmoother()
        smoother.step(bands(65535), 0L, 0.016f)
        repeat(20) { smoother.step(bands(65535), 0L, 0.016f) }
        val cap = smoother.caps[7]
        repeat(10) { smoother.step(bands(0), 0L, 0.016f) } // still within the hold
        assertEquals(cap, smoother.caps[7], 0.0001f)
        repeat(120) { smoother.step(bands(0), 0L, 0.016f) }
        assertTrue(smoother.caps[7] < cap)
        for (band in 0 until SPECTRUM_BANDS) {
            assertTrue(smoother.caps[band] >= smoother.bands[band] - 0.0001f)
            assertTrue(smoother.bands[band] in 0f..1f)
        }
    }

    @Test
    fun malformedBandArraysAndStereoWordsStayBounded() {
        val smoother = SpectrumSmoother()
        smoother.step(IntArray(3) { 999_999 }, -1L, 0.016f)
        assertTrue(smoother.bands.all { it in 0f..1f })
        assertTrue(smoother.left in 0f..1f && smoother.right in 0f..1f && smoother.peak in 0f..1f)
    }

    @Test
    fun hsvRoundTripsPrimaryColours() {
        val red = rgbToHsv(1f, 0f, 0f)
        assertEquals(0f, red[0], 0.01f)
        assertEquals(1f, red[1], 0.001f)
        assertEquals(1f, red[2], 0.001f)
        val green = hsvToRgb(120f, 1f, 1f)
        assertEquals(0f, green[0], 0.001f)
        assertEquals(1f, green[1], 0.001f)
        assertEquals(0f, green[2], 0.001f)
    }

    @Test
    fun neutralAccentsKeepTheirToneInsteadOfInventingHues() {
        assertNull(spectrumBandRgb(0.4f, 0.4f, 0.4f, 0))
        assertNull(spectrumBandRgb(0.1f, 0.105f, 0.1f, 12))
    }

    @Test
    fun bandColoursRotateAcrossTheSpectrumFromTheAccentsHue() {
        // #B88FF5, the purple accent in the screenshots (hue about 264 degrees).
        val accent = floatArrayOf(0.722f, 0.561f, 0.961f)
        val lowRgb = spectrumBandRgb(accent[0], accent[1], accent[2], 0)!!
        val highRgb = spectrumBandRgb(accent[0], accent[1], accent[2], SPECTRUM_BANDS - 1)!!
        val low = rgbToHsv(lowRgb[0], lowRgb[1], lowRgb[2])
        val high = rgbToHsv(highRgb[0], highRgb[1], highRgb[2])
        val accentHue = rgbToHsv(accent[0], accent[1], accent[2])[0]
        assertEquals(accentHue + SPECTRUM_HUE_LOW, low[0], 2f)
        assertEquals(accentHue + SPECTRUM_HUE_HIGH, high[0], 2f)
        assertTrue(low[1] >= 0.69f && high[2] >= 0.84f)
        assertTrue("bands must differ in hue", Math.abs(low[0] - high[0]) > 60f)
    }
}
