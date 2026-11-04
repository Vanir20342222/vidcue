package dev.ytui.android

import org.junit.Assert.*
import org.junit.Test

class ArtworkSourceTest {
    private val id = "aBcd_12-xyz"
    private val watch = "https://www.youtube.com/watch?v=$id"
    private val cover = "https://i.ytimg.com/vi/$id/hqdefault.jpg"

    @Test
    fun recommendationsGetCoversFromTheirWatchUrls() {
        assertEquals(cover, youtubeArtworkFallback(watch))
    }

    @Test
    fun suppliedArtworkAndSizedVariantsArePreserved() {
        assertNull(youtubeArtworkFallback(watch, thumbnail = "https://cdn.test/cover"))
        assertNull(youtubeArtworkFallback(watch, hasVariants = true))
    }

    @Test
    fun musicShortAndMobileWatchUrlsGetCovers() {
        for (url in
            listOf(
                "https://music.youtube.com/watch?v=$id&list=example",
                "https://m.youtube.com/watch?list=example&v=$id",
                "https://youtu.be/$id?t=12",
                "https://www.youtube.com/shorts/$id",
            )) assertEquals(cover, youtubeArtworkFallback(url, kind = "song"))
    }

    @Test
    fun emptyAndNullThumbnailDoNotPreventFallback() {
        assertEquals(cover, youtubeArtworkFallback(watch, thumbnail = "null"))
        assertEquals(cover, youtubeArtworkFallback(watch, thumbnail = "  "))
    }

    @Test
    fun unrelatedUrlsAndMalformedIdsDoNotInventCovers() {
        for (url in
            listOf(
                "https://video.test/watch?v=$id",
                "https://youtube.com.evil.test/watch?v=$id",
                "file:///watch?v=$id",
                "https://www.youtube.com/playlist?list=$id",
                "https://www.youtube.com/watch?v=invalid",
                "not a URL",
                "",
            )) assertNull(url, youtubeArtworkFallback(url))
    }

    @Test
    fun collectionsAndRadioKeepTheirOwnArtworkPolicy() {
        for (kind in listOf("playlist", "album", "artist", "channel", "station")) {
            assertNull(kind, youtubeArtworkFallback(watch, kind = kind))
        }
    }
}
