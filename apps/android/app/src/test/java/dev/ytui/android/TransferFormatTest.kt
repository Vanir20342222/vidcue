package dev.ytui.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class TransferFormatTest {
    @Test
    fun statesMapToTheToneThatDecidesTheirColourAndStats() {
        assertEquals(TransferTone.Active, transferTone("Downloading"))
        assertEquals(TransferTone.Paused, transferTone("Paused"))
        assertEquals(TransferTone.Failed, transferTone("Failed"))
        assertEquals(TransferTone.Failed, transferTone("Audio failed"))
        for (waiting in listOf("Queued", "Pending", "Restarting", "Waiting for audio", "Removing"))
            assertEquals(waiting, TransferTone.Waiting, transferTone(waiting))
    }

    @Test
    fun sizesReadAsDownloadedOfTotalAndNeverClaimAnUnknownTotal() {
        assertEquals("12 MB of 1.2 GB", downloadedLabel(12L shl 20, (1.2 * (1L shl 30)).toLong()))
        assertEquals("0 KB of 300 MB", downloadedLabel(0, 300L shl 20))
        assertEquals("12 MB", downloadedLabel(12L shl 20, 0))
        // A finished or over-reported transfer reads as a plain size, not "5 MB of 4 MB".
        assertEquals("5 MB", downloadedLabel(5L shl 20, 4L shl 20))
        assertNull(downloadedLabel(0, 0))
    }

    @Test
    fun timeLeftNeedsASpeedAndARemainingSize() {
        assertNull(secondsLeft(10, 100, 0.0))
        assertNull(secondsLeft(100, 100, 5.0))
        assertNull(secondsLeft(120, 100, 5.0))
        assertEquals(18L, secondsLeft(10, 100, 5.0))
        assertEquals("1 s left", timeLeftLabel(secondsLeft(99, 100, 50.0)!!))
        assertEquals("45 s left", timeLeftLabel(45))
        assertEquals("3 min left", timeLeftLabel(200))
        assertEquals("1 h 20 min left", timeLeftLabel(4800))
    }

    @Test
    fun speedAndPercentLabelsStayShort() {
        assertNull(speedLabel(0.0))
        assertEquals("2 MB/s", speedLabel(2.0 * (1L shl 20)))
        assertEquals("1%", percentLabel(0.014f))
        assertEquals("100%", percentLabel(1.4f))
        assertEquals("0%", percentLabel(-1f))
    }

    @Test
    fun offlineDetailsNameTheKindQualityAndSize() {
        assertEquals("Video · 1080p · 1.2 GB", offlineDetails(false, "1080", (1.2 * (1L shl 30)).toLong()))
        assertEquals("Video · Best quality", offlineDetails(false, "best", 0))
        assertEquals("Video", offlineDetails(false, "", 0))
        // Quality never applies to an audio-only save.
        assertEquals("Audio · 54 MB", offlineDetails(true, "1080", 54L shl 20))
    }

    @Test
    fun summaryCountsEachKindInAFixedOrder() {
        assertEquals("", transferSummary(emptyList()))
        assertEquals(
            "2 downloading · 1 waiting · 1 paused · 1 failed",
            transferSummary(
                listOf(
                    TransferTone.Failed,
                    TransferTone.Paused,
                    TransferTone.Active,
                    TransferTone.Waiting,
                    TransferTone.Active,
                )
            ),
        )
        assertEquals("1 paused", transferSummary(listOf(TransferTone.Paused)))
    }
}
