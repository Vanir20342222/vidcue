package dev.ytui.android

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ChannelAvatarTest {
    @Test
    fun providerChannelPictureIsReadFromMediaJsonAndDefaultsToBlank() {
        val withPicture =
            MediaCard(JSONObject().put("channel_thumbnail", "https://yt3.ggpht.com/synthetic=s88"))
        assertEquals("https://yt3.ggpht.com/synthetic=s88", withPicture.channelThumbnail)
        assertEquals("", MediaCard(JSONObject().put("creator", "Synthetic")).channelThumbnail)
    }

    @Test
    fun channelPictureCardIsArtworkOnlyAndNeverSharesAVideoCacheKey() {
        val picture = "https://yt3.ggpht.com/synthetic=s88"
        val avatar = channelPictureCard(picture)
        assertEquals(picture, avatar.artwork.optString("thumbnail"))

        val video =
            MediaCard(
                JSONObject()
                    .put("url", "https://www.youtube.com/watch?v=aaaaaaaaaaa")
                    .put("thumbnail", picture)
            )
        assertNotEquals(avatar.key, video.key)
    }
}
