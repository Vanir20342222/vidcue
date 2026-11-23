package dev.ytui.android

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import org.json.JSONObject

/** The "Vidcue" settings category: account, sync, devices and the social and personal hubs. */
@Composable
internal fun SyncPanel(vm: YtuiViewModel) {
    val status by vm.sync.status.collectAsState()
    LaunchedEffect(Unit) {
        vm.sync.start()
        runCatching { vm.sync.refreshStatus() }
    }
    Column(Modifier.fillMaxWidth().testTag("sync-panel"), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (status.optBoolean("signed_in")) SignedIn(vm, status) else SignedOut(vm, status)
        LocalLibraryTiles(vm)
    }
}

@Composable
private fun SignedOut(vm: YtuiViewModel, status: JSONObject) {
    val runner = rememberSyncRunner()
    var server by rememberSaveable { mutableStateOf(status.text("server").ifBlank { "https://" }) }
    var username by rememberSaveable { mutableStateOf(status.text("username")) }
    // Secrets live only in memory while this form is shown.
    var password by remember { mutableStateOf("") }
    var code by remember { mutableStateOf("") }
    if (status.text("reason") == "revoked")
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            StatusDot(MaterialTheme.colorScheme.error)
            Text("Device removed", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
        }
    SettingGroup("Account", showTitle = false) {
        Column(Modifier.padding(vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            MediaTextField(
                server,
                { server = it.take(256) },
                "Server",
                Modifier.testTag("sync-server"),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false),
            )
            MediaTextField(
                username,
                { username = it.take(64) },
                "Username",
                Modifier.testTag("sync-username"),
                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false),
            )
            PasswordField(password, { password = it.take(256) }, "Password", "sync-password")
            CodeField(code, { code = it }, "sync-code", "Authenticator or recovery code")
            ErrorLine(runner.error)
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Pill("Pair", Modifier.testTag("sync-pair-help"), PillStyle.Tonal, glyph = Glyph.Devices) {
                    vm.sync.open(SyncSheet.PairHelp)
                }
                Action(
                    "Sign in",
                    "sync-sign-in",
                    enabled = !runner.busy && server.isNotBlank() && username.isNotBlank() && password.isNotEmpty() && code.isNotBlank(),
                ) {
                    runner.run {
                        vm.sync.signIn(server, username, password, code)
                        password = ""
                        code = ""
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SignedIn(vm: YtuiViewModel, status: JSONObject) {
    val runner = rememberSyncRunner()
    val unread by vm.sync.unread.collectAsState()
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(Unit) {
        while (true) {
            now = System.currentTimeMillis()
            delay(30_000)
        }
    }
    val features = status.optJSONArray("features").strings().toSet()
    // Identity and state: who, where, and when it last synced.
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 4.dp).testTag("sync-identity"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Monogram(status.text("username"), 52.dp)
        Column(Modifier.weight(1f)) {
            Text(status.text("username"), style = MaterialTheme.typography.titleLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                StatusDot(stateColor(status), Modifier.testTag("sync-state-${status.optString("state")}"))
                Text(
                    listOf(
                            hostOf(status.text("server")),
                            SyncText.age(status.optLong("last_success"), now),
                        )
                        .filter { it.isNotBlank() }
                        .joinToString(" · "),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        val pending = status.optInt("pending")
        if (pending > 0)
            Text(
                pending.toString(),
                Modifier.semantics { contentDescription = "$pending changes waiting" },
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        if (runner.busy || status.optString("state") == "syncing") MediaSpinner(Modifier.size(24.dp))
        else
            RoundIconButton(Glyph.Refresh, "Sync now", Modifier.testTag("sync-now")) {
                runner.run { vm.sync.syncNow() }
            }
    }
    ErrorLine(runner.error ?: status.text("last_error").takeIf { status.optString("state") == "error" })
    val keyState = status.optString("key_state", "ok")
    if (keyState != "ok")
        ValueRow("Key", Modifier.testTag("sync-key-missing"), glyph = Glyph.Key, value = "!") {
            vm.sync.open(SyncSheet.KeyRelay)
        }
    // The hub: one tile per place, icons first.
    val tiles =
        buildList {
            if ("social" in features) {
                add(Triple(Glyph.People, "Friends", SyncSheet.Friends))
                add(Triple(Glyph.Inbox, "Inbox", SyncSheet.Inbox))
            }
            if ("jam" in features) add(Triple(Glyph.Jam, "Jam", SyncSheet.Jam))
            if ("realtime" in features) add(Triple(Glyph.Devices, "Devices", SyncSheet.Devices))
            if ("stats" in features) add(Triple(Glyph.History, "Stats", SyncSheet.Stats))
            if (status.optBoolean("is_admin")) add(Triple(Glyph.Settings, "Admin", SyncSheet.Admin))
        }
    TileGrid(tiles.map { (glyph, label, sheet) -> TileSpec(glyph, label, if (sheet == SyncSheet.Inbox) unread else 0) { vm.sync.open(sheet) } })
    if ("recs" in features) RecsToggle(vm)
    SettingGroup("Sync") {
        val collections = status.optJSONObject("collections") ?: JSONObject()
        FlowRow(
            Modifier.fillMaxWidth().padding(vertical = 8.dp).testTag("sync-collections"),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            for ((name, label) in SYNC_COLLECTIONS) {
                val on = collections.optBoolean(name, true)
                MediaChip(label, on, Modifier.testTag("sync-collection-$name")) {
                    vm.sync.act { vm.sync.setCollection(name, !on) }
                }
            }
        }
        if ("health" in features)
            SyncToggle("Playback health", status.optBoolean("health_reports"), "sync-health", Glyph.Info) {
                vm.sync.act { vm.sync.setHealthReports(it) }
            }
    }
    DevicesGroup(vm, status)
    SettingGroup("Security") {
        ValueRow("Key", Modifier.testTag("sync-key"), glyph = Glyph.Key, value = if (keyState == "ok") "" else "!") {
            vm.sync.open(SyncSheet.KeyRelay)
        }
        if (keyState == "ok")
            ValueRow("Pair a device", Modifier.testTag("sync-pair-show"), glyph = Glyph.Devices) {
                vm.sync.open(SyncSheet.PairShow)
            }
        ValueRow("Password & codes", Modifier.testTag("sync-security"), glyph = Glyph.Lock) {
            vm.sync.open(SyncSheet.Security)
        }
    }
    SignOut(vm)
}

internal fun hostOf(server: String): String =
    runCatching { java.net.URI(server).host }.getOrNull().orEmpty()

private fun stateColor(status: JSONObject): Color =
    when (status.optString("state")) {
        "idle" -> SyncColors.online
        "syncing" -> SyncColors.online.copy(alpha = 0.6f)
        "offline" -> Color.Gray
        else -> SyncColors.warning
    }

internal data class TileSpec(val glyph: Glyph, val label: String, val badge: Int = 0, val action: () -> Unit)

/** Tiles three to a row. */
@Composable
internal fun TileGrid(tiles: List<TileSpec>) {
    if (tiles.isEmpty()) return
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        for (row in tiles.chunked(3))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                for (tile in row)
                    SyncTile(tile.glyph, tile.label, "sync-tile-${tile.label}", Modifier.weight(1f), tile.badge, tile.action)
                repeat(3 - row.size) { Spacer(Modifier.weight(1f)) }
            }
    }
}

/** Playlists, followed channels and filters are local and work without an account. */
@Composable
private fun LocalLibraryTiles(vm: YtuiViewModel) {
    TileGrid(
        listOf(
            TileSpec(Glyph.Playlist, "Playlists") { vm.sync.open(SyncSheet.Playlists) },
            TileSpec(Glyph.Account, "Following") { vm.sync.open(SyncSheet.Following) },
            TileSpec(Glyph.Preview, "Filters") { vm.sync.open(SyncSheet.Filters) },
        )
    )
}

@Composable
private fun DevicesGroup(vm: YtuiViewModel, status: JSONObject) {
    val devices by vm.sync.devices.collectAsState()
    var confirm by remember { mutableStateOf<JSONObject?>(null) }
    LaunchedEffect(status.text("device_id")) { runCatching { vm.sync.refreshDevices() } }
    if (devices.isEmpty()) return
    SettingGroup("Devices") {
        for (device in devices) {
            val current = device.optBoolean("current")
            val playing = device.optJSONObject("now_playing")
            MediaListRow(
                device.text("name").ifBlank { "Device" },
                playing?.text("title").orEmpty(),
                Modifier.testTag("sync-device-${device.text("id")}"),
                artwork = { modifier ->
                    if (playing != null) Artwork(MediaCard(playing), modifier, thumbnailsOn(vm))
                    else GlyphTile(Glyph.Devices, modifier)
                },
                square = true,
                highlighted = current,
                trailing = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        if (device.optBoolean("online")) StatusDot(SyncColors.online)
                        if (!current)
                            MediaIconButton(Glyph.Close, "Remove ${device.text("name")}", "sync-revoke-${device.text("id")}") {
                                confirm = device
                            }
                    }
                },
            )
        }
    }
    val target = confirm
    if (target != null)
        MediaDialog(
            { confirm = null },
            target.text("name").ifBlank { "Device" },
            actions = {
                QuietAction("Cancel") { confirm = null }
                QuietAction("Remove", Modifier.testTag("sync-revoke-confirm"), destructive = true) {
                    confirm = null
                    vm.sync.act { vm.sync.revoke(target.text("id")) }
                }
            },
        ) {
            Text(
                "It is signed out and stops syncing.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
}

@Composable
private fun SignOut(vm: YtuiViewModel) {
    var open by remember { mutableStateOf(false) }
    var removeDevice by remember { mutableStateOf(true) }
    var forget by remember { mutableStateOf(false) }
    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
        QuietAction("Sign out", Modifier.testTag("sync-sign-out"), destructive = true) { open = true }
    }
    if (open)
        MediaDialog(
            { open = false },
            "Sign out",
            actions = {
                QuietAction("Cancel") { open = false }
                QuietAction("Sign out", Modifier.testTag("sync-sign-out-confirm"), destructive = true) {
                    open = false
                    vm.sync.act { vm.sync.signOut(removeDevice, forget) }
                }
            },
        ) {
            SyncToggle("Remove this device", removeDevice, "sync-sign-out-remove") { removeDevice = it }
            SyncToggle("Clear synced library", forget, "sync-sign-out-forget") { forget = it }
        }
}

/** Recovery codes, password and authenticator. Every change asks for the password again. */
@Composable
internal fun SecuritySheet(vm: YtuiViewModel, close: () -> Unit) {
    var page by remember { mutableStateOf("") }
    when (page) {
        "codes" -> RecoveryCodes(vm) { page = "" }
        "password" -> ChangePassword(vm) { page = "" }
        "totp" -> ChangeAuthenticator(vm) { page = "" }
        else ->
            Panel("Security", close) {
                var remaining by remember { mutableStateOf<Int?>(null) }
                LaunchedEffect(Unit) {
                    runCatching { remaining = vm.sync.obj("account_recovery_status").optInt("remaining") }
                }
                ValueRow("Recovery codes", Modifier.testTag("security-codes"), glyph = Glyph.Key, value = remaining?.toString().orEmpty()) {
                    page = "codes"
                }
                ValueRow("Password", Modifier.testTag("security-password"), glyph = Glyph.Lock) { page = "password" }
                ValueRow("Authenticator", Modifier.testTag("security-totp"), glyph = Glyph.Refresh) { page = "totp" }
            }
    }
}

@Composable
private fun RecoveryCodes(vm: YtuiViewModel, close: () -> Unit) {
    val runner = rememberSyncRunner()
    var password by remember { mutableStateOf("") }
    var code by remember { mutableStateOf("") }
    var codes by remember { mutableStateOf<List<String>>(emptyList()) }
    // New codes are shown once, in a secure window, and never stored.
    SecureDialog(close, "security-codes-sheet", secure = codes.isNotEmpty()) {
        GlyphBadge(Glyph.Key, 56.dp)
        if (codes.isEmpty()) {
            PasswordField(password, { password = it }, "Password", "codes-password")
            CodeField(code, { code = it }, "codes-code")
            ErrorLine(runner.error)
            Action("New codes", "codes-regenerate", enabled = password.isNotEmpty() && code.isNotBlank() && !runner.busy) {
                runner.run {
                    codes =
                        vm.sync.obj("account_recovery_regenerate", "password" to password, "code" to code.trim())
                            .optJSONArray("codes")
                            .strings()
                    password = ""
                    code = ""
                }
            }
        } else {
            Column(Modifier.testTag("codes-list"), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                for (item in codes) CodeText(item)
            }
            Action("Done", "codes-done") { close() }
        }
    }
}

@Composable
private fun ChangePassword(vm: YtuiViewModel, close: () -> Unit) {
    val runner = rememberSyncRunner()
    var current by remember { mutableStateOf("") }
    var code by remember { mutableStateOf("") }
    var next by remember { mutableStateOf("") }
    var repeat by remember { mutableStateOf("") }
    var others by remember { mutableStateOf(true) }
    Panel("Password", close) {
        PasswordField(current, { current = it }, "Current password", "password-current")
        CodeField(code, { code = it }, "password-code")
        PasswordField(next, { next = it }, "New password", "password-new")
        PasswordField(repeat, { repeat = it }, "Repeat", "password-repeat")
        SyncToggle("Sign out other devices", others, "password-sign-out-others") { others = it }
        ErrorLine(runner.error ?: "Passwords differ".takeIf { repeat.isNotEmpty() && repeat != next })
        Action(
            "Change",
            "password-change",
            enabled = current.isNotEmpty() && code.isNotBlank() && next.isNotEmpty() && next == repeat && !runner.busy,
        ) {
            runner.run {
                vm.sync.obj(
                    "account_password",
                    "current_password" to current,
                    "code" to code.trim(),
                    "new_password" to next,
                    "sign_out_others" to others,
                )
                current = ""
                next = ""
                repeat = ""
                code = ""
                Notices.show(Notice("Password changed", Glyph.Lock))
                close()
            }
        }
    }
}

@Composable
private fun ChangeAuthenticator(vm: YtuiViewModel, close: () -> Unit) {
    val runner = rememberSyncRunner()
    var password by remember { mutableStateOf("") }
    var code by remember { mutableStateOf("") }
    var setup by remember { mutableStateOf<JSONObject?>(null) }
    var confirm by remember { mutableStateOf("") }
    val started = setup
    SecureDialog(close, "totp-sheet", secure = started != null) {
        GlyphBadge(Glyph.Refresh, 56.dp)
        if (started == null) {
            PasswordField(password, { password = it }, "Password", "totp-password")
            CodeField(code, { code = it }, "totp-code", "Current code")
            ErrorLine(runner.error)
            Action("Continue", "totp-start", enabled = password.isNotEmpty() && code.isNotBlank() && !runner.busy) {
                runner.run {
                    setup = vm.sync.obj("account_totp_start", "password" to password, "code" to code.trim())
                    password = ""
                    code = ""
                }
            }
        } else {
            QrCode(started.optJSONArray("qr"), Modifier.fillMaxWidth(), "Authenticator QR code")
            CodeText(started.text("secret").chunked(4).joinToString(" "), size = 14)
            CodeField(confirm, { confirm = it }, "totp-confirm", "New code")
            ErrorLine(runner.error)
            Action("Confirm", "totp-confirm-action", enabled = confirm.isNotBlank() && !runner.busy) {
                runner.run {
                    vm.sync.obj("account_totp_confirm", "code" to confirm.trim())
                    Notices.show(Notice("Authenticator changed", Glyph.Check))
                    close()
                }
            }
        }
    }
}

/** Root host for the sync sheets; composed once next to the app's other sheets. */
@Composable
internal fun SyncSheetHost(vm: YtuiViewModel) {
    LaunchedEffect(Unit) { vm.sync.start() }
    val sheet by vm.sync.sheet.collectAsState()
    val close = { vm.sync.open(null) }
    when (val current = sheet) {
        null -> Unit
        SyncSheet.Friends -> FriendsSheet(vm, close)
        SyncSheet.Inbox -> InboxSheet(vm, close)
        SyncSheet.Jam -> JamSheet(vm, close)
        SyncSheet.Devices -> DevicesSheet(vm, close)
        SyncSheet.Playlists -> PlaylistsSheet(vm, close)
        is SyncSheet.Playlist -> PlaylistSheet(vm, current.id) { vm.sync.open(SyncSheet.Playlists) }
        SyncSheet.Stats -> StatsSheet(vm, close)
        SyncSheet.Wrapped -> WrappedSheet(vm) { vm.sync.open(SyncSheet.Stats) }
        SyncSheet.Filters -> FiltersSheet(vm, close)
        SyncSheet.Following -> FollowingSheet(vm, close)
        SyncSheet.Admin -> AdminSheet(vm, close)
        SyncSheet.Security -> SecuritySheet(vm, close)
        SyncSheet.PairShow -> PairShowSheet(vm, close)
        SyncSheet.PairHelp -> PairHelpSheet(vm, close)
        SyncSheet.KeyRelay -> KeyRelaySheet(vm, close)
        is SyncSheet.SendToFriend -> SendToFriendSheet(vm, current.media, close)
        is SyncSheet.AddToPlaylist -> AddToPlaylistSheet(vm, current.media, close)
    }
}

/** Card-menu rows: send, add to a playlist or the Jam, hide the channel. */
@Composable
internal fun SyncCardActions(vm: YtuiViewModel, media: MediaCard, done: () -> Unit) {
    val status by vm.sync.status.collectAsState()
    val room by vm.sync.jam.room.collectAsState()
    val features = if (status.optBoolean("signed_in")) status.optJSONArray("features").strings() else emptyList()
    val online = isOnlineMedia(media.value) && !media.collection
    if (online && "social" in features)
        ValueRow("Send to friend", Modifier.testTag("media-send-${media.key}"), glyph = Glyph.Inbox, chevron = false) {
            done()
            vm.sync.open(SyncSheet.SendToFriend(media))
        }
    if (online)
        ValueRow("Add to playlist", Modifier.testTag("media-playlist-${media.key}"), glyph = Glyph.Playlist, chevron = false) {
            done()
            vm.sync.open(SyncSheet.AddToPlaylist(media))
        }
    if (online && room != null)
        ValueRow("Add to Jam", Modifier.testTag("media-jam-${media.key}"), glyph = Glyph.Jam, chevron = false) {
            done()
            addToJam(vm, media)
        }
    if (Filters.channelValue(media) != null && media.kind !in setOf("channel", "artist"))
        ValueRow("Hide channel", Modifier.testTag("media-hide-${media.key}"), glyph = Glyph.Preview, chevron = false) {
            done()
            hideChannel(vm, media)
        }
}
