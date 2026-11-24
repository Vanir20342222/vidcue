package dev.ytui.android

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** Activity shortcuts, as in the You sheet: (label, dialog name, glyph). */
private val SIDEBAR_LIBRARY =
    listOf(
        Triple("History", "history", Glyph.History),
        Triple("Playback queue", "queue", Glyph.UpNext),
        Triple("Sleep timer", "sleep", Glyph.Sleep),
    )

private val RowShape = RoundedCornerShape(12.dp)

/**
 * Tablet navigation: a full-height floating sidebar beside the window's edge, with named destinations,
 * a Library group for the queue, history and sleep timer, and account and settings at the foot.
 * [compact] narrows it to icons with short labels underneath for windows without room for names
 * beside them.
 */
@Composable
internal fun TabletSidebar(
    tab: Int,
    choose: (Int) -> Unit,
    connected: Boolean,
    compact: Boolean,
    /** Movies (alpha) is listed only when turned on in Settings → Advanced, as on phones. */
    showMovies: Boolean = false,
    openDialog: (String) -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val dark = colors.background.luminance() < 0.4f
    val surface = lerp(colors.background, colors.surfaceContainer, 0.6f)
    Row(Modifier.fillMaxHeight()) {
        // A floating rounded panel, inset like the player panel on the other side, so it clears
        // the display's rounded corners instead of running square into them.
        Column(
            Modifier.padding(start = 20.dp, end = 12.dp, bottom = 20.dp)
                .width(if (compact) 92.dp else 248.dp)
                .fillMaxHeight()
                .clip(MediaTokens.dock)
                .background(surface)
                .testTag("navigation-rail")
        ) {
            Column(
                Modifier.weight(1f)
                    .verticalScroll(rememberScrollState())
                    .padding(
                        start = if (compact) 8.dp else 14.dp,
                        end = if (compact) 8.dp else 14.dp,
                        top = 16.dp,
                    ),
                verticalArrangement = Arrangement.spacedBy(if (compact) 10.dp else 4.dp),
            ) {
                Column(
                    Modifier.selectableGroup(),
                    verticalArrangement = Arrangement.spacedBy(if (compact) 10.dp else 4.dp),
                ) {
                    destinations(showMovies).forEach { (index, title, glyph) ->
                        Destination(index, title, glyph, tab == index, compact, dark) { choose(index) }
                    }
                }
                if (!compact) {
                    Box(
                        Modifier.padding(horizontal = 8.dp, vertical = 14.dp)
                            .fillMaxWidth()
                            .height(1.dp)
                            .background(colors.outlineVariant)
                    )
                    Text(
                        "YOUR ACTIVITY",
                        Modifier.padding(start = 14.dp, bottom = 6.dp),
                        style = MaterialTheme.typography.labelMedium,
                        color = colors.onSurfaceVariant,
                        letterSpacing = 1.sp,
                    )
                    SIDEBAR_LIBRARY.forEach { (title, name, glyph) ->
                        LibraryRow(title, glyph, "sidebar-$name") { openDialog(name) }
                    }
                }
            }
            Foot(connected, compact, openDialog)
        }
        Box(Modifier.width(1.dp).fillMaxHeight().background(colors.outlineVariant.copy(alpha = 0.6f)))
    }
}

@Composable
private fun Destination(
    index: Int,
    title: String,
    glyph: Glyph,
    selected: Boolean,
    compact: Boolean,
    dark: Boolean,
    choose: () -> Unit,
) {
    val motion = LocalMotion.current
    val colors = MaterialTheme.colorScheme
    val source = remember(index) { MutableInteractionSource() }
    val tint by
        animateColorAsState(
            if (selected) colors.onBackground else colors.onSurfaceVariant,
            motion.fade(200),
            label = "sidebar-icon",
        )
    val fill by
        animateColorAsState(
            if (selected) colors.onBackground.copy(alpha = if (dark) 0.11f else 0.08f)
            else colors.onBackground.copy(alpha = 0f),
            motion.fade(200),
            label = "sidebar-fill",
        )
    // Downloads catches covers flying in from Save, and bounces when one lands.
    val bounce = remember { Animatable(1f) }
    if (index == 0) DisposableEffect(Unit) { onDispose { DownloadFlight.target = null } }
    if (index == 0)
        LaunchedEffect(DownloadFlight.landed) {
            if (DownloadFlight.landed > 0 && motion.animate) {
                bounce.snapTo(1.25f)
                bounce.animateTo(1f, spring(0.4f, Spring.StiffnessMediumLow))
            }
        }
    val icon =
        @Composable {
            MediaIcon(
                glyph,
                Modifier.size(if (compact) 24.dp else 22.dp)
                    .pressScale(source, 0.9f)
                    .then(if (index == 0) Modifier.trackCentre { DownloadFlight.target = it } else Modifier)
                    .graphicsLayer {
                        scaleX = bounce.value
                        scaleY = bounce.value
                    },
                tint,
                filled = selected,
            )
        }
    val modifier =
        Modifier.fillMaxWidth()
            .selectable(selected, source, indication = null, role = Role.Tab, onClick = choose)
            .semantics { contentDescription = title }
            .testTag("tab-$index")
    if (compact)
        Column(
            modifier.padding(vertical = 2.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Box(
                Modifier.size(width = 56.dp, height = 32.dp).clip(MediaTokens.pill).background(fill),
                contentAlignment = Alignment.Center,
            ) {
                icon()
            }
            Text(
                title,
                style = MaterialTheme.typography.labelMedium,
                color = tint,
                fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    else
        Row(
            modifier.heightIn(min = 48.dp).clip(RowShape).background(fill).padding(horizontal = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            icon()
            Text(
                title,
                style = MaterialTheme.typography.titleMedium,
                color = tint,
                fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
}

@Composable
private fun LibraryRow(title: String, glyph: Glyph, tag: String, open: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val source = remember { MutableInteractionSource() }
    Row(
        Modifier.fillMaxWidth()
            .heightIn(min = 48.dp)
            .clip(RowShape)
            .clickable(source, LocalIndication.current, role = Role.Button, onClick = open)
            .padding(horizontal = 14.dp)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        MediaIcon(glyph, Modifier.size(20.dp).pressScale(source, 0.9f), colors.onSurfaceVariant)
        Text(
            title,
            style = MaterialTheme.typography.bodyLarge,
            color = colors.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** Account and settings at the foot of the sidebar. */
@Composable
private fun Foot(connected: Boolean, compact: Boolean, openDialog: (String) -> Unit) {
    val colors = MaterialTheme.colorScheme
    val accountLabel = if (connected) "Your account" else "Sign in"
    if (compact)
        Column(
            Modifier.fillMaxWidth().padding(bottom = 16.dp, top = 8.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            MediaIconButton(Glyph.Settings, "Settings", "settings-button") { openDialog("settings") }
            AccountButton(connected) { openDialog("account") }
        }
    else
        Row(
            Modifier.fillMaxWidth().padding(start = 14.dp, end = 10.dp, top = 8.dp, bottom = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Row(
                Modifier.weight(1f)
                    .heightIn(min = 48.dp)
                    .clip(RowShape)
                    .clickable(role = Role.Button) { openDialog("account") }
                    .semantics(mergeDescendants = true) { contentDescription = accountLabel }
                    .testTag("account-button")
                    .padding(horizontal = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Box(
                    Modifier.size(36.dp).clip(MediaTokens.pill).background(colors.surfaceContainerHigh),
                    contentAlignment = Alignment.Center,
                ) {
                    MediaIcon(Glyph.Account, Modifier.size(20.dp), colors.onBackground)
                }
                Text(
                    accountLabel,
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            MediaIconButton(Glyph.Settings, "Settings", "settings-button") { openDialog("settings") }
        }
}
