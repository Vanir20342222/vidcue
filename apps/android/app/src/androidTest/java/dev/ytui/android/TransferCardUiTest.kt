package dev.ytui.android

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertDoesNotExist
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

/** Synthetic rows only: no downloads, network or account are involved. */
class TransferCardUiTest {
    @get:Rule val compose = createComposeRule()

    private fun transfer(
        state: String,
        progress: Double = 0.0,
        bytes: Long = 0,
        total: Long = 0,
        id: String = "fixture-transfer",
    ): JSONObject =
        JSONObject()
            .put("id", id)
            .put("title", "Vercel Finally Caught Up")
            .put("state", state)
            .put("progress", progress)
            .put("bytes", bytes)
            .put("total", total)
            .put("audio", false)
            .put(
                "media",
                JSONObject()
                    .put("url", "https://example.test/watch?v=fixture")
                    .put("title", "Vercel Finally Caught Up")
                    .put("creator", "Fixture Channel")
                    .put("kind", "video"),
            )

    @Test
    fun runningTransferShowsStatusSizeAndPercentOnTheirOwnLines() {
        var toggled = 0
        var removed = 0
        compose.setContent {
            MaterialTheme {
                TransferCard(
                    transfer("Downloading", 0.01, 12L shl 20, 1200L shl 20),
                    thumbnails = false,
                    onToggle = { toggled++ },
                    onRemove = { removed++ },
                )
            }
        }
        compose.onNodeWithText("Vercel Finally Caught Up").assertIsDisplayed()
        compose.onNodeWithText("Fixture Channel").assertIsDisplayed()
        compose.onNodeWithText("Downloading").assertIsDisplayed()
        compose.onNodeWithText("1%").assertIsDisplayed()
        compose.onNodeWithText("12 MB of 1.2 GB").assertIsDisplayed()
        // No second snapshot has arrived, so the card says so instead of showing a made-up speed.
        compose.onNodeWithText("Measuring speed…").assertIsDisplayed()
        compose.onNodeWithText("Pause").assertIsDisplayed()
        compose.onNodeWithContentDescription("Pause download").performClick()
        compose.onNodeWithContentDescription("Remove Vercel Finally Caught Up").performClick()
        compose.runOnIdle {
            assertEquals(1, toggled)
            assertEquals(1, removed)
        }
    }

    @Test
    fun pausedTransferOffersResumeAndNoSpeed() {
        compose.setContent {
            MaterialTheme {
                TransferCard(
                    transfer("Paused", 0.4, 480L shl 20, 1200L shl 20),
                    thumbnails = false,
                    onToggle = {},
                    onRemove = {},
                )
            }
        }
        compose.onNodeWithText("Paused").assertIsDisplayed()
        compose.onNodeWithText("40%").assertIsDisplayed()
        compose.onNodeWithText("480 MB of 1.2 GB").assertIsDisplayed()
        compose.onNodeWithText("Resume").assertIsDisplayed()
        compose.onNodeWithContentDescription("Resume download").assertIsDisplayed()
        compose.onNodeWithText("Measuring speed…").assertDoesNotExist()
    }

    @Test
    fun failedTransferOffersRetryAndKeepsWhatWasDownloaded() {
        compose.setContent {
            MaterialTheme {
                TransferCard(
                    transfer("Failed", 0.25, 300L shl 20, 1200L shl 20),
                    thumbnails = false,
                    onToggle = {},
                    onRemove = {},
                )
            }
        }
        compose.onNodeWithText("Failed").assertIsDisplayed()
        compose.onNodeWithText("25%").assertIsDisplayed()
        compose.onNodeWithText("Retry").assertIsDisplayed()
        compose.onNodeWithContentDescription("Retry download").assertIsDisplayed()
    }

    @Test
    fun queuedTransferWithNothingDownloadedShowsNoNumbers() {
        compose.setContent {
            MaterialTheme {
                TransferCard(
                    transfer("Queued"),
                    thumbnails = false,
                    onToggle = {},
                    onRemove = {},
                )
            }
        }
        compose.onNodeWithText("Queued").assertIsDisplayed()
        compose.onNodeWithText("0%").assertDoesNotExist()
        compose.onNodeWithText("Pause").assertIsDisplayed()
    }

    @Test
    fun preparationRowsKeepTheirStatusTextAndRestartAction() {
        compose.setContent {
            // Reduced motion keeps the indeterminate bar still, so the test clock can settle.
            CompositionLocalProvider(LocalMotion provides Motion(reduced = true)) {
                MaterialTheme {
                    TransferCard(
                        JSONObject()
                            .put("id", "job:fixture")
                            .put("title", "Playlist preparation")
                            .put("state", "Preparing 3 / 12 · 1 failed")
                            .put("progress", 0),
                        thumbnails = false,
                        onToggle = {},
                        onRemove = {},
                    )
                }
            }
        }
        compose.onNodeWithText("Playlist preparation").assertIsDisplayed()
        // The word "failed" inside a progress message must not turn the row into a failure.
        compose.onNodeWithText("Preparing 3 / 12 · 1 failed").assertIsDisplayed()
        compose.onNodeWithContentDescription("Restart preparation").assertIsDisplayed()
        compose.onNodeWithText("0%").assertDoesNotExist()
    }
}
