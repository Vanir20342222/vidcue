package dev.ytui.android

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONObject
import java.util.Locale

/** One Media3 track as published by PlaybackService. */
internal data class PlayerTrack(
    val group: Int,
    val index: Int,
    val type: String,
    val language: String,
    val name: String,
    val kind: String,
    val height: Int,
    val selected: Boolean,
) {
    /** What a person reads: the readable caption label, else the language's own name. */
    val title: String
        get() =
            name.ifBlank {
                if (language.isBlank()) "Unknown language"
                else Captions.displayName(language)
            }
}

internal fun playerTracks(player: JSONObject): List<PlayerTrack> =
    player.optJSONArray("tracks")?.objects()?.map {
        PlayerTrack(
            it.optInt("group"),
            it.optInt("index"),
            it.optString("type"),
            it.optString("language"),
            it.optString("name").trim(),
            it.optString("kind"),
            it.optInt("height"),
            it.optBoolean("selected"),
        )
    } ?: emptyList()

/** "Off", or the caption track that is showing. */
internal fun captionSummary(player: JSONObject): String {
    val text = playerTracks(player).filter { it.type == "text" }
    if (text.isEmpty()) return "None"
    if (!player.optBoolean("captions_on")) return "Off"
    return text.firstOrNull { it.selected }?.title ?: "On"
}

/** The audio language that is playing, when there is a choice to make. */
internal fun audioSummary(player: JSONObject): String {
    val selected = player.optString("selected_audio_language", "")
    if (selected.isNotBlank()) return Captions.displayName(selected)
    return playerTracks(player).firstOrNull { it.type == "audio" && it.selected }?.title ?: ""
}

/** Whether the audio page has anything to choose between. */
internal fun audioChoices(player: JSONObject): Boolean {
    val extractor = player.optJSONArray("audio_tracks")?.length() ?: 0
    val media = playerTracks(player).count { it.type == "audio" }
    return !player.optBoolean("offline") && extractor > 0 || media > 1
}

@Composable
internal fun ChoiceRow(
    label: String,
    chosen: Boolean,
    modifier: Modifier = Modifier,
    detail: String = "",
    choose: () -> Unit,
) {
    SheetAction(
        text = {
            Column {
                Text(label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (detail.isNotBlank())
                    Text(
                        detail,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
            }
        },
        leadingIcon = { ChoiceMark(chosen) },
        onClick = choose,
        modifier = modifier.semantics { selected = chosen },
    )
}

/**
 * Captions: Off, the useful tracks (the uploader's, the automatic captions in the spoken
 * language, translations into the languages this device uses) and the remaining machine
 * translations folded away behind a searchable "More languages".
 */
@Composable
internal fun CaptionsOptions(player: JSONObject, command: (String, Array<Pair<String, Any?>>) -> Unit) {
    val text = playerTracks(player).filter { it.type == "text" }
    if (text.isEmpty()) {
        Text(
            "This video has no captions.",
            Modifier.padding(horizontal = 8.dp, vertical = 16.dp),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        return
    }
    val on = player.optBoolean("captions_on")
    val device = remember { Captions.deviceLanguages() }
    val (suggested, more) =
        text.partition { it.kind != Captions.TRANSLATED || it.language in device || (on && it.selected) }
    fun choose(track: PlayerTrack) =
        command("track", arrayOf("group" to track.group, "index" to track.index))
    ChoiceRow("Off", !on, Modifier.testTag("captions-off")) { command("captions_off", emptyArray()) }
    for (track in suggested) ChoiceRow(
        track.title,
        on && track.selected,
        Modifier.testTag("captions-${track.group}-${track.index}"),
        detail =
            when (track.kind) {
                Captions.UPLOADED -> "From the creator"
                Captions.GENERATED -> "Generated from the spoken audio"
                Captions.TRANSLATED -> "Machine translation"
                else -> ""
            },
    ) {
        choose(track)
    }
    if (more.isEmpty()) return
    var expanded by rememberSaveable { mutableStateOf(false) }
    var query by rememberSaveable { mutableStateOf("") }
    ValueRow(
        "More languages",
        Modifier.testTag("captions-more"),
        value = "${more.size}",
        glyph = Glyph.Captions,
        subtitle = "Machine translations",
        chevron = !expanded,
    ) {
        expanded = !expanded
    }
    AnimatedVisibility(expanded) {
        Column(Modifier.fillMaxWidth()) {
            MediaTextField(
                query,
                { query = it },
                "Find a language",
                Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 8.dp).testTag("captions-search"),
            )
            val needle = query.trim().lowercase(Locale.getDefault())
            val shown = more.filter { needle.isEmpty() || it.title.lowercase(Locale.getDefault()).contains(needle) }
            if (shown.isEmpty())
                Text(
                    "No language matches \"${query.trim()}\".",
                    Modifier.padding(horizontal = 8.dp, vertical = 12.dp),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            for (track in shown) ChoiceRow(
                track.title.removeSuffix(" (auto-translated)"),
                on && track.selected,
                Modifier.testTag("captions-${track.group}-${track.index}"),
            ) {
                choose(track)
            }
            Spacer(Modifier.height(8.dp))
        }
    }
}

/** Audio: the extractor's languages (re-resolves the stream) and any tracks inside the stream. */
@Composable
internal fun AudioTrackOptions(player: JSONObject, command: (String, Array<Pair<String, Any?>>) -> Unit) {
    AudioLanguageOptions(player) { language -> command("audio_language", arrayOf("value" to language)) }
    val audio = playerTracks(player).filter { it.type == "audio" }
    if (audio.size > 1) {
        SectionHeader("Tracks in this stream")
        for (track in audio) ChoiceRow(
            track.title,
            track.selected,
            Modifier.testTag("audio-track-${track.group}-${track.index}"),
        ) {
            command("track", arrayOf("group" to track.group, "index" to track.index))
        }
    }
    if (!audioChoices(player))
        Text(
            "This media has one audio track.",
            Modifier.padding(horizontal = 8.dp, vertical = 16.dp),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
}
