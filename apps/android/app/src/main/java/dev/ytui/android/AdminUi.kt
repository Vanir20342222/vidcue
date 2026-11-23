package dev.ytui.android

import android.content.Intent
import androidx.compose.foundation.layout.*
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONObject

/** Invite lifetimes offered as chips, in hours. */
internal val INVITE_HOURS = listOf(24, 72, 168)

/** Admin section: users, invites, reset links, devices, disable/enable and playback health. */
@Composable
internal fun AdminSheet(vm: YtuiViewModel, close: () -> Unit) {
    var users by remember { mutableStateOf<List<JSONObject>>(emptyList()) }
    var health by remember { mutableStateOf<JSONObject?>(null) }
    var user by remember { mutableStateOf<JSONObject?>(null) }
    var inviting by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var revision by remember { mutableIntStateOf(0) }
    LaunchedEffect(revision) {
        try {
            users = vm.sync.list("admin_users")
        } catch (cancelled: kotlinx.coroutines.CancellationException) {
            throw cancelled
        } catch (failure: Exception) {
            error = SyncText.message(failure)
        }
        if (vm.sync.has("health")) health = runCatching { vm.sync.obj("admin_health", "days" to 7) }.getOrNull()
    }
    val selected = user
    if (selected != null) {
        AdminUserSheet(vm, selected) {
            user = null
            revision++
        }
        return
    }
    Panel("Admin", close, actions = { MediaIconButton(Glyph.Plus, "Invite", "admin-invite") { inviting = true } }) {
        ErrorLine(error)
        health?.let { summary ->
            val start = summary.optJSONObject("by_kind")?.optJSONObject("start")
            Row(Modifier.fillMaxWidth().testTag("admin-health"), horizontalArrangement = Arrangement.SpaceEvenly) {
                BigNumber(users.size.toString(), "users")
                BigNumber(summary.optInt("users_reporting").toString(), "reporting")
                if (start != null && start.optInt("total") > 0) BigNumber(SyncText.percent(start.optDouble("ok_rate", Double.NaN)), "starts ok")
            }
        }
        for (entry in users) {
            val disabled = entry.optBoolean("disabled")
            Row(
                Modifier.fillMaxWidth().heightIn(min = 56.dp).testTag("admin-user-${entry.text("id")}"),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Monogram(entry.text("username"), 40.dp)
                Column(Modifier.weight(1f)) {
                    Text(
                        entry.text("username"),
                        style = MaterialTheme.typography.bodyLarge,
                        color = if (disabled) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        MediaIcon(Glyph.Devices, Modifier.size(14.dp), MaterialTheme.colorScheme.onSurfaceVariant)
                        Text(
                            listOf(entry.optInt("devices").toString(), SyncText.age(entry.optLong("last_seen"), System.currentTimeMillis()))
                                .filter { it.isNotBlank() }
                                .joinToString(" · "),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                if (entry.optBoolean("is_admin")) MediaIcon(Glyph.Key, Modifier.size(18.dp), MaterialTheme.colorScheme.primary)
                if (disabled) MediaIcon(Glyph.Lock, Modifier.size(18.dp), MaterialTheme.colorScheme.error)
                MediaIconButton(Glyph.Forward, entry.text("username"), "admin-open-${entry.text("id")}") { user = entry }
            }
        }
    }
    if (inviting) AdminInviteDialog(vm) { inviting = false }
}

/** Shares a link through Android's share sheet; the link is only ever handed to apps the user picks. */
@Composable
private fun rememberShare(): (String) -> Unit {
    val context = LocalContext.current
    return remember(context) {
        { link: String ->
            val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, link)
            context.startActivity(Intent.createChooser(send, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        }
    }
}

@Composable
private fun AdminInviteDialog(vm: YtuiViewModel, close: () -> Unit) {
    val runner = rememberSyncRunner()
    val share = rememberShare()
    var password by remember { mutableStateOf("") }
    var admin by remember { mutableStateOf(false) }
    var hours by remember { mutableIntStateOf(72) }
    var invite by remember { mutableStateOf<JSONObject?>(null) }
    val made = invite
    SecureDialog(close, "admin-invite-sheet", secure = made != null) {
        GlyphBadge(Glyph.People, 56.dp)
        if (made == null) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                for (option in INVITE_HOURS)
                    MediaChip(if (option % 24 == 0) "${option / 24}d" else "${option}h", hours == option, Modifier.testTag("admin-invite-hours-$option")) {
                        hours = option
                    }
            }
            SyncToggle("Admin", admin, "admin-invite-admin", Glyph.Key) { admin = it }
            PasswordField(password, { password = it }, "Password", "admin-invite-password")
            ErrorLine(runner.error)
            Action("Create", "admin-invite-create", enabled = password.isNotEmpty() && !runner.busy) {
                runner.run {
                    invite = vm.sync.obj("admin_invite", "password" to password, "admin" to admin, "hours" to hours)
                    password = ""
                }
            }
        } else {
            QrCode(made.optJSONArray("qr"), Modifier.fillMaxWidth(), "Invite QR code")
            val link = made.text("url")
            if (link.isNotBlank())
                Pill("Share", Modifier.testTag("admin-invite-share"), PillStyle.Inverse, glyph = Glyph.Share) { share(link) }
        }
    }
}

@Composable
private fun AdminUserSheet(vm: YtuiViewModel, user: JSONObject, close: () -> Unit) {
    val id = user.text("id")
    var devices by remember { mutableStateOf<List<JSONObject>>(emptyList()) }
    var action by remember { mutableStateOf("") }
    var revision by remember { mutableIntStateOf(0) }
    LaunchedEffect(revision) { runCatching { devices = vm.sync.list("admin_devices", "user_id" to id) } }
    val disabled = user.optBoolean("disabled")
    Panel(user.text("username"), close) {
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) { Monogram(user.text("username"), 72.dp) }
        ValueRow("Reset link", Modifier.testTag("admin-reset"), glyph = Glyph.Link) { action = "reset" }
        ValueRow("Sign out everywhere", Modifier.testTag("admin-revoke-all"), glyph = Glyph.Devices) { action = "revoke" }
        ValueRow(if (disabled) "Enable" else "Disable", Modifier.testTag("admin-toggle"), glyph = Glyph.Lock) {
            action = if (disabled) "enable" else "disable"
        }
        if (devices.isNotEmpty()) SectionHeader("Devices")
        for (device in devices)
            Row(
                Modifier.fillMaxWidth().heightIn(min = 52.dp).testTag("admin-device-${device.text("id")}"),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                GlyphTile(Glyph.Devices, Modifier.size(36.dp))
                Column(Modifier.weight(1f)) {
                    Text(device.text("name").ifBlank { "Device" }, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(SyncText.age(device.optLong("last_seen"), System.currentTimeMillis()), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                MediaIconButton(Glyph.Close, "Remove", "admin-device-revoke-${device.text("id")}") { action = "revoke:${device.text("id")}" }
            }
    }
    when {
        action == "reset" -> AdminResetDialog(vm, id) { action = "" }
        action.isNotEmpty() ->
            AdminConfirmDialog(vm, id, action) {
                action = ""
                revision++
            }
    }
}

@Composable
private fun AdminResetDialog(vm: YtuiViewModel, userId: String, close: () -> Unit) {
    val runner = rememberSyncRunner()
    val share = rememberShare()
    var password by remember { mutableStateOf("") }
    var code by remember { mutableStateOf("") }
    var reset by remember { mutableStateOf<JSONObject?>(null) }
    val made = reset
    SecureDialog(close, "admin-reset-sheet", secure = made != null) {
        GlyphBadge(Glyph.Link, 56.dp)
        if (made == null) {
            PasswordField(password, { password = it }, "Password", "admin-reset-password")
            CodeField(code, { code = it }, "admin-reset-code")
            ErrorLine(runner.error)
            Action("Create", "admin-reset-create", enabled = password.isNotEmpty() && code.isNotBlank() && !runner.busy) {
                runner.run {
                    reset = vm.sync.obj("admin_reset", "user_id" to userId, "password" to password, "code" to code.trim())
                    password = ""
                    code = ""
                }
            }
        } else {
            QrCode(made.optJSONArray("qr"), Modifier.fillMaxWidth(), "Reset QR code")
            val link = made.text("url")
            if (link.isNotBlank())
                Pill("Share", Modifier.testTag("admin-reset-share"), PillStyle.Inverse, glyph = Glyph.Share) { share(link) }
        }
    }
}

/** Password confirmation for revoke, disable and enable (`revoke`, `revoke:<device>`, `disable`, `enable`). */
@Composable
private fun AdminConfirmDialog(vm: YtuiViewModel, userId: String, action: String, close: () -> Unit) {
    val runner = rememberSyncRunner()
    var password by remember { mutableStateOf("") }
    val title =
        when {
            action == "disable" -> "Disable"
            action == "enable" -> "Enable"
            else -> "Remove"
        }
    MediaDialog(
        close,
        title,
        actions = {
            QuietAction("Cancel") { close() }
            QuietAction(title, Modifier.testTag("admin-confirm"), enabled = password.isNotEmpty() && !runner.busy, destructive = action != "enable") {
                runner.run {
                    when {
                        action == "disable" -> vm.sync.obj("admin_disable", "user_id" to userId, "password" to password)
                        action == "enable" -> vm.sync.obj("admin_enable", "user_id" to userId, "password" to password)
                        action == "revoke" -> vm.sync.obj("admin_revoke", "user_id" to userId, "password" to password)
                        action.startsWith("revoke:") ->
                            vm.sync.obj("admin_revoke", "user_id" to userId, "password" to password, "device_id" to action.removePrefix("revoke:"))
                    }
                    password = ""
                    close()
                }
            }
        },
    ) {
        PasswordField(password, { password = it }, "Password", "admin-confirm-password")
        ErrorLine(runner.error)
    }
}
