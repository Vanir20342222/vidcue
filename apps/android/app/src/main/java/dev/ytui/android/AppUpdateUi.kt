package dev.ytui.android

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.progressSemantics
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

/**
 * "New version available" sheet. Checks GitHub once when the app opens (signed builds only,
 * at most every few hours, and only when enabled in About), then offers
 * Update now · Later · Skip this version.
 */
@Composable
internal fun AppUpdateHost() {
    val context = LocalContext.current
    LaunchedEffect(Unit) { AppUpdates.checkOnStart(context) }
    val state by AppUpdates.state.collectAsState()
    val release =
        when (val current = state) {
            is UpdateState.Available -> current.release
            is UpdateState.Downloading -> current.release
            is UpdateState.Installing -> current.release
            is UpdateState.NeedsPermission -> current.release
            is UpdateState.Failed -> current.release
            else -> null
        } ?: return
    val busy = state is UpdateState.Downloading || state is UpdateState.Installing
    Panel("New version available", { AppUpdates.dismiss() }) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp).testTag("app-update"),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                "Vidcue ${release.versionName}",
                style = MaterialTheme.typography.headlineSmall,
            )
            Text(
                "You have ${BuildConfig.VERSION_NAME}. The update installs over this app and keeps your account, settings and library.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            when (val current = state) {
                is UpdateState.Downloading -> {
                    MediaProgressBar(
                        current.progress,
                        Modifier.fillMaxWidth()
                            .progressSemantics(current.progress)
                            .testTag("app-update-progress"),
                    )
                    Text(
                        "Downloading… ${(current.progress * 100).toInt()}%",
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                is UpdateState.Installing ->
                    Text(
                        "Confirm the update in Android's installation screen. Vidcue restarts when it is done.",
                        style = MaterialTheme.typography.bodyMedium,
                    )
                is UpdateState.NeedsPermission -> {
                    Text(
                        "Android needs your permission once: turn on “Allow from this source” for Vidcue, come back and tap Update now.",
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    Pill("Open Android settings", Modifier.testTag("app-update-permission"), PillStyle.Tonal) {
                        AppUpdates.openInstallPermission(context)
                    }
                }
                is UpdateState.Failed ->
                    Text(
                        current.text,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                else -> {}
            }
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Pill(
                    if (state is UpdateState.Failed) "Try again" else "Update now",
                    Modifier.testTag("app-update-now"),
                    PillStyle.Primary,
                    enabled = !busy,
                    glyph = Glyph.Download,
                ) {
                    AppUpdates.start(context, release)
                }
                Pill("Later", Modifier.testTag("app-update-later"), PillStyle.Tonal) { AppUpdates.dismiss() }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Pill("Skip this version", Modifier.testTag("app-update-skip"), PillStyle.Quiet, enabled = !busy) {
                    AppUpdates.skip(context, release)
                }
                if (release.page.startsWith("https://github.com/"))
                    Pill("Release notes", style = PillStyle.Quiet, glyph = Glyph.Link) {
                        runCatching {
                            context.startActivity(
                                Intent(Intent.ACTION_VIEW, Uri.parse(release.page))
                                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                            )
                        }
                    }
            }
        }
    }
}

/** About panel rows: automatic check switch and "Check for updates" with its result. */
@Composable
internal fun AppUpdateSettings() {
    val context = LocalContext.current
    var automatic by remember { mutableStateOf(AppUpdates.enabled(context)) }
    val state by AppUpdates.state.collectAsState()
    Row(
        Modifier.fillMaxWidth()
            .heightIn(min = 60.dp)
            .testTag("app-update-automatic")
            .toggleable(automatic, role = Role.Switch) {
                automatic = it
                AppUpdates.setEnabled(context, it)
            }
            .padding(horizontal = 10.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                "Check for updates automatically",
                style = MaterialTheme.typography.bodyLarge,
                fontWeight = FontWeight.Medium,
            )
            Text(
                "Asks GitHub when Vidcue opens. Nothing installs without your OK.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        MediaSwitch(automatic)
    }
    ValueRow(
        "Check for updates",
        Modifier.testTag("app-update-check"),
        subtitle =
            when (val current = state) {
                UpdateState.Checking -> "Checking…"
                is UpdateState.Note -> current.text
                is UpdateState.Available -> "Vidcue ${current.release.versionName} is available."
                else -> "Version ${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE})"
            },
        glyph = Glyph.Refresh,
        enabled = state != UpdateState.Checking,
    ) {
        AppUpdates.checkNow(context)
    }
}
