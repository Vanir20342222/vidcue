package dev.ytui.android

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class StationDetailsTest {
    private fun station(title: String, country: String, tags: String) =
        MediaCard(JSONObject().put("title", title).put("creator", country).put("description", tags).put("kind", "station"))

    @Test
    fun genreThenCountry() {
        assertEquals("Lounge · France", stationDetails(station("Jazz FM", "France", "jazz,lounge")))
        assertEquals("Smooth Jazz · Germany", stationDetails(station("Radio Eins", "Germany", "smooth jazz, news")))
    }

    @Test
    fun missingPartsAreLeftOut() {
        assertEquals("Germany", stationDetails(station("Jazz Radio", "Germany", "jazz")))
        assertEquals("Ambient", stationDetails(station("Drift", "", "ambient")))
        assertEquals("", stationDetails(station("Drift", "", "")))
    }

    @Test
    fun collectionLengthsReadLikeMusicApps() {
        assertEquals("1 min", totalLength(10))
        assertEquals("48 min", totalLength(48 * 60 + 10))
        assertEquals("1 hr", totalLength(3600))
        assertEquals("1 hr 12 min", totalLength(72 * 60))
    }
}
