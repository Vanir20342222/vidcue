package dev.ytui.android

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.LinkAnnotation
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class WatchDetailsTest {
    private val details =
        JSONObject()
            .put("published_text", "3 days ago")
            .put("like_count", 1234)
            .put("subscribers_text", "1.23M subscribers")

    @Test
    fun detailsFillOnlyWhatTheStreamLacks() {
        val media = JSONObject().put("url", "https://media.invalid/v").put("view_count", 10)
        val merged = withWatchDetails(media, details)!!
        assertEquals("3 days ago", merged.optString("published_text"))
        assertEquals(1234L, merged.optLong("like_count"))
        assertEquals("1.23M subscribers", subscriberLabel(merged))
        // The original is not modified.
        assertFalse(media.has("published_text"))
    }

    @Test
    fun streamValuesWin() {
        val media = JSONObject().put("upload_date", "20261101").put("like_count", 5)
        val merged = withWatchDetails(media, details)!!
        assertFalse(merged.has("published_text"))
        assertEquals(5L, merged.optLong("like_count"))
    }

    @Test
    fun noDetailsLeavesTheMediaAlone() {
        val media = JSONObject().put("title", "Clip")
        assertSame(media, withWatchDetails(media, null))
        assertSame(media, withWatchDetails(media, JSONObject()))
        assertNull(withWatchDetails(null, details))
        assertEquals("", subscriberLabel(media))
    }

    @Test
    fun descriptionLinksCoverOnlyTheUrls() {
        val text =
            "Merch: https://www.shopmuye.com\n\nMods Used\nBeamNG.Earth:\n" +
                "https://github.com/TecnaGamer/BeamNG.Earth/releases. (see https://example.com/a_(b))"
        val opened = mutableListOf<String>()
        val linked = linkedDescription(text, Color.Blue) { opened += it }
        // The visible text is unchanged.
        assertEquals(text, linked.text)
        val links = linked.getLinkAnnotations(0, linked.length)
        assertEquals(
            listOf(
                "https://www.shopmuye.com",
                "https://github.com/TecnaGamer/BeamNG.Earth/releases",
                "https://example.com/a_(b)",
            ),
            links.map { linked.text.substring(it.start, it.end) },
        )
        links.forEach { (it.item as LinkAnnotation.Clickable).linkInteractionListener!!.onClick(it.item) }
        assertEquals(links.map { (it.item as LinkAnnotation.Clickable).tag }, opened)
    }

    @Test
    fun descriptionWithoutLinksIsPlainText() {
        val linked = linkedDescription("No links, just www.example.com text", Color.Blue) {}
        assertEquals(0, linked.getLinkAnnotations(0, linked.length).size)
        assertEquals(emptyList<Pair<Int, String>>(), SharedLinks.urlSpans(""))
    }
}
