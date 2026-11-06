package dev.ytui.android

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

class AudioLanguageUiTest {
    @get:Rule val compose = createComposeRule()

    @Test fun extractorLanguagesCanBeChosenAndResetToOriginal() {
        var chosen: String? = null
        val player = JSONObject()
            .put("audio_language", "")
            .put("selected_audio_language", "en")
            .put("audio_tracks", JSONArray()
                .put(JSONObject().put("language", "en").put("label", "en · Original"))
                .put(JSONObject().put("language", "zh").put("label", "zh")))
        compose.setContent {
            MaterialTheme { Column { AudioLanguageOptions(player) { chosen = it } } }
        }
        compose.onNodeWithTag("audio-language-en").assertIsDisplayed().performClick()
        compose.runOnIdle { assertEquals("en", chosen) }
        compose.onNodeWithTag("audio-language-zh").assertIsDisplayed().performClick()
        compose.runOnIdle { assertEquals("zh", chosen) }
        compose.onNodeWithTag("audio-language-original").performClick()
        compose.runOnIdle { assertEquals("", chosen) }
    }
}
