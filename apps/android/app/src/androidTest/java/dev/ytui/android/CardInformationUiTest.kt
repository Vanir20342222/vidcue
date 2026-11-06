package dev.ytui.android

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import org.json.JSONObject
import org.junit.Rule
import org.junit.Test

/** Provider labels and optional details must react immediately to the card switches. */
class CardInformationUiTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun switchesShowAndHideAvailableDetails() {
        val media = JSONObject()
            .put("views_text", "12K views")
            .put("published_text", "3 days ago")
            .put("like_count", 2)
            .put("description", "A fixture description")
            .put("section", "Recommended")
        val keys = listOf("show_views", "show_upload_date", "show_likes", "show_description", "show_section")
        fun snapshot(enabled: Boolean) = JSONObject().apply { keys.forEach { put(it, enabled) } }
        val settings = mutableStateOf(snapshot(true))
        compose.setContent {
            MaterialTheme { Column { CardInformation(media, settings.value) } }
        }
        compose.onNodeWithText("12K views · 3 days ago · 2 likes").assertIsDisplayed()
        compose.onNodeWithText("A fixture description").assertIsDisplayed()
        compose.onNodeWithText("Recommended").assertIsDisplayed()

        for ((key, expected) in listOf(
            "show_views" to "12K views",
            "show_upload_date" to "3 days ago",
            "show_likes" to "2 likes",
            "show_description" to "A fixture description",
            "show_section" to "Recommended",
        )) {
            compose.runOnIdle { settings.value = snapshot(false).put(key, true) }
            compose.onNodeWithText(expected).assertIsDisplayed()
        }
        compose.runOnIdle { settings.value = snapshot(false) }
        compose.onNodeWithTag("video-information").assertDoesNotExist()
        compose.onNodeWithTag("card-description").assertDoesNotExist()
        compose.onNodeWithTag("card-section").assertDoesNotExist()
    }

    @Test
    fun unavailableDetailsDoNotShowNullOrInventCounts() {
        compose.setContent {
            MaterialTheme {
                Column {
                    CardInformation(
                        JSONObject().put("description", JSONObject.NULL).put("section", JSONObject.NULL),
                        JSONObject().put("show_likes", true).put("show_description", true).put("show_section", true),
                    )
                }
            }
        }
        compose.onNodeWithTag("video-information").assertDoesNotExist()
        compose.onNodeWithTag("card-description").assertDoesNotExist()
        compose.onNodeWithTag("card-section").assertDoesNotExist()
    }
}
