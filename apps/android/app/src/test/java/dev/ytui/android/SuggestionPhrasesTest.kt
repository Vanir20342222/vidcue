package dev.ytui.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SuggestionPhrasesTest {
    @Test
    fun artistListsOfferEachName() {
        val phrases = suggestionPhrases("UB40, Toto, Huey Lewis & The News, Fleetwood Mac")
        assertTrue("Fleetwood Mac" in phrases)
        assertTrue("UB40" in phrases)
        assertFalse(phrases.any { it.startsWith("UB40, Toto") && it.length > 40 })
    }

    @Test
    fun videoTitlesDropTheirTagsAndSplitArtistFromSong() {
        val phrases = suggestionPhrases("Foreigner - I Want To Know What Love Is (Official Music Video)")
        assertTrue("Foreigner" in phrases)
        assertTrue("I Want To Know What Love Is" in phrases)
        assertFalse(phrases.any { "Official" in it })
    }

    @Test
    fun countsAreNotSuggested() {
        assertEquals(listOf("Lady Gaga"), suggestionPhrases("Lady Gaga • 794M plays"))
        assertTrue("Simon & Garfunkel" in suggestionPhrases("Simon & Garfunkel"))
    }
}
