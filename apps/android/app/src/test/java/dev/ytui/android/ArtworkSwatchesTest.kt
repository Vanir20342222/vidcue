package dev.ytui.android

import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ArtworkSwatchesTest {
    private fun argb(r: Int, g: Int, b: Int) = (0xff shl 24) or (r shl 16) or (g shl 8) or b

    private fun hue(colour: Color): Float {
        val hsv = FloatArray(3)
        val r = colour.red
        val g = colour.green
        val b = colour.blue
        val max = maxOf(r, g, b)
        val delta = max - minOf(r, g, b)
        hsv[0] =
            when {
                delta == 0f -> 0f
                max == r -> 60f * (((g - b) / delta) % 6f)
                max == g -> 60f * (((b - r) / delta) + 2f)
                else -> 60f * (((r - g) / delta) + 4f)
            }
        return (hsv[0] + 360f) % 360f
    }

    @Test
    fun twoColourCoverKeepsBothHuesInsteadOfTheirMuddyAverage() {
        // Orange leaves over green trees, like the Einaudi cover: their average would be brown.
        val pixels = IntArray(1000) { if (it < 550) argb(70, 130, 40) else argb(230, 120, 30) }
        val swatches = artworkSwatches(pixels)
        assertEquals(2, swatches.size)
        val hues = swatches.map(::hue)
        assertTrue("green found: $hues", hues.any { it in 80f..120f })
        assertTrue("orange found: $hues", hues.any { it in 20f..40f })
        // Lifted to read on the dark player.
        assertTrue(swatches.all { maxOf(it.red, it.green, it.blue) >= 0.8f })
    }

    @Test
    fun greyCoversGiveTheirOwnLightGreyAndBordersDoNotWin() {
        val grey = artworkSwatches(IntArray(400) { argb(120, 120, 125) })
        assertEquals(1, grey.size)
        assertTrue(grey[0].red > 0.75f && kotlin.math.abs(grey[0].red - grey[0].green) < 0.05f)
        // A black frame around a blue picture: the picture's blue is the colour.
        val framed = artworkSwatches(IntArray(400) { if (it < 250) argb(5, 5, 5) else argb(40, 80, 210) })
        assertEquals(1, framed.size)
        assertTrue(hue(framed[0]) in 210f..235f)
        assertTrue(artworkSwatches(IntArray(0)).isEmpty())
    }
}
