package dev.ytui.android

import org.junit.Assert.assertEquals
import org.junit.Test

class MusicTitlesTest {
    @Test
    fun artistPrefixAndVideoTagsGo() {
        assertEquals("Wunder", MusicTitles.clean("AYLIVA x APACHE 207 - Wunder (Official Video)", "AYLIVA & Apache 207"))
        assertEquals(
            "I Want To Know What Love Is",
            MusicTitles.clean("Foreigner - I Want To Know What Love Is (Official Music Video)", "Foreigner"),
        )
        assertEquals("Thunderstruck", MusicTitles.clean("AC/DC - Thunderstruck (Official Video)", "AC/DC"))
        assertEquals("Daylight", MusicTitles.clean("Daylight (Official Music Video)", "David Kushner"))
    }

    @Test
    fun editionsAndUnrelatedDashesStay() {
        assertEquals("The Chain (2004 Remaster)", MusicTitles.clean("The Chain (2004 Remaster)", "Fleetwood Mac"))
        assertEquals("Stay - Live", MusicTitles.clean("Stay - Live", "Rihanna"))
        assertEquals("Paparazzi", MusicTitles.clean("Paparazzi", "Lady Gaga"))
    }

    @Test
    fun aTitleThatIsOnlyATagIsKept() {
        assertEquals("(Official Video)", MusicTitles.clean("(Official Video)", "Someone"))
    }

    @Test
    fun bylinesKeepNamesOnly() {
        assertEquals("AYLIVA", cleanByline("Song • AYLIVA • 3:05"))
        assertEquals("Dua Lipa", cleanByline("Dua Lipa • 361M plays"))
        assertEquals("Fleetwood Mac · Rumours", cleanByline("Song · Fleetwood Mac · Rumours · 4:30"))
        assertEquals("Album · Fleetwood Mac", cleanByline("Album • Fleetwood Mac", keepKind = true))
    }
}
