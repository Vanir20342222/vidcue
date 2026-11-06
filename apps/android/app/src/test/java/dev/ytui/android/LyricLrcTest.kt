package dev.ytui.android

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class LyricLrcTest {
    @Test
    fun lrcLinesKeepWordTagsForTheEditor() {
        val line =
            JSONObject(
                """{"at_ms":10000,"text":"Hold me","end_ms":10900,
                   "words":[{"at_ms":10000,"text":"Hold "},{"at_ms":10400,"text":"me"}]}"""
            )
        assertEquals("[00:10.000]<00:10.000>Hold <00:10.400>me<00:10.900>", lrcLine(line))
        assertEquals("[01:02.003]Plain", lrcLine(JSONObject("""{"at_ms":62003,"text":"Plain"}""")))
    }
}
