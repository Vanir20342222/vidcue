package dev.ytui.android

import dev.ytui.android.DescriptionTarget.Channel
import dev.ytui.android.DescriptionTarget.Hashtag
import dev.ytui.android.DescriptionTarget.Link
import dev.ytui.android.DescriptionTarget.Time
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class DescriptionLinksTest {
    private fun shown(text: String) =
        DescriptionLinks.spans(text).map { text.substring(it.start, it.end) to it.target }

    @Test
    fun youtubeChannelUrlsOpenAsChannels() {
        val text =
            "Live Channel: https://www.youtube.com/@muyelive\n" +
                "Clips: https://youtube.com/@MuYeClips/videos\n" +
                "Old: https://www.youtube.com/channel/UCsyntheticchannel0000ab\n" +
                "Video: https://youtu.be/aaaaaaaaaaa"
        assertEquals(
            listOf(
                "https://www.youtube.com/@muyelive" to Channel("@muyelive"),
                "https://youtube.com/@MuYeClips/videos" to Channel("@MuYeClips"),
                "https://www.youtube.com/channel/UCsyntheticchannel0000ab" to
                    Channel("UCsyntheticchannel0000ab"),
                "https://youtu.be/aaaaaaaaaaa" to Link("https://youtu.be/aaaaaaaaaaa"),
            ),
            shown(text),
        )
    }

    @Test
    fun bareWwwHandlesHashtagsAndTimestamps() {
        val text =
            "Shop at www.shopmuye.com. Follow @themuye, mail me@example.com.\n" +
                "#BeamNG #Earth_Mod #1\n0:00 Intro\n1:02:03 Ending (at 10:30am)"
        assertEquals(
            listOf(
                "www.shopmuye.com" to Link("https://www.shopmuye.com"),
                "@themuye" to Channel("@themuye"),
                "#BeamNG" to Hashtag("#BeamNG"),
                "#Earth_Mod" to Hashtag("#Earth_Mod"),
                "0:00" to Time(0),
                "1:02:03" to Time(3723),
            ),
            shown(text),
        )
    }

    @Test
    fun partsOfUrlsAndNumbersAreNotSplitOff() {
        val text =
            "https://example.com/page#section https://x.com/@who 3.14:15 1:75 9:99:00 v1.2:30"
        assertEquals(
            listOf("https://example.com/page#section", "https://x.com/@who"),
            shown(text).map { it.first },
        )
        assertEquals(Link("https://x.com/@who"), shown(text)[1].second)
    }

    @Test
    fun handlesFollowYoutubeRules() {
        assertTrue(DescriptionLinks.isHandle("@muyelive"))
        assertFalse(DescriptionLinks.isHandle("@ab"))
        assertFalse(DescriptionLinks.isHandle("muyelive"))
        assertFalse(DescriptionLinks.isHandle("@bad/path"))
        // Music links keep their own pages.
        assertEquals(
            Link("https://music.youtube.com/@artist"),
            DescriptionLinks.linkTarget("https://music.youtube.com/@artist"),
        )
    }
}
