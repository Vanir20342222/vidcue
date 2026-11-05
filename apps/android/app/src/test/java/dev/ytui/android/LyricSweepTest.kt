package dev.ytui.android

import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
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

    private val words =
        listOf(LyricWord(10_000, "Hold "), LyricWord(10_400, "me "), LyricWord(10_700, "now"))

    @Test
    fun lineSyncedLyricsLightUpWholeInsteadOfGuessingAFill() {
        assertNull(lyricSweep(10_500.0, emptyList(), null, 19_000))
    }

    @Test
    fun fillReachesEachWordWhenItIsSungNotWhenTheMusicMoves() {
        // "Hold " is 5 of 11 characters; it is crossed between 10.0 s and 10.4 s.
        assertEquals(0f, lyricSweep(9_900.0, words, 11_200, 19_000)!!, 0f)
        assertEquals(2.5f / 11, lyricSweep(10_200.0, words, 11_200, 19_000)!!, 0.001f)
        assertEquals(5f / 11, lyricSweep(10_400.0, words, 11_200, 19_000)!!, 0.001f)
        // The closing tag ends the line at 11.2 s, long before the next line at 19 s.
        assertEquals(1f, lyricSweep(11_200.0, words, 11_200, 19_000)!!, 0f)
        assertEquals(1f, lyricSweep(15_000.0, words, 11_200, 19_000)!!, 0f)
    }

    @Test
    fun lastWordWithoutAClosingTimeFillsBrieflyAndNeverPassesTheNextLine() {
        assertEquals(1f, lyricSweep(10_700.0 + LYRIC_LAST_WORD_MS, words, null, 19_000)!!, 0f)
        assertEquals(1f, lyricSweep(10_800.0, words, null, 10_800)!!, 0f)
        assertTrue(lyricSweep(10_750.0, words, null, 19_000)!! < 1f)
    }

    /** Colour lerp can differ from its end point by float rounding, so compare channels. */
    private fun assertSameColour(expected: Color, actual: Color) {
        assertEquals(expected.red, actual.red, 0.001f)
        assertEquals(expected.green, actual.green, 0.001f)
        assertEquals(expected.blue, actual.blue, 0.001f)
        assertEquals(expected.alpha, actual.alpha, 0.001f)
    }
}
