package dev.ytui.android

import dev.ytui.android.ColorGradeMath.Grade
import dev.ytui.android.ColorGradeMath.Stats
import kotlin.math.abs
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ColorGradeMathTest {
    private fun apply(m: FloatArray, r: Int, g: Int, b: Int): DoubleArray =
        DoubleArray(3) { row ->
            m[row * 5] * r + m[row * 5 + 1] * g + m[row * 5 + 2] * b + m[row * 5 + 4].toDouble()
        }

    @Test
    fun offIsTheDefaultAndNeutral() {
        assertEquals("off", ColorGradeMath.PRESETS.first())
        assertTrue(ColorGradeMath.preset("off").neutral)
        assertTrue(ColorGradeMath.preset("sepia").neutral)
        val identity = floatArrayOf(1f, 0f, 0f, 0f, 0f, 0f, 1f, 0f, 0f, 0f, 0f, 0f, 1f, 0f, 0f, 0f, 0f, 0f, 1f, 0f)
        assertArrayEquals(identity, ColorGradeMath.matrix(Grade()), 1e-4f)
    }

    @Test
    fun presetsMoveColoursTheExpectedWay() {
        val warm = apply(ColorGradeMath.matrix(ColorGradeMath.preset("warm")), 128, 128, 128)
        assertTrue("warm raises red over blue", warm[0] > warm[2])
        val cool = apply(ColorGradeMath.matrix(ColorGradeMath.preset("cool")), 128, 128, 128)
        assertTrue(cool[2] > cool[0])
        val vivid = apply(ColorGradeMath.matrix(ColorGradeMath.preset("vivid")), 200, 100, 100)
        assertTrue("vivid widens the red lead", vivid[0] - vivid[1] > 100)
        val brighter = apply(ColorGradeMath.matrix(Grade(gamma = 8.0)), 128, 128, 128)
        assertTrue(brighter[1] > 128)
        for (name in ColorGradeMath.PRESETS) {
            val p = ColorGradeMath.preset(name)
            for (v in listOf(p.brightness, p.contrast, p.saturation, p.gamma, p.temperature))
                assertTrue(abs(v) <= 15)
        }
    }

    @Test
    fun statsIgnoreBlackFramesAndReadGreyAsNeutral() {
        assertNull(ColorGradeMath.stats(IntArray(64) { 0xff000000.toInt() }))
        assertNull(ColorGradeMath.stats(IntArray(0)))
        val grey = ColorGradeMath.stats(IntArray(64) { 0xff808080.toInt() })!!
        assertEquals(128 / 255.0, grey.mean, 1e-3)
        assertEquals(0.0, grey.saturation, 1e-3)
        assertEquals(0.0, grey.cast, 1e-3)
        val warm = ColorGradeMath.stats(IntArray(64) { 0xffc08060.toInt() })!!
        assertTrue(warm.cast > 0 && warm.saturation > 0)
    }

    @Test
    fun autoGradeIsSlightAndPointsTheRightWay() {
        val dark = ColorGradeMath.auto(Stats(mean = 0.2, low = 0.1, high = 0.35, saturation = 0.08, cast = 0.1))
        assertTrue(dark.gamma > 0 && dark.contrast > 0 && dark.saturation > 0 && dark.temperature < 0)
        val bright = ColorGradeMath.auto(Stats(mean = 0.7, low = 0.3, high = 0.95, saturation = 0.45, cast = -0.1))
        assertTrue(bright.gamma < 0 && bright.saturation < 0 && bright.brightness < 0 && bright.temperature > 0)
        val balanced = ColorGradeMath.auto(Stats(mean = 0.42, low = 0.12, high = 0.72, saturation = 0.2, cast = 0.0))
        assertTrue(balanced.rounded().neutral)
        for (g in listOf(dark, bright))
            for (v in listOf(g.brightness, g.contrast, g.saturation, g.gamma, g.temperature))
                assertTrue(abs(v) <= 8)
    }

    @Test
    fun combinedGradesStayWithinTheLimit() {
        val total = (ColorGradeMath.preset("vivid") + Grade(saturation = 100.0)).rounded()
        assertEquals(30.0, total.saturation, 0.0)
        val half = Grade(gamma = 4.0).blend(Grade(gamma = 8.0), 0.5)
        assertEquals(6.0, half.gamma, 1e-9)
    }
}
