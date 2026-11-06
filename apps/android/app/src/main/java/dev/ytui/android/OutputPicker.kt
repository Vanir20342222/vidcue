package dev.ytui.android

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.os.Build
import android.provider.Settings
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import org.json.JSONObject

/** Where audio is going, as published by PlaybackService ("output"); null before it starts. */
internal data class AudioOutput(val id: Int, val kind: String, val name: String) {
    val bluetooth: Boolean
        get() = kind == "bluetooth"
}

internal val PlayerState.output: AudioOutput?
    get() = value.optJSONObject("output")?.let(::audioOutput)?.takeIf { it.kind.isNotEmpty() }

internal val PlayerState.outputs: List<AudioOutput>
    get() = value.optJSONObject("output")?.optJSONArray("devices")?.objects()?.map(::audioOutput).orEmpty()

private fun audioOutput(json: JSONObject) =
    AudioOutput(json.optInt("id"), json.optString("kind"), json.optString("name"))

internal fun outputGlyph(kind: String): Glyph =
    when (kind) {
        "bluetooth" -> Glyph.Bluetooth
        "wired", "usb" -> Glyph.Headphones
        else -> Glyph.Volume
    }

/**
 * Android's own media output switcher, which can also connect paired devices that are not
 * active yet. Falls back to the older settings panel, then to Bluetooth settings.
 */
internal fun openSystemOutputSwitcher(context: Context) {
    if (Build.VERSION.SDK_INT >= 34 &&
        android.media.MediaRouter2.getInstance(context).showSystemOutputSwitcher()
    ) return
    val panel =
        Intent("com.android.settings.panel.action.MEDIA_OUTPUT")
            .putExtra("com.android.settings.panel.extra.PACKAGE_NAME", context.packageName)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    try {
        context.startActivity(panel)
    } catch (missing: ActivityNotFoundException) {
        try {
            context.startActivity(
                Intent(Settings.ACTION_BLUETOOTH_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            )
        } catch (alsoMissing: ActivityNotFoundException) {}
    }
}

/** Output choices for the player: connected devices, the system default, and more devices. */
@Composable
internal fun OutputOptions(vm: YtuiViewModel, done: () -> Unit) {
    val player by vm.player.collectAsState()
    val context = LocalContext.current
    val current = player.output
    val automatic = player.value.optJSONObject("output")?.optBoolean("automatic", true) ?: true
    val colors = MaterialTheme.colorScheme
    for (device in player.outputs) {
        val selected = device.id == current?.id
        SheetAction(
            modifier = Modifier.testTag("output-${device.id}"),
            text = {
                Column {
                    Text(device.name, maxLines = 1)
                    if (selected)
                        Text(
                            if (automatic) "Playing here" else "Playing here · chosen",
                            style = MaterialTheme.typography.bodySmall,
                            color = colors.onSurfaceVariant,
                        )
                }
            },
            leadingIcon = {
                MediaIcon(
                    outputGlyph(device.kind),
                    tint = if (selected) colors.primary else androidx.compose.material3.LocalContentColor.current,
                    filled = selected,
                )
            },
            trailing = { if (selected) MediaIcon(Glyph.Check, tint = colors.primary) },
            onClick = {
                vm.command("output_device", "id" to device.id)
                done()
            },
        )
    }
    if (!automatic)
        SheetAction(
            modifier = Modifier.testTag("output-automatic"),
            text = { Text("Use system default") },
            leadingIcon = { MediaIcon(Glyph.Refresh) },
            onClick = {
                vm.command("output_device", "id" to 0)
                done()
            },
        )
    SheetAction(
        modifier = Modifier.testTag("output-more"),
        text = {
            Column {
                Text("More devices")
                Text(
                    "Connect paired Bluetooth or cast devices",
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.onSurfaceVariant,
                )
            }
        },
        leadingIcon = { MediaIcon(Glyph.Bluetooth) },
        onClick = {
            openSystemOutputSwitcher(context)
            done()
        },
    )
}
