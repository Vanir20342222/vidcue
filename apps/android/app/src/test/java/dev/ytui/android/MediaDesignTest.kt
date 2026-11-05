package dev.ytui.android

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.graphics.luminance
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class MediaDesignTest {
    @Test
    fun nativeTransportInkHasIconContrastEvenOnWhiteVideo() {
        assertTrue(
            mediaContrast(
                MediaTokens.videoInk,
                MediaTokens.videoChromeDisc.compositeOver(Color.White),
            ) >= 3f
        )
        for (accent in
            listOf(
                Color(0xff6ddbc1),
                Color(0xff9d0006),
                Color(0xff006f62),
                Color(0xff8839ef),
            )) {
            assertTrue(
                "Dark accents in light/imported palettes also need a readable video timeline",
                mediaContrast(accent, mediaOnAccent(accent)) >= 4.5f,
            )
        }
    }

    @Test
    fun secondaryInkStaysReadableOnRaisedImportedSurfaces() {
        for (surface in (0..255 step 8).map { Color(it, it, it) }) {
            for (accent in listOf(Color(0xff6ddbc1), Color(0xff9d0006), Color(0xff8839ef), Color.Gray)) {
                assertTrue(mediaContrast(mediaBrand(accent, surface), surface) >= 4.5f)
            }
            for (ink in listOf(Color.Black, Color.White, Color.Gray, Color.Cyan, Color.Magenta)) {
                assertTrue(mediaContrast(mediaReadableText(ink, surface), surface) >= 4.5f)
            }
        }
        assertTrue(mediaContrast(MediaTokens.neutralText, MediaTokens.neutralBackground) >= 4.5f)
        assertTrue(mediaContrast(MediaTokens.neutralText, MediaTokens.neutralSurface) >= 4.5f)
        assertEquals(MediaTokens.neutralBackground.red, MediaTokens.neutralBackground.green)
        assertEquals(MediaTokens.neutralBackground.green, MediaTokens.neutralBackground.blue)
    }

    @Test
    fun progressSurvivesStopUnknownDurationAndInvalidTelemetry() {
        for ((position, duration) in
            listOf(
                0f to 0f,
                8f to 0f,
                Float.NaN to 8f,
                8f to Float.NaN,
                Float.POSITIVE_INFINITY to 8f,
                8f to Float.POSITIVE_INFINITY,
                8f to -1f,
            )) assertEquals(0f, mediaProgress(position, duration), 0f)
        assertEquals(0.5f, mediaProgress(4f, 8f), 0f)
        assertEquals(1f, mediaProgress(12f, 8f), 0f)
        assertEquals(0f, mediaProgress(-1f, 8f), 0f)
    }

    @Test
    fun accentControlsKeepReadableTextAcrossImportedColours() {
        val colors =
            (0..255).map { Color(it, it, it) } +
                listOf(0, 64, 128, 192, 255).flatMap { r ->
                    listOf(0, 64, 128, 192, 255).flatMap { g ->
                        listOf(0, 64, 128, 192, 255).map { b -> Color(r, g, b) }
                    }
                }
        colors.forEach { color ->
            val a = color.luminance()
            val b = mediaOnAccent(color).luminance()
            val contrast = (maxOf(a, b) + 0.05f) / (minOf(a, b) + 0.05f)
            assertTrue("Accent $color needs WCAG AA text contrast; was $contrast", contrast >= 4.5f)
        }
    }

    @Test
    fun videoTimebarKeylineStaysDarkForMidToneAccents() {
        // Nothing red, vidcue-light teal, catppuccin-latte mauve and flexoki-light blue all
        // previously got a white keyline because white ink suits text on them.
        for (hex in listOf(0xffd71921, 0xff006f62, 0xff8839ef, 0xff205ea6, 0xff80cbc4, 0xfffabd2f)) {
            val accent = Color(hex)
            assertEquals("keyline for ${hex.toString(16)}", Color.Black, mediaTimeBarBacking(accent))
            assertTrue(mediaContrast(accent, mediaTimeBarBacking(accent)) >= 3f)
        }
        // Very dark imported accents still get a light keyline so the played part reads.
        for (hex in listOf(0xff000000, 0xff1a1a40, 0xff3a0a0a)) {
            val accent = Color(hex)
            assertEquals(Color.White, mediaTimeBarBacking(accent))
            assertTrue(mediaContrast(accent, mediaTimeBarBacking(accent)) >= 3f)
        }
    }
}
