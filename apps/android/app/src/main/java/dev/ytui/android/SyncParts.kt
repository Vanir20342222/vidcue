package dev.ytui.android

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.view.WindowManager
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.SecureFlagPolicy
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

/**
 * Runs one network action for a sheet at a time and keeps its error for an inline line. Password
 * and code fields stay in plain `remember` state next to it, never in saved instance state.
 */
@Stable
internal class SyncRunner(private val scope: CoroutineScope) {
    var busy by mutableStateOf(false)
        private set

    var error by mutableStateOf<String?>(null)

    fun run(block: suspend () -> Unit) {
        if (busy) return
        busy = true
        error = null
        scope.launch {
            try {
                block()
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (failure: Exception) {
                error = SyncText.message(failure)
            } finally {
                busy = false
            }
        }
    }
}

@Composable
internal fun rememberSyncRunner(): SyncRunner {
    val scope = rememberCoroutineScope()
    return remember(scope) { SyncRunner(scope) }
}

/** Short, sanitized text helpers shared by the sync screens. */
internal object SyncText {
    /** Engine messages are already sanitized; this only bounds their length. */
    fun message(error: Throwable): String =
        (error.message ?: "").trim().ifBlank { "Something went wrong" }.take(200)

    /** "now", "5m", "3h", "2d": how long ago, in one short token. */
    fun age(at: Long, now: Long): String {
        if (at <= 0L) return ""
        val seconds = ((now - at) / 1000).coerceAtLeast(0)
        return when {
            seconds < 60 -> "now"
            seconds < 3600 -> "${seconds / 60}m"
            seconds < 86_400 -> "${seconds / 3600}h"
            else -> "${seconds / 86_400}d"
        }
    }

    /** Hours with one decimal under ten, whole hours above. */
    fun hours(seconds: Double): String {
        val hours = seconds.coerceAtLeast(0.0) / 3600
        return if (hours < 10) String.format(java.util.Locale.ROOT, "%.1f", hours)
        else hours.toLong().toString()
    }

    fun percent(rate: Double): String =
        if (!rate.isFinite()) "–" else "${(rate.coerceIn(0.0, 1.0) * 100).toInt()}%"
}

internal fun JSONArray?.strings(): List<String> =
    if (this == null) emptyList() else (0 until length()).mapNotNull { optString(it).takeIf(String::isNotEmpty) }

/** `optString` that treats JSON null as missing. */
internal fun JSONObject.text(key: String): String =
    if (isNull(key)) "" else optString(key)

/** An online media URL: only these are ever sent to the sync server. */
internal fun isOnlineMedia(media: JSONObject?): Boolean {
    val url = media?.text("url").orEmpty()
    return url.startsWith("https://", ignoreCase = true) || url.startsWith("http://", ignoreCase = true)
}

internal tailrec fun Context.findActivity(): Activity? =
    when (this) {
        is Activity -> this
        is ContextWrapper -> baseContext.findActivity()
        else -> null
    }

/** Keeps screenshots, screen recording and the Recents thumbnail away while [active]. */
@Composable
internal fun SecureWindow(active: Boolean = true) {
    val activity = LocalContext.current.findActivity()
    DisposableEffect(activity, active) {
        val window = activity?.window
        if (active) window?.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        onDispose { if (active) window?.clearFlags(WindowManager.LayoutParams.FLAG_SECURE) }
    }
}

/**
 * Centred card in its own secure window, for QR codes, recovery codes and key fingerprints: the
 * dialog window itself carries FLAG_SECURE, and so does the activity behind it while shown.
 */
@Composable
internal fun SecureDialog(
    onDismiss: () -> Unit,
    tag: String,
    secure: Boolean = true,
    content: @Composable ColumnScope.() -> Unit,
) {
    SecureWindow(secure)
    Dialog(
        onDismiss,
        DialogProperties(
            usePlatformDefaultWidth = false,
            securePolicy = if (secure) SecureFlagPolicy.SecureOn else SecureFlagPolicy.Inherit,
        ),
    ) {
        Column(
            Modifier.padding(20.dp)
                .widthIn(max = 420.dp)
                .fillMaxWidth()
                .heightIn(max = LocalConfiguration.current.screenHeightDp.dp * 0.9f)
                .clip(MediaTokens.dialog)
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                .verticalScroll(rememberScrollState())
                .padding(20.dp)
                .testTag(tag),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            CompositionLocalProvider(LocalContentColor provides MaterialTheme.colorScheme.onSurface) {
                Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.TopEnd) { SheetClose(onDismiss) }
                content()
            }
        }
    }
}

/** A switch row with an optional glyph tile, matching the settings rows. */
@Composable
internal fun SyncToggle(
    title: String,
    value: Boolean,
    tag: String,
    glyph: Glyph? = null,
    enabled: Boolean = true,
    changed: (Boolean) -> Unit,
) {
    Row(
        Modifier.fillMaxWidth()
            .heightIn(min = 56.dp)
            .clip(MediaTokens.medium)
            .toggleable(value, enabled = enabled, role = Role.Switch, onValueChange = changed)
            .padding(horizontal = 10.dp, vertical = 8.dp)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        if (glyph != null) GlyphTile(glyph, Modifier.size(36.dp).clip(MediaTokens.medium))
        Text(
            title,
            Modifier.weight(1f),
            style = MaterialTheme.typography.bodyLarge,
            fontWeight = FontWeight.Medium,
        )
        MediaSwitch(value, enabled)
    }
}

/** Square hub tile: a glyph over one word, with an optional count badge. */
@Composable
internal fun SyncTile(
    glyph: Glyph,
    label: String,
    tag: String,
    modifier: Modifier = Modifier,
    badge: Int = 0,
    action: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val source = remember { MutableInteractionSource() }
    Column(
        modifier
            .pressScale(source, 0.94f)
            .heightIn(min = 84.dp)
            .clip(MediaTokens.card)
            .background(colors.onSurface.copy(alpha = 0.07f))
            .clickable(source, SoftIndication, role = Role.Button, onClick = action)
            .padding(vertical = 12.dp, horizontal = 4.dp)
            .testTag(tag)
            .semantics { contentDescription = if (badge > 0) "$label, $badge new" else label },
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterVertically),
    ) {
        Box {
            MediaIcon(glyph, Modifier.size(26.dp))
            if (badge > 0)
                Box(
                    Modifier.align(Alignment.TopEnd)
                        .offset(x = 8.dp, y = (-6).dp)
                        .heightIn(min = 16.dp)
                        .widthIn(min = 16.dp)
                        .background(colors.primary, CircleShape)
                        .padding(horizontal = 4.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        if (badge > 99) "99+" else badge.toString(),
                        style = MaterialTheme.typography.labelSmall,
                        color = colors.onPrimary,
                    )
                }
        }
        Text(label, style = MaterialTheme.typography.labelMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** A large figure over a one-word caption. */
@Composable
internal fun BigNumber(value: String, caption: String, modifier: Modifier = Modifier) {
    Column(modifier, horizontalAlignment = Alignment.CenterHorizontally) {
        Text(
            value,
            style = MaterialTheme.typography.displaySmall.copy(fontWeight = FontWeight.SemiBold),
            maxLines = 1,
        )
        Text(
            caption,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
        )
    }
}

/** Monogram circle for a person; no pictures of people are loaded. */
@Composable
internal fun Monogram(name: String, size: Dp = 40.dp, online: Boolean? = null) {
    Box {
        ChannelAvatar(name, size, loadImage = false)
        if (online != null)
            Box(
                Modifier.align(Alignment.BottomEnd)
                    .size(size * 0.3f)
                    .background(MaterialTheme.colorScheme.background, CircleShape)
                    .padding(2.dp)
                    .background(
                        if (online) SyncColors.online else MaterialTheme.colorScheme.outline,
                        CircleShape,
                    )
            )
    }
}

internal object SyncColors {
    val online = Color(0xff2fbf71)
    val warning = Color(0xffe0a030)
}

/** A small coloured dot for a state. */
@Composable
internal fun StatusDot(color: Color, modifier: Modifier = Modifier) {
    Box(modifier.size(10.dp).background(color, CircleShape))
}

/** Inline error under a form; quiet, never an overlay. */
@Composable
internal fun ErrorLine(text: String?) {
    if (text.isNullOrBlank()) return
    Text(
        text,
        Modifier.fillMaxWidth().padding(horizontal = 4.dp).testTag("sync-error"),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.error,
    )
}

@Composable
internal fun PasswordField(value: String, onChange: (String) -> Unit, label: String, tag: String) =
    MediaTextField(
        value,
        onChange,
        label,
        Modifier.testTag(tag),
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false),
        visualTransformation = PasswordVisualTransformation(),
    )

/** Authenticator (six digits) or recovery code: free text, no autocorrect. */
@Composable
internal fun CodeField(value: String, onChange: (String) -> Unit, tag: String, label: String = "Code") =
    MediaTextField(
        value,
        { onChange(it.take(32)) },
        label,
        Modifier.testTag(tag),
        keyboardOptions =
            KeyboardOptions(keyboardType = KeyboardType.Ascii, autoCorrectEnabled = false),
    )

/** Monospaced block for codes shown once. */
@Composable
internal fun CodeText(text: String, modifier: Modifier = Modifier, size: Int = 18) {
    Text(
        text,
        modifier,
        style =
            MaterialTheme.typography.titleMedium.copy(
                fontFamily = FontFamily.Monospace,
                fontSize = size.sp,
                letterSpacing = 1.sp,
            ),
        textAlign = TextAlign.Center,
    )
}

/** Thumbnail-only card for artwork URLs that are not media (creators, stats). */
internal fun artworkCard(key: String, thumbnail: String, kind: String = "video"): MediaCard =
    MediaCard(JSONObject().put("kind", kind).put("url", "sync-art:$key").put("thumbnail", thumbnail))

internal fun thumbnailsOn(vm: YtuiViewModel): Boolean = vm.config.value.optBoolean("thumbnails", true)
