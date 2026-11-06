package dev.ytui.android

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import org.json.JSONObject

/** Extractor languages are separate from the one audio stream already in Media3. */
@Composable
internal fun AudioLanguageOptions(player: JSONObject, choose: (String) -> Unit) {
    if (player.optBoolean("offline") || player.optBoolean("preview")) return
    val tracks = player.optJSONArray("audio_tracks") ?: return
    if (tracks.length() == 0) return
    val requested = player.optString("audio_language", "")
    val selected = player.optString("selected_audio_language", "")
    SectionHeader("Audio language")
    ChoiceRow(
        "Original",
        requested.isEmpty(),
        Modifier.testTag("audio-language-original"),
        detail = "Chosen automatically",
    ) {
        choose("")
    }
    for (i in 0 until tracks.length()) {
        val track = tracks.optJSONObject(i) ?: continue
        val language = track.optString("language")
        val original = track.optString("label").contains("Original")
        ChoiceRow(
            Captions.displayName(language),
            requested.isNotEmpty() && language == selected,
            Modifier.testTag("audio-language-$language"),
            detail = if (original) "Original audio" else "",
        ) {
            choose(language)
        }
    }
}
