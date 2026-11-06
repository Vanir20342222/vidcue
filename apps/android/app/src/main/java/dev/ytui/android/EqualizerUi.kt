package dev.ytui.android

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/**
 * Equalizer settings over the shared engine config: `equalizer` names "off", a built-in preset
 * or a key of `equalizer_custom` (ten dB gains, 31 Hz to 16 kHz). The engine validates and
 * applies saved changes to playback right away; this object only shapes the JSON draft.
 */
internal object EqualizerModel {
    const val OFF = "off"
    const val MAX_GAIN = 12f
    const val MAX_CUSTOM = 16
    val BANDS = listOf(31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000)
    private val NAME = Regex("[a-z0-9-]{1,24}")

    fun bandLabel(hz: Int): String = if (hz >= 1000) "${hz / 1000} kHz" else "$hz Hz"

    fun label(name: String): String =
        if (name == OFF) "Off"
        else name.split('-').joinToString(" ") { it.replaceFirstChar(Char::uppercaseChar) }

    fun gainLabel(gain: Float): String {
        val rounded = Math.round(gain)
        return if (rounded > 0) "+$rounded dB" else "$rounded dB"
    }

    fun builtins(catalog: JSONObject): List<String> {
        val presets = catalog.optJSONArray("presets") ?: return emptyList()
        return (0 until presets.length()).map { presets.getJSONObject(it).getString("name") }
    }

    fun customNames(config: JSONObject): List<String> =
        config.optJSONObject("equalizer_custom")?.keys()?.asSequence()?.sorted()?.toList()
            ?: emptyList()

    fun names(catalog: JSONObject, config: JSONObject): List<String> =
        listOf(OFF) + builtins(catalog) + customNames(config)

    fun selected(config: JSONObject): String = config.optString("equalizer", OFF).ifEmpty { OFF }

    fun isCustom(config: JSONObject, name: String): Boolean =
        config.optJSONObject("equalizer_custom")?.has(name) == true

    fun gains(catalog: JSONObject, config: JSONObject, name: String): List<Float> {
        val values =
            config.optJSONObject("equalizer_custom")?.optJSONArray(name)
                ?: catalog.optJSONArray("presets")?.let { presets ->
                    (0 until presets.length())
                        .map { presets.getJSONObject(it) }
                        .firstOrNull { it.optString("name") == name }
                        ?.optJSONArray("gains")
                }
        return BANDS.indices.map { (values?.optDouble(it, 0.0) ?: 0.0).toFloat() }
    }

    fun select(draft: JSONObject, name: String) {
        draft.put("equalizer", name)
    }

    fun setGain(draft: JSONObject, name: String, band: Int, gain: Float) {
        val curve = draft.optJSONObject("equalizer_custom")?.optJSONArray(name) ?: return
        curve.put(band, Math.round(gain.coerceIn(-MAX_GAIN, MAX_GAIN)).toDouble())
    }

    /** First free "custom", "custom-2", … name. */
    fun freshName(catalog: JSONObject, config: JSONObject): String {
        val taken = names(catalog, config).toSet()
        return generateSequence(1) { it + 1 }
            .map { if (it == 1) "custom" else "custom-$it" }
            .first { it !in taken }
    }

    /** Copies `gains` into a new custom preset, selects it and returns its name. */
    fun createCustom(draft: JSONObject, catalog: JSONObject, gains: List<Float>): String {
        val name = freshName(catalog, draft)
        val custom = draft.optJSONObject("equalizer_custom") ?: JSONObject()
        custom.put(name, JSONArray(gains.map { Math.round(it).toDouble() }))
        draft.put("equalizer_custom", custom)
        draft.put("equalizer", name)
        return name
    }

    /** Null when `name` can replace `current`, otherwise the reason it cannot. */
    fun nameError(catalog: JSONObject, config: JSONObject, current: String, name: String): String? =
        when {
            !NAME.matches(name) -> "Use 1–24 lowercase letters, digits or dashes"
            name == current -> null
            name == OFF || name in builtins(catalog) -> "That name belongs to a built-in preset"
            isCustom(config, name) -> "You already have a preset with that name"
            else -> null
        }

    fun rename(draft: JSONObject, from: String, to: String) {
        val custom = draft.optJSONObject("equalizer_custom") ?: return
        val curve = custom.optJSONArray(from) ?: return
        custom.remove(from)
        custom.put(to, curve)
        if (selected(draft) == from) draft.put("equalizer", to)
    }

    fun delete(draft: JSONObject, name: String) {
        draft.optJSONObject("equalizer_custom")?.remove(name)
        if (selected(draft) == name) draft.put("equalizer", OFF)
    }
}

/** Bands, limits and built-in presets from the engine; empty until loaded or if unavailable. */
@Composable
internal fun rememberEqualizerCatalog(): JSONObject =
    produceState(JSONObject()) {
        value =
            runCatching {
                    withContext(Dispatchers.IO) {
                        RustRpc.call(JSONObject().put("op", "equalizer_presets")) as? JSONObject
                    }
                }
                .getOrNull() ?: JSONObject()
    }.value

/**
 * Preset chips, a response curve and, for custom presets, one slider per band. Used by Settings →
 * Playback and the onboarding Playback step; every change saves through [YtuiViewModel.updateConfig].
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun EqualizerSettings(
    vm: YtuiViewModel,
    dark: Boolean,
    config: JSONObject,
    enabled: Boolean = true,
    modifier: Modifier = Modifier,
) {
    val catalog = rememberEqualizerCatalog()
    val selected = EqualizerModel.selected(config)
    val custom = EqualizerModel.isCustom(config, selected)
    val gains = EqualizerModel.gains(catalog, config, selected)
    var renaming by remember { mutableStateOf<String?>(null) }
    Column(
        modifier.fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .padding(16.dp)
            .testTag("equalizer"),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            MediaIcon(Glyph.Volume, Modifier.size(24.dp), MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.width(16.dp))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text("Equalizer", style = MaterialTheme.typography.titleMedium)
                Text(
                    if (selected == EqualizerModel.OFF) "Audio plays as recorded"
                    else "${EqualizerModel.label(selected)} · applies to music and videos",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        EqualizerCurve(gains, selected != EqualizerModel.OFF)
        FlowRow(
            Modifier.fillMaxWidth().selectableGroup(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            for (name in EqualizerModel.names(catalog, config))
                PresetChip(EqualizerModel.label(name), name == selected, enabled, "equalizer-preset-$name") {
                    vm.updateConfig(dark) { EqualizerModel.select(it, name) }
                }
        }
        if (custom) {
            EqualizerModel.BANDS.forEachIndexed { band, hz ->
                BandSlider(hz, gains[band], enabled) { gain ->
                    vm.updateConfig(dark) { EqualizerModel.setGain(it, selected, band, gain) }
                }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            val count = EqualizerModel.customNames(config).size
            SecondaryAction(
                if (selected == EqualizerModel.OFF || selected == "flat") "New custom preset"
                else "Customize a copy",
                Modifier.testTag("equalizer-new"),
                enabled = enabled && count < EqualizerModel.MAX_CUSTOM,
                glyph = Glyph.Plus,
            ) {
                vm.updateConfig(dark) { EqualizerModel.createCustom(it, catalog, gains) }
            }
            if (custom) {
                QuietAction("Rename", Modifier.testTag("equalizer-rename"), enabled) {
                    renaming = selected
                }
                QuietAction("Delete", Modifier.testTag("equalizer-delete"), enabled, destructive = true) {
                    vm.updateConfig(dark) { EqualizerModel.delete(it, selected) }
                }
            }
        }
    }
    renaming?.let { current ->
        var text by remember(current) { mutableStateOf(current) }
        val error = EqualizerModel.nameError(catalog, config, current, text.trim())
        Panel("Rename preset", { renaming = null }) {
            MediaTextField(text, { text = it.lowercase() }, "Preset name", Modifier.fillMaxWidth().testTag("equalizer-name"))
            Text(
                error ?: "Lowercase letters, digits and dashes",
                style = MaterialTheme.typography.bodySmall,
                color =
                    if (error != null) MaterialTheme.colorScheme.error
                    else MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Action("Save", "equalizer-name-save", enabled = error == null) {
                val name = text.trim()
                renaming = null
                if (name != current) vm.updateConfig(dark) { EqualizerModel.rename(it, current, name) }
            }
        }
    }
}

/** The curve through the ten band gains, with a 0 dB guide; muted when the equalizer is off. */
@Composable
private fun EqualizerCurve(gains: List<Float>, active: Boolean) {
    val line = if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline
    val guide = MaterialTheme.colorScheme.outlineVariant
    val description =
        EqualizerModel.BANDS.indices.joinToString(", ") {
            "${EqualizerModel.bandLabel(EqualizerModel.BANDS[it])} ${EqualizerModel.gainLabel(gains[it])}"
        }
    Canvas(
        Modifier.fillMaxWidth().height(88.dp).semantics { contentDescription = "Equalizer curve: $description" }
    ) {
        val step = size.width / (gains.size - 1)
        fun y(gain: Float) = size.height / 2 - gain / EqualizerModel.MAX_GAIN * (size.height / 2 - 6.dp.toPx())
        drawLine(guide, Offset(0f, size.height / 2), Offset(size.width, size.height / 2), 1.dp.toPx())
        val points = gains.mapIndexed { index, gain -> Offset(index * step, y(gain)) }
        val path = Path().apply {
            moveTo(points.first().x, points.first().y)
            // Smooth cubic segments with horizontal tangents at each band.
            for (index in 1 until points.size) {
                val from = points[index - 1]
                val to = points[index]
                val middle = (from.x + to.x) / 2
                cubicTo(middle, from.y, middle, to.y, to.x, to.y)
            }
        }
        val fill = Path().apply {
            addPath(path)
            lineTo(size.width, size.height / 2)
            lineTo(0f, size.height / 2)
            close()
        }
        drawPath(fill, line.copy(alpha = 0.14f))
        drawPath(path, line, style = Stroke(2.5.dp.toPx(), cap = StrokeCap.Round))
        points.forEach { drawCircle(line, 3.dp.toPx(), it) }
    }
}

@Composable
private fun PresetChip(label: String, selected: Boolean, enabled: Boolean, tag: String, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.heightIn(min = 48.dp)
            .clip(MediaTokens.pill)
            .background(if (selected) colors.onBackground else colors.surfaceContainerHigh)
            .border(BorderStroke(1.dp, if (selected) colors.onBackground else colors.outlineVariant), MediaTokens.pill)
            .selectable(selected, enabled = enabled, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = 16.dp)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (selected) {
            MediaIcon(Glyph.Check, Modifier.size(18.dp), colors.background)
            Spacer(Modifier.width(6.dp))
        }
        Text(
            label,
            style = MaterialTheme.typography.labelLarge,
            color = if (selected) colors.background else colors.onSurface,
        )
    }
}

/** One band: frequency, a 1 dB-step slider that saves when released, and the current gain. */
@Composable
private fun BandSlider(hz: Int, gain: Float, enabled: Boolean, commit: (Float) -> Unit) {
    var value by remember(hz, gain) { mutableFloatStateOf(gain) }
    val label = EqualizerModel.bandLabel(hz)
    Row(
        Modifier.fillMaxWidth().testTag("equalizer-band-$hz"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            label,
            Modifier.widthIn(min = 56.dp),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Slider(
            value,
            { value = Math.round(it).toFloat() },
            Modifier.weight(1f).semantics {
                contentDescription = label
                stateDescription = EqualizerModel.gainLabel(value)
            },
            enabled = enabled,
            valueRange = -EqualizerModel.MAX_GAIN..EqualizerModel.MAX_GAIN,
            steps = (EqualizerModel.MAX_GAIN * 2).toInt() - 1,
            onValueChangeFinished = { if (value != gain) commit(value) },
        )
        Text(
            EqualizerModel.gainLabel(value),
            Modifier.widthIn(min = 52.dp),
            style = MaterialTheme.typography.labelMedium,
            fontWeight = FontWeight.SemiBold,
        )
    }
}
