package dev.ytui.android

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ChannelLinkTest {
    @Test
    fun onlyWellFormedChannelIdsOpenAChannel() {
        assertTrue(isChannelId("UCsyntheticchannel0000ab"))
        assertTrue(isChannelId("UC_synthetic-channel0000"))
        assertFalse(isChannelId(""))
        assertFalse(isChannelId("UCshort"))
        assertFalse(isChannelId("VLsyntheticchannel0000ab"))
        assertFalse(isChannelId("UCsyntheticchannel0000a/"))
        assertFalse(isChannelId("UCsyntheticchannel0000é"))
    }

    @Test
    fun cardsReadTheirPublishingChannelAndDefaultToBlank() {
        val linked =
            MediaCard(JSONObject().put("title", "Video").put("channel_id", "UCsyntheticchannel0000ab"))
        assertEquals("UCsyntheticchannel0000ab", linked.channelId)
        assertEquals("", MediaCard(JSONObject().put("title", "Video")).channelId)
        assertEquals("", MediaCard(JSONObject().put("channel_id", "null")).channelId)
    }
}
