package dev.ytui.android

import androidx.compose.foundation.layout.*
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import org.json.JSONObject

/** Key fingerprints are twelve digits, compared by the user in four groups of three. */
internal object Fingerprints {
    /** The four groups, or empty when [raw] is not exactly twelve digits (with optional spaces). */
    fun groups(raw: String): List<String> {
        val compact = raw.filter { it != ' ' }
        if (compact.length != 12 || compact.any { it !in '0'..'9' }) return emptyList()
        return compact.chunked(3)
    }

    /** Screen readers read digits one by one, group by group. */
    fun spoken(groups: List<String>): String = groups.joinToString(", ") { it.toCharArray().joinToString(" ") }
}

/** Two lines of two groups in large monospace, the form both devices show. */
@Composable
internal fun FingerprintText(raw: String, tag: String) {
    val groups = Fingerprints.groups(raw)
    if (groups.isEmpty()) return
    Column(
        Modifier.testTag(tag).semantics { contentDescription = Fingerprints.spoken(groups) },
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        CodeText("${groups[0]}  ${groups[1]}", size = 34)
        CodeText("${groups[2]}  ${groups[3]}", size = 34)
    }
}

/**
 * Requester side: this device has no (or a mismatched) account key. It starts a relay request
 * and shows its fingerprint while it waits; the other device must show the same digits before
 * the user approves there. The key arrives as `sync_key_state` "ok", which closes this sheet.
 */
@Composable
internal fun KeyRelaySheet(vm: YtuiViewModel, close: () -> Unit) {
    val key by vm.sync.key.collectAsState()
    val runner = rememberSyncRunner()
    var request by remember { mutableStateOf<JSONObject?>(null) }
    val waiting = request
    LaunchedEffect(Unit) { runCatching { vm.sync.key.value = vm.sync.obj("key_fingerprint") } }
    LaunchedEffect(waiting) {
        val expires = waiting?.optLong("expires_at") ?: return@LaunchedEffect
        while (System.currentTimeMillis() < expires) delay(1_000)
        request = null
    }
    SecureDialog(close, "key-relay", secure = false) {
        GlyphBadge(Glyph.Key, 64.dp, if (key.optString("state") == "ok") SyncColors.online else MaterialTheme.colorScheme.onSurface)
        when {
            key.optString("state") == "ok" -> {
                FingerprintText(key.text("fingerprint"), "key-own-fingerprint")
                KeyRequestsList(vm)
            }
            waiting != null -> {
                FingerprintText(waiting.text("fingerprint"), "key-request-fingerprint")
                MediaSpinner()
            }
            else -> {
                ErrorLine(runner.error)
                Action("Request key", "key-request-start", enabled = !runner.busy) {
                    runner.run { request = vm.sync.obj("key_request_start") }
                }
            }
        }
    }
}

/** Pending requests from this account's other devices, for when the event was missed. */
@Composable
private fun KeyRequestsList(vm: YtuiViewModel) {
    var requests by remember { mutableStateOf<List<JSONObject>>(emptyList()) }
    LaunchedEffect(Unit) { runCatching { requests = vm.sync.list("key_requests") } }
    for (request in requests)
        ValueRow(
            request.text("device_name").ifBlank { "Device" },
            Modifier.testTag("key-request-${request.text("id")}"),
            glyph = Glyph.Devices,
        ) {
            vm.sync.keyRequest.value = request
        }
}

/**
 * Granter side, hosted at the app root: another device asks for the key. Approving sends it
 * sealed to that device's public key; the user approves only when both screens show the same
 * twelve digits. Deny simply drops it (the request expires on the server).
 */
@Composable
internal fun KeyRequestSheet(vm: YtuiViewModel) {
    val request by vm.sync.keyRequest.collectAsState()
    val current = request ?: return
    val runner = rememberSyncRunner()
    val close = { vm.sync.keyRequest.value = null }
    SecureDialog(close, "key-grant", secure = false) {
        GlyphBadge(Glyph.Key, 64.dp)
        Text(
            current.text("device_name").ifBlank { "Device" },
            style = MaterialTheme.typography.titleLarge,
            textAlign = TextAlign.Center,
        )
        FingerprintText(current.text("fingerprint"), "key-grant-fingerprint")
        ErrorLine(runner.error)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            QuietAction("Deny", Modifier.testTag("key-deny"), destructive = true) { close() }
            Action("Approve", "key-approve", enabled = !runner.busy && Fingerprints.groups(current.text("fingerprint")).isNotEmpty()) {
                runner.run {
                    vm.sync.obj("key_grant", "id" to current.text("id"))
                    close()
                    Notices.show(Notice("Key sent", Glyph.Key))
                }
            }
        }
    }
}
