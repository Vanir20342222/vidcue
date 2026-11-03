package dev.ytui.android

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import org.json.JSONObject
import java.util.Locale

/** Extractor languages are separate from the one audio stream already in Media3. */
@Composable
internal fun AudioLanguageOptions(player: JSONObject, choose: (String) -> Unit) {
    if (player.optBoolean("offline") || player.optBoolean("preview")) return
    val tracks = player.optJSONArray("audio_tracks") ?: return
    if (tracks.length() == 0) return
    val requested = player.optString("audio_language", "")
    val selected = player.optString("selected_audio_language", "")
    Text("Audio language", Modifier.padding(8.dp), style = MaterialTheme.typography.titleSmall)
    TextButton(onClick = { choose("") }, modifier = Modifier.testTag("audio-language-original")) {
        Text((if (requested.isEmpty()) "✓ " else "") + "Original (automatic)")
    }
    for (i in 0 until tracks.length()) {
        val track = tracks.optJSONObject(i) ?: continue
        val language = track.optString("language")
        val name = Locale.forLanguageTag(language).displayName.ifBlank { language }
        TextButton(onClick = { choose(language) }, modifier = Modifier.testTag("audio-language-$language")) {
            Text((if (language == selected) "✓ " else "") + track.optString("label", language).replaceFirst(language, name))
        }
    }
}
