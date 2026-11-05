package dev.ytui.android

import androidx.compose.ui.geometry.Offset
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class DownloadFlightTest {
    private val card = MediaCard(JSONObject().put("title", "Synthetic"))

    @Before fun clean() = DownloadFlight.reset()

    @After fun tidy() = DownloadFlight.reset()

    @Test
    fun visibleNavigationTabCatchesTheCoverWithoutAShortcut() {
        DownloadFlight.target = Offset(900f, 2000f)
        DownloadFlight.launch(card, Offset(100f, 100f))
        val flight = DownloadFlight.flight!!
        assertFalse(flight.toShortcut)
        assertNull(DownloadFlight.shortcut)
        DownloadFlight.land(flight.id)
        assertEquals(1L, DownloadFlight.landed)
        assertEquals(0L, DownloadFlight.shortcutLanded)
    }

    @Test
    fun coveredNavigationSendsTheCoverToATemporaryShortcut() {
        DownloadFlight.target = Offset(900f, 2000f)
        DownloadFlight.navigationHidden = true
        DownloadFlight.launch(card, null)
        val flight = DownloadFlight.flight!!
        assertTrue(flight.toShortcut)
        assertEquals(flight.id, DownloadFlight.shortcut)
        DownloadFlight.land(flight.id)
        assertNull(DownloadFlight.flight)
        assertEquals(1L, DownloadFlight.shortcutLanded)
        assertEquals(0L, DownloadFlight.landed)
        // Still tappable after landing until its timer dismisses it.
        assertEquals(flight.id, DownloadFlight.shortcut)
        DownloadFlight.dismissShortcut(flight.id)
        assertNull(DownloadFlight.shortcut)
    }

    @Test
    fun missingNavigationTabAlsoUsesTheShortcut() {
        assertTrue(DownloadFlight.needsShortcut(navigationHidden = false, target = null))
        assertFalse(DownloadFlight.needsShortcut(navigationHidden = false, target = Offset.Zero))
    }

    @Test
    fun anOlderTimerCannotHideANewerDownloadsShortcut() {
        DownloadFlight.navigationHidden = true
        DownloadFlight.launch(card, null)
        val first = DownloadFlight.flight!!.id
        DownloadFlight.launch(card, null)
        val second = DownloadFlight.flight!!.id
        DownloadFlight.land(first) // superseded flight: ignored
        assertEquals(0L, DownloadFlight.shortcutLanded)
        DownloadFlight.dismissShortcut(first)
        assertEquals(second, DownloadFlight.shortcut)
    }
}
