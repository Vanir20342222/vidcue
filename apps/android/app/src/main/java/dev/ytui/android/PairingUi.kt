package dev.ytui.android

import androidx.compose.foundation.layout.*
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import org.json.JSONObject

/** Recognises pairing links before anything else looks at them. */
internal object PairLinks {
    const val MAX_LENGTH = 512

    /**
     * True for `vidcue://pair?…` within the length limit. Only the shape is checked here; the
     * engine (`sync_pair_inspect`) validates server, code and key before the confirm sheet shows.
     */
    fun recognize(raw: String): Boolean {
        val text = raw.trim()
        if (text.isEmpty() || text.length > MAX_LENGTH || text.any { it.isWhitespace() || it.code < 0x20 }) return false
        val uri =
            try {
                java.net.URI(text)
            } catch (invalid: java.net.URISyntaxException) {
                return false
            }
        return uri.scheme.equals("vidcue", ignoreCase = true) &&
            uri.rawAuthority.equals("pair", ignoreCase = true) &&
            uri.rawPath.isNullOrEmpty() &&
            !uri.rawQuery.isNullOrEmpty()
    }
}

/**
 * "Show pairing QR": asks for the password again (the server requires it), then shows the QR
 * the engine drew from the link plus the account key fragment, with a countdown. The link text
 * itself is never shown: it carries the key.
 */
@Composable
internal fun PairShowSheet(vm: YtuiViewModel, close: () -> Unit) {
    val runner = rememberSyncRunner()
    var password by remember { mutableStateOf("") }
    var pairing by remember { mutableStateOf<JSONObject?>(null) }
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    val current = pairing
    val expires = current?.optLong("expires_at") ?: 0L
    LaunchedEffect(current) {
        while (current != null) {
            now = System.currentTimeMillis()
            if (now >= expires) {
                pairing = null
                break
            }
            delay(1_000)
        }
    }
    SecureDialog(close, "pair-show", secure = current != null) {
        GlyphBadge(Glyph.Devices, 64.dp)
        if (current == null) {
            PasswordField(password, { password = it }, "Password", "pair-password")
            ErrorLine(runner.error)
            Action("Show QR", "pair-start", enabled = password.isNotEmpty() && !runner.busy) {
                runner.run {
                    pairing = vm.sync.obj("sync_pair_start", "password" to password)
                    password = ""
                }
            }
        } else {
            QrCode(current.optJSONArray("qr"), Modifier.fillMaxWidth(), "Pairing QR code")
            val left = ((expires - now) / 1000).coerceAtLeast(0)
            Text(
                "%d:%02d".format(left / 60, left % 60),
                Modifier.testTag("pair-countdown"),
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/** How to pair this device: scan the other device's QR with the camera, or paste its link. */
@Composable
internal fun PairHelpSheet(vm: YtuiViewModel, close: () -> Unit) {
    var link by remember { mutableStateOf("") }
    var error by remember { mutableStateOf<String?>(null) }
    Panel("Pair", close) {
        Column(
            Modifier.fillMaxWidth(),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            GlyphBadge(Glyph.Devices, 72.dp)
            Text(
                "Scan the QR on your other device with the camera",
                style = MaterialTheme.typography.bodyLarge,
                textAlign = TextAlign.Center,
            )
        }
        MediaTextField(link, { link = it.take(PairLinks.MAX_LENGTH + 1); error = null }, "Or paste the link", Modifier.testTag("pair-link"))
        ErrorLine(error)
        Action("Continue", "pair-link-continue", enabled = link.isNotBlank()) {
            if (PairLinks.recognize(link)) {
                vm.sync.pairLink.value = link.trim()
                link = ""
                close()
            } else error = "Not a pairing link"
        }
    }
}

/**
 * Confirms an incoming `vidcue://pair` link before anything is sent: the full server origin
 * (scheme, host and port) is always shown, with a key mark when the link carries the account
 * key, and joining needs a tap. Links can come from any page or app, so after the claim
 * [PairAccountSheet] asks again, naming the account, before anything syncs.
 */
@Composable
internal fun PairConfirmSheet(vm: YtuiViewModel) {
    val link by vm.sync.pairLink.collectAsState()
    val uri = link ?: return
    val status by vm.sync.status.collectAsState()
    val runner = rememberSyncRunner()
    var inspected by remember(uri) { mutableStateOf<JSONObject?>(null) }
    var invalid by remember(uri) { mutableStateOf<String?>(null) }
    LaunchedEffect(uri) {
        try {
            inspected = vm.sync.obj("sync_pair_inspect", "uri" to uri)
        } catch (cancelled: kotlinx.coroutines.CancellationException) {
            throw cancelled
        } catch (failure: Exception) {
            invalid = SyncText.message(failure)
        }
    }
    val close = { vm.sync.pairLink.value = null }
    SecureDialog(close, "pair-confirm") {
        GlyphBadge(Glyph.Devices, 64.dp)
        val found = inspected
        when {
            invalid != null -> ErrorLine(invalid)
            found == null -> MediaSpinner()
            else -> {
                Text(
                    found.text("server").ifBlank { found.text("host") },
                    Modifier.testTag("pair-host"),
                    style = MaterialTheme.typography.titleLarge,
                    textAlign = TextAlign.Center,
                    maxLines = 3,
                    overflow = TextOverflow.Ellipsis,
                )
                if (!found.optBoolean("secure", true))
                    Text(
                        "Not encrypted",
                        Modifier.testTag("pair-insecure"),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                if (found.optBoolean("has_key"))
                    MediaIcon(Glyph.Key, Modifier.size(28.dp).testTag("pair-has-key"), SyncColors.online)
                val busySignedIn = status.optBoolean("signed_in")
                if (busySignedIn)
                    Text(
                        "Sign out first",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                ErrorLine(runner.error)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    QuietAction("Cancel", Modifier.testTag("pair-cancel")) { close() }
                    Action("Join", "pair-join", enabled = !busySignedIn && !runner.busy) {
                        runner.run { vm.sync.claimPairing(uri) }
                    }
                }
            }
        }
    }
}

/**
 * After a pairing link signed this device in: names the account and server and asks before
 * the library is copied there. The YouTube sign-in is only synced when switched on here.
 */
@Composable
internal fun PairAccountSheet(vm: YtuiViewModel) {
    val status by vm.sync.status.collectAsState()
    if (!status.optBoolean("awaiting_confirmation") || !status.optBoolean("signed_in")) return
    val runner = rememberSyncRunner()
    var youtube by remember { mutableStateOf(false) }
    val cancel = { runner.run { vm.sync.signOut(removeDevice = true, forgetData = false) } }
    SecureDialog({ cancel() }, "pair-account", secure = false) {
        Monogram(status.text("username"), 64.dp)
        Text(
            status.text("username"),
            Modifier.testTag("pair-account-user"),
            style = MaterialTheme.typography.headlineSmall,
            textAlign = TextAlign.Center,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Text(
            status.text("server"),
            Modifier.testTag("pair-account-server"),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        Text(
            "Your library will sync to this account",
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.Center,
        )
        SyncToggle("YouTube sign-in", youtube, "pair-account-youtube", Glyph.Key) { youtube = it }
        ErrorLine(runner.error)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            QuietAction("Cancel", Modifier.testTag("pair-account-cancel"), enabled = !runner.busy) { cancel() }
            Action("Sync", "pair-account-confirm", enabled = !runner.busy) {
                runner.run { vm.sync.confirmPairing(youtube) }
            }
        }
    }
}
