package dev.ytui.android

import org.junit.Assert.*
import org.junit.Test

class SharedLinksTest {
    private fun kind(url: String, radio: Boolean = false) = SharedLinks.classify(url, radio).kind

    @Test
    fun youtubeVideosStayVideosOnEveryTab() {
        for (radio in listOf(false, true)) {
            for (url in
                listOf(
                    "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                    "https://youtube.com/watch?v=dQw4w9WgXcQ&t=42",
                    "https://m.youtube.com/watch?v=dQw4w9WgXcQ",
                    "https://youtu.be/dQw4w9WgXcQ?si=share",
                    "https://www.youtube.com/shorts/abcdefghijk",
                    "https://www.youtube.com/live/abcdefghijk",
                    "https://www.youtube-nocookie.com/embed/abcdefghijk",
                    "HTTPS://WWW.YOUTUBE.COM/watch?v=dQw4w9WgXcQ",
                )) {
                val link = SharedLinks.classify(url, radio)
                assertEquals(url, "video", link.kind)
                assertFalse("$url must not become a live station", link.live)
                assertEquals(url, link.url)
                assertEquals("", link.id)
            }
        }
    }

    @Test
    fun youtubeMusicIsAudioAndPlaylistsAreCollections() {
        assertEquals("song", kind("https://music.youtube.com/watch?v=fixture"))
        assertEquals("song", kind("https://music.youtube.com/watch?v=fixture&list=PLfixture"))
        val playlist = SharedLinks.classify("https://music.youtube.com/playlist?list=PLfixture", true)
        assertEquals("playlist", playlist.kind)
        assertEquals("PLfixture", playlist.id)
        assertFalse(playlist.live)
        assertEquals("playlist", kind("https://www.youtube.com/playlist/?list=PLfixture"))
        // A watch page with a video plays the video; without one it names only its playlist.
        assertEquals("video", kind("https://www.youtube.com/watch?v=fixture&list=PLfixture"))
        val listOnly = SharedLinks.classify("https://www.youtube.com/watch?list=PLfixture", false)
        assertEquals("playlist", listOnly.kind)
        assertEquals("PLfixture", listOnly.id)
        assertEquals("video", kind("https://youtube.com/playlist?list="))
        assertEquals("video", kind("https://youtu.be/fixture?list=PLfixture"))
    }

    @Test
    fun otherSitesAreStationsOnlyOnTheRadioTab() {
        val station = SharedLinks.classify("https://radio.invalid/stream.mp3", radioTab = true)
        assertEquals("station", station.kind)
        assertTrue(station.live)
        val video = SharedLinks.classify("https://media.invalid/playlist?list=PLfixture", false)
        assertEquals("video", video.kind)
        assertFalse(video.live)
        assertEquals("", video.id)
        // Look-alike hosts are not YouTube.
        assertEquals("station", kind("https://notyoutube.com/watch?v=x", radio = true))
        assertEquals("station", kind("https://youtube.com.invalid/watch?v=x", radio = true))
    }

    @Test
    fun rejectsAnythingButPlainHttpUrls() {
        for (raw in
            listOf(
                "ftp://youtube.com/watch?v=x",
                "youtube.com/watch?v=x",
                "https://user:secret@youtube.com/watch?v=x",
                "https:///watch",
                "https://youtu be/x",
            )) {
            assertTrue(raw, runCatching { SharedLinks.classify(raw, false) }.isFailure)
        }
    }

    @Test
    fun extractsUrlsFromSharedText() {
        assertEquals(
            listOf("https://youtu.be/dQw4w9WgXcQ"),
            SharedLinks.urls("Never Gonna Give You Up https://youtu.be/dQw4w9WgXcQ"),
        )
        assertEquals(
            listOf("https://www.youtube.com/watch?v=abc", "http://example.invalid/a_(b)"),
            SharedLinks.urls(
                "Watch this (https://www.youtube.com/watch?v=abc). See http://example.invalid/a_(b)!"
            ),
        )
        assertEquals(emptyList<String>(), SharedLinks.urls(null))
        assertEquals(emptyList<String>(), SharedLinks.urls(""))
        assertEquals(emptyList<String>(), SharedLinks.urls("lofi hip hop radio"))
        assertEquals(emptyList<String>(), SharedLinks.urls("https:// nothing"))
    }

    @Test
    fun picksTheFirstYoutubeMediaLinkFromSharedText() {
        assertEquals(
            "https://youtu.be/dQw4w9WgXcQ?si=x",
            SharedLinks.mediaLink("Title\nhttps://youtu.be/dQw4w9WgXcQ?si=x"),
        )
        assertEquals(
            "https://www.youtube.com/shorts/abcdefghijk",
            SharedLinks.mediaLink(
                "via https://blog.invalid/post and https://www.youtube.com/shorts/abcdefghijk."
            ),
        )
        assertEquals(
            "https://music.youtube.com/playlist?list=PLfixture",
            SharedLinks.mediaLink("Album: https://music.youtube.com/playlist?list=PLfixture"),
        )
        // Text, other sites and YouTube pages that are not media stay a search.
        assertNull(SharedLinks.mediaLink(null))
        assertNull(SharedLinks.mediaLink("lofi hip hop radio"))
        assertNull(SharedLinks.mediaLink("https://radio.invalid/stream.mp3"))
        assertNull(SharedLinks.mediaLink("https://www.youtube.com/@channel"))
        assertNull(SharedLinks.mediaLink("https://www.youtube.com/"))
        assertNull(SharedLinks.mediaLink("https://youtu.be/"))
        assertNull(SharedLinks.mediaLink("https://www.youtube.com/watch?feature=share"))
        assertNull(SharedLinks.mediaLink("https://www.youtube.com/playlist?list="))
    }

    @Test
    fun everyPlayableLinkIsAcceptedByClassify() {
        for (url in
            listOf(
                "https://www.youtube.com/watch?v=a",
                "http://youtube.com/watch?v=a",
                "https://youtu.be/a",
                "https://m.youtube.com/shorts/a",
                "https://www.youtube.com/live/a",
                "https://www.youtube.com/embed/a",
                "https://music.youtube.com/watch?v=a",
                "https://www.youtube.com/playlist?list=PL",
                "https://www.youtube.com/watch?list=PL",
            )) {
            assertTrue(url, SharedLinks.playable(url))
            assertNotEquals(url, "station", SharedLinks.classify(url, radioTab = true).kind)
        }
    }
}
