package dev.ytui.android

import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class LyricSweepTest {
    @Test
    fun emptyLineAndUnstartedSweepAreUnsungAndFinishedSweepIsSung() {
        assertEquals(0f, lyricSungFraction(0, 0, 1f), 0f)
        for (index in 0 until 10) {
            assertEquals(0f, lyricSungFraction(index, 10, 0f), 0f)
            assertEquals(1f, lyricSungFraction(index, 10, 1f), 0f)
        }
    }

    @Test
    fun sweepFollowsReadingOrderSoALaterCharacterNeverLeadsAnEarlierOne() {
        for (sweep in listOf(0.1f, 0.35f, 0.5f, 0.82f)) {
            val fractions = (0 until 60).map { lyricSungFraction(it, 60, sweep) }
            assertTrue(
                "Sung fraction must not increase along the line at sweep $sweep",
                fractions.zipWithNext().all { (before, after) -> before >= after },
            )
        }
    }

    @Test
    fun secondVisualLineStaysUnsungWhileTheFirstIsStillFilling() {
        // A 60-character lyric that wraps near the middle: at 30% the start is sung
        // and the wrapped tail is not, rather than both lines filling together.
        assertEquals(1f, lyricSungFraction(5, 60, 0.3f), 0f)
        assertEquals(0f, lyricSungFraction(55, 60, 0.3f), 0f)
    }

    @Test
    fun characterAtTheSweepPointIsHalfwayThroughTheSoftEdge() {
        // Centre of index 49 out of 100 is 0.495.
        assertEquals(0.5f, lyricSungFraction(49, 100, 0.495f), 0.001f)
    }

    @Test
    fun surrogatePairsStayInOneColourSpan() {
        val sung = Color(0xffffe9c0)
        val unsung = Color(0x99ffe9c0)
        val line = karaokeLine("a😀b", 0.5f, sung, unsung)

        assertEquals("a😀b", line.text)
        val spans = line.spanStyles
        assertEquals(3, spans.size)
        assertEquals(1, spans[1].start)
        assertEquals(3, spans[1].end)
        assertEquals(3, spans[2].start)
        assertEquals(4, spans[2].end)
    }

    @Test
    fun finishedAndUnstartedLinesUseTheSungAndUnsungColours() {
        val sung = Color(0xffffe9c0)
        val unsung = Color(0x99ffe9c0)

        karaokeLine("Picking up", 1f, sung, unsung).spanStyles.forEach {
            assertSameColour(sung, it.item.color)
        }
        karaokeLine("Picking up", 0f, sung, unsung).spanStyles.forEach {
            assertSameColour(unsung, it.item.color)
        }
    }

    @Test
    fun instrumentalGapAfterALineDoesNotSlowTheFill() {
        // "Hold me now" is three syllables sung in ~1 s, then eight seconds of music.
        val text = "Hold me now"
        val sungMs = lyricSyllables(text) * LYRIC_MS_PER_SYLLABLE
        assertEquals(3, lyricSyllables(text))
        assertEquals(1f, lyricSweep(10_000.0 + sungMs, 10_000, 19_000, text), 0f)
        assertEquals(0.5f, lyricSweep(10_000.0 + sungMs / 2, 10_000, 19_000, text), 0.001f)
    }

    @Test
    fun aNextLineSoonerThanTheEstimateStillFinishesTheFillOnTime() {
        val text = "a long line with very many syllables in it, sung quickly"
        assertTrue(lyricSyllables(text) * LYRIC_MS_PER_SYLLABLE > 2000)
        assertEquals(0.5f, lyricSweep(1_000.0, 0, 2_000, text), 0.001f)
        assertEquals(1f, lyricSweep(2_000.0, 0, 2_000, text), 0f)
    }

    @Test
    fun sweepStaysWithinRangeBeforeTheLineAndForTheLastLine() {
        assertEquals(0f, lyricSweep(500.0, 1_000, null, "Oh"), 0f)
        assertEquals(1f, lyricSweep(1_000.0 + LYRIC_LAST_LINE_MS, 1_000, null, "a ".repeat(40)), 0f)
        // Shortest lines still take a moment so the fill is visible.
        assertEquals(0.5f, lyricSweep(LYRIC_MIN_SUNG_MS / 2.0, 0, 10_000, "Oh"), 0.001f)
    }

    @Test
    fun syllablesCountVowelGroupsAndCharactersInScriptsWithoutVowels() {
        assertEquals(1, lyricSyllables("rhythm"))
        assertEquals(4, lyricSyllables("Café au lait"))
        assertEquals(3, lyricSyllables("Привет мир"))
        assertEquals(4, lyricSyllables("君の名は"))
        assertEquals(0, lyricSyllables("♪ ... !"))
    }

    /** Colour lerp can differ from its end point by float rounding, so compare channels. */
    private fun assertSameColour(expected: Color, actual: Color) {
        assertEquals(expected.red, actual.red, 0.001f)
        assertEquals(expected.green, actual.green, 0.001f)
        assertEquals(expected.blue, actual.blue, 0.001f)
        assertEquals(expected.alpha, actual.alpha, 0.001f)
    }
}
